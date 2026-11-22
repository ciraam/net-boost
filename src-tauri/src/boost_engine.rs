use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use tauri::{AppHandle, Emitter};

use crate::firewall::{
    create_firewall_block_rule, remove_all_netboost_firewall_rules, remove_firewall_block_rule,
};
use crate::icmp::PingMonitor;
use crate::logger::{ps_single_quote, CommandLogger};
use crate::network::{
    benchmark_dns_parallel, get_active_network_adapter, get_eee_props_to_disable,
};
use crate::processes::{
    control_windows_service, find_pid_by_exe, query_windows_service_status,
    scan_background_traffic, NativeGameWatcher,
};
use crate::registry::{
    apply_tcpip_tweaks, enable_qos_do_not_use_nla, read_registry_snapshot,
    restore_registry_from_backup,
};
use crate::security::{is_admin_cached, sanitize_exe_name, sanitize_policy_suffix};
use crate::store::AppStore;
use crate::tunnel::TunnelManager;
use crate::types::{
    ActionResult, BackupAdapterState, BackupPausedService, BackupState, BoostOptions, BoostState,
    BoostStepStatus, FirewallBlockRule, GameProfile, RollbackResult, SecondaryDnsAdapterBackup,
    TunnelActionResult,
};

fn default_steps() -> Vec<BoostStepStatus> {
    vec![
        BoostStepStatus {
            key: "backup".into(),
            label: "Sauvegarde JSON d'origine".into(),
            status: "idle".into(),
            detail: "En attente".into(),
        },
        BoostStepStatus {
            key: "tunnel".into(),
            label: "Tunnel EU Split (WireSock/WG)".into(),
            status: "idle".into(),
            detail: "En attente".into(),
        },
        BoostStepStatus {
            key: "qos".into(),
            label: "Priorité QoS Jeu (DSCP 46 + NLA)".into(),
            status: "idle".into(),
            detail: "En attente".into(),
        },
        BoostStepStatus {
            key: "dns".into(),
            label: "Benchmark DNS + Flush".into(),
            status: "idle".into(),
            detail: "En attente".into(),
        },
        BoostStepStatus {
            key: "tcpip".into(),
            label: "TCPNoDelay / AckFreq / Throttling".into(),
            status: "idle".into(),
            detail: "En attente".into(),
        },
        BoostStepStatus {
            key: "eee".into(),
            label: "Désactivation EEE Carte Réseau".into(),
            status: "idle".into(),
            detail: "En attente".into(),
        },
        BoostStepStatus {
            key: "background".into(),
            label: "Contrôle trafic arrière-plan".into(),
            status: "idle".into(),
            detail: "En attente".into(),
        },
    ]
}

struct EngineInner {
    state: BoostState,
    throttled_exes: HashSet<String>,
    paused_services: HashSet<String>,
    firewall_rules: Vec<FirewallBlockRule>,
}

#[derive(Clone)]
pub struct BoostEngine {
    store: AppStore,
    tunnel: TunnelManager,
    ping_monitor: PingMonitor,
    watcher: NativeGameWatcher,
    logger: CommandLogger,
    app: Arc<Mutex<Option<AppHandle>>>,
    inner: Arc<Mutex<EngineInner>>,
}

impl BoostEngine {
    pub fn new(
        store: AppStore,
        tunnel: TunnelManager,
        ping_monitor: PingMonitor,
        logger: CommandLogger,
    ) -> Self {
        let mut throttled_exes = HashSet::new();
        let mut paused_services = HashSet::new();
        let mut firewall_rules = Vec::new();

        if let Some(backup) = store.read_backup() {
            for e in backup.throttled_exes {
                throttled_exes.insert(e.to_ascii_lowercase());
            }
            for s in backup.services_paused {
                paused_services.insert(s.name.to_ascii_lowercase());
            }
            firewall_rules = backup.firewall_rules_created;
        }

        let state = BoostState {
            active: false,
            busy: false,
            started_at: None,
            target_game: None,
            target_host: "1.1.1.1".into(),
            tracked_pid: None,
            applied_dns: None,
            steps: default_steps(),
            has_backup: store.has_backup(),
            backup_path: store.backup_path().to_string_lossy().to_string(),
        };

        Self {
            store,
            tunnel,
            ping_monitor,
            watcher: NativeGameWatcher::new(),
            logger,
            app: Arc::new(Mutex::new(None)),
            inner: Arc::new(Mutex::new(EngineInner {
                state,
                throttled_exes,
                paused_services,
                firewall_rules,
            })),
        }
    }

    pub fn attach_app(&self, handle: AppHandle) {
        if let Ok(mut g) = self.app.lock() {
            *g = Some(handle);
        }
    }

    fn emit_state(&self) {
        let has_b = self.store.has_backup();
        let snap = if let Ok(mut g) = self.inner.lock() {
            g.state.has_backup = has_b;
            g.state.clone()
        } else {
            return;
        };
        if let Ok(app_g) = self.app.lock() {
            if let Some(app) = app_g.as_ref() {
                let _ = app.emit("event:boostState", &snap);
            }
        }
    }

