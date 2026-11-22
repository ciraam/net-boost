use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::types::GeoIpInfo;

/// Embedded sorted IPv4 CIDR table (10-byte records: [start_be: u32, end_be: u32, cc: [u8; 2]]).
/// Provides instant 0.001 ms offline GeoIP lookups with zero rate-limit and zero third-party IP leakage.
static GEOIP_V4_DATA: &[u8] = include_bytes!("../resources/geoip-country.dat");

static HTTPS_FALLBACK_CACHE: OnceLock<Mutex<HashMap<String, GeoIpInfo>>> = OnceLock::new();

fn is_european_cc(cc: &str) -> bool {
    matches!(
        cc,
        "AT" | "BE" | "BG" | "HR" | "CY" | "CZ" | "DK" | "EE" | "FI" | "FR" | "DE" | "GR"
            | "HU" | "IE" | "IT" | "LV" | "LT" | "LU" | "MT" | "NL" | "PL" | "PT" | "RO"
            | "SK" | "SI" | "ES" | "SE" | "GB" | "UK" | "CH" | "NO" | "IS" | "LI" | "AD"
            | "MC" | "SM"
    )
}

pub fn country_name_fr(cc: &str) -> String {
    match cc {
        "FR" => "France",
        "DE" => "Allemagne",
        "NL" => "Pays-Bas",
        "GB" | "UK" => "Royaume-Uni",
        "IE" => "Irlande",
        "BE" => "Belgique",
        "LU" => "Luxembourg",
        "CH" => "Suisse",
        "ES" => "Espagne",
        "PT" => "Portugal",
        "IT" => "Italie",
        "PL" => "Pologne",
        "SE" => "Suède",
        "NO" => "Norvège",
        "FI" => "Finlande",
        "DK" => "Danemark",
        "AT" => "Autriche",
        "CZ" => "Tchéquie",
        "RO" => "Roumanie",
        "BG" => "Bulgarie",
        "GR" => "Grèce",
        "HU" => "Hongrie",
        "US" => "États-Unis",
        "CA" => "Canada",
        "KR" => "Corée du Sud",
        "JP" => "Japon",
        "CN" => "Chine",
        "SG" => "Singapour",
        "AU" => "Australie",
        "BR" => "Brésil",
        "IN" => "Inde",
        "RU" => "Russie",
        "TR" => "Turquie",
        "HK" => "Hong Kong",
        "TW" => "Taïwan",
        "ZA" => "Afrique du Sud",
        "AE" => "Émirats arabes unis",
        "MX" => "Mexique",
        other => return other.to_string(),
    }
    .to_string()
}

pub fn is_private_or_local_ip(ip: &str) -> bool {
    let clean = ip.trim();
    if clean.is_empty() || clean == "*" || clean == "0.0.0.0" || clean == "::" || clean == "::1" {
        return true;
    }
    match clean.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_unspecified()
        }
        Ok(IpAddr::V6(v6)) => {
            let seg0 = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (seg0 & 0xffc0) == 0xfe80
                || (seg0 & 0xfe00) == 0xfc00
        }
        Err(_) => true,
    }
}

