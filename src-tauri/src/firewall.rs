use chrono::Utc;
use windows::core::BSTR;
use windows::Win32::Foundation::VARIANT_TRUE;
use windows::Win32::NetworkManagement::WindowsFirewall::{
    INetFwPolicy2, INetFwRule, NetFwPolicy2, NetFwRule, NET_FW_ACTION_BLOCK,
    NET_FW_PROFILE2_ALL, NET_FW_RULE_DIR_IN, NET_FW_RULE_DIR_OUT,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};

use crate::logger::CommandLogger;
use crate::security::{is_admin_cached, sanitize_ip_or_cidr};
use crate::types::FirewallBlockRule;

struct ComGuard {
    should_uninit: bool,
}

impl ComGuard {
    fn init() -> Self {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self {
            should_uninit: hr.is_ok(),
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            unsafe { CoUninitialize() };
        }
    }
}

fn add_single_rule(
    policy: &INetFwPolicy2,
    name: &str,
    remote_addr: &str,
    exe_path: Option<&str>,
    outbound: bool,
) -> Result<(), String> {
    unsafe {
        let rules = policy.Rules().map_err(|e| format!("INetFwPolicy2::Rules: {}", e))?;
        let _ = rules.Remove(&BSTR::from(name));

        let rule: INetFwRule = CoCreateInstance(&NetFwRule, None, CLSCTX_ALL)
            .map_err(|e| format!("CoCreateInstance(NetFwRule): {}", e))?;

        rule.SetName(&BSTR::from(name)).map_err(|e| e.to_string())?;
        rule.SetDescription(&BSTR::from("Règle temporaire générée par NetBoost"))
            .map_err(|e| e.to_string())?;
        rule.SetDirection(if outbound {
            NET_FW_RULE_DIR_OUT
        } else {
            NET_FW_RULE_DIR_IN
        })
        .map_err(|e| e.to_string())?;
        rule.SetAction(NET_FW_ACTION_BLOCK).map_err(|e| e.to_string())?;
        rule.SetRemoteAddresses(&BSTR::from(remote_addr))
            .map_err(|e| e.to_string())?;
        rule.SetProfiles(NET_FW_PROFILE2_ALL.0)
            .map_err(|e| e.to_string())?;
        rule.SetEnabled(VARIANT_TRUE).map_err(|e| e.to_string())?;

        if let Some(path) = exe_path {
            if !path.is_empty() && path.to_ascii_lowercase().ends_with(".exe") {
                let _ = rule.SetApplicationName(&BSTR::from(path));
            }
        }

        rules.Add(&rule).map_err(|e| format!("INetFwRules::Add: {}", e))?;
        Ok(())
    }
}

/// Creates native Outbound + Inbound Windows Firewall block rules via COM INetFwPolicy2 (zero PowerShell).
pub fn create_firewall_block_rule(
    ip_or_cidr: &str,
    exe_name: Option<&str>,
    exe_path: Option<&str>,
    country_code: Option<&str>,
    logger: &CommandLogger,
) -> Result<FirewallBlockRule, String> {
    if !is_admin_cached() {
        let msg = "Droits Administrateur (UAC) requis pour créer une règle Pare-feu Windows (INetFwPolicy2).".to_string();
        logger.info("firewall", &format!("INetFwPolicy2 Block {}", ip_or_cidr), &msg, "error");
        return Err(msg);
    }

    let clean_addr = sanitize_ip_or_cidr(ip_or_cidr)?;
    let safe_id: String = clean_addr
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let rule_base = format!("NetBoost_Block_{}", safe_id);
    let out_name = format!("{}_Out", rule_base);
    let in_name = format!("{}_In", rule_base);

    let _com = ComGuard::init();
    let policy: INetFwPolicy2 = unsafe {
        CoCreateInstance(&NetFwPolicy2, None, CLSCTX_ALL)
            .map_err(|e| format!("CoCreateInstance(NetFwPolicy2): {}", e))?
    };

    add_single_rule(&policy, &out_name, &clean_addr, exe_path, true)?;
    add_single_rule(&policy, &in_name, &clean_addr, exe_path, false)?;

    let display_name = match country_code {
        Some(cc) if !cc.is_empty() => format!("NetBoost Block {} ({})", clean_addr, cc),
        _ => format!("NetBoost Block {}", clean_addr),
    };

    logger.info(
        "firewall",
        &format!("INetFwPolicy2::Rules::Add({}, Remote={})", rule_base, clean_addr),
        "Règles Pare-feu Windows Entrant & Sortant créées nativement (COM).",
        "ok",
    );

    Ok(FirewallBlockRule {
        rule_name: rule_base,
        display_name,
        remote_address: clean_addr,
        exe_name: exe_name.map(|s| s.to_string()),
        country_code: country_code.map(|s| s.to_string()),
        created_at: Utc::now().to_rfc3339(),
    })
}

pub fn remove_firewall_block_rule(rule_base: &str, logger: &CommandLogger) -> Result<(), String> {
    if !is_admin_cached() {
        let msg = "Droits Administrateur (UAC) requis pour supprimer une règle Pare-feu Windows.".to_string();
        logger.info("firewall", &format!("INetFwPolicy2 Remove {}", rule_base), &msg, "error");
        return Err(msg);
    }

    let safe: String = rule_base
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();

    let _com = ComGuard::init();
    let policy: INetFwPolicy2 = unsafe {
        CoCreateInstance(&NetFwPolicy2, None, CLSCTX_ALL)
            .map_err(|e| format!("CoCreateInstance(NetFwPolicy2): {}", e))?
    };
    let rules = unsafe { policy.Rules().map_err(|e| e.to_string())? };

    for name in [format!("{}_Out", safe), format!("{}_In", safe), safe.clone()] {
        unsafe {
            let _ = rules.Remove(&BSTR::from(name.as_str()));
        }
    }

    logger.info(
        "firewall",
        &format!("INetFwPolicy2::Rules::Remove({}*)", safe),
        "Règle Pare-feu Windows supprimée nativement (COM).",
        "ok",
    );
    Ok(())
}

/// Removes all tracked `NetBoost_Block_*` Windows Firewall rules cleanly via COM INetFwPolicy2.
pub fn remove_all_netboost_firewall_rules(
    tracked: &[FirewallBlockRule],
    logger: &CommandLogger,
) -> Result<(), String> {
    if !is_admin_cached() {
        return Ok(());
    }
    let _com = ComGuard::init();
    let policy: INetFwPolicy2 = unsafe {
        CoCreateInstance(&NetFwPolicy2, None, CLSCTX_ALL)
            .map_err(|e| format!("CoCreateInstance(NetFwPolicy2): {}", e))?
    };
    let rules = unsafe { policy.Rules().map_err(|e| e.to_string())? };

    for r in tracked {
        for name in [
            format!("{}_Out", r.rule_name),
            format!("{}_In", r.rule_name),
            r.rule_name.clone(),
        ] {
            unsafe {
                let _ = rules.Remove(&BSTR::from(name.as_str()));
            }
        }
    }

    logger.info(
        "rollback",
        "INetFwPolicy2::Rules::Remove(NetBoost_Block_*)",
        &format!("{} règle(s) Pare-feu nettoyée(s) nativement (COM).", tracked.len()),
        "ok",
    );
    Ok(())
}
