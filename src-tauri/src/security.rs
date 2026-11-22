use std::ffi::OsStr;
use std::net::{IpAddr, Ipv4Addr};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use std::sync::OnceLock;

use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, HWND, LocalFree};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};
use windows::Win32::Security::WinTrust::{
    WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0,
    WINTRUST_DATA_STATE_ACTION, WINTRUST_FILE_INFO,
    WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY,
    WTD_UI_NONE,
};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::logger::CommandLogger;

static ADMIN_CACHE: OnceLock<bool> = OnceLock::new();

pub fn to_wide_null(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Checks Windows Administrator token elevation natively via Win32 OpenProcessToken + GetTokenInformation.
/// Cached once per process lifetime in OnceLock (zero PowerShell calls).
pub fn is_admin_cached() -> bool {
    *ADMIN_CACHE.get_or_init(|| unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut ret_len: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    })
}

/// Requests UAC elevation by relaunching the current executable via Win32 ShellExecuteW("runas").
pub fn relaunch_as_admin(logger: &CommandLogger) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_str = exe.to_string_lossy().to_string();
    logger.info(
        "uac",
        &format!("ShellExecuteW(runas, \"{}\")", exe_str),
        "Demande d'élévation UAC envoyée à Windows...",
        "warn",
    );

    let verb = to_wide_null("runas");
    let file = to_wide_null(&exe_str);
    let hinst = unsafe {
        ShellExecuteW(
            HWND(ptr::null_mut()),
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if (hinst.0 as isize) > 32 {
        logger.info("uac", "UAC Elevation", "Instance Administrateur lancée avec succès.", "ok");
        Ok(())
    } else {
        let msg = "Élévation UAC annulée par l'utilisateur ou refusée par Windows.".to_string();
        logger.info("uac", "UAC Elevation", &msg, "error");
        Err(msg)
    }
}

/// Encrypts sensitive bytes at rest using Windows DPAPI (CryptProtectData, scoped to current user).
pub fn dpapi_encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let mut in_blob = CRYPT_INTEGER_BLOB {
            cbData: plaintext.len() as u32,
            pbData: plaintext.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(
            &mut in_blob,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .map_err(|e| format!("Échec CryptProtectData DPAPI : {}", e))?;

        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let result = slice.to_vec();
        let _ = LocalFree(HLOCAL(out_blob.pbData as *mut _));
        Ok(result)
    }
}

/// Decrypts DPAPI-protected bytes via Win32 CryptUnprotectData.
pub fn dpapi_decrypt(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let mut in_blob = CRYPT_INTEGER_BLOB {
            cbData: ciphertext.len() as u32,
            pbData: ciphertext.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(
            &mut in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .map_err(|e| format!("Échec CryptUnprotectData DPAPI : {}", e))?;

        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let result = slice.to_vec();
        let _ = LocalFree(HLOCAL(out_blob.pbData as *mut _));
        Ok(result)
    }
}

/// Verifies a Windows PE binary's Authenticode signature natively via Win32 WinVerifyTrust.
pub fn verify_authenticode_signature(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    let wide_path = to_wide_null(&path.to_string_lossy());
    unsafe {
        let mut file_info = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: PCWSTR(wide_path.as_ptr()),
            hFile: HANDLE::default(),
            pgKnownSubject: ptr::null_mut(),
        };
        let mut data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            pPolicyCallbackData: ptr::null_mut(),
            pSIPClientData: ptr::null_mut(),
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: WTD_REVOKE_NONE,
            dwUnionChoice: WTD_CHOICE_FILE,
            Anonymous: WINTRUST_DATA_0 {
                pFile: &mut file_info,
            },
            dwStateAction: WINTRUST_DATA_STATE_ACTION(WTD_STATEACTION_VERIFY.0),
            hWVTStateData: HANDLE::default(),
            pwszURLReference: windows::core::PWSTR::null(),
            dwProvFlags: windows::Win32::Security::WinTrust::WINTRUST_DATA_PROVIDER_FLAGS(0),
            dwUIContext: windows::Win32::Security::WinTrust::WINTRUST_DATA_UICONTEXT(0),
            pSignatureSettings: ptr::null_mut(),
        };

        let mut action: GUID = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let status = WinVerifyTrust(
            HWND(ptr::null_mut()),
            &mut action,
            &mut data as *mut _ as *mut _,
        );

        data.dwStateAction = WINTRUST_DATA_STATE_ACTION(WTD_STATEACTION_CLOSE.0);
        let _ = WinVerifyTrust(
            HWND(ptr::null_mut()),
            &mut action,
            &mut data as *mut _ as *mut _,
        );

        status == 0
    }
}

