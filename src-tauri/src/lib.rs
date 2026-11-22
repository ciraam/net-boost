pub mod boost_engine;
pub mod firewall;
pub mod geoip;
pub mod icmp;
pub mod inspector;
pub mod logger;
pub mod network;
pub mod processes;
pub mod registry;
pub mod security;
pub mod store;
pub mod tunnel;
pub mod types;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, State, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::DialogExt;

use crate::boost_engine::BoostEngine;
use crate::icmp::{ping_once_native, PingMonitor, TracerouteRunner};
use crate::inspector::inspect_live_connections;
use crate::logger::CommandLogger;
use crate::network::{benchmark_dns_parallel, get_active_network_adapter};
use crate::processes::{scan_background_traffic, scan_running_processes};
use crate::security::{is_admin_cached, relaunch_as_admin};
use crate::store::{strip_wireguard_secrets, AppStore};
use crate::tunnel::{parse_wireguard_conf, TunnelManager};
use crate::types::{
    ActionResult, AppConfig, BackgroundSnapshot, BackupState, BoostOptions, BoostState,
    CommandLogEntry, DnsBenchmarkResult, GameProfile, ImportConfResult, InspectOptions,
    InspectorSnapshot, PartialAppConfig, PickExeResult, ProcessScanResult, RollbackResult,
    SimpleResult, SystemInfo, TunnelActionResult, TunnelState,
};

pub struct AppState {
    pub store: AppStore,
    pub logger: CommandLogger,
    pub ping_monitor: PingMonitor,
    pub traceroute: TracerouteRunner,
    pub tunnel: TunnelManager,
    pub boost_engine: BoostEngine,
    pub orphan_backup_at_start: Arc<AtomicBool>,
    pub quitting: Arc<AtomicBool>,
}

fn get_cli_arg(flag: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    for i in 1..args.len() {
        if args[i] == flag {
            return args.get(i + 1).cloned();
        }
        if let Some(val) = args[i].strip_prefix(&format!("{}=", flag)) {
            return Some(val.to_string());
        }
    }
    None
}

#[tauri::command]
async fn get_system_info(state: State<'_, AppState>) -> Result<SystemInfo, String> {
    let logger = state.logger.clone();
    let active_adapter =
        tokio::task::spawn_blocking(move || get_active_network_adapter(false, &logger))
            .await
            .ok()
            .flatten();

    let backup = state.store.read_backup();
    let has_backup = state.store.has_backup();
    let is_boosted = state.boost_engine.get_state().active;
    let has_orphan =
        state.orphan_backup_at_start.load(Ordering::SeqCst) && has_backup && !is_boosted;

    Ok(SystemInfo {
        is_admin: is_admin_cached(),
        hostname: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "WINDOWS-PC".into()),
        os_caption: "Windows 10/11 (Tauri v2 + Rust natif)".into(),
        active_adapter,
        has_backup,
        has_orphan_backup: has_orphan,
        backup_timestamp: backup.map(|b| b.created_at),
        backup_path: state.store.backup_path().to_string_lossy().to_string(),
    })
}

#[tauri::command]
fn relaunch_admin(app: AppHandle, state: State<'_, AppState>) -> SimpleResult {
    match relaunch_as_admin(&state.logger) {
        Ok(()) => {
            state.quitting.store(true, Ordering::SeqCst);
            let app_c = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(350));
                app_c.exit(0);
            });
            SimpleResult { ok: true, error: None }
        }
        Err(e) => SimpleResult {
            ok: false,
            error: Some(e),
        },
    }
}

/// Opens and focuses the main WebView2 window, recreating it from `tauri.conf.json`
/// if it was destroyed during System Tray hibernation (0 MB UI / 0 MB VRAM / 0% CPU).
fn open_or_focus_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }

    if let Some(win_cfg) = app.config().app.windows.first() {
        if let Ok(builder) = WebviewWindowBuilder::from_config(app, win_cfg) {
            if let Ok(win) = builder.visible(true).build() {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
                if let Some(state) = app.try_state::<AppState>() {
                    state.logger.info(
                        "system",
                        "Fenêtre NetBoost recréée depuis le System Tray",
                        "Interface WebView2 restaurée à la demande.",
                        "ok",
                    );
                }
            }
        }
    }
}

#[tauri::command]
fn minimize_to_tray(app: AppHandle, state: State<'_, AppState>) -> SimpleResult {
    state.ping_monitor.stop();
    state.traceroute.cancel();
    state.logger.info(
        "system",
        "Mode System Tray (0 Mo VRAM / 0% CPU)",
        "Fenêtre WebView2 détruite et ping arrêté ; NetBoost reste actif en arrière-plan.",
        "ok",
    );
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.destroy();
    }
    SimpleResult { ok: true, error: None }
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.store.load_public_config()
}

