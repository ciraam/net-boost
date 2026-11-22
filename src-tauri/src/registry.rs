use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};
use winreg::RegKey;

use crate::logger::CommandLogger;
use crate::security::{is_valid_interface_registry_path, normalize_interface_subkey};
use crate::types::{BackupRegistryState, BackupRegistryStringValue, BackupRegistryValue};

pub const MULTIMEDIA_PROFILE_SUBKEY: &str =
    r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
pub const TCPIP_QOS_SUBKEY: &str = r"SYSTEM\CurrentControlSet\Services\Tcpip\QoS";

pub fn build_interface_registry_path(interface_guid: Option<&str>) -> Option<String> {
    let guid = interface_guid?.trim();
    let full = format!(
        r"HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{}",
        guid
    );
    if is_valid_interface_registry_path(&full) {
        Some(full)
    } else {
        None
    }
}

fn read_dword(root: &RegKey, subkey: &str, name: &str) -> BackupRegistryValue {
    if let Ok(key) = root.open_subkey_with_flags(subkey, KEY_READ) {
        if let Ok(val) = key.get_value::<u32, _>(name) {
            return BackupRegistryValue {
                existed: true,
                value: Some(val),
            };
        }
    }
    BackupRegistryValue {
        existed: false,
        value: None,
    }
}

fn read_string(root: &RegKey, subkey: &str, name: &str) -> BackupRegistryStringValue {
    if let Ok(key) = root.open_subkey_with_flags(subkey, KEY_READ) {
        if let Ok(val) = key.get_value::<String, _>(name) {
            return BackupRegistryStringValue {
                existed: true,
                value: Some(val),
            };
        }
    }
    BackupRegistryStringValue {
        existed: false,
        value: None,
    }
}

/// Reads original TCP/IP, Multimedia & QoS NLA registry values natively via winreg (zero PowerShell).
pub fn read_registry_snapshot(interface_guid: Option<&str>, logger: &CommandLogger) -> BackupRegistryState {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let iface_path = build_interface_registry_path(interface_guid);

    let (tcp_ack, tcp_nodelay) = if let Some(sub) = iface_path.as_deref().and_then(normalize_interface_subkey) {
        (
            read_dword(&hklm, &sub, "TcpAckFrequency"),
            read_dword(&hklm, &sub, "TCPNoDelay"),
        )
    } else {
        (BackupRegistryValue::default(), BackupRegistryValue::default())
    };

    let throttling = read_dword(&hklm, MULTIMEDIA_PROFILE_SUBKEY, "NetworkThrottlingIndex");
    let qos_nla = read_string(&hklm, TCPIP_QOS_SUBKEY, "Do not use NLA");

    logger.info(
        "system",
        "winreg::RegKey::open_subkey (TCP/IP, Multimedia, QoS NLA)",
        &format!(
            "Snapshot registre lu nativement (AckFreq={:?}, NoDelay={:?}, Throttling={:?}, QoS_NLA={:?})",
            tcp_ack.value, tcp_nodelay.value, throttling.value, qos_nla.value
        ),
        "ok",
    );

    BackupRegistryState {
        interface_key_path: iface_path,
        tcp_ack_frequency: tcp_ack,
        tcp_no_delay: tcp_nodelay,
        network_throttling_index: throttling,
        qos_do_not_use_nla: Some(qos_nla),
    }
}

/// Sets `HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\QoS` -> `"Do not use NLA" = "1"` natively via winreg.
pub fn enable_qos_do_not_use_nla(logger: &CommandLogger) -> Result<(), String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (key, _) = hklm
        .create_subkey(TCPIP_QOS_SUBKEY)
        .map_err(|e| format!("Échec ouverture clé QoS ({}): {}", TCPIP_QOS_SUBKEY, e))?;
    key.set_value("Do not use NLA", &"1")
        .map_err(|e| format!("Échec écriture 'Do not use NLA' = '1' : {}", e))?;

    logger.info(
        "qos",
        r#"winreg::set_value(HKLM\...\Tcpip\QoS, "Do not use NLA" = "1")"#,
        "QoS Do not use NLA = 1 activé nativement (DSCP actif hors domaine AD).",
        "ok",
    );
    Ok(())
}

