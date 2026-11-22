use std::fs;
use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, Emitter};

use crate::logger::CommandLogger;
use crate::security::{
    is_admin_cached, restrict_file_acl_to_current_user, sanitize_host,
    verify_authenticode_signature,
};
use crate::types::{TunnelActionResult, TunnelClientsStatus, TunnelState, WireGuardProfile};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const WG_TUNNEL_NAME: &str = "NetBoostWG";

/// Strictly restricted to trusted Program Files installation paths (never PATH / where.exe under UAC).
const WIRESOCK_TRUSTED_PATHS: &[&str] = &[
    r"C:\Program Files\WireSock VPN Client\bin\wiresock-client.exe",
    r"C:\Program Files\WireSock Secure Connect\bin\wiresock-client.exe",
    r"C:\Program Files\WireSock\wiresock-client.exe",
    r"C:\Program Files (x86)\WireSock VPN Client\bin\wiresock-client.exe",
];

const WIREGUARD_TRUSTED_PATHS: &[&str] = &[
    r"C:\Program Files\WireGuard\wireguard.exe",
    r"C:\Program Files (x86)\WireGuard\wireguard.exe",
];

pub fn detect_tunnel_clients(logger: &CommandLogger) -> TunnelClientsStatus {
    let mut wiresock_path = None;
    let mut wiresock_signed = false;
    for p in WIRESOCK_TRUSTED_PATHS {
        let path = Path::new(p);
        if path.exists() {
            if verify_authenticode_signature(path) {
                wiresock_path = Some((*p).to_string());
                wiresock_signed = true;
                break;
            } else {
                logger.info(
                    "tunnel",
                    &format!("WinVerifyTrust refusé : {}", p),
                    "Binaire WireSock ignoré car sa signature Authenticode est invalide.",
                    "warn",
                );
            }
        }
    }

    let mut wireguard_path = None;
    let mut wireguard_signed = false;
    for p in WIREGUARD_TRUSTED_PATHS {
        let path = Path::new(p);
        if path.exists() {
            if verify_authenticode_signature(path) {
                wireguard_path = Some((*p).to_string());
                wireguard_signed = true;
                break;
            } else {
                logger.info(
                    "tunnel",
                    &format!("WinVerifyTrust refusé : {}", p),
                    "Binaire WireGuard ignoré car sa signature Authenticode est invalide.",
                    "warn",
                );
            }
        }
    }

    TunnelClientsStatus {
        wiresock_path,
        wireguard_path,
        wiresock_signed,
        wireguard_signed,
    }
}

pub fn parse_wireguard_conf(raw_text: &str, file_name: &str) -> Result<WireGuardProfile, String> {
    let mut section = String::new();
    let mut has_private_key = false;
    let mut public_key = String::new();
    let mut endpoint = String::new();
    let mut interface_address = String::new();
    let mut dns_val = String::new();
    let mut allowed_ips_original = String::new();

    for raw_line in raw_text.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_ascii_lowercase();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim().to_ascii_lowercase();
        let val = v.trim();

        if section == "interface" {
            if key == "privatekey" && val.len() > 10 {
                has_private_key = true;
            } else if key == "address" {
                interface_address = val.to_string();
            } else if key == "dns" {
                dns_val = val.to_string();
            }
        } else if section == "peer" {
            if key == "publickey" {
                public_key = val.to_string();
            } else if key == "endpoint" {
                endpoint = val.to_string();
            } else if key == "allowedips" {
                allowed_ips_original = val.to_string();
            }
        }
    }

    if !has_private_key || public_key.is_empty() || endpoint.is_empty() {
        return Err(
            "Fichier .conf WireGuard incomplet : [Interface] PrivateKey ainsi que [Peer] PublicKey et Endpoint sont obligatoires."
                .into(),
        );
    }

    let public_key_preview = if public_key.len() > 14 {
        format!("{}…{}", &public_key[..8], &public_key[public_key.len() - 6..])
    } else {
        public_key
    };

    Ok(WireGuardProfile {
        file_name: file_name.to_string(),
        raw_conf: Some(raw_text.to_string()),
        encrypted_conf: None,
        is_encrypted: false,
        endpoint,
        interface_address: if interface_address.is_empty() {
            "Non spécifiée".into()
        } else {
            interface_address
        },
        dns: if dns_val.is_empty() {
            "Automatique".into()
        } else {
            dns_val
        },
        allowed_ips_original: if allowed_ips_original.is_empty() {
            "0.0.0.0/0".into()
        } else {
            allowed_ips_original
        },
        public_key_preview,
        imported_at: Utc::now().to_rfc3339(),
    })
}

