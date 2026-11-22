use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use chrono::Local;
use sysinfo::{ProcessesToUpdate, System};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
};
use windows::Win32::Networking::WinSock::AF_INET;
use windows::Win32::System::Services::{
    CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, QueryServiceStatusEx,
    StartServiceW, SC_MANAGER_CONNECT, SC_STATUS_PROCESS_INFO, SERVICE_CONTROL_STOP,
    SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START, SERVICE_STATUS,
    SERVICE_STATUS_PROCESS, SERVICE_STOP, SERVICE_STOPPED,
};
use windows::Win32::System::Threading::{
    CreateEventW, OpenProcess, SetEvent, WaitForMultipleObjects, INFINITE, PROCESS_SYNCHRONIZE,
};

use crate::logger::CommandLogger;
use crate::security::{is_admin_cached, to_wide_null};
use crate::types::{
    BackgroundAppInfo, BackgroundSnapshot, ProcessScanResult, RunningProcessInfo,
    WindowsServiceInfo,
};

const KNOWN_GAME_EXES: &[&str] = &[
    "valorant-win64-shipping.exe",
    "cs2.exe",
    "csgo.exe",
    "league of legends.exe",
    "fortniteclient-win64-shipping.exe",
    "r5apex.exe",
    "overwatch.exe",
    "cod.exe",
    "modernwarfare.exe",
    "rocketleague.exe",
    "minecraft.windows.exe",
    "javaw.exe",
    "gta5.exe",
    "escapefromtarkov.exe",
    "tslgame.exe",
    "dota2.exe",
    "rainbowsix.exe",
    "rainbowsix_vulkan.exe",
    "eldenring.exe",
    "cyberpunk2077.exe",
    "bf2042.exe",
    "destiny2.exe",
    "warframe.x64.exe",
    "pubg.exe",
    "helldivers2.exe",
    "baldursgate3.exe",
    "bg3.exe",
    "marvel-win64-shipping.exe",
    "deadlock.exe",
    "pathofexile_x64.exe",
    "wow.exe",
    "diablo iv.exe",
    "genshinimpact.exe",
    "robloxplayerbeta.exe",
    "rustclient.exe",
    "dayz_x64.exe",
    "huntgame.exe",
    "thefinals.exe",
];

const EXCLUDED_LAUNCHERS: &[&str] = &[
    "steam.exe",
    "steamwebhelper.exe",
    "epicgameslauncher.exe",
    "epicwebhelper.exe",
    "riotclientservices.exe",
    "riotclientux.exe",
    "battle.net.exe",
    "agent.exe",
    "eadesktop.exe",
    "upc.exe",
    "ubisoftconnect.exe",
    "galaxyclient.exe",
    "crashhandler.exe",
];

const GAME_PATH_MARKERS: &[&str] = &[
    "\\steamapps\\common\\",
    "\\epic games\\",
    "\\riot games\\",
    "\\battle.net\\",
    "\\ea games\\",
    "\\origin games\\",
    "\\ubisoft\\ubisoft game launcher\\games\\",
    "\\xboxgames\\",
    "\\gog galaxy\\games\\",
    "\\games\\",
];

const BACKGROUND_CATALOG: &[(&str, &str, &str)] = &[
    ("chrome.exe", "Google Chrome", "browser"),
    ("msedge.exe", "Microsoft Edge", "browser"),
    ("firefox.exe", "Mozilla Firefox", "browser"),
    ("opera.exe", "Opera / Opera GX", "browser"),
    ("brave.exe", "Brave Browser", "browser"),
    ("vivaldi.exe", "Vivaldi", "browser"),
    ("steam.exe", "Steam Client", "launcher"),
    ("steamwebhelper.exe", "Steam WebHelper", "launcher"),
    ("epicgameslauncher.exe", "Epic Games Launcher", "launcher"),
    ("battle.net.exe", "Battle.net Launcher", "launcher"),
    ("agent.exe", "Blizzard Update Agent", "updater"),
    ("riotclientservices.exe", "Riot Client", "launcher"),
    ("eadesktop.exe", "EA App", "launcher"),
    ("upc.exe", "Ubisoft Connect", "launcher"),
    ("galaxyclient.exe", "GOG Galaxy", "launcher"),
    ("xboxpcapp.exe", "Xbox App", "launcher"),
    ("onedrive.exe", "Microsoft OneDrive", "updater"),
    ("discord.exe", "Discord", "other"),
    ("spotify.exe", "Spotify", "other"),
    ("qbittorrent.exe", "qBittorrent", "updater"),
    ("dropbox.exe", "Dropbox", "updater"),
    ("ms-teams.exe", "Microsoft Teams", "other"),
    ("teams.exe", "Microsoft Teams", "other"),
    ("nvcontainer.exe", "NVIDIA Container", "updater"),
];