/// Applies TCPNoDelay=1, TcpAckFrequency=1, NetworkThrottlingIndex=0xFFFFFFFF natively via winreg.
pub fn apply_tcpip_tweaks(interface_key_path: Option<&str>, logger: &CommandLogger) -> Result<(), String> {
    let subkey = interface_key_path
        .and_then(normalize_interface_subkey)
        .ok_or_else(|| {
            let msg = format!(
                "Chemin registre d'interface refusé (format GUID invalide) : {:?}",
                interface_key_path
            );
            logger.info("tcpip", "Validation chemin Registre", &msg, "error");
            msg
        })?;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let iface = hklm
        .open_subkey_with_flags(&subkey, KEY_WRITE)
        .map_err(|e| format!("Accès refusé sur {} : {}", subkey, e))?;
    iface
        .set_value("TcpAckFrequency", &1u32)
        .map_err(|e| format!("Échec TcpAckFrequency=1 : {}", e))?;
    iface
        .set_value("TCPNoDelay", &1u32)
        .map_err(|e| format!("Échec TCPNoDelay=1 : {}", e))?;

    let mm = hklm
        .open_subkey_with_flags(MULTIMEDIA_PROFILE_SUBKEY, KEY_WRITE)
        .map_err(|e| format!("Accès refusé sur Multimedia\\SystemProfile : {}", e))?;
    mm.set_value("NetworkThrottlingIndex", &0xFFFF_FFFFu32)
        .map_err(|e| format!("Échec NetworkThrottlingIndex=0xFFFFFFFF : {}", e))?;

    logger.info(
        "tcpip",
        "winreg::set_value(TCPNoDelay=1, TcpAckFrequency=1, NetworkThrottlingIndex=0xFFFFFFFF)",
        "Optimisations Registre TCP/IP appliquées nativement.",
        "ok",
    );
    Ok(())
}

/// Restores TCPNoDelay, TcpAckFrequency, NetworkThrottlingIndex, and QoS "Do not use NLA" natively via winreg.
/// Validates interfaceKeyPath strictly against the GUID allowlist before touching HKLM.
pub fn restore_registry_from_backup(
    registry: &BackupRegistryState,
    logger: &CommandLogger,
) -> Result<(), String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    if let Some(raw_path) = registry.interface_key_path.as_deref() {
        let subkey = normalize_interface_subkey(raw_path).ok_or_else(|| {
            let msg = format!(
                "Chemin registre suspect dans backup-state.json ignoré par sécurité : {}",
                raw_path
            );
            logger.info("rollback", "Validation sécurité Rollback Registre", &msg, "error");
            msg
        })?;

        let iface = hklm
            .open_subkey_with_flags(&subkey, KEY_WRITE)
            .map_err(|e| format!("Échec ouverture {} : {}", subkey, e))?;

        if registry.tcp_ack_frequency.existed {
            if let Some(v) = registry.tcp_ack_frequency.value {
                iface.set_value("TcpAckFrequency", &v).map_err(|e| e.to_string())?;
            }
        } else {
            let _ = iface.delete_value("TcpAckFrequency");
        }

        if registry.tcp_no_delay.existed {
            if let Some(v) = registry.tcp_no_delay.value {
                iface.set_value("TCPNoDelay", &v).map_err(|e| e.to_string())?;
            }
        } else {
            let _ = iface.delete_value("TCPNoDelay");
        }
    }

    let mm = hklm
        .open_subkey_with_flags(MULTIMEDIA_PROFILE_SUBKEY, KEY_WRITE)
        .map_err(|e| format!("Échec ouverture Multimedia\\SystemProfile : {}", e))?;
    if registry.network_throttling_index.existed {
        if let Some(v) = registry.network_throttling_index.value {
            mm.set_value("NetworkThrottlingIndex", &v)
                .map_err(|e| e.to_string())?;
        }
    } else {
        let _ = mm.delete_value("NetworkThrottlingIndex");
    }

    if let Some(qos_nla) = &registry.qos_do_not_use_nla {
        if let Ok(qos) = hklm.open_subkey_with_flags(TCPIP_QOS_SUBKEY, KEY_WRITE) {
            if qos_nla.existed {
                if let Some(val) = &qos_nla.value {
                    let _ = qos.set_value("Do not use NLA", val);
                }
            } else {
                let _ = qos.delete_value("Do not use NLA");
            }
        }
    }

    logger.info(
        "rollback",
        "winreg::restore(TCPNoDelay, TcpAckFrequency, NetworkThrottlingIndex, QoS NLA)",
        "Clés Registre TCP/IP & QoS NLA restaurées nativement.",
        "ok",
    );
    Ok(())
}