pub fn mask_private_key_for_preview(conf: &str) -> String {
    conf.lines()
        .map(|line| {
            if let Some((k, _)) = line.split_once('=') {
                if k.trim().eq_ignore_ascii_case("privatekey") {
                    return format!("{} = [MASQUÉ_PAR_SÉCURITÉ]", k.trim());
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn generate_split_tunnel_conf(
    raw_conf: &str,
    mode: &str,
    exe_name: Option<&str>,
    allowed_ips: &[String],
) -> String {
    let mut output = Vec::new();
    let mut in_peer = false;
    let mut injected = false;

    let emit_directives = |out: &mut Vec<String>| {
        if mode == "wiresock" {
            if let Some(exe) = exe_name {
                if !exe.is_empty() {
                    out.push(format!("AllowedApps = {}", exe));
                }
            }
            out.push("DisallowedIPs = 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 127.0.0.0/8".into());
        } else {
            let cidrs = if allowed_ips.is_empty() {
                "0.0.0.0/0".to_string()
            } else {
                allowed_ips
                    .iter()
                    .map(|ip| if ip.contains('/') { ip.clone() } else { format!("{}/32", ip) })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            out.push(format!("AllowedIPs = {}", cidrs));
        }
    };

    for line in raw_conf.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_peer && !injected {
                emit_directives(&mut output);
                injected = true;
            }
            in_peer = lower == "[peer]";
            output.push(line.to_string());
            continue;
        }

        if in_peer {
            if mode == "wiresock" && (lower.starts_with("allowedapps") || lower.starts_with("disallowedips")) {
                continue;
            }
            if mode == "wireguard" && lower.starts_with("allowedips") {
                continue;
            }
        }
        output.push(line.to_string());
    }

    if in_peer && !injected {
        emit_directives(&mut output);
    }

    format!("{}\n", output.join("\n"))
}

fn resolve_host_ipv4s(host: &str) -> Vec<String> {
    let Ok(clean) = sanitize_host(host) else {
        return Vec::new();
    };
    if let Ok(v4) = clean.parse::<Ipv4Addr>() {
        return vec![v4.to_string()];
    }
    let Ok(addrs) = format!("{}:0", clean).to_socket_addrs() else {
        return Vec::new();
    };
    let mut ips = Vec::new();
    for a in addrs {
        if let IpAddr::V4(v4) = a.ip() {
            let s = v4.to_string();
            if !ips.contains(&s) {
                ips.push(s);
            }
        }
    }
    ips
}

struct TunnelRuntime {
    state: TunnelState,
    wiresock_child: Option<Child>,
    wireguard_service_installed: bool,
}

#[derive(Clone)]
pub struct TunnelManager {
    tunnels_dir: PathBuf,
    runtime: Arc<Mutex<TunnelRuntime>>,
    app: Arc<Mutex<Option<AppHandle>>>,
    logger: CommandLogger,
}

impl TunnelManager {
    pub fn new(user_data_dir: &Path, logger: CommandLogger) -> Self {
        let tunnels_dir = user_data_dir.join("tunnels");
        let _ = fs::create_dir_all(&tunnels_dir);
        Self {
            tunnels_dir,
            runtime: Arc::new(Mutex::new(TunnelRuntime {
                state: TunnelState {
                    active: false,
                    busy: false,
                    mode: "wiresock".into(),
                    clients: TunnelClientsStatus::default(),
                    active_exe: None,
                    active_allowed_ips: Vec::new(),
                    endpoint: None,
                    started_at: None,
                    last_error: None,
                    generated_preview: None,
                },
                wiresock_child: None,
                wireguard_service_installed: false,
            })),
            app: Arc::new(Mutex::new(None)),
            logger,
        }
    }

    pub fn attach_app(&self, handle: AppHandle) {
        if let Ok(mut g) = self.app.lock() {
            *g = Some(handle);
        }
    }

    fn emit_state(&self) {
        if let (Ok(rt), Ok(app_g)) = (self.runtime.lock(), self.app.lock()) {
            if let Some(app) = app_g.as_ref() {
                let _ = app.emit("event:tunnelState", &rt.state);
            }
        }
    }

    pub fn cleanup_stale_configs(&self) -> usize {
        let mut removed = 0usize;
        if let Ok(rd) = fs::read_dir(&self.tunnels_dir) {
            for entry in rd.flatten() {
                if entry.path().is_file() && fs::remove_file(entry.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        if removed > 0 {
            self.logger.info(
                "tunnel",
                "Nettoyage dossier tunnels/ au démarrage",
                &format!("{} fichier(s) temporaire(s) résiduel(s) supprimé(s).", removed),
                "ok",
            );
        }
        removed
    }

    pub fn refresh_clients(&self) -> TunnelState {
        let clients = detect_tunnel_clients(&self.logger);
        let snap = if let Ok(mut rt) = self.runtime.lock() {
            rt.state.clients = clients;
            rt.state.clone()
        } else {
            return self.get_state();
        };
        self.emit_state();
        snap
    }

    pub fn get_state(&self) -> TunnelState {
        self.runtime
            .lock()
            .map(|rt| rt.state.clone())
            .unwrap_or(TunnelState {
                active: false,
                busy: false,
                mode: "wiresock".into(),
                clients: TunnelClientsStatus::default(),
                active_exe: None,
                active_allowed_ips: Vec::new(),
                endpoint: None,
                started_at: None,
                last_error: None,
                generated_preview: None,
            })
    }

    pub fn is_active(&self) -> bool {
        self.runtime
            .lock()
            .map(|rt| rt.state.active || rt.wiresock_child.is_some() || rt.wireguard_service_installed)
            .unwrap_or(false)
    }

    pub fn start_tunnel(
        &self,
        profile: &WireGuardProfile,
        raw_conf: &str,
        mode: &str,
        exe_name: Option<&str>,
        target_host: &str,
        extra_ips: &[String],
    ) -> TunnelActionResult {
        {
            if let Ok(mut rt) = self.runtime.lock() {
                if rt.state.busy {
                    return TunnelActionResult {
                        ok: false,
                        message: "Opération tunnel déjà en cours.".into(),
                        state: rt.state.clone(),
                    };
                }
                rt.state.busy = true;
                rt.state.mode = mode.to_string();
                rt.state.last_error = None;
            }
        }
        self.emit_state();

        let res = self.start_tunnel_inner(profile, raw_conf, mode, exe_name, target_host, extra_ips);

        if let Ok(mut rt) = self.runtime.lock() {
            rt.state.busy = false;
        }
        self.emit_state();

        TunnelActionResult {
            ok: res.is_ok(),
            message: match res {
                Ok(m) => m,
                Err(e) => e,
            },
            state: self.get_state(),
        }
    }

    fn start_tunnel_inner(
        &self,
        profile: &WireGuardProfile,
        raw_conf: &str,
        mode: &str,
        exe_name: Option<&str>,
        target_host: &str,
        extra_ips: &[String],
    ) -> Result<String, String> {
        if !is_admin_cached() {
            let msg = "Droits Administrateur (UAC) requis pour démarrer le tunnel WireSock / WireGuard.".to_string();
            if let Ok(mut rt) = self.runtime.lock() {
                rt.state.last_error = Some(msg.clone());
            }
            self.logger.info("tunnel", &format!("Tunnel {}", mode), &msg, "error");
            return Err(msg);
        }

        let clients = detect_tunnel_clients(&self.logger);
        if let Ok(mut rt) = self.runtime.lock() {
            rt.state.clients = clients.clone();
        }
        self.stop_tunnel_internal(true);

        let mut allowed_ips = resolve_host_ipv4s(target_host);
        for ip in extra_ips {
            if ip.parse::<Ipv4Addr>().is_ok() && !allowed_ips.contains(ip) {
                allowed_ips.push(ip.clone());
            }
        }

        let generated_conf = generate_split_tunnel_conf(raw_conf, mode, exe_name, &allowed_ips);
        let preview = mask_private_key_for_preview(&generated_conf);
        if let Ok(mut rt) = self.runtime.lock() {
            rt.state.generated_preview = Some(preview);
        }

        if mode == "wiresock" {
            let Some(bin) = clients.wiresock_path else {
                let msg = "Binaire signé wiresock-client.exe introuvable dans Program Files.".to_string();
                if let Ok(mut rt) = self.runtime.lock() {
                    rt.state.last_error = Some(msg.clone());
                }
                self.logger.info("tunnel", "wiresock-client.exe", &msg, "error");
                return Err(msg);
            };

            let conf_path = self.tunnels_dir.join("netboost-wiresock.conf");
            fs::write(&conf_path, &generated_conf).map_err(|e| e.to_string())?;
            restrict_file_acl_to_current_user(&conf_path, &self.logger);

            self.logger.info(
                "tunnel",
                &format!("\"{}\" run -config \"{}\"", bin, conf_path.display()),
                &format!("Démarrage WireSock Split Tunnel vers {}...", profile.endpoint),
                "ok",
            );

            let mut child = Command::new(&bin)
                .args(["run", "-config", &conf_path.to_string_lossy(), "-log-level", "all"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|e| format!("Échec lancement wiresock-client.exe : {}", e))?;

            std::thread::sleep(Duration::from_millis(800));
            if let Ok(Some(status)) = child.try_wait() {
                self.cleanup_stale_configs();
                let msg = format!("WireSock s'est arrêté immédiatement (code {:?}).", status.code());
                if let Ok(mut rt) = self.runtime.lock() {
                    rt.state.last_error = Some(msg.clone());
                }
                return Err(msg);
            }

            if let Ok(mut rt) = self.runtime.lock() {
                rt.wiresock_child = Some(child);
                rt.state.active = true;
                rt.state.active_exe = exe_name.map(|s| s.to_string());
                rt.state.active_allowed_ips = allowed_ips;
                rt.state.endpoint = Some(profile.endpoint.clone());
                rt.state.started_at = Some(Utc::now().to_rfc3339());
            }

            Ok(format!("Tunnel WireSock actif vers {}.", profile.endpoint))
        } else {
            let Some(bin) = clients.wireguard_path else {
                let msg = "Binaire signé wireguard.exe introuvable dans Program Files.".to_string();
                if let Ok(mut rt) = self.runtime.lock() {
                    rt.state.last_error = Some(msg.clone());
                }
                self.logger.info("tunnel", "wireguard.exe", &msg, "error");
                return Err(msg);
            };

            let conf_path = self.tunnels_dir.join(format!("{}.conf", WG_TUNNEL_NAME));
            fs::write(&conf_path, &generated_conf).map_err(|e| e.to_string())?;
            restrict_file_acl_to_current_user(&conf_path, &self.logger);

            let conf_s = conf_path.to_string_lossy().to_string();
            let res = self
                .logger
                .run_exe(&bin, &["/installtunnelservice", &conf_s], "tunnel", false);
            if !res.ok {
                self.cleanup_stale_configs();
                let msg = if res.stderr.is_empty() {
                    "Échec wireguard.exe /installtunnelservice.".to_string()
                } else {
                    res.stderr
                };
                if let Ok(mut rt) = self.runtime.lock() {
                    rt.state.last_error = Some(msg.clone());
                }
                return Err(msg);
            }

            if let Ok(mut rt) = self.runtime.lock() {
                rt.wireguard_service_installed = true;
                rt.state.active = true;
                rt.state.active_exe = exe_name.map(|s| s.to_string());
                rt.state.active_allowed_ips = allowed_ips;
                rt.state.endpoint = Some(profile.endpoint.clone());
                rt.state.started_at = Some(Utc::now().to_rfc3339());
            }

            Ok(format!(
                "Tunnel WireGuard ({}) actif vers {}.",
                WG_TUNNEL_NAME, profile.endpoint
            ))
        }
    }

    fn stop_tunnel_internal(&self, silent_if_idle: bool) -> String {
        let mut did_stop = false;

        let (child_opt, wg_installed, wg_bin) = if let Ok(mut rt) = self.runtime.lock() {
            let c = rt.wiresock_child.take();
            let w = rt.wireguard_service_installed;
            rt.wireguard_service_installed = false;
            (c, w, rt.state.clients.wireguard_path.clone())
        } else {
            (None, false, None)
        };

        if let Some(mut child) = child_opt {
            let pid = child.id();
            let _ = child.kill();
            let _ = child.wait();
            self.logger.info(
                "tunnel",
                "Arrêt WireSock",
                &format!("Processus wiresock-client (PID {}) arrêté.", pid),
                "ok",
            );
            did_stop = true;
        }

        if wg_installed {
            let bin = wg_bin.or_else(|| {
                WIREGUARD_TRUSTED_PATHS
                    .iter()
                    .find(|p| Path::new(p).exists())
                    .map(|s| (*s).to_string())
            });
            if let Some(b) = bin {
                let _ = self
                    .logger
                    .run_exe(&b, &["/uninstalltunnelservice", WG_TUNNEL_NAME], "tunnel", false);
                did_stop = true;
            }
        }

        self.cleanup_stale_configs();

        if let Ok(mut rt) = self.runtime.lock() {
            rt.state.active = false;
            rt.state.started_at = None;
            rt.state.active_exe = None;
            rt.state.active_allowed_ips.clear();
            rt.state.endpoint = None;
        }

        if !did_stop && !silent_if_idle {
            self.logger.info("tunnel", "Arrêt Tunnel", "Aucun tunnel actif à couper.", "ok");
        }

        if did_stop {
            "Tunnel EU coupé et profil temporaire nettoyé.".into()
        } else {
            "Aucun tunnel actif.".into()
        }
    }

    pub fn stop_tunnel(&self) -> TunnelActionResult {
        if let Ok(mut rt) = self.runtime.lock() {
            rt.state.busy = true;
        }
        self.emit_state();

        let msg = self.stop_tunnel_internal(false);

        if let Ok(mut rt) = self.runtime.lock() {
            rt.state.busy = false;
        }
        self.emit_state();

        TunnelActionResult {
            ok: true,
            message: msg,
            state: self.get_state(),
        }
    }
}
