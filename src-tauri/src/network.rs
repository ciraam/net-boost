use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use windows::Win32::Foundation::ERROR_BUFFER_OVERFLOW;
use windows::Win32::NetworkManagement::IpHelper::{
    GetAdaptersAddresses, GetBestRoute, GAA_FLAG_INCLUDE_PREFIX, IP_ADAPTER_ADDRESSES_LH,
    MIB_IPFORWARDROW,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

use crate::logger::{ps_single_quote, CommandLogger};
use crate::types::{AdapterAdvancedProp, DnsBenchmarkResult, NetworkAdapterInfo};

fn format_link_speed(bits_per_sec: u64) -> String {
    if bits_per_sec >= 1_000_000_000 {
        let gbps = bits_per_sec as f64 / 1_000_000_000.0;
        if (gbps - gbps.round()).abs() < 0.05 {
            format!("{} Gbps", gbps.round() as u64)
        } else {
            format!("{:.1} Gbps", gbps)
        }
    } else if bits_per_sec >= 1_000_000 {
        format!("{} Mbps", bits_per_sec / 1_000_000)
    } else if bits_per_sec > 0 {
        format!("{} Kbps", bits_per_sec / 1_000)
    } else {
        "Up".to_string()
    }
}

unsafe fn pwstr_to_string(ptr: windows::core::PWSTR) -> String {
    if ptr.0.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.0.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr.0, len))
}

unsafe fn pstr_to_string(ptr: windows::core::PSTR) -> String {
    if ptr.0.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.0.add(len) != 0 {
        len += 1;
    }
    String::from_utf8_lossy(std::slice::from_raw_parts(ptr.0, len)).to_string()
}

fn read_eee_properties_for_adapter(adapter_name: &str, logger: &CommandLogger) -> Vec<AdapterAdvancedProp> {
    let script = format!(
        r#"
$adv = Get-NetAdapterAdvancedProperty -Name {} -ErrorAction SilentlyContinue | Where-Object {{
  $_.DisplayName -match 'EEE|Energy|Green|Power Saving|Ultra Low Power|Economie|Économie|Alimentation' -or
  $_.RegistryKeyword -match 'EEE|Energy|Green|PowerSav|ULP'
}}
$res = foreach ($p in $adv) {{
  [PSCustomObject]@{{
    displayName = [string]$p.DisplayName
    registryKeyword = [string]$p.RegistryKeyword
    displayValue = [string]$p.DisplayValue
    registryValue = [string]($p.RegistryValue -join ',')
  }}
}}
@($res) | ConvertTo-Json -Depth 3 -Compress
"#,
        ps_single_quote(adapter_name)
    );
    let out = logger.run_powershell(
        &script,
        "adapter",
        &format!("Get-NetAdapterAdvancedProperty \"{}\" (EEE)", adapter_name),
        true,
    );
    if !out.ok || out.stdout.is_empty() {
        return Vec::new();
    }
    if let Ok(vec) = serde_json::from_str::<Vec<AdapterAdvancedProp>>(&out.stdout) {
        return vec;
    }
    if let Ok(single) = serde_json::from_str::<AdapterAdvancedProp>(&out.stdout) {
        return vec![single];
    }
    Vec::new()
}