/// Native process scan using sysinfo (zero PowerShell).
pub fn scan_running_processes() -> ProcessScanResult {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut by_exe: HashMap<String, RunningProcessInfo> = HashMap::new();

    for (pid, proc_info) in sys.processes() {
        let pid_u32 = pid.as_u32();
        if pid_u32 <= 4 {
            continue;
        }
        let exe_path = proc_info
            .exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let name = proc_info.name().to_string_lossy().to_string();
        let exe_name = if name.to_ascii_lowercase().ends_with(".exe") {
            name.clone()
        } else {
            format!("{}.exe", name)
        };
        let lower_exe = exe_name.to_ascii_lowercase();
        let lower_path = exe_path.to_ascii_lowercase();

        let is_excluded = EXCLUDED_LAUNCHERS.contains(&lower_exe.as_str());
        let matches_known = KNOWN_GAME_EXES.contains(&lower_exe.as_str());
        let matches_path = GAME_PATH_MARKERS.iter().any(|m| lower_path.contains(m));
        let is_likely_game = !is_excluded && (matches_known || matches_path);

        let memory_mb = ((proc_info.memory() as f64 / (1024.0 * 1024.0)) * 10.0).round() / 10.0;

        let entry = RunningProcessInfo {
            pid: pid_u32,
            name: exe_name.trim_end_matches(".exe").to_string(),
            exe_name,
            exe_path,
            memory_mb,
            window_title: String::new(),
            is_likely_game,
        };

        let replace = by_exe
            .get(&lower_exe)
            .map(|prev| entry.memory_mb > prev.memory_mb)
            .unwrap_or(true);
        if replace {
            by_exe.insert(lower_exe, entry);
        }
    }

    let mut all: Vec<RunningProcessInfo> = by_exe.into_values().collect();
    all.sort_by(|a, b| {
        b.memory_mb
            .partial_cmp(&a.memory_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let games = all.iter().filter(|p| p.is_likely_game).cloned().collect();
    ProcessScanResult { games, all }
}

pub fn find_pid_by_exe(exe_name: &str) -> Option<u32> {
    let target = exe_name.trim().to_ascii_lowercase();
    let target_no_ext = target.trim_end_matches(".exe");
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let mut best: Option<(u32, u64)> = None;
    for (pid, p) in sys.processes() {
        let name = p.name().to_string_lossy().to_ascii_lowercase();
        if name == target || name.trim_end_matches(".exe") == target_no_ext {
            let mem = p.memory();
            if best.map(|(_, m)| mem > m).unwrap_or(true) {
                best = Some((pid.as_u32(), mem));
            }
        }
    }
    best.map(|(pid, _)| pid)
}

fn count_tcp_connections_by_pid() -> HashMap<u32, usize> {
    let mut counts = HashMap::new();
    unsafe {
        let mut size: u32 = 0;
        let _ = GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        );
        if size == 0 {
            return counts;
        }
        let mut buf = vec![0u8; size as usize];
        let ret = GetExtendedTcpTable(
            Some(buf.as_mut_ptr() as *mut _),
            &mut size,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        );
        if ret != 0 {
            return counts;
        }
        let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
        for row in rows {
            *counts.entry(row.dwOwningPid).or_insert(0) += 1;
        }
    }
    counts
}

pub fn query_windows_service_status(service_name: &str) -> Option<(String, String)> {
    let lower = service_name.to_ascii_lowercase();
    let display = match lower.as_str() {
        "wuauserv" => "Windows Update",
        "bits" => "Service de transfert intelligent en arrière-plan (BITS)",
        "dosvc" => "Optimisation de livraison (Delivery Optimization)",
        _ => service_name,
    };

    unsafe {
        let scm = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT).ok()?;
        let svc_w = to_wide_null(service_name);
        let svc = OpenServiceW(scm, PCWSTR(svc_w.as_ptr()), SERVICE_QUERY_STATUS).ok();
        let status_str = if let Some(h) = svc {
            let mut ssp = SERVICE_STATUS_PROCESS::default();
            let mut needed = 0u32;
            let ok = QueryServiceStatusEx(
                h,
                SC_STATUS_PROCESS_INFO,
                Some(std::slice::from_raw_parts_mut(
                    &mut ssp as *mut _ as *mut u8,
                    std::mem::size_of::<SERVICE_STATUS_PROCESS>(),
                )),
                &mut needed,
            )
            .is_ok();
            let _ = CloseServiceHandle(h);
            if ok {
                if ssp.dwCurrentState == SERVICE_RUNNING {
                    "Running".to_string()
                } else if ssp.dwCurrentState == SERVICE_STOPPED {
                    "Stopped".to_string()
                } else {
                    format!("State({})", ssp.dwCurrentState.0)
                }
            } else {
                "Unknown".to_string()
            }
        } else {
            let _ = CloseServiceHandle(scm);
            return None;
        };
        let _ = CloseServiceHandle(scm);
        Some((display.to_string(), status_str))
    }
}

/// Starts or stops a whitelisted Windows service natively via Win32 Service Control Manager (zero PowerShell).
pub fn control_windows_service(
    service_name: &str,
    pause: bool,
    logger: &CommandLogger,
) -> Result<(), String> {
    let clean = service_name.trim().to_ascii_lowercase();
    if !matches!(clean.as_str(), "wuauserv" | "bits" | "dosvc") {
        return Err(format!("Service non autorisé : {}", service_name));
    }
    if !is_admin_cached() {
        let msg = format!(
            "Droits Administrateur (UAC) requis pour contrôler le service {}.",
            clean
        );
        logger.info("background", &format!("Win32 SCM {}", clean), &msg, "error");
        return Err(msg);
    }

    unsafe {
        let scm = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT)
            .map_err(|e| format!("OpenSCManagerW: {}", e))?;
        let svc_w = to_wide_null(&clean);
        let access = if pause {
            SERVICE_STOP | SERVICE_QUERY_STATUS
        } else {
            SERVICE_START | SERVICE_QUERY_STATUS
        };
        let svc = OpenServiceW(scm, PCWSTR(svc_w.as_ptr()), access).map_err(|e| {
            let _ = CloseServiceHandle(scm);
            format!("OpenServiceW({}): {}", clean, e)
        })?;

        let result = if pause {
            let mut status = SERVICE_STATUS::default();
            ControlService(svc, SERVICE_CONTROL_STOP, &mut status)
                .map_err(|e| format!("ControlService(STOP, {}): {}", clean, e))
        } else {
            StartServiceW(svc, None).map_err(|e| format!("StartServiceW({}): {}", clean, e))
        };

        let _ = CloseServiceHandle(svc);
        let _ = CloseServiceHandle(scm);

        match result {
            Ok(()) => {
                logger.info(
                    "background",
                    &format!("Win32 SCM {}({})", if pause { "StopService" } else { "StartService" }, clean),
                    &format!("Service {} {}.", clean, if pause { "mis en pause" } else { "relancé" }),
                    "ok",
                );
                Ok(())
            }
            Err(e) => {
                logger.info("background", &format!("Win32 SCM {}", clean), &e, "error");
                Err(e)
            }
        }
    }
}

