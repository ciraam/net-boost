use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandLogEntry {
    pub id: String,
    pub timestamp: String,
    pub category: String,
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub status: String, // "running" | "ok" | "error" | "warn"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterAdvancedProp {
    pub display_name: String,
    pub registry_keyword: String,
    pub display_value: String,
    pub registry_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAdapterInfo {
    pub name: String,
    pub description: String,
    pub interface_index: u32,
    pub interface_guid: String,
    pub link_speed: String,
    pub status: String,
    pub ipv4: String,
    pub current_dns: Vec<String>,
    pub dns_is_dhcp: bool,
    pub eee_properties: Vec<AdapterAdvancedProp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub is_admin: bool,
    pub hostname: String,
    pub os_caption: String,
    pub active_adapter: Option<NetworkAdapterInfo>,
    pub has_backup: bool,
    pub has_orphan_backup: bool,
    pub backup_timestamp: Option<String>,
    pub backup_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProfile {
    pub id: String,
    pub name: String,
    pub exe_name: String,
    pub exe_path: String,
    pub server_host: String,
    pub added_manually: bool,
    #[serde(default)]
    pub is_running: Option<bool>,
    #[serde(default)]
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe_name: String,
    pub exe_path: String,
    pub memory_mb: f64,
    pub window_title: String,
    pub is_likely_game: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessScanResult {
    pub games: Vec<RunningProcessInfo>,
    pub all: Vec<RunningProcessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPreset {
    pub id: String,
    pub name: String,
    pub region: String,
    pub host: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsBenchmarkResult {
    pub ip: String,
    pub name: String,
    pub avg_ms: Option<f64>,
    pub min_ms: Option<f64>,
    pub max_ms: Option<f64>,
    pub success_count: u32,
    pub total_count: u32,
    #[serde(default)]
    pub fastest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireGuardProfile {
    pub file_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_conf: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_conf: Option<String>,
    #[serde(default)]
    pub is_encrypted: bool,
    pub endpoint: String,
    pub interface_address: String,
    pub dns: String,
    pub allowed_ips_original: String,
    pub public_key_preview: String,
    pub imported_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TunnelClientsStatus {
    pub wiresock_path: Option<String>,
    pub wireguard_path: Option<String>,
    pub wiresock_signed: bool,
    pub wireguard_signed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelState {
    pub active: bool,
    pub busy: bool,
    pub mode: String, // "wiresock" | "wireguard"
    pub clients: TunnelClientsStatus,
    pub active_exe: Option<String>,
    pub active_allowed_ips: Vec<String>,
    pub endpoint: Option<String>,
    pub started_at: Option<String>,
    pub last_error: Option<String>,
    pub generated_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoIpInfo {
    pub ip: String,
    pub country: String,
    pub country_code: String,
    pub city: String,
    pub isp: String,
    #[serde(rename = "as")]
    pub asn: String,
    pub is_eu: bool,
    pub is_private: bool,
    pub source: String, // "offline" | "https" | "local"
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InspectOptions {
    #[serde(default)]
    pub capture_udp_etw: bool,
    #[serde(default)]
    pub allow_https_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveConnectionEntry {
    pub id: String,
    pub protocol: String, // "TCP" | "UDP"
    pub pid: u32,
    pub exe_name: String,
    pub exe_path: String,
    pub local_address: String,
    pub local_port: u16,
    pub remote_address: String,
    pub remote_port: u16,
    pub state: String,
    pub geo: GeoIpInfo,
    pub is_blocked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirewallBlockRule {
    pub rule_name: String,
    pub display_name: String,
    pub remote_address: String,
    pub exe_name: Option<String>,
    pub country_code: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorSnapshot {
    pub target_exe: Option<String>,
    pub connections: Vec<LiveConnectionEntry>,
    pub blocked_rules: Vec<FirewallBlockRule>,
    pub non_eu_count: usize,
    pub etw_udp_captured: bool,
    pub etw_udp_count: usize,
    pub etw_note: Option<String>,
    pub scanned_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoostOptions {
    #[serde(default)]
    pub enable_tunnel: bool,
    #[serde(default = "default_true")]
    pub enable_qos: bool,
    #[serde(default = "default_true")]
    pub enable_dns: bool,
    #[serde(default = "default_true")]
    pub enable_tcp_ip: bool,
    #[serde(default = "default_true")]
    pub disable_eee: bool,
    #[serde(default)]
    pub pause_windows_update: bool,
    #[serde(default)]
    pub throttle_background_apps: bool,
    #[serde(default = "default_true")]
    pub auto_rollback_on_game_exit: bool,
}

fn default_true() -> bool {
    true
}

impl Default for BoostOptions {
    fn default() -> Self {
        Self {
            enable_tunnel: false,
            enable_qos: true,
            enable_dns: true,
            enable_tcp_ip: true,
            disable_eee: true,
            pause_windows_update: false,
            throttle_background_apps: false,
            auto_rollback_on_game_exit: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoostStepStatus {
    pub key: String,
    pub label: String,
    pub status: String, // "idle" | "running" | "applied" | "skipped" | "error"
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoostState {
    pub active: bool,
    pub busy: bool,
    pub started_at: Option<String>,
    pub target_game: Option<GameProfile>,
    pub target_host: String,
    pub tracked_pid: Option<u32>,
    pub applied_dns: Option<Vec<String>>,
    pub steps: Vec<BoostStepStatus>,
    pub has_backup: bool,
    pub backup_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupRegistryValue {
    pub existed: bool,
    pub value: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupRegistryStringValue {
    pub existed: bool,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupAdapterState {
    pub name: String,
    pub interface_index: u32,
    pub interface_guid: String,
    pub dns_is_dhcp: bool,
    pub original_dns: Vec<String>,
    pub eee_properties: Vec<AdapterAdvancedProp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecondaryDnsAdapterBackup {
    pub name: String,
    pub interface_index: u32,
    pub dns_is_dhcp: bool,
    pub original_dns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupRegistryState {
    pub interface_key_path: Option<String>,
    pub tcp_ack_frequency: BackupRegistryValue,
    pub tcp_no_delay: BackupRegistryValue,
    pub network_throttling_index: BackupRegistryValue,
    #[serde(default)]
    pub qos_do_not_use_nla: Option<BackupRegistryStringValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPausedService {
    pub name: String,
    pub original_status: String,
    pub original_start_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupState {
    pub version: u32,
    pub created_at: String,
    pub adapter: Option<BackupAdapterState>,
    #[serde(default)]
    pub secondary_dns_adapters: Vec<SecondaryDnsAdapterBackup>,
    pub registry: BackupRegistryState,
    pub qos_policies_created: Vec<String>,
    #[serde(default)]
    pub throttled_exes: Vec<String>,
    #[serde(default)]
    pub firewall_rules_created: Vec<FirewallBlockRule>,
    #[serde(default)]
    pub tunnel_was_started: bool,
    pub services_paused: Vec<BackupPausedService>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingSample {
    pub seq: u64,
    pub timestamp: i64,
    pub time_label: String,
    pub host: String,
    pub rtt_ms: Option<f64>,
    pub jitter_ms: f64,
    pub loss_pct: f64,
    pub lost: bool,
    pub phase: String, // "before" | "after"
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PingStats {
    pub sent: u64,
    pub received: u64,
    pub lost: u64,
    pub loss_pct: f64,
    pub current_ms: Option<f64>,
    pub avg_ms: Option<f64>,
    pub min_ms: Option<f64>,
    pub max_ms: Option<f64>,
    pub jitter_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BeforeAfterComparison {
    pub before: PingStats,
    pub after: PingStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingEventPayload {
    pub sample: PingSample,
    pub stats: PingStats,
    pub comparison: BeforeAfterComparison,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracerouteHop {
    pub hop: u32,
    pub rtt1: Option<f64>,
    pub rtt2: Option<f64>,
    pub rtt3: Option<f64>,
    pub avg_ms: Option<f64>,
    pub ip: Option<String>,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracerouteEvent {
    #[serde(rename = "type")]
    pub event_type: String, // "start" | "hop" | "done" | "error"
    pub host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop: Option<TracerouteHop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsServiceInfo {
    pub name: String,
    pub display_name: String,
    pub status: String,
    pub start_type: String,
    pub paused_by_app: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundAppInfo {
    pub exe_name: String,
    pub display_name: String,
    pub category: String, // "browser" | "launcher" | "updater" | "other"
    pub pids: Vec<u32>,
    pub instance_count: usize,
    pub total_memory_mb: f64,
    pub tcp_connections: usize,
    pub throttled_by_app: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundSnapshot {
    pub services: Vec<WindowsServiceInfo>,
    pub apps: Vec<BackgroundAppInfo>,
    pub scanned_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub games: Vec<GameProfile>,
    pub presets: Vec<ServerPreset>,
    pub selected_game_id: Option<String>,
    pub selected_host: String,
    pub boost_options: BoostOptions,
    pub tunnel_mode: String,
    pub wireguard_profile: Option<WireGuardProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialAppConfig {
    pub games: Option<Vec<GameProfile>>,
    pub presets: Option<Vec<ServerPreset>>,
    pub selected_game_id: Option<Option<String>>,
    pub selected_host: Option<String>,
    pub boost_options: Option<BoostOptions>,
    pub tunnel_mode: Option<String>,
    pub wireguard_profile: Option<Option<WireGuardProfile>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackResult {
    pub ok: bool,
    pub message: String,
    pub state: BoostState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelActionResult {
    pub ok: bool,
    pub message: String,
    pub state: TunnelState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickExeResult {
    pub canceled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exe_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exe_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConfResult {
    pub canceled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<WireGuardProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