/// Discovers the active default-route Windows network adapter natively via Win32
/// GetBestRoute + GetAdaptersAddresses + winreg (zero PowerShell unless `include_eee` is true).
pub fn get_active_network_adapter(
    include_eee: bool,
    logger: &CommandLogger,
) -> Option<NetworkAdapterInfo> {
    let best_if_index = unsafe {
        let mut route = MIB_IPFORWARDROW::default();
        // Best route to 1.1.1.1 (0x01010101)
        let dest = u32::from_ne_bytes([1, 1, 1, 1]);
        if GetBestRoute(dest, 0, &mut route) == 0 {
            Some(route.dwForwardIfIndex)
        } else {
            None
        }
    };

    unsafe {
        let mut buf_len: u32 = 16 * 1024;
        let mut buf = vec![0u8; buf_len as usize];
        let mut ret = GetAdaptersAddresses(
            AF_INET.0 as u32,
            GAA_FLAG_INCLUDE_PREFIX,
            None,
            Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
            &mut buf_len,
        );
        if ret == ERROR_BUFFER_OVERFLOW.0 {
            buf.resize(buf_len as usize, 0);
            ret = GetAdaptersAddresses(
                AF_INET.0 as u32,
                GAA_FLAG_INCLUDE_PREFIX,
                None,
                Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
                &mut buf_len,
            );
        }
        if ret != 0 {
            return None;
        }

        let mut fallback: Option<NetworkAdapterInfo> = None;
        let mut cur = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;

        while !cur.is_null() {
            let a = &*cur;
            let if_index = a.Anonymous1.Anonymous.IfIndex;
            let is_up = a.OperStatus == IfOperStatusUp;

            if is_up {
                let name = pwstr_to_string(a.FriendlyName);
                let description = pwstr_to_string(a.Description);
                let interface_guid = pstr_to_string(a.AdapterName);
                let link_speed = format_link_speed(a.TransmitLinkSpeed.max(a.ReceiveLinkSpeed));

                // Extract IPv4 unicast address
                let mut ipv4 = String::new();
                let mut ua = a.FirstUnicastAddress;
                while !ua.is_null() {
                    let sa = (*ua).Address.lpSockaddr;
                    if !sa.is_null() && (*sa).sa_family == AF_INET {
                        let sin = &*(sa as *const SOCKADDR_IN);
                        let ip = Ipv4Addr::from(sin.sin_addr.S_un.S_addr.to_ne_bytes());
                        if !ip.is_loopback() {
                            ipv4 = ip.to_string();
                            break;
                        }
                    }
                    ua = (*ua).Next;
                }

                // Extract IPv4 DNS servers
                let mut current_dns = Vec::new();
                let mut da = a.FirstDnsServerAddress;
                while !da.is_null() {
                    let sa = (*da).Address.lpSockaddr;
                    if !sa.is_null() && (*sa).sa_family == AF_INET {
                        let sin = &*(sa as *const SOCKADDR_IN);
                        let ip = Ipv4Addr::from(sin.sin_addr.S_un.S_addr.to_ne_bytes());
                        current_dns.push(ip.to_string());
                    }
                    da = (*da).Next;
                }

                // Check DHCP vs static DNS via winreg NameServer
                let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
                let reg_sub = format!(
                    r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{}",
                    interface_guid
                );
                let dns_is_dhcp = hklm
                    .open_subkey_with_flags(&reg_sub, KEY_READ)
                    .ok()
                    .and_then(|k| k.get_value::<String, _>("NameServer").ok())
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true);

                let info = NetworkAdapterInfo {
                    name,
                    description,
                    interface_index: if_index,
                    interface_guid,
                    link_speed,
                    status: "Up".to_string(),
                    ipv4: ipv4.clone(),
                    current_dns,
                    dns_is_dhcp,
                    eee_properties: Vec::new(),
                };

                if best_if_index == Some(if_index) {
                    let mut chosen = info;
                    if include_eee {
                        chosen.eee_properties = read_eee_properties_for_adapter(&chosen.name, logger);
                    }
                    return Some(chosen);
                }
                if fallback.is_none() && !ipv4.is_empty() {
                    fallback = Some(info);
                }
            }
            cur = a.Next;
        }

        if let Some(mut fb) = fallback {
            if include_eee {
                fb.eee_properties = read_eee_properties_for_adapter(&fb.name, logger);
            }
            return Some(fb);
        }
        None
    }
}

pub fn get_eee_props_to_disable(props: &[AdapterAdvancedProp]) -> Vec<AdapterAdvancedProp> {
    props
        .iter()
        .filter(|p| {
            let dv = p.display_value.to_ascii_lowercase();
            dv != "disabled" && dv != "désactivé" && dv != "off" && p.registry_value.trim() != "0"
        })
        .cloned()
        .collect()
}

/// Builds a minimal real RFC 1035 DNS A-record UDP query packet.
fn build_dns_query(tx_id: u16, domain: &str) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(64);
    pkt.extend_from_slice(&tx_id.to_be_bytes());
    pkt.extend_from_slice(&0x0100u16.to_be_bytes()); // Standard query, Recursion Desired
    pkt.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT = 1
    pkt.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT = 0
    pkt.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT = 0
    pkt.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT = 0
    for label in domain.split('.') {
        let bytes = label.as_bytes();
        pkt.push(bytes.len() as u8);
        pkt.extend_from_slice(bytes);
    }
    pkt.push(0); // root label
    pkt.extend_from_slice(&1u16.to_be_bytes()); // QTYPE = A
    pkt.extend_from_slice(&1u16.to_be_bytes()); // QCLASS = IN
    pkt
}