#[tauri::command]
fn save_config(partial: PartialAppConfig, state: State<'_, AppState>) -> Result<AppConfig, String> {
    state.store.save_config(partial)
}

#[tauri::command]
async fn pick_executable(app: AppHandle) -> PickExeResult {
    let picked = app
        .dialog()
        .file()
        .set_title("Sélectionner l'exécutable du jeu (.exe)")
        .add_filter("Exécutables Windows (*.exe)", &["exe"])
        .blocking_pick_file();

    let Some(file_path) = picked else {
        return PickExeResult {
            canceled: true,
            exe_path: None,
            exe_name: None,
            suggested_name: None,
        };
    };

    let path_buf = file_path.into_path().unwrap_or_default();
    let exe_path = path_buf.to_string_lossy().to_string();
    let exe_name = path_buf
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let suggested = exe_name
        .trim_end_matches(".exe")
        .trim_end_matches(".EXE")
        .trim_end_matches("-Win64-Shipping")
        .replace(['_', '-'], " ")
        .trim()
        .to_string();

    PickExeResult {
        canceled: false,
        exe_path: Some(exe_path),
        exe_name: Some(exe_name),
        suggested_name: Some(suggested),
    }
}

#[tauri::command]
async fn scan_processes() -> ProcessScanResult {
    tokio::task::spawn_blocking(scan_running_processes)
        .await
        .unwrap_or(ProcessScanResult {
            games: Vec::new(),
            all: Vec::new(),
        })
}

#[tauri::command]
async fn benchmark_dns(state: State<'_, AppState>) -> Result<Vec<DnsBenchmarkResult>, String> {
    Ok(benchmark_dns_parallel(&state.logger).await)
}

#[tauri::command]
async fn apply_dns_server(
    servers: Vec<String>,
    state: State<'_, AppState>,
) -> Result<ActionResult, String> {
    let engine = state.boost_engine.clone();
    Ok(tokio::task::spawn_blocking(move || engine.apply_dns_servers(&servers))
        .await
        .unwrap_or(ActionResult {
            ok: false,
            message: "Erreur interne DNS".into(),
        }))
}

#[tauri::command]
fn get_boost_state(state: State<'_, AppState>) -> BoostState {
    state.boost_engine.get_state()
}

#[tauri::command]
async fn start_boost(
    game: Option<GameProfile>,
    host: String,
    options: BoostOptions,
    state: State<'_, AppState>,
) -> Result<BoostState, String> {
    Ok(state.boost_engine.start_boost(game, host, options).await)
}

#[tauri::command]
async fn stop_boost(state: State<'_, AppState>) -> Result<BoostState, String> {
    let engine = state.boost_engine.clone();
    let orphan = state.orphan_backup_at_start.clone();
    let res = tokio::task::spawn_blocking(move || engine.rollback_all())
        .await
        .ok();
    if let Some(r) = res {
        if r.ok {
            orphan.store(false, Ordering::SeqCst);
        }
        return Ok(r.state);
    }
    Ok(state.boost_engine.get_state())
}

#[tauri::command]
async fn rollback_all(state: State<'_, AppState>) -> Result<RollbackResult, String> {
    let engine = state.boost_engine.clone();
    let orphan = state.orphan_backup_at_start.clone();
    let res = tokio::task::spawn_blocking(move || engine.rollback_all())
        .await
        .unwrap_or_else(|_| RollbackResult {
            ok: false,
            message: "Erreur interne rollback".into(),
            state: state.boost_engine.get_state(),
        });
    if res.ok {
        orphan.store(false, Ordering::SeqCst);
    }
    Ok(res)
}

#[tauri::command]
fn get_backup_json(state: State<'_, AppState>) -> Option<BackupState> {
    state.store.read_backup()
}

#[tauri::command]
async fn scan_background(state: State<'_, AppState>) -> Result<BackgroundSnapshot, String> {
    let throttled = state.boost_engine.get_throttled_exes();
    let paused = state.boost_engine.get_paused_services();
    Ok(tokio::task::spawn_blocking(move || scan_background_traffic(&throttled, &paused))
        .await
        .unwrap_or(BackgroundSnapshot {
            services: Vec::new(),
            apps: Vec::new(),
            scanned_at: String::new(),
        }))
}

#[tauri::command]
async fn toggle_app_throttle(
    exe_name: String,
    throttle: bool,
    state: State<'_, AppState>,
) -> Result<ActionResult, String> {
    let engine = state.boost_engine.clone();
    Ok(tokio::task::spawn_blocking(move || engine.toggle_app_throttle(&exe_name, throttle))
        .await
        .unwrap_or(ActionResult {
            ok: false,
            message: "Erreur interne QoS".into(),
        }))
}