    fn emit_notice(&self, msg: &str) {
        self.ping_monitor.set_phase("before");
        if let Ok(app_g) = self.app.lock() {
            if let Some(app) = app_g.as_ref() {
                let _ = app.emit("event:autoRollbackNotice", msg);
            }
        }
    }

    pub fn get_state(&self) -> BoostState {
        let has_b = self.store.has_backup();
        if let Ok(mut g) = self.inner.lock() {
            g.state.has_backup = has_b;
            return g.state.clone();
        }
        BoostState {
            active: false,
            busy: false,
            started_at: None,
            target_game: None,
            target_host: "1.1.1.1".into(),
            tracked_pid: None,
            applied_dns: None,
            steps: default_steps(),
            has_backup: has_b,
            backup_path: self.store.backup_path().to_string_lossy().to_string(),
        }
    }

    pub fn get_throttled_exes(&self) -> HashSet<String> {
        self.inner
            .lock()
            .map(|g| g.throttled_exes.clone())
            .unwrap_or_default()
    }

    pub fn get_paused_services(&self) -> HashSet<String> {
        self.inner
            .lock()
            .map(|g| g.paused_services.clone())
            .unwrap_or_default()
    }

    pub fn get_firewall_rules(&self) -> Vec<FirewallBlockRule> {
        self.inner
            .lock()
            .map(|g| g.firewall_rules.clone())
            .unwrap_or_default()
    }

    pub fn has_pending_modifications(&self) -> bool {
        let active_or_tracked = self
            .inner
            .lock()
            .map(|g| {
                g.state.active
                    || !g.throttled_exes.is_empty()
                    || !g.paused_services.is_empty()
                    || !g.firewall_rules.is_empty()
            })
            .unwrap_or(false);
        active_or_tracked || self.store.has_backup() || self.tunnel.is_active()
    }

    fn update_step(&self, key: &str, status: &str, detail: &str) {
        if let Ok(mut g) = self.inner.lock() {
            if let Some(step) = g.state.steps.iter_mut().find(|s| s.key == key) {
                step.status = status.to_string();
                step.detail = detail.to_string();
            }
        }
        self.emit_state();
    }

    pub fn ensure_backup_before_change(&self) -> Result<BackupState, String> {
        if let Some(existing) = self.store.read_backup() {
            self.logger.info(
                "system",
                "Sauvegarde JSON existante conservée",
                &format!("Fichier : {}", self.store.backup_path().display()),
                "ok",
            );
            return Ok(existing);
        }

        let adapter = get_active_network_adapter(true, &self.logger);
        let registry = read_registry_snapshot(
            adapter.as_ref().map(|a| a.interface_guid.as_str()),
            &self.logger,
        );

        let backup = BackupState {
            version: 1,
            created_at: Utc::now().to_rfc3339(),
            adapter: adapter.as_ref().map(|a| BackupAdapterState {
                name: a.name.clone(),
                interface_index: a.interface_index,
                interface_guid: a.interface_guid.clone(),
                dns_is_dhcp: a.dns_is_dhcp,
                original_dns: a.current_dns.clone(),
                eee_properties: a.eee_properties.clone(),
            }),
            secondary_dns_adapters: Vec::new(),
            registry,
            qos_policies_created: Vec::new(),
            throttled_exes: Vec::new(),
            firewall_rules_created: Vec::new(),
            tunnel_was_started: false,
            services_paused: Vec::new(),
        };

        self.store.write_backup(&backup)?;
        self.logger.info(
            "system",
            "Sauvegarde JSON créée avant modification",
            &format!(
                "Sauvegardé dans {} (Adaptateur: {})",
                self.store.backup_path().display(),
                adapter.as_ref().map(|a| a.name.as_str()).unwrap_or("Aucun")
            ),
            "ok",
        );
        self.emit_state();
        Ok(backup)
    }

    fn resolve_current_dns_adapter(&self, backup: &mut BackupState) -> Option<(String, u32)> {
        if let Some(live) = get_active_network_adapter(false, &self.logger) {
            let differs = backup
                .adapter
                .as_ref()
                .map(|b| b.interface_index != live.interface_index)
                .unwrap_or(true);
            if differs
                && !backup
                    .secondary_dns_adapters
                    .iter()
                    .any(|s| s.interface_index == live.interface_index)
            {
                backup.secondary_dns_adapters.push(SecondaryDnsAdapterBackup {
                    name: live.name.clone(),
                    interface_index: live.interface_index,
                    dns_is_dhcp: live.dns_is_dhcp,
                    original_dns: live.current_dns.clone(),
                });
                let _ = self.store.write_backup(backup);
            }
            return Some((live.name, live.interface_index));
        }
        backup
            .adapter
            .as_ref()
            .map(|a| (a.name.clone(), a.interface_index))
    }

