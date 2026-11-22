import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AppConfig,
  BackgroundSnapshot,
  BackupState,
  BeforeAfterComparison,
  BoostOptions,
  BoostState,
  CommandLogEntry,
  DnsBenchmarkResult,
  GameProfile,
  InspectOptions,
  InspectorSnapshot,
  NetBoostAPI,
  PingSample,
  PingStats,
  RunningProcessInfo,
  SystemInfo,
  TracerouteEvent,
  TunnelMode,
  TunnelState,
  WireGuardProfile,
} from '../types/ipc';

function subscribeEvent<T>(eventName: string, cb: (payload: T) => void): () => void {
  let active = true;
  let unlistenFn: UnlistenFn | null = null;

  void listen<T>(eventName, (ev) => {
    if (active) cb(ev.payload);
  }).then((fn) => {
    if (!active) {
      fn();
    } else {
      unlistenFn = fn;
    }
  });

  return () => {
    active = false;
    if (unlistenFn) {
      unlistenFn();
      unlistenFn = null;
    }
  };
}

export const netboostApi: NetBoostAPI = {
  getSystemInfo: () => invoke<SystemInfo>('get_system_info'),
  relaunchAsAdmin: () => invoke<{ ok: boolean; error?: string }>('relaunch_admin'),
  minimizeToTray: () => invoke<{ ok: boolean }>('minimize_to_tray'),
  getConfig: () => invoke<AppConfig>('get_config'),
  saveConfig: (partial: Partial<AppConfig>) => invoke<AppConfig>('save_config', { partial }),
  pickExecutable: () =>
    invoke<{ canceled: boolean; exePath?: string; exeName?: string; suggestedName?: string }>('pick_executable'),
  scanProcesses: () => invoke<{ games: RunningProcessInfo[]; all: RunningProcessInfo[] }>('scan_processes'),
  benchmarkDns: () => invoke<DnsBenchmarkResult[]>('benchmark_dns'),
  applyDnsServer: (servers: string[]) => invoke<{ ok: boolean; message: string }>('apply_dns_server', { servers }),
  getBoostState: () => invoke<BoostState>('get_boost_state'),
  startBoost: (params: { game: GameProfile | null; host: string; options: BoostOptions }) =>
    invoke<BoostState>('start_boost', {
      game: params.game,
      host: params.host,
      options: params.options,
    }),
  stopBoost: () => invoke<BoostState>('stop_boost'),
  rollbackAll: () => invoke<{ ok: boolean; message: string; state: BoostState }>('rollback_all'),
  getBackupJson: () => invoke<BackupState | null>('get_backup_json'),
  scanBackground: () => invoke<BackgroundSnapshot>('scan_background'),
  toggleAppThrottle: (exeName: string, throttle: boolean) =>
    invoke<{ ok: boolean; message: string }>('toggle_app_throttle', { exeName, throttle }),
  toggleServicePause: (serviceName: string, pause: boolean) =>
    invoke<{ ok: boolean; message: string }>('toggle_service_pause', { serviceName, pause }),

  // Packet / Connection Inspector + Offline GeoIP + ETW UDP + COM Firewall
  inspectConnections: (exeName: string | null, options?: InspectOptions) =>
    invoke<InspectorSnapshot>('inspect_connections', { exeName, options: options ?? {} }),
  blockRemoteIp: (params: {
    ipOrCidr: string;
    exeName?: string | null;
    exePath?: string | null;
    countryCode?: string | null;
  }) =>
    invoke<{ ok: boolean; message: string }>('block_remote_ip', {
      ipOrCidr: params.ipOrCidr,
      exeName: params.exeName ?? null,
      exePath: params.exePath ?? null,
      countryCode: params.countryCode ?? null,
    }),
  unblockFirewallRule: (ruleName: string) =>
    invoke<{ ok: boolean; message: string }>('unblock_firewall_rule', { ruleName }),

  // WireGuard / WireSock EU Split Tunnel
  importWireGuardConf: () =>
    invoke<{ canceled: boolean; profile?: WireGuardProfile; error?: string }>('import_wireguard_conf'),
  clearWireGuardConf: () => invoke<{ ok: boolean }>('clear_wireguard_conf'),
  getTunnelState: () => invoke<TunnelState>('get_tunnel_state'),
  startTunnel: (params: {
    mode: TunnelMode;
    exeName: string | null;
    targetHost: string;
    extraIps?: string[];
  }) =>
    invoke<{ ok: boolean; message: string; state: TunnelState }>('start_tunnel', {
      mode: params.mode,
      exeName: params.exeName,
      targetHost: params.targetHost,
      extraIps: params.extraIps ?? [],
    }),
  stopTunnel: () => invoke<{ ok: boolean; message: string; state: TunnelState }>('stop_tunnel'),

  // Native Win32 IcmpSendEcho Monitoring & Traceroute
  startPingMonitor: (host: string) => invoke<{ ok: boolean; error?: string }>('start_ping_monitor', { host }),
  stopPingMonitor: () => invoke<{ ok: boolean }>('stop_ping_monitor'),
  resetComparison: () => invoke<{ ok: boolean }>('reset_comparison'),
  runTraceroute: (host: string) => invoke<{ ok: boolean; error?: string }>('run_traceroute', { host }),
  cancelTraceroute: () => invoke<{ ok: boolean }>('cancel_traceroute'),
  getLogs: () => invoke<CommandLogEntry[]>('get_logs'),
  clearLogs: () => invoke<void>('clear_logs'),

  // Tauri v2 Events
  onLog: (cb) => subscribeEvent<CommandLogEntry>('event:log', cb),
  onPingSample: (cb) =>
    subscribeEvent<{ sample: PingSample; stats: PingStats; comparison: BeforeAfterComparison }>('event:ping', cb),
  onTracerouteEvent: (cb) => subscribeEvent<TracerouteEvent>('event:traceroute', cb),
  onBoostStateChanged: (cb) => subscribeEvent<BoostState>('event:boostState', cb),
  onTunnelStateChanged: (cb) => subscribeEvent<TunnelState>('event:tunnelState', cb),
  onAutoRollbackNotice: (cb) => subscribeEvent<string>('event:autoRollbackNotice', cb),
};