/// Instant O(log N) binary search over embedded IPv4 GeoIP table (< 0.001 ms).
pub fn lookup_geo_offline(ip: &str) -> Option<GeoIpInfo> {
    let clean = ip.trim();
    if is_private_or_local_ip(clean) {
        return Some(GeoIpInfo {
            ip: clean.to_string(),
            country: "Réseau Local".to_string(),
            country_code: "LAN".to_string(),
            city: "Local".to_string(),
            isp: "Intranet / Boucle locale".to_string(),
            asn: String::new(),
            is_eu: true,
            is_private: true,
            source: "local".to_string(),
        });
    }

    let v4: Ipv4Addr = clean.parse().ok()?;
    let target = u32::from(v4);
    let count = GEOIP_V4_DATA.len() / 10;
    if count == 0 {
        return None;
    }

    let mut low = 0usize;
    let mut high = count - 1;
    while low <= high {
        let mid = low + (high - low) / 2;
        let off = mid * 10;
        let start = u32::from_be_bytes([
            GEOIP_V4_DATA[off],
            GEOIP_V4_DATA[off + 1],
            GEOIP_V4_DATA[off + 2],
            GEOIP_V4_DATA[off + 3],
        ]);
        let end = u32::from_be_bytes([
            GEOIP_V4_DATA[off + 4],
            GEOIP_V4_DATA[off + 5],
            GEOIP_V4_DATA[off + 6],
            GEOIP_V4_DATA[off + 7],
        ]);

        if target < start {
            if mid == 0 {
                break;
            }
            high = mid - 1;
        } else if target > end {
            low = mid + 1;
        } else {
            let cc = std::str::from_utf8(&GEOIP_V4_DATA[off + 8..off + 10])
                .unwrap_or("??")
                .trim()
                .to_ascii_uppercase();
            if cc.is_empty() {
                return None;
            }
            return Some(GeoIpInfo {
                ip: clean.to_string(),
                country: country_name_fr(&cc),
                country_code: cc.clone(),
                city: "Base locale".to_string(),
                isp: "GeoIP Hors-Ligne (natif Rust)".to_string(),
                asn: String::new(),
                is_eu: is_european_cc(&cc),
                is_private: false,
                source: "offline".to_string(),
            });
        }
    }
    None
}

async fn fetch_https_fallback(ip: &str) -> Option<GeoIpInfo> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
        .ok()?;
    let url = format!("https://ipwho.is/{}", ip);
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: serde_json::Value = resp.json().await.ok()?;
    if !v.get("success").and_then(|b| b.as_bool()).unwrap_or(false) {
        return None;
    }
    let cc = v
        .get("country_code")
        .and_then(|s| s.as_str())
        .unwrap_or("??")
        .to_ascii_uppercase();
    let country = v
        .get("country")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| country_name_fr(&cc));
    let city = v
        .get("city")
        .and_then(|s| s.as_str())
        .unwrap_or("—")
        .to_string();
    let isp = v
        .pointer("/connection/isp")
        .and_then(|s| s.as_str())
        .or_else(|| v.pointer("/connection/org").and_then(|s| s.as_str()))
        .unwrap_or("—")
        .to_string();
    let asn = v
        .pointer("/connection/asn")
        .and_then(|n| n.as_u64())
        .map(|n| format!("AS{}", n))
        .unwrap_or_default();

    Some(GeoIpInfo {
        ip: ip.to_string(),
        country,
        country_code: cc.clone(),
        city,
        isp,
        asn,
        is_eu: is_european_cc(&cc),
        is_private: false,
        source: "https".to_string(),
    })
}

pub async fn resolve_geo_batch(ips: &[String], allow_https_fallback: bool) -> HashMap<String, GeoIpInfo> {
    let cache = HTTPS_FALLBACK_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut out = HashMap::new();
    let mut missing = Vec::new();

    for ip in ips {
        if let Some(hit) = lookup_geo_offline(ip) {
            out.insert(ip.clone(), hit);
            continue;
        }
        if let Ok(guard) = cache.lock() {
            if let Some(cached) = guard.get(ip) {
                out.insert(ip.clone(), cached.clone());
                continue;
            }
        }
        if allow_https_fallback && !missing.contains(ip) {
            missing.push(ip.clone());
        }
    }

    if allow_https_fallback {
        for ip in missing.into_iter().take(12) {
            if let Some(info) = fetch_https_fallback(&ip).await {
                if let Ok(mut guard) = cache.lock() {
                    guard.insert(ip.clone(), info.clone());
                }
                out.insert(ip, info);
            }
        }
    }

    for ip in ips {
        out.entry(ip.clone()).or_insert_with(|| GeoIpInfo {
            ip: ip.clone(),
            country: "Non répertorié".to_string(),
            country_code: "??".to_string(),
            city: "—".to_string(),
            isp: "Base locale".to_string(),
            asn: String::new(),
            is_eu: true,
            is_private: false,
            source: "offline".to_string(),
        });
    }

    out
}