    pub fn block_remote_address(
        &self,
        ip_or_cidr: &str,
        exe_name: Option<&str>,
        exe_path: Option<&str>,
        country_code: Option<&str>,
    ) -> ActionResult {
        let mut backup = match self.ensure_backup_before_change() {
            Ok(b) => b,
            Err(e) => return ActionResult { ok: false, message: e },
        };

        match create_firewall_block_rule(ip_or_cidr, exe_name, exe_path, country_code, &self.logger)
        {
            Ok(rule) => {
                if let Ok(mut g) = self.inner.lock() {
                    if !g.firewall_rules.iter().any(|r| r.rule_name == rule.rule_name) {
                        g.firewall_rules.push(rule.clone());
                    }
                    backup.firewall_rules_created = g.firewall_rules.clone();
                }
                let _ = self.store.write_backup(&backup);
                self.emit_state();
                ActionResult {
                    ok: true,
                    message: format!(
                        "Blocage Pare-feu Windows (COM INetFwPolicy2) actif sur {}.",
                        rule.remote_address
                    ),
                }
            }
            Err(e) => ActionResult { ok: false, message: e },
        }
    }

    pub fn unblock_remote_rule(&self, rule_name: &str) -> ActionResult {
        match remove_firewall_block_rule(rule_name, &self.logger) {
            Ok(()) => {
                if let Ok(mut g) = self.inner.lock() {
                    g.firewall_rules.retain(|r| r.rule_name != rule_name);
                    if let Some(mut b) = self.store.read_backup() {
                        b.firewall_rules_created = g.firewall_rules.clone();
                        let _ = self.store.write_backup(&b);
                    }
                }
                self.emit_state();
                ActionResult {
                    ok: true,
                    message: format!("Règle Pare-feu {} supprimée.", rule_name),
                }
            }
            Err(e) => ActionResult { ok: false, message: e },
        }
    }

    pub fn start_split_tunnel(
        &self,
        mode: &str,
        exe_name: Option<&str>,
        target_host: &str,
        extra_ips: &[String],
    ) -> TunnelActionResult {
        let decrypted = match self.store.get_decrypted_wireguard_conf() {
            Ok(Some(v)) => v,
            Ok(None) => {
                return TunnelActionResult {
                    ok: false,
                    message: "Aucun fichier .conf WireGuard importé.".into(),
                    state: self.tunnel.get_state(),
                }
            }
            Err(e) => {
                return TunnelActionResult {
                    ok: false,
                    message: e,
                    state: self.tunnel.get_state(),
                }
            }
        };

        let mut backup = match self.ensure_backup_before_change() {
            Ok(b) => b,
            Err(e) => {
                return TunnelActionResult {
                    ok: false,
                    message: e,
                    state: self.tunnel.get_state(),
                }
            }
        };

        let res = self.tunnel.start_tunnel(
            &decrypted.0,
            &decrypted.1,
            mode,
            exe_name,
            target_host,
            extra_ips,
        );
        if res.ok {
            backup.tunnel_was_started = true;
            let _ = self.store.write_backup(&backup);
            self.emit_state();
        }
        res
    }

    pub fn apply_dns_servers(&self, servers: &[String]) -> ActionResult {
        if !is_admin_cached() {
            let msg = "Droits Administrateur (UAC) requis pour modifier les serveurs DNS.".to_string();
            self.logger.info("dns", "Set-DnsClientServerAddress", &msg, "error");
            return ActionResult { ok: false, message: msg };
        }

        let mut backup = match self.ensure_backup_before_change() {
            Ok(b) => b,
            Err(e) => return ActionResult { ok: false, message: e },
        };

        let Some((adapter_name, if_index)) = self.resolve_current_dns_adapter(&mut backup) else {
            return ActionResult {
                ok: false,
                message: "Aucun adaptateur réseau actif détecté.".into(),
            };
        };

        let clean: Vec<String> = servers
            .iter()
            .filter(|s| s.trim().parse::<std::net::Ipv4Addr>().is_ok())
            .map(|s| s.trim().to_string())
            .collect();
        if clean.is_empty() {
            return ActionResult {
                ok: false,
                message: "Aucune adresse DNS IPv4 valide fournie.".into(),
            };
        }

        let addr_list = clean
            .iter()
            .map(|s| ps_single_quote(s))
            .collect::<Vec<_>>()
            .join(",");
        let cmd = format!(
            "Set-DnsClientServerAddress -InterfaceIndex {} -ServerAddresses @({}); Clear-DnsClientCache",
            if_index, addr_list
        );
        let res = self.logger.run_powershell(&cmd, "dns", &cmd, false);
        let _ = self.logger.run_exe("ipconfig.exe", &["/flushdns"], "dns", false);

        if res.ok {
            if let Ok(mut g) = self.inner.lock() {
                g.state.applied_dns = Some(clean.clone());
            }
            self.emit_state();
            ActionResult {
                ok: true,
                message: format!(
                    "DNS appliqué sur {} ({}) + cache DNS vidé.",
                    adapter_name,
                    clean.join(", ")
                ),
            }
        } else {
            ActionResult {
                ok: false,
                message: if res.stderr.is_empty() {
                    "Échec de l'application DNS.".into()
                } else {
                    res.stderr
                },
            }
        }
    }

