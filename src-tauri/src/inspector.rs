use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};

use chrono::Local;
use serde::Deserialize;
use sysinfo::{ProcessesToUpdate, System};
use uuid::Uuid;
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCPTABLE_OWNER_PID, MIB_UDPTABLE_OWNER_PID,
    TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::AF_INET;

use crate::geoip::{is_private_or_local_ip, resolve_geo_batch};
use crate::logger::CommandLogger;
use crate::security::{is_admin_cached, to_slash24};
use crate::types::{
    FirewallBlockRule, GeoIpInfo, InspectOptions, InspectorSnapshot, LiveConnectionEntry,
};

#[derive(Debug, Clone)]
pub struct RawConnRow {
    pub protocol: String,
    pub pid: u32,
    pub exe_name: String,
    pub exe_path: String,
    pub local_address: String,
    pub local_port: u16,
    pub remote_address: String,
    pub remote_port: u16,
    pub state: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawEtwUdpEvent {
    pub event_id: u32,
    pub pid: u32,
    pub saddr: String,
    pub daddr: String,
    pub sport: u16,
    pub dport: u16,
}

fn tcp_state_name(state: u32) -> &'static str {
    match state {
        1 => "Closed",
        2 => "Listen",
        3 => "SynSent",
        4 => "SynReceived",
        5 => "Established",
        6 => "FinWait1",
        7 => "FinWait2",
        8 => "CloseWait",
        9 => "Closing",
        10 => "LastAck",
        11 => "TimeWait",
        12 => "DeleteTcb",
        _ => "Unknown",
    }
}

fn port_from_network_u32(val: u32) -> u16 {
    u16::from_be((val & 0xFFFF) as u16)
}