/// Restricts NTFS ACLs on temporary .conf files to the current user + SYSTEM (*S-1-5-18).
pub fn restrict_file_acl_to_current_user(path: &Path, logger: &CommandLogger) -> bool {
    let username = std::env::var("USERNAME").unwrap_or_default();
    if username.is_empty() {
        return false;
    }
    let path_str = path.to_string_lossy().to_string();
    let user_grant = format!("{}:(F)", username);
    let res = logger.run_exe(
        "icacls.exe",
        &[&path_str, "/inheritance:r", "/grant:r", &user_grant, "*S-1-5-18:(F)"],
        "tunnel",
        true,
    );
    if !res.ok {
        logger.info(
            "tunnel",
            &format!("icacls \"{}\"", path.file_name().unwrap_or_default().to_string_lossy()),
            &format!("Avertissement ACL NTFS : {}", res.stderr),
            "warn",
        );
    }
    res.ok
}

/// Strictly validates an IPv4/IPv6 address or RFC hostname.
/// Rejects leading '-' or '/' to block CLI argument injection.
pub fn sanitize_host(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > 253 {
        return Err("Hôte ou adresse IP invalide.".to_string());
    }
    if trimmed.starts_with('-') || trimmed.starts_with('/') {
        return Err(format!("Adresse cible refusée (préfixe d'argument interdit) : {}", trimmed));
    }
    if trimmed.parse::<IpAddr>().is_ok() {
        return Ok(trimmed.to_string());
    }
    // RFC 1123 hostname validation
    for label in trimmed.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err(format!("Hôte ou nom de domaine invalide : {}", trimmed));
        }
        let bytes = label.as_bytes();
        if !bytes[0].is_ascii_alphanumeric() || !bytes[bytes.len() - 1].is_ascii_alphanumeric() {
            return Err(format!("Hôte ou nom de domaine invalide : {}", trimmed));
        }
        if !bytes.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'-') {
            return Err(format!("Hôte ou nom de domaine invalide : {}", trimmed));
        }
    }
    Ok(trimmed.to_string())
}

pub fn sanitize_exe_name(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let base = trimmed
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("");
    if base.is_empty()
        || base.starts_with('-')
        || !base.to_ascii_lowercase().ends_with(".exe")
        || !base
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ' ' | '(' | ')'))
    {
        return Err(format!("Nom d'exécutable invalide : {}", input));
    }
    Ok(base.to_string())
}

pub fn sanitize_policy_suffix(input: &str) -> String {
    input
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .take(48)
        .collect()
}

/// Strictly validates IPv4/IPv6 or CIDR block using Rust std::net::IpAddr.
pub fn sanitize_ip_or_cidr(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.starts_with('-') || trimmed.starts_with('/') {
        return Err(format!("Adresse IP ou plage CIDR invalide : {}", input));
    }
    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.len() > 2 {
        return Err(format!("Adresse IP ou plage CIDR invalide : {}", input));
    }
    let ip: IpAddr = parts[0]
        .parse()
        .map_err(|_| format!("Adresse IP invalide : {}", input))?;
    if parts.len() == 2 {
        let mask_str = parts[1];
        if mask_str.is_empty() || mask_str.len() > 3 || !mask_str.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!("Masque CIDR invalide : {}", input));
        }
        let mask: u8 = mask_str
            .parse()
            .map_err(|_| format!("Masque CIDR invalide : {}", input))?;
        let max_mask = match ip {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        if mask > max_mask {
            return Err(format!("Masque CIDR hors limites (0-{}) : {}", max_mask, input));
        }
        return Ok(format!("{}/{}", ip, mask));
    }
    Ok(ip.to_string())
}

pub fn to_slash24(ip: &str) -> String {
    if let Ok(v4) = ip.trim().parse::<Ipv4Addr>() {
        let [a, b, c, _] = v4.octets();
        return format!("{}.{}.{}.0/24", a, b, c);
    }
    ip.trim().to_string()
}

/// Validates exact Tcpip interface registry subkey format:
/// `SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{GUID}`
/// (accepting optional `HKLM:\` prefix for backward compatibility with backup-state.json).
pub fn is_valid_interface_registry_path(key_path: &str) -> bool {
    let stripped = key_path
        .strip_prefix("HKLM:\\")
        .or_else(|| key_path.strip_prefix("HKLM\\"))
        .unwrap_or(key_path);
    let prefix = "SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\{";
    if !stripped.starts_with(prefix) || !stripped.ends_with('}') {
        return false;
    }
    let guid_body = &stripped[prefix.len()..stripped.len() - 1];
    !guid_body.is_empty() && guid_body.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

pub fn normalize_interface_subkey(key_path: &str) -> Option<String> {
    if !is_valid_interface_registry_path(key_path) {
        return None;
    }
    let stripped = key_path
        .strip_prefix("HKLM:\\")
        .or_else(|| key_path.strip_prefix("HKLM\\"))
        .unwrap_or(key_path);
    Some(stripped.to_string())
}