    pub fn toggle_app_throttle(&self, exe_name: &str, throttle: bool) -> ActionResult {
        if !is_admin_cached() {
            let msg = "Droits Administrateur (UAC) requis pour appliquer une politique QoS DSCP 8.".to_string();
            self.logger.info("background", &format!("QoS DSCP 8 ({})", exe_name), &msg, "error");
            return ActionResult { ok: false, message: msg };
        }

        let clean_exe = match sanitize_exe_name(exe_name) {
            Ok(e) => e,
            Err(err) => return ActionResult { ok: false, message: err },
        };
        let policy_name = format!("NetBoost_Throttle_{}", sanitize_policy_suffix(&clean_exe));

        if throttle {
            let mut backup = match self.ensure_backup_before_change() {
                Ok(b) => b,
                Err(e) => return ActionResult { ok: false, message: e },
            };
            let _ = enable_qos_do_not_use_nla(&self.logger);
            let script = format!(
                "Remove-NetQosPolicy -Name {} -Confirm:$false -ErrorAction SilentlyContinue\n\
                 New-NetQosPolicy -Name {} -AppPathNameMatchCondition {} -DSCPAction 8 -IPProtocolMatchCondition Both -NetworkProfile All | Out-Null\n\
                 Write-Output 'DSCP 8 appliqué'",
                ps_single_quote(&policy_name),
                ps_single_quote(&policy_name),
                ps_single_quote(&clean_exe)
            );
            let res = self.logger.run_powershell(
                &script,
                "background",
                &format!(
                    "New-NetQosPolicy -Name \"{}\" -AppPathNameMatchCondition \"{}\" -DSCPAction 8",
                    policy_name, clean_exe
                ),
                false,
            );
            if res.ok {
                let lower = clean_exe.to_ascii_lowercase();
                if let Ok(mut g) = self.inner.lock() {
                    g.throttled_exes.insert(lower.clone());
                }
                if !backup.qos_policies_created.contains(&policy_name) {
                    backup.qos_policies_created.push(policy_name);
                }
                if !backup.throttled_exes.contains(&lower) {
                    backup.throttled_exes.push(lower);
                }
                let _ = self.store.write_backup(&backup);
                self.emit_state();
                ActionResult {
                    ok: true,
                    message: format!("{} bridé en basse priorité (DSCP 8).", clean_exe),
                }
            } else {
                ActionResult {
                    ok: false,
                    message: res.stderr,
                }
            }
        } else {
            let script = format!(
                "Remove-NetQosPolicy -Name {} -Confirm:$false -ErrorAction SilentlyContinue",
                ps_single_quote(&policy_name)
            );
            let res = self.logger.run_powershell(
                &script,
                "background",
                &format!("Remove-NetQosPolicy -Name \"{}\"", policy_name),
                false,
            );
            let lower = clean_exe.to_ascii_lowercase();
            if let Ok(mut g) = self.inner.lock() {
                g.throttled_exes.remove(&lower);
            }
            if let Some(mut b) = self.store.read_backup() {
                b.qos_policies_created.retain(|p| p != &policy_name);
                b.throttled_exes.retain(|e| e != &lower);
                let _ = self.store.write_backup(&b);
            }
            self.emit_state();
            ActionResult {
                ok: res.ok,
                message: if res.ok {
                    format!("Bridage retiré pour {}.", clean_exe)
                } else {
                    res.stderr
                },
            }
        }
    }

    pub fn toggle_service_pause(&self, service_name: &str, pause: bool) -> ActionResult {
        let clean = service_name.trim().to_ascii_lowercase();
        if pause {
            let mut backup = match self.ensure_backup_before_change() {
                Ok(b) => b,
                Err(e) => return ActionResult { ok: false, message: e },
            };
            let orig_status = query_windows_service_status(&clean)
                .map(|(_, st)| st)
                .unwrap_or_else(|| "Running".into());
            match control_windows_service(&clean, true, &self.logger) {
                Ok(()) => {
                    if let Ok(mut g) = self.inner.lock() {
                        g.paused_services.insert(clean.clone());
                    }
                    if !backup
                        .services_paused
                        .iter()
                        .any(|s| s.name.eq_ignore_ascii_case(&clean))
                    {
                        backup.services_paused.push(BackupPausedService {
                            name: clean.clone(),
                            original_status: orig_status,
                            original_start_type: "Windows SCM".into(),
                        });
                        let _ = self.store.write_backup(&backup);
                    }
                    self.emit_state();
                    ActionResult {
                        ok: true,
                        message: format!("Service {} mis en pause (Win32 SCM).", clean),
                    }
                }
                Err(e) => ActionResult { ok: false, message: e },
            }
        } else {
            match control_windows_service(&clean, false, &self.logger) {
                Ok(()) => {
                    if let Ok(mut g) = self.inner.lock() {
                        g.paused_services.remove(&clean);
                    }
                    if let Some(mut b) = self.store.read_backup() {
                        b.services_paused
                            .retain(|s| !s.name.eq_ignore_ascii_case(&clean));
                        let _ = self.store.write_backup(&b);
                    }
                    self.emit_state();
                    ActionResult {
                        ok: true,
                        message: format!("Service {} relancé (Win32 SCM).", clean),
                    }
                }
                Err(e) => ActionResult { ok: false, message: e },
            }
        }
    }