/// Reads active IPv4 TCP connections and local UDP endpoints natively via Win32 iphlpapi (zero PowerShell).
fn read_native_sockets(target_exe: Option<&str>) -> Vec<RawConnRow> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let filter_lower = target_exe.map(|s| s.trim().to_ascii_lowercase());
    let filter_no_ext = filter_lower.as_deref().map(|s| s.trim_end_matches(".exe"));

    let mut proc_map: HashMap<u32, (String, String)> = HashMap::new();
    let mut target_pids: HashSet<u32> = HashSet::new();

    for (pid, p) in sys.processes() {
        let pid_u32 = pid.as_u32();
        if pid_u32 <= 4 {
            continue;
        }
        let path = p.exe().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        let raw_name = p.name().to_string_lossy().to_string();
        let exe_name = if raw_name.to_ascii_lowercase().ends_with(".exe") {
            raw_name
        } else {
            format!("{}.exe", raw_name)
        };
        if let Some(f_no_ext) = filter_no_ext {
            let lower = exe_name.to_ascii_lowercase();
            if lower.trim_end_matches(".exe") == f_no_ext || lower == filter_lower.as_deref().unwrap_or("") {
                target_pids.insert(pid_u32);
            }
        }
        proc_map.insert(pid_u32, (exe_name, path));
    }

    let mut rows = Vec::new();

    // 1. Native Win32 GetExtendedTcpTable (IPv4)
    unsafe {
        let mut size = 0u32;
        let _ = GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        );
        if size > 0 {
            let mut buf = vec![0u8; size as usize];
            if GetExtendedTcpTable(
                Some(buf.as_mut_ptr() as *mut _),
                &mut size,
                false,
                AF_INET.0 as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            ) == 0
            {
                let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
                let entries =
                    std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
                for c in entries {
                    let pid = c.dwOwningPid;
                    if filter_no_ext.is_some() && !target_pids.contains(&pid) {
                        continue;
                    }
                    let remote_ip = Ipv4Addr::from(c.dwRemoteAddr.to_ne_bytes());
                    if remote_ip.is_unspecified() || remote_ip.is_loopback() {
                        continue;
                    }
                    let local_ip = Ipv4Addr::from(c.dwLocalAddr.to_ne_bytes());
                    let (exe_name, exe_path) = proc_map
                        .get(&pid)
                        .cloned()
                        .unwrap_or_else(|| (format!("PID_{}", pid), String::new()));

                    rows.push(RawConnRow {
                        protocol: "TCP".to_string(),
                        pid,
                        exe_name,
                        exe_path,
                        local_address: local_ip.to_string(),
                        local_port: port_from_network_u32(c.dwLocalPort),
                        remote_address: remote_ip.to_string(),
                        remote_port: port_from_network_u32(c.dwRemotePort),
                        state: tcp_state_name(c.dwState).to_string(),
                    });
                }
            }
        }
    }

    // 2. Native Win32 GetExtendedUdpTable fallback (local bound sockets for targeted game)
    if filter_no_ext.is_some() && !target_pids.is_empty() {
        unsafe {
            let mut size = 0u32;
            let _ = GetExtendedUdpTable(
                None,
                &mut size,
                false,
                AF_INET.0 as u32,
                UDP_TABLE_OWNER_PID,
                0,
            );
            if size > 0 {
                let mut buf = vec![0u8; size as usize];
                if GetExtendedUdpTable(
                    Some(buf.as_mut_ptr() as *mut _),
                    &mut size,
                    false,
                    AF_INET.0 as u32,
                    UDP_TABLE_OWNER_PID,
                    0,
                ) == 0
                {
                    let table = &*(buf.as_ptr() as *const MIB_UDPTABLE_OWNER_PID);
                    let entries = std::slice::from_raw_parts(
                        table.table.as_ptr(),
                        table.dwNumEntries as usize,
                    );
                    for u in entries {
                        let pid = u.dwOwningPid;
                        if !target_pids.contains(&pid) {
                            continue;
                        }
                        let local_ip = Ipv4Addr::from(u.dwLocalAddr.to_ne_bytes());
                        let (exe_name, exe_path) = proc_map
                            .get(&pid)
                            .cloned()
                            .unwrap_or_else(|| (format!("PID_{}", pid), String::new()));

                        rows.push(RawConnRow {
                            protocol: "UDP".to_string(),
                            pid,
                            exe_name,
                            exe_path,
                            local_address: local_ip.to_string(),
                            local_port: port_from_network_u32(u.dwLocalPort),
                            remote_address: "*".to_string(),
                            remote_port: 0,
                            state: "Socket local (fallback)".to_string(),
                        });
                    }
                }
            }
        }
    }

    rows.truncate(150);
    rows
}

/// Pure parser transforming raw Microsoft-Windows-Kernel-Network ETW UDP Send/Recv events
/// (Event IDs 42/43 IPv4, 58/59 IPv6) into deduplicated remote UDP game server rows.
pub fn parse_etw_udp_events(
    events: &[RawEtwUdpEvent],
    target_pids: &HashSet<u32>,
    exe_name: &str,
    exe_path: &str,
) -> Vec<RawConnRow> {
    let mut dedup: HashMap<String, RawConnRow> = HashMap::new();

    for ev in events {
        if !target_pids.contains(&ev.pid) || !matches!(ev.event_id, 42 | 43 | 58 | 59) {
            continue;
        }
        let saddr = ev.saddr.trim();
        let daddr = ev.daddr.trim();
        if saddr.is_empty() || daddr.is_empty() {
            continue;
        }

        let s_priv = is_private_or_local_ip(saddr);
        let d_priv = is_private_or_local_ip(daddr);

        let (remote_addr, remote_port, local_addr, local_port) = if !d_priv && s_priv {
            (daddr, ev.dport, saddr, ev.sport)
        } else if !s_priv && d_priv {
            (saddr, ev.sport, daddr, ev.dport)
        } else if ev.event_id == 42 || ev.event_id == 58 {
            (daddr, ev.dport, saddr, ev.sport)
        } else {
            (saddr, ev.sport, daddr, ev.dport)
        };

        if is_private_or_local_ip(remote_addr) || remote_addr.parse::<IpAddr>().is_err() {
            continue;
        }

        let key = format!("UDP|{}|{}|{}", exe_name.to_ascii_lowercase(), remote_addr, remote_port);
        dedup.entry(key).or_insert_with(|| RawConnRow {
            protocol: "UDP".to_string(),
            pid: ev.pid,
            exe_name: exe_name.to_string(),
            exe_path: exe_path.to_string(),
            local_address: local_addr.to_string(),
            local_port,
            remote_address: remote_addr.to_string(),
            remote_port,
            state: "UDP ETW (Actif)".to_string(),
        });
    }

    dedup.into_values().collect()
}