pub fn scan_background_traffic(
    throttled_exes: &HashSet<String>,
    paused_services: &HashSet<String>,
) -> BackgroundSnapshot {
    let mut services = Vec::new();
    for name in ["wuauserv", "bits", "DoSvc"] {
        if let Some((display_name, status)) = query_windows_service_status(name) {
            services.push(WindowsServiceInfo {
                name: name.to_string(),
                display_name,
                status,
                start_type: "Windows SCM".to_string(),
                paused_by_app: paused_services.contains(&name.to_ascii_lowercase()),
            });
        }
    }

    let tcp_by_pid = count_tcp_connections_by_pid();
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let catalog: HashMap<&str, (&str, &str)> = BACKGROUND_CATALOG
        .iter()
        .map(|(exe, label, cat)| (*exe, (*label, *cat)))
        .collect();

    let mut grouped: HashMap<String, BackgroundAppInfo> = HashMap::new();
    for (pid, p) in sys.processes() {
        let pid_u32 = pid.as_u32();
        if pid_u32 <= 4 {
            continue;
        }
        let raw_name = p.name().to_string_lossy().to_string();
        let exe_name = if raw_name.to_ascii_lowercase().ends_with(".exe") {
            raw_name
        } else {
            format!("{}.exe", raw_name)
        };
        let lower = exe_name.to_ascii_lowercase();
        let tcp_c = *tcp_by_pid.get(&pid_u32).unwrap_or(&0);
        let cat_entry = catalog.get(lower.as_str()).copied();

        if cat_entry.is_none()
            && (tcp_c < 3 || matches!(lower.as_str(), "system.exe" | "svchost.exe" | "netboost.exe"))
        {
            continue;
        }

        let mem_mb = ((p.memory() as f64 / (1024.0 * 1024.0)) * 10.0).round() / 10.0;

        grouped
            .entry(lower.clone())
            .and_modify(|e| {
                e.pids.push(pid_u32);
                e.instance_count += 1;
                e.total_memory_mb = ((e.total_memory_mb + mem_mb) * 10.0).round() / 10.0;
                e.tcp_connections += tcp_c;
            })
            .or_insert_with(|| BackgroundAppInfo {
                exe_name: exe_name.clone(),
                display_name: cat_entry
                    .map(|(l, _)| l.to_string())
                    .unwrap_or_else(|| exe_name.trim_end_matches(".exe").to_string()),
                category: cat_entry.map(|(_, c)| c.to_string()).unwrap_or_else(|| "other".into()),
                pids: vec![pid_u32],
                instance_count: 1,
                total_memory_mb: mem_mb,
                tcp_connections: tcp_c,
                throttled_by_app: throttled_exes.contains(&lower),
            });
    }

    let mut apps: Vec<BackgroundAppInfo> = grouped.into_values().collect();
    apps.sort_by(|a, b| {
        b.tcp_connections.cmp(&a.tcp_connections).then_with(|| {
            b.total_memory_mb
                .partial_cmp(&a.total_memory_mb)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    BackgroundSnapshot {
        services,
        apps,
        scanned_at: Local::now().format("%H:%M:%S").to_string(),
    }
}

/// 0% CPU native Win32 Game Exit Watcher using OpenProcess(PROCESS_SYNCHRONIZE) + WaitForMultipleObjects.
/// Blocks kernel-side with zero CPU cycles until the exact millisecond the targeted game process terminates.
#[derive(Clone, Default)]
pub struct NativeGameWatcher {
    cancel_event_raw: Arc<Mutex<Option<isize>>>,
    cancelled: Arc<AtomicBool>,
}

impl NativeGameWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stop(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut g) = self.cancel_event_raw.lock() {
            if let Some(raw) = g.take() {
                unsafe {
                    let h = HANDLE(raw as *mut _);
                    let _ = SetEvent(h);
                }
            }
        }
    }

    pub fn watch_pid<F>(&self, pid: u32, exe_name: String, logger: CommandLogger, on_exit: F) -> bool
    where
        F: FnOnce(String) + Send + 'static,
    {
        self.stop();
        self.cancelled.store(false, Ordering::SeqCst);

        let proc_handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) };
        let Ok(proc_h) = proc_handle else {
            return false;
        };

        let cancel_ev = unsafe { CreateEventW(None, true, false, PCWSTR::null()) };
        let Ok(ev_h) = cancel_ev else {
            unsafe { let _ = CloseHandle(proc_h); }
            return false;
        };

        let proc_raw = proc_h.0 as isize;
        let ev_raw = ev_h.0 as isize;

        if let Ok(mut g) = self.cancel_event_raw.lock() {
            *g = Some(ev_raw);
        }

        logger.info(
            "system",
            &format!("Win32 OpenProcess + WaitForSingleObject(PID {} — {})", pid, exe_name),
            "Surveillance fermeture jeu active à 0% CPU (attente noyau Win32).",
            "ok",
        );

        let cancel_flag = self.cancelled.clone();
        let ev_slot = self.cancel_event_raw.clone();

        thread::spawn(move || {
            let handles = [HANDLE(proc_raw as *mut _), HANDLE(ev_raw as *mut _)];
            let wait_res = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };

            unsafe {
                let _ = CloseHandle(HANDLE(proc_raw as *mut _));
                let _ = CloseHandle(HANDLE(ev_raw as *mut _));
            }
            if let Ok(mut g) = ev_slot.lock() {
                if *g == Some(ev_raw) {
                    *g = None;
                }
            }

            // WAIT_OBJECT_0 means handles[0] (the game process) signaled exit!
            if wait_res == WAIT_OBJECT_0 && !cancel_flag.load(Ordering::SeqCst) {
                on_exit(exe_name);
            }
        });

        true
    }
}