    pub async fn start_boost(
        &self,
        game: Option<GameProfile>,
        host: String,
        options: BoostOptions,
    ) -> BoostState {
        {
            if let Ok(mut g) = self.inner.lock() {
                if g.state.busy {
                    return g.state.clone();
                }
                g.state.busy = true;
                g.state.target_game = game.clone();
                g.state.target_host = host.clone();
                g.state.steps = default_steps();
            }
        }
        self.emit_state();

        let is_admin = is_admin_cached();
        if !is_admin {
            self.logger.info(
                "uac",
                "Vérification UAC avant Boost",
                "Application non lancée en Administrateur : les modifications Tunnel / QoS / DNS / Registre / EEE nécessitent UAC.",
                "warn",
            );
        }

        // 1. Mandatory JSON backup before any modification
        self.update_step("backup", "running", "Capture de la configuration Windows actuelle...");
        let mut backup = match self.ensure_backup_before_change() {
            Ok(b) => {
                self.update_step(
                    "backup",
                    "applied",
                    &format!(
                        "Sauvegardé ({})",
                        b.adapter.as_ref().map(|a| a.name.as_str()).unwrap_or("sans adaptateur")
                    ),
                );
                b
            }
            Err(e) => {
                self.update_step("backup", "error", &e);
                if let Ok(mut g) = self.inner.lock() {
                    g.state.busy = false;
                }
                self.emit_state();
                return self.get_state();
            }
        };

        let mut any_applied = false;

        // 2. EU Split Tunnel
        if !options.enable_tunnel {
            if self.tunnel.is_active() {
                self.update_step("tunnel", "running", "Coupure du tunnel EU actif (désactivé)...");
                let _ = self.tunnel.stop_tunnel();
                self.update_step("tunnel", "skipped", "Tunnel EU coupé (désactivé dans les options)");
            } else {
                self.update_step("tunnel", "skipped", "Désactivé (activable dans le cockpit ou Tunnel EU)");
            }
        } else {
            let cfg = self.store.load_config();
            match self.store.get_decrypted_wireguard_conf() {
                Ok(Some((profile, raw_conf))) => {
                    self.update_step(
                        "tunnel",
                        "running",
                        &format!("Lancement Split Tunnel ({}) vers {}...", cfg.tunnel_mode, profile.endpoint),
                    );
                    let t_res = self.tunnel.start_tunnel(
                        &profile,
                        &raw_conf,
                        &cfg.tunnel_mode,
                        game.as_ref().map(|g| g.exe_name.as_str()),
                        &host,
                        &[],
                    );
                    if t_res.ok {
                        backup.tunnel_was_started = true;
                        let _ = self.store.write_backup(&backup);
                        self.update_step(
                            "tunnel",
                            "applied",
                            &format!("{} → {}", cfg.tunnel_mode, profile.endpoint),
                        );
                        any_applied = true;
                    } else {
                        self.update_step("tunnel", "error", &t_res.message);
                    }
                }
                Ok(None) => {
                    self.update_step("tunnel", "skipped", "Aucun fichier .conf WireGuard importé");
                }
                Err(e) => {
                    self.update_step("tunnel", "error", &e);
                }
            }
        }

        // 3. QoS Priority (DSCP 46 + Do not use NLA = 1)
        if options.enable_qos {
            if let Some(ref g_prof) = game {
                self.update_step("qos", "running", &format!("Application DSCP 46 + NLA=1 sur {}...", g_prof.exe_name));
                match sanitize_exe_name(&g_prof.exe_name) {
                    Ok(clean_exe) => {
                        let policy_name = format!("NetBoost_Game_{}", sanitize_policy_suffix(&clean_exe));
                        let _ = enable_qos_do_not_use_nla(&self.logger);
                        let script = format!(
                            "Remove-NetQosPolicy -Name {} -Confirm:$false -ErrorAction SilentlyContinue\n\
                             New-NetQosPolicy -Name {} -AppPathNameMatchCondition {} -DSCPAction 46 -IPProtocolMatchCondition Both -NetworkProfile All | Select-Object Name, DSCPValue | ConvertTo-Json -Compress",
                            ps_single_quote(&policy_name),
                            ps_single_quote(&policy_name),
                            ps_single_quote(&clean_exe)
                        );
                        let res = self.logger.run_powershell(
                            &script,
                            "qos",
                            &format!(
                                "New-NetQosPolicy -Name \"{}\" -AppPathNameMatchCondition \"{}\" -DSCPAction 46",
                                policy_name, clean_exe
                            ),
                            false,
                        );
                        if res.ok {
                            if !backup.qos_policies_created.contains(&policy_name) {
                                backup.qos_policies_created.push(policy_name);
                                let _ = self.store.write_backup(&backup);
                            }
                            self.update_step("qos", "applied", &format!("DSCP 46 actif (NLA=1) sur {}", clean_exe));
                            any_applied = true;
                        } else {
                            self.update_step(
                                "qos",
                                "error",
                                if is_admin {
                                    res.stderr.lines().next().unwrap_or("Erreur New-NetQosPolicy")
                                } else {
                                    "Droits Administrateur UAC requis"
                                },
                            );
                        }
                    }
                    Err(e) => self.update_step("qos", "error", &e),
                }
            } else {
                self.update_step("qos", "skipped", "Aucun jeu (.exe) sélectionné");
            }
        } else {
            self.update_step("qos", "skipped", "Désactivé dans les options");
        }

        // 4. Parallel Native UDP DNS Benchmark + Apply on Live Active Adapter + Flush
        if options.enable_dns {
            self.update_step("dns", "running", "Benchmark UDP natif 1.1.1.1 / 8.8.8.8 / 9.9.9.9...");
            let bench = benchmark_dns_parallel(&self.logger).await;
            let target_adapter = self.resolve_current_dns_adapter(&mut backup);
            let mut valid: Vec<_> = bench.into_iter().filter(|r| r.avg_ms.is_some()).collect();
            valid.sort_by(|a, b| {
                a.avg_ms
                    .unwrap_or(9999.0)
                    .partial_cmp(&b.avg_ms.unwrap_or(9999.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            if let (Some(best), Some((ad_name, if_idx))) = (valid.first(), target_adapter) {
                let chosen: Vec<String> = valid.iter().take(2).map(|v| v.ip.clone()).collect();
                let addr_list = chosen
                    .iter()
                    .map(|s| ps_single_quote(s))
                    .collect::<Vec<_>>()
                    .join(",");
                let cmd = format!(
                    "Set-DnsClientServerAddress -InterfaceIndex {} -ServerAddresses @({}); Clear-DnsClientCache",
                    if_idx, addr_list
                );
                let res = self.logger.run_powershell(&cmd, "dns", &cmd, false);
                let _ = self.logger.run_exe("ipconfig.exe", &["/flushdns"], "dns", false);
                if res.ok {
                    if let Ok(mut g) = self.inner.lock() {
                        g.state.applied_dns = Some(chosen);
                    }
                    self.update_step(
                        "dns",
                        "applied",
                        &format!(
                            "{} ({}, {} ms) sur {}",
                            best.name,
                            best.ip,
                            best.avg_ms.unwrap_or(0.0),
                            ad_name
                        ),
                    );
                    any_applied = true;
                } else {
                    self.update_step(
                        "dns",
                        "error",
                        if is_admin {
                            res.stderr.lines().next().unwrap_or("Erreur DNS")
                        } else {
                            "Droits Administrateur UAC requis"
                        },
                    );
                }
            } else {
                self.update_step("dns", "error", "Aucun adaptateur actif ou DNS joignable");
            }
        } else {
            self.update_step("dns", "skipped", "Désactivé dans les options");
        }

        // 5. Native winreg TCP/IP Optimizations
        if options.enable_tcp_ip {
            self.update_step("tcpip", "running", "Écriture winreg TCPNoDelay, TcpAckFrequency, Throttling...");
            match apply_tcpip_tweaks(backup.registry.interface_key_path.as_deref(), &self.logger) {
                Ok(()) => {
                    self.update_step("tcpip", "applied", "TCPNoDelay=1, AckFreq=1, Throttling=OFF");
                    any_applied = true;
                }
                Err(e) => self.update_step(
                    "tcpip",
                    "error",
                    if is_admin { &e } else { "Droits Administrateur UAC requis" },
                ),
            }
        } else {
            self.update_step("tcpip", "skipped", "Désactivé dans les options");
        }

        // 6. Disable Adapter EEE
        if options.disable_eee {
            self.update_step("eee", "running", "Vérification propriétés EEE de la carte réseau...");
            if let Some(ref ad) = backup.adapter {
                let targets = get_eee_props_to_disable(&ad.eee_properties);
                if ad.eee_properties.is_empty() {
                    self.update_step("eee", "skipped", &format!("Aucune option EEE exposée sur {}", ad.name));
                } else if targets.is_empty() {
                    self.update_step("eee", "applied", &format!("EEE déjà désactivé sur {}", ad.name));
                    any_applied = true;
                } else {
                    let lines: Vec<String> = targets
                        .iter()
                        .map(|p| {
                            format!(
                                "try {{ Set-NetAdapterAdvancedProperty -Name {} -RegistryKeyword {} -RegistryValue 0 -NoRestart -ErrorAction Stop }} catch {{ Set-NetAdapterAdvancedProperty -Name {} -RegistryKeyword {} -DisplayValue 'Disabled' -NoRestart -ErrorAction Stop }}",
                                ps_single_quote(&ad.name),
                                ps_single_quote(&p.registry_keyword),
                                ps_single_quote(&ad.name),
                                ps_single_quote(&p.registry_keyword),
                            )
                        })
                        .collect();
                    let script = format!("{}\nWrite-Output 'EEE disabled'", lines.join("\n"));
                    let res = self.logger.run_powershell(
                        &script,
                        "adapter",
                        &format!("Set-NetAdapterAdvancedProperty \"{}\" EEE -> 0", ad.name),
                        false,
                    );
                    if res.ok {
                        self.update_step("eee", "applied", &format!("{} option(s) EEE désactivée(s)", targets.len()));
                        any_applied = true;
                    } else {
                        self.update_step(
                            "eee",
                            "error",
                            if is_admin {
                                res.stderr.lines().next().unwrap_or("Erreur EEE")
                            } else {
                                "Droits Administrateur UAC requis"
                            },
                        );
                    }
                }
            } else {
                self.update_step("eee", "error", "Aucun adaptateur actif");
            }
        } else {
            self.update_step("eee", "skipped", "Désactivé dans les options");
        }

        // 7. Optional Background Traffic Controls
        if options.pause_windows_update || options.throttle_background_apps {
            self.update_step("background", "running", "Application restrictions arrière-plan...");
            let mut notes = Vec::new();
            if options.pause_windows_update {
                for svc in ["wuauserv", "bits"] {
                    if self.toggle_service_pause(svc, true).ok {
                        notes.push(format!("{} en pause", svc));
                    }
                }
            }
            if options.throttle_background_apps {
                let snap = scan_background_traffic(&self.get_throttled_exes(), &self.get_paused_services());
                let mut count = 0;
                for app in snap.apps.into_iter().take(6) {
                    if self.toggle_app_throttle(&app.exe_name, true).ok {
                        count += 1;
                    }
                }
                notes.push(format!("{} app(s) DSCP 8", count));
            }
            if !notes.is_empty() {
                self.update_step("background", "applied", &notes.join(" • "));
                any_applied = true;
            } else {
                self.update_step(
                    "background",
                    "error",
                    if is_admin { "Échec contrôle arrière-plan" } else { "Droits Administrateur UAC requis" },
                );
            }
        } else {
            self.update_step("background", "skipped", "Gestion manuelle dans l'onglet Arrière-plan");
        }

        // Start 0% CPU Win32 WaitForSingleObject Game Exit Watcher if game is running
        self.watcher.stop();
        let mut tracked_pid = None;
        if any_applied && options.auto_rollback_on_game_exit {
            if let Some(ref g_prof) = game {
                if let Some(pid) = find_pid_by_exe(&g_prof.exe_name) {
                    tracked_pid = Some(pid);
                    let engine_clone = self.clone();
                    self.watcher.watch_pid(
                        pid,
                        g_prof.exe_name.clone(),
                        self.logger.clone(),
                        move |exited_exe| {
                            engine_clone.logger.info(
                                "rollback",
                                &format!("Fermeture instantanée détectée : {}", exited_exe),
                                "Restauration auto complète (Rollback + coupure tunnel + suppression Pare-feu/QoS)...",
                                "warn",
                            );
                            let _ = engine_clone.rollback_all();
                            engine_clone.emit_notice(&format!(
                                "Le jeu {} s'est fermé — restauration complète et coupure tunnel exécutées.",
                                exited_exe
                            ));
                        },
                    );
                }
            }
        }

        if let Ok(mut g) = self.inner.lock() {
            g.state.active = any_applied;
            g.state.started_at = if any_applied {
                Some(Utc::now().to_rfc3339())
            } else {
                None
            };
            g.state.tracked_pid = tracked_pid;
            g.state.busy = false;
        }
        if any_applied {
            self.ping_monitor.set_phase("after");
        }
        self.emit_state();
        self.get_state()
    }

    pub fn rollback_all(&self) -> RollbackResult {
        self.watcher.stop();
        if let Ok(mut g) = self.inner.lock() {
            g.state.busy = true;
        }
        self.emit_state();

        // 1. Always stop active WireSock / WireGuard tunnel first
        if self.tunnel.is_active() {
            let _ = self.tunnel.stop_tunnel();
        }

        let Some(backup) = self.store.read_backup() else {
            self.reset_internal_state();
            self.ping_monitor.set_phase("before");
            self.logger.info(
                "rollback",
                "Rollback",
                "Aucune sauvegarde active à restaurer : la configuration système est déjà d'origine.",
                "ok",
            );
            self.emit_state();
            return RollbackResult {
                ok: true,
                message: "Aucune modification active à restaurer.".into(),
                state: self.get_state(),
            };
        };

        let is_admin = is_admin_cached();
        let mut errors = Vec::new();
        self.logger.info(
            "rollback",
            "Démarrage Rollback complet",
            &format!("Restauration depuis {}", self.store.backup_path().display()),
            "warn",
        );

        // 2. Remove all NetBoost_Block_* Windows Firewall rules via COM INetFwPolicy2
        let tracked_fw = if !backup.firewall_rules_created.is_empty() {
            backup.firewall_rules_created.clone()
        } else {
            self.get_firewall_rules()
        };
        if let Err(e) = remove_all_netboost_firewall_rules(&tracked_fw, &self.logger) {
            if is_admin {
                errors.push(format!("Pare-feu: {}", e));
            }
        }

        // 3. Remove all NetBoost_* QoS policies
        let qos_res = self.logger.run_powershell(
            "Get-NetQosPolicy -ErrorAction SilentlyContinue | Where-Object { $_.Name -like 'NetBoost_*' } | Remove-NetQosPolicy -Confirm:$false -ErrorAction SilentlyContinue; Write-Output 'QoS NetBoost_* nettoyées'",
            "rollback",
            "Remove-NetQosPolicy -Name \"NetBoost_*\" -Confirm:$false",
            false,
        );
        if !qos_res.ok && is_admin {
            errors.push(format!("QoS: {}", qos_res.stderr));
        }

        // 4. Restore original DNS on primary + secondary switched adapters + flush
        let mut dns_targets: Vec<(u32, bool, Vec<String>)> = Vec::new();
        if let Some(ref ad) = backup.adapter {
            dns_targets.push((ad.interface_index, ad.dns_is_dhcp, ad.original_dns.clone()));
        }
        for sec in &backup.secondary_dns_adapters {
            if !dns_targets.iter().any(|(idx, _, _)| *idx == sec.interface_index) {
                dns_targets.push((sec.interface_index, sec.dns_is_dhcp, sec.original_dns.clone()));
            }
        }
        if !dns_targets.is_empty() {
            for (idx, is_dhcp, orig_dns) in dns_targets {
                let cmd = if is_dhcp || orig_dns.is_empty() {
                    format!(
                        "Set-DnsClientServerAddress -InterfaceIndex {} -ResetServerAddresses; Clear-DnsClientCache",
                        idx
                    )
                } else {
                    let list = orig_dns
                        .iter()
                        .map(|s| ps_single_quote(s))
                        .collect::<Vec<_>>()
                        .join(",");
                    format!(
                        "Set-DnsClientServerAddress -InterfaceIndex {} -ServerAddresses @({}); Clear-DnsClientCache",
                        idx, list
                    )
                };
                let res = self.logger.run_powershell(&cmd, "rollback", &cmd, false);
                if !res.ok {
                    errors.push(format!("DNS (#{}): {}", idx, if res.stderr.is_empty() { "UAC requis" } else { &res.stderr }));
                }
            }
            let _ = self.logger.run_exe("ipconfig.exe", &["/flushdns"], "rollback", false);
        }

        // 5. Restore TCP/IP & QoS NLA registry natively via winreg
        if let Err(e) = restore_registry_from_backup(&backup.registry, &self.logger) {
            errors.push(format!("Registre: {}", e));
        }

        // 6. Restore Adapter EEE properties
        if let Some(ref ad) = backup.adapter {
            if !ad.eee_properties.is_empty() {
                let lines: Vec<String> = ad
                    .eee_properties
                    .iter()
                    .map(|p| {
                        format!(
                            "try {{ Set-NetAdapterAdvancedProperty -Name {} -RegistryKeyword {} -RegistryValue {} -NoRestart -ErrorAction Stop }} catch {{ Set-NetAdapterAdvancedProperty -Name {} -RegistryKeyword {} -DisplayValue {} -NoRestart -ErrorAction SilentlyContinue }}",
                            ps_single_quote(&ad.name),
                            ps_single_quote(&p.registry_keyword),
                            ps_single_quote(&p.registry_value),
                            ps_single_quote(&ad.name),
                            ps_single_quote(&p.registry_keyword),
                            ps_single_quote(&p.display_value),
                        )
                    })
                    .collect();
                let res = self.logger.run_powershell(
                    &format!("{}\nWrite-Output 'EEE restauré'", lines.join("\n")),
                    "rollback",
                    &format!("Restauration EEE sur {}", ad.name),
                    false,
                );
                if !res.ok {
                    errors.push(format!("EEE: {}", res.stderr));
                }
            }
        }

        // 7. Resume Windows services natively via Win32 SCM
        for svc in &backup.services_paused {
            if svc.original_status.eq_ignore_ascii_case("running") {
                if let Err(e) = control_windows_service(&svc.name, false, &self.logger) {
                    errors.push(format!("Service {}: {}", svc.name, e));
                }
            }
        }

        self.reset_internal_state();
        self.ping_monitor.set_phase("before");

        if errors.is_empty() {
            self.store.delete_backup();
        }

        let ok = errors.is_empty();
        let message = if ok {
            "Rollback complet terminé : tunnel coupé, règles Pare-feu/QoS/NLA supprimées et configuration Windows restaurée.".into()
        } else {
            format!("Rollback terminé avec avertissements : {}", errors.join(" | "))
        };
        self.logger.info("rollback", "Résultat Rollback", &message, if ok { "ok" } else { "warn" });
        self.emit_state();

        RollbackResult {
            ok,
            message,
            state: self.get_state(),
        }
    }

    fn reset_internal_state(&self) {
        if let Ok(mut g) = self.inner.lock() {
            g.state.active = false;
            g.state.busy = false;
            g.state.started_at = None;
            g.state.tracked_pid = None;
            g.state.applied_dns = None;
            g.state.steps = default_steps();
            g.throttled_exes.clear();
            g.paused_services.clear();
            g.firewall_rules.clear();
        }
    }
}