pub fn merge_connection_rows(base_rows: Vec<RawConnRow>, etw_udp_rows: Vec<RawConnRow>) -> Vec<RawConnRow> {
    let etw_exes: HashSet<String> = etw_udp_rows
        .iter()
        .map(|r| r.exe_name.to_ascii_lowercase())
        .collect();
    let mut dedup: HashMap<String, RawConnRow> = HashMap::new();

    for r in etw_udp_rows.into_iter().chain(base_rows.into_iter()) {
        if r.remote_address.is_empty() {
            continue;
        }
        if r.protocol == "UDP" && r.remote_address == "*" && etw_exes.contains(&r.exe_name.to_ascii_lowercase()) {
            continue;
        }
        let key = format!(
            "{}|{}|{}|{}",
            r.protocol,
            r.exe_name.to_ascii_lowercase(),
            r.remote_address,
            r.remote_port
        );
        dedup.entry(key).or_insert(r);
    }

    dedup.into_values().collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EtwScriptOutput {
    pids: Vec<u32>,
    exe_name: String,
    exe_path: String,
    events: Vec<RawEtwUdpEvent>,
    note: Option<String>,
}

fn capture_udp_flows_etw(target_exe: &str, duration_ms: u32, logger: &CommandLogger) -> (Vec<RawConnRow>, Option<String>) {
    let safe_filter = target_exe
        .trim_end_matches(".exe")
        .trim_end_matches(".EXE")
        .replace('\'', "''");
    let trace_id = &Uuid::new_v4().to_string()[..8];
    let dur = duration_ms.clamp(1500, 4000);

    let script = format!(
        r#"
$filterName = '{safe_filter}'
$procs = @(Get-Process -ErrorAction SilentlyContinue | Where-Object {{
  $_.Id -gt 4 -and ($_.ProcessName -ieq $filterName -or "$($_.ProcessName).exe" -ieq "{safe_filter}.exe")
}})
if ($procs.Count -eq 0) {{
  [PSCustomObject]@{{ pids = @(); exeName = "{safe_filter}.exe"; exePath = ''; events = @(); note = 'Processus cible non actif pendant la capture ETW.' }} | ConvertTo-Json -Depth 3 -Compress
  exit 0
}}

$pidList = @($procs | ForEach-Object {{ [int]$_.Id }})
$firstPath = ''
try {{ $firstPath = [string]$procs[0].Path }} catch {{}}
$exeName = if ($firstPath) {{ [System.IO.Path]::GetFileName($firstPath) }} else {{ "$($procs[0].ProcessName).exe" }}

$sessionName = 'NetBoost_UDP_Capture'
$etlPath = Join-Path $env:TEMP "netboost_udp_{trace_id}.etl"
$captured = New-Object System.Collections.Generic.List[object]

try {{
  Remove-NetEventSession -Name $sessionName -ErrorAction SilentlyContinue | Out-Null
  New-NetEventSession -Name $sessionName -LocalFilePath $etlPath -CaptureMode SaveToFile | Out-Null
  Add-NetEventProvider -Name 'Microsoft-Windows-Kernel-Network' -SessionName $sessionName -MatchAnyKeyword 0x30 -Level 4 | Out-Null
  Start-NetEventSession -Name $sessionName | Out-Null
  Start-Sleep -Milliseconds {dur}
  Stop-NetEventSession -Name $sessionName -ErrorAction SilentlyContinue | Out-Null
  Remove-NetEventSession -Name $sessionName -ErrorAction SilentlyContinue | Out-Null

  if (Test-Path $etlPath) {{
    $evts = Get-WinEvent -Path $etlPath -Oldest -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -in 42, 43, 58, 59 }}
    foreach ($e in $evts) {{
      $xml = [xml]$e.ToXml()
      $dataMap = @{{}}
      foreach ($d in $xml.Event.EventData.Data) {{ $dataMap[$d.Name] = $d.'#text' }}
      $evPid = if ($dataMap.ContainsKey('PID')) {{ [int]$dataMap['PID'] }} elseif ($e.ProcessId) {{ [int]$e.ProcessId }} else {{ 0 }}
      if ($pidList -contains $evPid) {{
        $captured.Add([PSCustomObject]@{{
          eventId = [int]$e.Id
          pid = $evPid
          saddr = [string]$dataMap['saddr']
          daddr = [string]$dataMap['daddr']
          sport = if ($dataMap.ContainsKey('sport')) {{ [int]$dataMap['sport'] }} else {{ 0 }}
          dport = if ($dataMap.ContainsKey('dport')) {{ [int]$dataMap['dport'] }} else {{ 0 }}
        }})
      }}
    }}
  }}
}} finally {{
  Stop-NetEventSession -Name $sessionName -ErrorAction SilentlyContinue | Out-Null
  Remove-NetEventSession -Name $sessionName -ErrorAction SilentlyContinue | Out-Null
  if (Test-Path $etlPath) {{ Remove-Item -Path $etlPath -Force -ErrorAction SilentlyContinue }}
}}

[PSCustomObject]@{{
  pids = $pidList
  exeName = $exeName
  exePath = $firstPath
  events = @($captured | Select-Object -First 400)
  note = $null
}} | ConvertTo-Json -Depth 4 -Compress
"#
    );

    let res = logger.run_powershell(
        &script,
        "inspector",
        &format!(
            "New-NetEventSession ETW (Microsoft-Windows-Kernel-Network UDP 2.5s sur {})",
            target_exe
        ),
        false,
    );

    if !res.ok || res.stdout.is_empty() {
        return (
            Vec::new(),
            Some(if !res.stderr.is_empty() {
                format!(
                    "Capture ETW UDP indisponible : {}",
                    res.stderr.lines().next().unwrap_or("")
                )
            } else {
                "Capture ETW UDP sans résultat.".to_string()
            }),
        );
    }

    match serde_json::from_str::<EtwScriptOutput>(&res.stdout) {
        Ok(parsed) => {
            let pid_set: HashSet<u32> = parsed.pids.into_iter().collect();
            let rows = parse_etw_udp_events(&parsed.events, &pid_set, &parsed.exe_name, &parsed.exe_path);
            logger.info(
                "inspector",
                &format!("Résultat ETW UDP ({})", target_exe),
                &format!(
                    "{} événement(s) UDP capturé(s) → {} IP distante(s) publique(s).",
                    parsed.events.len(),
                    rows.len()
                ),
                "ok",
            );
            let note = parsed.note.or_else(|| {
                if rows.is_empty() {
                    Some("Capture ETW 2.5s terminée : aucun paquet UDP public détecté pendant la fenêtre.".into())
                } else {
                    None
                }
            });
            (rows, note)
        }
        Err(_) => (Vec::new(), Some("Erreur d'analyse des événements ETW UDP.".into())),
    }
}