#[tauri::command]
async fn toggle_service_pause(
    service_name: String,
    pause: bool,
    state: State<'_, AppState>,
) -> Result<ActionResult, String> {
    let engine = state.boost_engine.clone();
    Ok(tokio::task::spawn_blocking(move || engine.toggle_service_pause(&service_name, pause))
        .await
        .unwrap_or(ActionResult {
            ok: false,
            message: "Erreur interne SCM".into(),
        }))
}

#[tauri::command]
async fn inspect_connections(
    exe_name: Option<String>,
    options: Option<InspectOptions>,
    state: State<'_, AppState>,
) -> Result<InspectorSnapshot, String> {
    let blocked = state.boost_engine.get_firewall_rules();
    Ok(inspect_live_connections(exe_name, blocked, options.unwrap_or_default(), &state.logger).await)
}

#[tauri::command]
async fn block_remote_ip(
    ip_or_cidr: String,
    exe_name: Option<String>,
    exe_path: Option<String>,
    country_code: Option<String>,
    state: State<'_, AppState>,
) -> Result<ActionResult, String> {
    let engine = state.boost_engine.clone();
    Ok(tokio::task::spawn_blocking(move || {
        engine.block_remote_address(
            &ip_or_cidr,
            exe_name.as_deref(),
            exe_path.as_deref(),
            country_code.as_deref(),
        )
    })
    .await
    .unwrap_or(ActionResult {
        ok: false,
        message: "Erreur interne Pare-feu".into(),
    }))
}

#[tauri::command]
async fn unblock_firewall_rule(
    rule_name: String,
    state: State<'_, AppState>,
) -> Result<ActionResult, String> {
    let engine = state.boost_engine.clone();
    Ok(tokio::task::spawn_blocking(move || engine.unblock_remote_rule(&rule_name))
        .await
        .unwrap_or(ActionResult {
            ok: false,
            message: "Erreur interne Pare-feu".into(),
        }))
}

#[tauri::command]
async fn import_wireguard_conf(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ImportConfResult, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Importer une configuration WireGuard (.conf)")
        .add_filter("Configuration WireGuard (*.conf)", &["conf"])
        .blocking_pick_file();

    let Some(file_path) = picked else {
        return Ok(ImportConfResult {
            canceled: true,
            profile: None,
            error: None,
        });
    };

    let path_buf = file_path.into_path().unwrap_or_default();
    let file_name = path_buf
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "tunnel.conf".into());

    match fs::read_to_string(&path_buf)
        .map_err(|e| e.to_string())
        .and_then(|raw| parse_wireguard_conf(&raw, &file_name))
    {
        Ok(parsed) => {
            let partial = PartialAppConfig {
                games: None,
                presets: None,
                selected_game_id: None,
                selected_host: None,
                boost_options: None,
                tunnel_mode: None,
                wireguard_profile: Some(Some(parsed.clone())),
            };
            match state.store.save_config(partial) {
                Ok(saved) => {
                    let safe = saved
                        .wireguard_profile
                        .or_else(|| strip_wireguard_secrets(Some(parsed)));
                    if let Some(ref p) = safe {
                        state.logger.info(
                            "tunnel",
                            &format!("Import .conf : {}", p.file_name),
                            &format!(
                                "Relais Endpoint={} | Chiffrement DPAPI Windows (CryptProtectData) | Clé={}",
                                p.endpoint, p.public_key_preview
                            ),
                            "ok",
                        );
                    }
                    Ok(ImportConfResult {
                        canceled: false,
                        profile: safe,
                        error: None,
                    })
                }
                Err(e) => Ok(ImportConfResult {
                    canceled: false,
                    profile: None,
                    error: Some(e),
                }),
            }
        }
        Err(e) => {
            state.logger.info("tunnel", "Import .conf WireGuard", &e, "error");
            Ok(ImportConfResult {
                canceled: false,
                profile: None,
                error: Some(e),
            })
        }
    }
}

#[tauri::command]
fn clear_wireguard_conf(state: State<'_, AppState>) -> SimpleResult {
    if state.tunnel.is_active() {
        let _ = state.tunnel.stop_tunnel();
    }
    let _ = state.store.save_config(PartialAppConfig {
        games: None,
        presets: None,
        selected_game_id: None,
        selected_host: None,
        boost_options: None,
        tunnel_mode: None,
        wireguard_profile: Some(None),
    });
    state
        .logger
        .info("tunnel", "Suppression profil .conf", "Profil WireGuard supprimé.", "ok");
    SimpleResult { ok: true, error: None }
}

#[tauri::command]
async fn get_tunnel_state(state: State<'_, AppState>) -> Result<TunnelState, String> {
    let tunnel = state.tunnel.clone();
    Ok(tokio::task::spawn_blocking(move || tunnel.refresh_clients())
        .await
        .unwrap_or_else(|_| state.tunnel.get_state()))
}