async fn probe_dns_once(server_ip: &str, domain: &str, tx_id: u16) -> Option<f64> {
    let target: SocketAddr = format!("{}:53", server_ip).parse().ok()?;
    let sock = UdpSocket::bind("0.0.0.0:0").await.ok()?;
    let query = build_dns_query(tx_id, domain);
    let mut recv_buf = [0u8; 512];

    let start = Instant::now();
    sock.send_to(&query, target).await.ok()?;
    let recv_res = tokio::time::timeout(Duration::from_millis(2000), sock.recv_from(&mut recv_buf)).await;
    let Ok(Ok((len, _))) = recv_res else {
        return None;
    };
    if len < 12 {
        return None;
    }
    let resp_id = u16::from_be_bytes([recv_buf[0], recv_buf[1]]);
    if resp_id != tx_id {
        return None;
    }
    let ms = (start.elapsed().as_secs_f64() * 1000.0 * 10.0).round() / 10.0;
    Some(ms)
}

/// Real parallel native UDP DNS benchmark across 1.1.1.1, 8.8.8.8, and 9.9.9.9 (zero subprocesses).
pub async fn benchmark_dns_parallel(logger: &CommandLogger) -> Vec<DnsBenchmarkResult> {
    let candidates = [
        ("1.1.1.1", "Cloudflare"),
        ("8.8.8.8", "Google DNS"),
        ("9.9.9.9", "Quad9"),
    ];
    let domains = ["cloudflare.com", "google.com", "riotgames.com", "steampowered.com"];

    logger.info(
        "dns",
        "Benchmark DNS UDP natif parallèle (1.1.1.1, 8.8.8.8, 9.9.9.9)",
        "Requêtes RFC 1035 réelles en parallèle sur cloudflare.com, google.com, riotgames.com, steampowered.com...",
        "ok",
    );

    let mut handles = Vec::new();
    for (idx, (ip, name)) in candidates.iter().enumerate() {
        let ip_s = (*ip).to_string();
        let name_s = (*name).to_string();
        handles.push(tokio::spawn(async move {
            let mut times = Vec::new();
            for (d_idx, dom) in domains.iter().enumerate() {
                let tx_id = 0x4A00u16 + (idx as u16 * 16) + d_idx as u16;
                if let Some(ms) = probe_dns_once(&ip_s, dom, tx_id).await {
                    times.push(ms);
                }
            }
            if times.is_empty() {
                DnsBenchmarkResult {
                    ip: ip_s,
                    name: name_s,
                    avg_ms: None,
                    min_ms: None,
                    max_ms: None,
                    success_count: 0,
                    total_count: domains.len() as u32,
                    fastest: false,
                }
            } else {
                let avg = ((times.iter().sum::<f64>() / times.len() as f64) * 10.0).round() / 10.0;
                let min = times.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                DnsBenchmarkResult {
                    ip: ip_s,
                    name: name_s,
                    avg_ms: Some(avg),
                    min_ms: Some(min),
                    max_ms: Some(max),
                    success_count: times.len() as u32,
                    total_count: domains.len() as u32,
                    fastest: false,
                }
            }
        }));
    }

    let mut results = Vec::new();
    for h in handles {
        if let Ok(r) = h.await {
            results.push(r);
        }
    }

    if let Some((best_idx, _)) = results
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.avg_ms.map(|ms| (i, ms)))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    {
        results[best_idx].fastest = true;
    }

    let summary = results
        .iter()
        .map(|r| {
            format!(
                "{} ({}): {}{}",
                r.name,
                r.ip,
                r.avg_ms
                    .map(|v| format!("{} ms", v))
                    .unwrap_or_else(|| "timeout".into()),
                if r.fastest { " ★" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join(" | ");
    logger.info("dns", "Résultat Benchmark DNS natif", &summary, "ok");

    results
}