pub async fn inspect_live_connections(
    target_exe: Option<String>,
    blocked_rules: Vec<FirewallBlockRule>,
    options: InspectOptions,
    logger: &CommandLogger,
) -> InspectorSnapshot {
    let target_clone = target_exe.clone();
    let base_rows = tokio::task::spawn_blocking(move || read_native_sockets(target_clone.as_deref()))
        .await
        .unwrap_or_default();

    let mut etw_rows = Vec::new();
    let mut etw_captured = false;
    let mut etw_note = None;

    if options.capture_udp_etw {
        if let Some(exe) = target_exe.clone() {
            if !is_admin_cached() {
                let msg = "Capture ETW UDP requiert les droits Administrateur (UAC) — fallback GetExtendedUdpTable utilisé.".to_string();
                logger.info("inspector", "Capture ETW UDP", &msg, "warn");
                etw_note = Some(msg);
            } else {
                let log_c = logger.clone();
                let (rows, note) = tokio::task::spawn_blocking(move || capture_udp_flows_etw(&exe, 2500, &log_c))
                    .await
                    .unwrap_or_default();
                etw_rows = rows;
                etw_note = note;
                etw_captured = true;
            }
        }
    }

    let etw_count = etw_rows.len();
    let merged = merge_connection_rows(base_rows, etw_rows);
    let unique_ips: Vec<String> = {
        let mut seen = HashSet::new();
        merged
            .iter()
            .filter(|r| seen.insert(r.remote_address.clone()))
            .map(|r| r.remote_address.clone())
            .collect()
    };

    let geo_map = resolve_geo_batch(&unique_ips, options.allow_https_fallback).await;
    let blocked_set: HashSet<String> = blocked_rules.iter().map(|b| b.remote_address.clone()).collect();

    let mut connections: Vec<LiveConnectionEntry> = merged
        .into_iter()
        .map(|r| {
            let geo = geo_map.get(&r.remote_address).cloned().unwrap_or_else(|| GeoIpInfo {
                ip: r.remote_address.clone(),
                country: "Inconnu".into(),
                country_code: "??".into(),
                city: "—".into(),
                isp: "Base locale".into(),
                asn: String::new(),
                is_eu: true,
                is_private: is_private_or_local_ip(&r.remote_address),
                source: "offline".into(),
            });
            let cidr24 = to_slash24(&r.remote_address);
            let is_blocked = blocked_set.contains(&r.remote_address) || blocked_set.contains(&cidr24);
            LiveConnectionEntry {
                id: format!("{}-{}-{}-{}-{}", r.protocol, r.pid, r.remote_address, r.remote_port, r.local_port),
                protocol: r.protocol,
                pid: r.pid,
                exe_name: r.exe_name,
                exe_path: r.exe_path,
                local_address: r.local_address,
                local_port: r.local_port,
                remote_address: r.remote_address,
                remote_port: r.remote_port,
                state: r.state,
                geo,
                is_blocked,
            }
        })
        .collect();

    connections.sort_by(|a, b| {
        let a_udp = if a.protocol == "UDP" && !a.geo.is_private { 0 } else { 1 };
        let b_udp = if b.protocol == "UDP" && !b.geo.is_private { 0 } else { 1 };
        let a_score = if !a.geo.is_private && !a.geo.is_eu { 0 } else if !a.geo.is_private { 1 } else { 2 };
        let b_score = if !b.geo.is_private && !b.geo.is_eu { 0 } else if !b.geo.is_private { 1 } else { 2 };
        a_udp
            .cmp(&b_udp)
            .then_with(|| a_score.cmp(&b_score))
            .then_with(|| a.exe_name.cmp(&b.exe_name))
    });

    let non_eu_count = connections
        .iter()
        .filter(|c| !c.geo.is_private && !c.geo.is_eu)
        .count();

    InspectorSnapshot {
        target_exe,
        connections,
        blocked_rules,
        non_eu_count,
        etw_udp_captured: etw_captured,
        etw_udp_count: etw_count,
        etw_note,
        scanned_at: Local::now().format("%H:%M:%S").to_string(),
    }
}