#[tauri::command]
async fn start_tunnel(
    mode: String,
    exe_name: Option<String>,
    target_host: String,
    extra_ips: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<TunnelActionResult, String> {
    let engine = state.boost_engine.clone();
    let ips = extra_ips.unwrap_or_default();
    Ok(tokio::task::spawn_blocking(move || {
        engine.start_split_tunnel(&mode, exe_name.as_deref(), &target_host, &ips)
    })
    .await
    .unwrap_or_else(|_| TunnelActionResult {
        ok: false,
        message: "Erreur interne tunnel".into(),
        state: state.tunnel.get_state(),
    }))
}

#[tauri::command]
async fn stop_tunnel(state: State<'_, AppState>) -> Result<TunnelActionResult, String> {
    let tunnel = state.tunnel.clone();
    Ok(tokio::task::spawn_blocking(move || tunnel.stop_tunnel())
        .await
        .unwrap_or_else(|_| TunnelActionResult {
            ok: false,
            message: "Erreur interne arrêt tunnel".into(),
            state: state.tunnel.get_state(),
        }))
}

#[tauri::command]
fn start_ping_monitor(host: String, state: State<'_, AppState>) -> SimpleResult {
    match state.ping_monitor.start(&host) {
        Ok(()) => SimpleResult { ok: true, error: None },
        Err(e) => {
            state.logger.info("monitor", &format!("ping {}", host), &e, "error");
            SimpleResult {
                ok: false,
                error: Some(e),
            }
        }
    }
}

#[tauri::command]
fn stop_ping_monitor(state: State<'_, AppState>) -> SimpleResult {
    state.ping_monitor.stop();
    SimpleResult { ok: true, error: None }
}

#[tauri::command]
fn reset_comparison(state: State<'_, AppState>) -> SimpleResult {
    state.ping_monitor.reset_comparison();
    SimpleResult { ok: true, error: None }
}

#[tauri::command]
fn run_traceroute(host: String, state: State<'_, AppState>) -> SimpleResult {
    match state.traceroute.run(&host) {
        Ok(()) => SimpleResult { ok: true, error: None },
        Err(e) => {
            state.logger.info("monitor", &format!("tracert {}", host), &e, "error");
            SimpleResult {
                ok: false,
                error: Some(e),
            }
        }
    }
}

#[tauri::command]
fn cancel_traceroute(state: State<'_, AppState>) -> SimpleResult {
    state.traceroute.cancel();
    SimpleResult { ok: true, error: None }
}

#[tauri::command]
fn get_logs(state: State<'_, AppState>) -> Vec<CommandLogEntry> {
    state.logger.get_logs()
}

#[tauri::command]
fn clear_logs(state: State<'_, AppState>) {
    state.logger.clear();
}

/// Captures a Win32 HWND via PrintWindow (PW_RENDERFULLCONTENT) into a PNG file for `--screenshot`.
fn capture_hwnd_to_png(hwnd: windows::Win32::Foundation::HWND, out_path: &Path) -> Result<(), String> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
        ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    unsafe {
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect).map_err(|e| e.to_string())?;
        let width = (rect.right - rect.left).max(1);
        let height = (rect.bottom - rect.top).max(1);

        let hdc_screen = GetDC(hwnd);
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbm = CreateCompatibleBitmap(hdc_screen, width, height);
        let old = SelectObject(hdc_mem, hbm);

        // PW_RENDERFULLCONTENT = 0x00000002 captures DirectComposition / WebView2 even when covered!
        let _ = PrintWindow(hwnd, hdc_mem, PRINT_WINDOW_FLAGS(2));

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bgra = vec![0u8; (width * height * 4) as usize];
        GetDIBits(
            hdc_mem,
            hbm,
            0,
            height as u32,
            Some(bgra.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(hdc_mem, old);
        let _ = DeleteObject(hbm);
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(hwnd, hdc_screen);

        // Convert BGRA -> RGBA
        let mut rgba = vec![0u8; bgra.len()];
        for (src, dst) in bgra.chunks_exact(4).zip(rgba.chunks_exact_mut(4)) {
            dst[0] = src[2];
            dst[1] = src[1];
            dst[2] = src[0];
            dst[3] = 255;
        }

        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let file = fs::File::create(out_path).map_err(|e| e.to_string())?;
        let mut encoder = png::Encoder::new(file, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn run() {
    let screenshot_path = get_cli_arg("--screenshot");
    let self_test_path = get_cli_arg("--self-test");
    let is_automated = screenshot_path.is_some() || self_test_path.is_some();

    let logger = CommandLogger::new();
    let ping_monitor = PingMonitor::new(logger.clone());
    let traceroute = TracerouteRunner::new(logger.clone());

    let user_data_dir: PathBuf = std::env::var("APPDATA")
        .map(|a| PathBuf::from(a).join("NetBoost"))
        .unwrap_or_else(|_| std::env::temp_dir().join("NetBoost"));

    let store = AppStore::new(user_data_dir.clone());
    let tunnel = TunnelManager::new(&user_data_dir, logger.clone());
    tunnel.cleanup_stale_configs();

    let orphan_backup = Arc::new(AtomicBool::new(store.has_backup()));
    let boost_engine = BoostEngine::new(
        store.clone(),
        tunnel.clone(),
        ping_monitor.clone(),
        logger.clone(),
    );

    logger.info(
        "system",
        "NetBoost (Tauri v2 + Rust) démarré",
        &format!("Dossier données : {}", user_data_dir.display()),
        "ok",
    );
    if orphan_backup.load(Ordering::SeqCst) {
        logger.info(
            "rollback",
            "Sauvegarde orpheline détectée au démarrage",
            "backup-state.json trouvé suite à un arrêt imprévu. Restauration (Rollback) recommandée.",
            "warn",
        );
    }

    let quitting = Arc::new(AtomicBool::new(false));
    let state = AppState {
        store: store.clone(),
        logger: logger.clone(),
        ping_monitor: ping_monitor.clone(),
        traceroute: traceroute.clone(),
        tunnel: tunnel.clone(),
        boost_engine: boost_engine.clone(),
        orphan_backup_at_start: orphan_backup.clone(),
        quitting: quitting.clone(),
    };

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(move |app| {
            let handle = app.handle().clone();
            logger.attach_app(handle.clone());
            ping_monitor.attach_app(handle.clone());
            traceroute.attach_app(handle.clone());
            tunnel.attach_app(handle.clone());
            boost_engine.attach_app(handle.clone());

            // Setup System Tray for zero-overhead background minimization during matches
            let open_item = MenuItem::with_id(app, "open", "Ouvrir NetBoost", true, None::<&str>)?;
            let rollback_item = MenuItem::with_id(app, "rollback", "Restaurer la config (Rollback)", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quitter (avec Rollback)", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&open_item, &rollback_item, &quit_item])?;

            let tray_icon_bytes = include_bytes!("../icons/icon.png");
            let tray_image = tauri::image::Image::from_bytes(tray_icon_bytes)?;

            let quit_flag = quitting.clone();
            let be_tray = boost_engine.clone();
            let pm_tray = ping_monitor.clone();
            let tr_tray = traceroute.clone();

            let _tray = TrayIconBuilder::new()
                .icon(tray_image)
                .tooltip("NetBoost — Tactical Telemetry")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "open" => {
                        open_or_focus_main_window(app);
                    }
                    "rollback" => {
                        let be = be_tray.clone();
                        std::thread::spawn(move || {
                            let _ = be.rollback_all();
                        });
                    }
                    "quit" => {
                        quit_flag.store(true, Ordering::SeqCst);
                        pm_tray.stop();
                        tr_tray.cancel();
                        if be_tray.has_pending_modifications() {
                            let _ = be_tray.rollback_all();
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        open_or_focus_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            // Show window cleanly (without stealing focus in automated --screenshot / --self-test mode)
            if let Some(win) = app.get_webview_window("main") {
                if is_automated {
                    unsafe {
                        use windows::Win32::Foundation::HWND;
                        use windows::Win32::UI::WindowsAndMessaging::{
                            SetWindowPos, ShowWindow, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE,
                            SWP_NOSIZE, SW_SHOWNOACTIVATE,
                        };
                        if let Ok(hwnd_raw) = win.hwnd() {
                            let hwnd = HWND(hwnd_raw.0);
                            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                            let _ = SetWindowPos(
                                hwnd,
                                HWND_BOTTOM,
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                        }
                    }
                } else {
                    let _ = win.show();
                    let _ = win.set_focus();
                }

                // Automated --screenshot mode
                if let Some(out_png) = screenshot_path.clone() {
                    let hwnd_isize = win.hwnd().ok().map(|h| h.0 as isize);
                    let app_c = handle.clone();
                    let pm_c = ping_monitor.clone();
                    let tr_c = traceroute.clone();
                    let q_c = quitting.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(3800));
                        let out_png_c = out_png.clone();
                        let app_exit = app_c.clone();
                        let pm_exit = pm_c.clone();
                        let tr_exit = tr_c.clone();
                        let q_exit = q_c.clone();
                        let _ = app_c.run_on_main_thread(move || {
                            if let Some(raw) = hwnd_isize {
                                let hwnd = windows::Win32::Foundation::HWND(raw as *mut _);
                                let _ = capture_hwnd_to_png(hwnd, Path::new(&out_png_c));
                            }
                            q_exit.store(true, Ordering::SeqCst);
                            pm_exit.stop();
                            tr_exit.cancel();
                            app_exit.exit(0);
                        });
                    });
                }

                // Automated --self-test mode
                if let Some(out_json) = self_test_path.clone() {
                    let app_c = handle.clone();
                    let logger_c = logger.clone();
                    let pm_c = ping_monitor.clone();
                    let tr_c = traceroute.clone();
                    let tunnel_c = tunnel.clone();
                    let q_c = quitting.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(2200)).await;
                        let adapter = get_active_network_adapter(false, &logger_c);
                        let procs = scan_running_processes();
                        let ping_ms = ping_once_native("1.1.1.1", 2000).ok().flatten();
                        let dns_res = benchmark_dns_parallel(&logger_c).await;
                        let dns_best = dns_res.iter().find(|d| d.fastest);
                        let insp = inspect_live_connections(None, Vec::new(), InspectOptions::default(), &logger_c).await;
                        let t_state = tunnel_c.refresh_clients();

                        let report = serde_json::json!({
                            "version": 1,
                            "steps": [
                                {
                                    "step": "move",
                                    "passed": adapter.is_some() && !procs.all.is_empty(),
                                    "detail": match adapter {
                                        Some(ref a) => format!(
                                            "Native Win32 adapter {} ({}), {} processes, {} TCP connections ({} non-EU offline GeoIP), WireSock={}, WireGuard={}",
                                            a.name, a.ipv4, procs.all.len(), insp.connections.len(), insp.non_eu_count,
                                            t_state.clients.wiresock_path.is_some(), t_state.clients.wireguard_path.is_some()
                                        ),
                                        None => "No active network adapter detected".into()
                                    }
                                },
                                {
                                    "step": "act",
                                    "passed": ping_ms.is_some() && dns_best.is_some(),
                                    "detail": format!(
                                        "Native IcmpSendEcho 1.1.1.1 = {:?} ms; Native UDP DNS fastest = {} ({:?} ms)",
                                        ping_ms,
                                        dns_best.map(|d| d.ip.as_str()).unwrap_or("none"),
                                        dns_best.and_then(|d| d.avg_ms)
                                    )
                                },
                                {
                                    "step": "hud",
                                    "passed": app_c.get_webview_window("main").is_some(),
                                    "detail": "Tauri v2 WebView2 Tactical Telemetry window + System Tray initialized"
                                }
                            ]
                        });

                        let p = PathBuf::from(&out_json);
                        if let Some(parent) = p.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        let _ = fs::write(&p, serde_json::to_string_pretty(&report).unwrap_or_default());
                        q_c.store(true, Ordering::SeqCst);
                        pm_c.stop();
                        tr_c.cancel();
                        app_c.exit(0);
                    });
                }
            }

            Ok(())
        })
        .on_window_event(move |window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if !state.quitting.load(Ordering::SeqCst)
                    && !is_automated
                    && (state.boost_engine.get_state().active || state.tunnel.is_active())
                {
                    // Destroy WebView2 window and stop ping monitor to free 100% of msedgewebview2.exe (0 MB VRAM / 0% CPU)
                    api.prevent_close();
                    state.ping_monitor.stop();
                    state.traceroute.cancel();
                    state.logger.info(
                        "system",
                        "Mode System Tray (0 Mo VRAM / 0% CPU)",
                        "Fenêtre WebView2 détruite pendant la session active ; cliquez sur l'icône Tray pour rouvrir NetBoost.",
                        "ok",
                    );
                    let _ = window.destroy();
                } else {
                    // Normal close when neither Boost nor Tunnel is active -> allow full exit with rollback
                    state.quitting.store(true, Ordering::SeqCst);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_system_info,
            relaunch_admin,
            minimize_to_tray,
            get_config,
            save_config,
            pick_executable,
            scan_processes,
            benchmark_dns,
            apply_dns_server,
            get_boost_state,
            start_boost,
            stop_boost,
            rollback_all,
            get_backup_json,
            scan_background,
            toggle_app_throttle,
            toggle_service_pause,
            inspect_connections,
            block_remote_ip,
            unblock_firewall_rule,
            import_wireguard_conf,
            clear_wireguard_conf,
            get_tunnel_state,
            start_tunnel,
            stop_tunnel,
            start_ping_monitor,
            stop_ping_monitor,
            reset_comparison,
            run_traceroute,
            cancel_traceroute,
            get_logs,
            clear_logs,
        ])
        .build(tauri::generate_context!())
        .expect("Erreur d'initialisation Tauri v2");

    app.run(move |app_handle, event| {
        if let RunEvent::ExitRequested { api, .. } = event {
            let state = app_handle.state::<AppState>();
            if !is_automated && !state.quitting.load(Ordering::SeqCst) {
                // Keep Rust background process + TrayIcon + 0% CPU Game Watcher alive after WebView2 destruction
                api.prevent_exit();
                return;
            }
            state.ping_monitor.stop();
            state.traceroute.cancel();
            if !is_automated && state.boost_engine.has_pending_modifications() {
                state.logger.info(
                    "rollback",
                    "Fermeture application détectée",
                    "Restauration auto complète : coupure tunnel + suppression Pare-feu/QoS + Rollback Windows...",
                    "warn",
                );
                let _ = state.boost_engine.rollback_all();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::geoip::lookup_geo_offline;
    use crate::icmp::compute_ping_stats;
    use crate::inspector::{merge_connection_rows, parse_etw_udp_events, RawConnRow, RawEtwUdpEvent};
    use crate::registry::build_interface_registry_path;
    use crate::security::{
        dpapi_decrypt, dpapi_encrypt, is_valid_interface_registry_path, sanitize_exe_name,
        sanitize_host, sanitize_ip_or_cidr, to_slash24,
    };
    use crate::store::AppStore;
    use crate::tunnel::{
        generate_split_tunnel_conf, mask_private_key_for_preview, parse_wireguard_conf,
    };
    use crate::types::{PartialAppConfig, PingSample};

    const SAMPLE_WG_CONF: &str = "\
[Interface]
PrivateKey = c2VjcmV0X3ByaXZhdGVfa2V5X2V4YW1wbGVfMTIzNDU2Nzg=
Address = 10.66.66.2/32
DNS = 1.1.1.1

[Peer]
PublicKey = cHVibGljX2tleV9leGFtcGxlXzk4NzY1NDMyMTA5ODc2NTQ=
Endpoint = 185.12.34.56:51820
AllowedIPs = 0.0.0.0/0, ::/0
PersistentKeepalive = 25";

    #[test]
    fn test_strict_sanitizers() {
        assert_eq!(sanitize_host("1.1.1.1").unwrap(), "1.1.1.1");
        assert_eq!(sanitize_host("fra-de-ping.vultr.com").unwrap(), "fra-de-ping.vultr.com");
        assert!(sanitize_host("-t").is_err());
        assert!(sanitize_host("/a").is_err());
        assert!(sanitize_host("1.1.1.1; calc.exe").is_err());

        assert_eq!(sanitize_ip_or_cidr("185.60.112.0/24").unwrap(), "185.60.112.0/24");
        assert_eq!(to_slash24("121.254.17.88"), "121.254.17.0/24");
        assert!(sanitize_ip_or_cidr("999.1.1.1").is_err());
        assert!(sanitize_ip_or_cidr("1.2.3.4/33").is_err());
        assert!(sanitize_ip_or_cidr("-flag").is_err());

        assert_eq!(sanitize_exe_name(r"C:\Games\cs2.exe").unwrap(), "cs2.exe");
        assert!(sanitize_exe_name("-evil.exe").is_err());
        assert!(sanitize_exe_name("script.ps1").is_err());

        let valid_reg = r"HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{C38F1B9D-F548-40F3-B431-70CF719792C9}";
        assert!(is_valid_interface_registry_path(valid_reg));
        assert_eq!(
            build_interface_registry_path(Some("{C38F1B9D-F548-40F3-B431-70CF719792C9}")).as_deref(),
            Some(valid_reg)
        );
        assert!(!is_valid_interface_registry_path(r"HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run"));
        assert!(!is_valid_interface_registry_path(r"HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\..\evil"));
    }

    #[test]
    fn test_windows_dpapi_roundtrip_and_store_encryption() {
        let secret = b"PrivateKey = super_secret_wg_key_12345";
        let enc = dpapi_encrypt(secret).expect("DPAPI encrypt failed");
        assert_ne!(enc, secret);
        let dec = dpapi_decrypt(&enc).expect("DPAPI decrypt failed");
        assert_eq!(dec, secret);

        let tmp = std::env::temp_dir().join(format!("netboost_test_{}", uuid::Uuid::new_v4()));
        let store = AppStore::new(tmp.clone());
        let parsed = parse_wireguard_conf(SAMPLE_WG_CONF, "eu-fra.conf").unwrap();
        let saved = store
            .save_config(PartialAppConfig {
                games: None,
                presets: None,
                selected_game_id: None,
                selected_host: None,
                boost_options: None,
                tunnel_mode: None,
                wireguard_profile: Some(Some(parsed)),
            })
            .unwrap();

        let pub_prof = saved.wireguard_profile.unwrap();
        assert!(pub_prof.is_encrypted);
        assert!(pub_prof.raw_conf.is_none());
        assert!(pub_prof.encrypted_conf.is_none());

        let on_disk = std::fs::read_to_string(tmp.join("netboost-config.json")).unwrap();
        assert!(!on_disk.contains("c2VjcmV0X3ByaXZhdGVfa2V5X2V4YW1wbGVfMTIzNDU2Nzg="));
        assert!(on_disk.contains("\"encryptedConf\""));

        let (_, decrypted) = store.get_decrypted_wireguard_conf().unwrap().unwrap();
        assert_eq!(decrypted, SAMPLE_WG_CONF);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_wireguard_parser_and_split_tunnel() {
        let p = parse_wireguard_conf(SAMPLE_WG_CONF, "eu.conf").unwrap();
        assert_eq!(p.endpoint, "185.12.34.56:51820");
        assert!(mask_private_key_for_preview(SAMPLE_WG_CONF).contains("[MASQUÉ_PAR_SÉCURITÉ]"));

        let ws = generate_split_tunnel_conf(SAMPLE_WG_CONF, "wiresock", Some("cs2.exe"), &[]);
        assert!(ws.contains("AllowedApps = cs2.exe"));
        assert!(ws.contains("DisallowedIPs = 10.0.0.0/8"));

        let wg = generate_split_tunnel_conf(
            SAMPLE_WG_CONF,
            "wireguard",
            Some("cs2.exe"),
            &["185.60.112.157".into()],
        );
        assert!(wg.contains("AllowedIPs = 185.60.112.157/32"));
        assert!(!wg.contains("AllowedIPs = 0.0.0.0/0"));
    }

    #[test]
    fn test_offline_geoip_and_etw_udp_merge() {
        let lan = lookup_geo_offline("192.168.1.10").unwrap();
        assert!(lan.is_private);
        assert_eq!(lan.country_code, "LAN");

        let us = lookup_geo_offline("8.8.8.8").unwrap();
        assert_eq!(us.country_code, "US");
        assert!(!us.is_eu);

        let de = lookup_geo_offline("88.198.10.1").unwrap();
        assert_eq!(de.country_code, "DE");
        assert!(de.is_eu);

        let kr = lookup_geo_offline("121.254.17.88").unwrap();
        assert_eq!(kr.country_code, "KR");
        assert!(!kr.is_eu);

        let mut pids = HashSet::new();
        pids.insert(4242);
        let etw = parse_etw_udp_events(
            &[
                RawEtwUdpEvent {
                    event_id: 42,
                    pid: 4242,
                    saddr: "10.21.40.104".into(),
                    daddr: "121.254.17.88".into(),
                    sport: 55000,
                    dport: 27015,
                },
                RawEtwUdpEvent {
                    event_id: 42,
                    pid: 4242,
                    saddr: "10.21.40.104".into(),
                    daddr: "121.254.17.88".into(),
                    sport: 55000,
                    dport: 27015,
                },
                RawEtwUdpEvent {
                    event_id: 43,
                    pid: 9999,
                    saddr: "8.8.8.8".into(),
                    daddr: "10.21.40.104".into(),
                    sport: 53,
                    dport: 55001,
                },
            ],
            &pids,
            "cs2.exe",
            r"C:\Games\cs2.exe",
        );
        assert_eq!(etw.len(), 1);
        assert_eq!(etw[0].remote_address, "121.254.17.88");

        let merged = merge_connection_rows(
            vec![RawConnRow {
                protocol: "UDP".into(),
                pid: 4242,
                exe_name: "cs2.exe".into(),
                exe_path: r"C:\Games\cs2.exe".into(),
                local_address: "0.0.0.0".into(),
                local_port: 55000,
                remote_address: "*".into(),
                remote_port: 0,
                state: "Socket local (fallback)".into(),
            }],
            etw,
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].remote_address, "121.254.17.88");
    }

    #[test]
    fn test_ping_stats_calculation() {
        let samples = vec![
            PingSample {
                seq: 1,
                timestamp: 1,
                time_label: "12:00:01".into(),
                host: "1.1.1.1".into(),
                rtt_ms: Some(10.0),
                jitter_ms: 0.0,
                loss_pct: 0.0,
                lost: false,
                phase: "before".into(),
            },
            PingSample {
                seq: 2,
                timestamp: 2,
                time_label: "12:00:02".into(),
                host: "1.1.1.1".into(),
                rtt_ms: Some(14.0),
                jitter_ms: 4.0,
                loss_pct: 0.0,
                lost: false,
                phase: "before".into(),
            },
            PingSample {
                seq: 3,
                timestamp: 3,
                time_label: "12:00:03".into(),
                host: "1.1.1.1".into(),
                rtt_ms: None,
                jitter_ms: 0.0,
                loss_pct: 33.3,
                lost: true,
                phase: "before".into(),
            },
        ];
        let st = compute_ping_stats(&samples);
        assert_eq!(st.sent, 3);
        assert_eq!(st.received, 2);
        assert_eq!(st.lost, 1);
        assert_eq!(st.avg_ms, Some(12.0));
        assert_eq!(st.jitter_ms, 4.0);
        assert!((st.loss_pct - 33.3).abs() < 0.2);
    }
}
