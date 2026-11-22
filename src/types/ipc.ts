export type LogCategory =
  | 'system'
  | 'qos'
  | 'dns'
  | 'tcpip'
  | 'adapter'
  | 'background'
  | 'monitor'
  | 'rollback'
  | 'uac'
  | 'inspector'
  | 'firewall'
  | 'tunnel';

export interface CommandLogEntry {
  id: string;
  timestamp: string;
  category: LogCategory;
  command: string;
  exitCode: number | null;
  stdout: string;
  stderr: string;
  durationMs: number;
  status: 'running' | 'ok' | 'error' | 'warn';
}

export interface AdapterAdvancedProp {
  displayName: string;
  registryKeyword: string;
  displayValue: string;
  registryValue: string;
}

export interface NetworkAdapterInfo {
  name: string;
  description: string;
  interfaceIndex: number;
  interfaceGuid: string;
  linkSpeed: string;
  status: string;
  ipv4: string;
  currentDns: string[];
  dnsIsDhcp: boolean;
  eeeProperties: AdapterAdvancedProp[];
}

export interface SystemInfo {
  isAdmin: boolean;
  hostname: string;
  osCaption: string;
  activeAdapter: NetworkAdapterInfo | null;
  hasBackup: boolean;
  hasOrphanBackup: boolean;
  backupTimestamp: string | null;
  backupPath: string;
}

export interface GameProfile {
  id: string;
  name: string;
  exeName: string;
  exePath: string;
  serverHost: string;
  addedManually: boolean;
  isRunning?: boolean;
  pid?: number;
}

export interface RunningProcessInfo {
  pid: number;
  name: string;
  exeName: string;
  exePath: string;
  memoryMb: number;
  windowTitle: string;
  isLikelyGame: boolean;
}

export interface ServerPreset {
  id: string;
  name: string;
  region: 'Cloudflare' | 'UE' | 'NA' | 'DNS' | 'Custom';
  host: string;
  description: string;
}

export interface DnsBenchmarkResult {
  ip: string;
  name: string;
  avgMs: number | null;
  minMs: number | null;
  maxMs: number | null;
  successCount: number;
  totalCount: number;
  fastest?: boolean;
}

export type TunnelMode = 'wiresock' | 'wireguard';

export interface WireGuardProfile {
  fileName: string;
  /** Encrypted via Electron safeStorage on disk; omitted from renderer IPC snapshots */
  rawConf?: string;
  encryptedConf?: string;
  isEncrypted?: boolean;
  endpoint: string;
  interfaceAddress: string;
  dns: string;
  allowedIpsOriginal: string;
  publicKeyPreview: string;
  importedAt: string;
}

export interface TunnelClientsStatus {
  wiresockPath: string | null;
  wireguardPath: string | null;
  wiresockSigned: boolean;
  wireguardSigned: boolean;
}

export interface TunnelState {
  active: boolean;
  busy: boolean;
  mode: TunnelMode;
  clients: TunnelClientsStatus;
  activeExe: string | null;
  activeAllowedIps: string[];
  endpoint: string | null;
  startedAt: string | null;
  lastError: string | null;
  generatedPreview: string | null;
}

export interface GeoIpInfo {
  ip: string;
  country: string;
  countryCode: string;
  city: string;
  isp: string;
  as: string;
  isEu: boolean;
  isPrivate: boolean;
  source?: 'offline' | 'https' | 'local';
}

export interface InspectOptions {
  captureUdpEtw?: boolean;
  allowHttpsFallback?: boolean;
}

export interface LiveConnectionEntry {
  id: string;
  protocol: 'TCP' | 'UDP';
  pid: number;
  exeName: string;
  exePath: string;
  localAddress: string;
  localPort: number;
  remoteAddress: string;
  remotePort: number;
  state: string;
  geo: GeoIpInfo;
  isBlocked: boolean;
}

export interface FirewallBlockRule {
  ruleName: string;
  displayName: string;
  remoteAddress: string;
  exeName: string | null;
  countryCode: string | null;
  createdAt: string;
}

export interface InspectorSnapshot {
  targetExe: string | null;
  connections: LiveConnectionEntry[];
  blockedRules: FirewallBlockRule[];
  nonEuCount: number;
  etwUdpCaptured?: boolean;
  etwUdpCount?: number;
  etwNote?: string | null;
  scannedAt: string;
}

export interface BoostOptions {
  enableTunnel: boolean;
  enableQos: boolean;
  enableDns: boolean;
  enableTcpIp: boolean;
  disableEee: boolean;
  pauseWindowsUpdate: boolean;
  throttleBackgroundApps: boolean;
  autoRollbackOnGameExit: boolean;
}

export interface BoostStepStatus {
  key: 'backup' | 'tunnel' | 'qos' | 'dns' | 'tcpip' | 'eee' | 'background';
  label: string;
  status: 'idle' | 'running' | 'applied' | 'skipped' | 'error';
  detail: string;
}

export interface BoostState {
  active: boolean;
  busy: boolean;
  startedAt: string | null;
  targetGame: GameProfile | null;
  targetHost: string;
  trackedPid: number | null;
  appliedDns: string[] | null;
  steps: BoostStepStatus[];
  hasBackup: boolean;
  backupPath: string;
}

export interface BackupRegistryValue {
  existed: boolean;
  value: number | null;
}

export interface BackupRegistryStringValue {
  existed: boolean;
  value: string | null;
}

export interface BackupState {
  version: number;
  createdAt: string;
  adapter: {
    name: string;
    interfaceIndex: number;
    interfaceGuid: string;
    dnsIsDhcp: boolean;
    originalDns: string[];
    eeeProperties: AdapterAdvancedProp[];
  } | null;
  secondaryDnsAdapters?: Array<{
    name: string;
    interfaceIndex: number;
    dnsIsDhcp: boolean;
    originalDns: string[];
  }>;
  registry: {
    interfaceKeyPath: string | null;
    tcpAckFrequency: BackupRegistryValue;
    tcpNoDelay: BackupRegistryValue;
    networkThrottlingIndex: BackupRegistryValue;
    qosDoNotUseNla?: BackupRegistryStringValue;
  };
  qosPoliciesCreated: string[];
  throttledExes?: string[];
  firewallRulesCreated?: FirewallBlockRule[];
  tunnelWasStarted?: boolean;
  servicesPaused: Array<{
    name: string;
    originalStatus: string;
    originalStartType: string;
  }>;
}

export interface PingSample {
  seq: number;
  timestamp: number;
  timeLabel: string;
  host: string;
  rttMs: number | null;
  jitterMs: number;
  lossPct: number;
  lost: boolean;
  phase: 'before' | 'after';
}

export interface PingStats {
  sent: number;
  received: number;
  lost: number;
  lossPct: number;
  currentMs: number | null;
  avgMs: number | null;
  minMs: number | null;
  maxMs: number | null;
  jitterMs: number;
}

export interface BeforeAfterComparison {
  before: PingStats;
  after: PingStats;
}

export interface TracerouteHop {
  hop: number;
  rtt1: number | null;
  rtt2: number | null;
  rtt3: number | null;
  avgMs: number | null;
  ip: string | null;
  timedOut: boolean;
}

export interface TracerouteEvent {
  type: 'start' | 'hop' | 'done' | 'error';
  host: string;
  hop?: TracerouteHop;
  message?: string;
}

export interface WindowsServiceInfo {
  name: string;
  displayName: string;
  status: string;
  startType: string;
  pausedByApp: boolean;
}

export interface BackgroundAppInfo {
  exeName: string;
  displayName: string;
  category: 'browser' | 'launcher' | 'updater' | 'other';
  pids: number[];
  instanceCount: number;
  totalMemoryMb: number;
  tcpConnections: number;
  throttledByApp: boolean;
}

export interface BackgroundSnapshot {
  services: WindowsServiceInfo[];
  apps: BackgroundAppInfo[];
  scannedAt: string;
}

export interface AppConfig {
  games: GameProfile[];
  presets: ServerPreset[];
  selectedGameId: string | null;
  selectedHost: string;
  boostOptions: BoostOptions;
  tunnelMode: TunnelMode;
  wireguardProfile: WireGuardProfile | null;
}

export interface NetBoostAPI {
  getSystemInfo: () => Promise<SystemInfo>;
  relaunchAsAdmin: () => Promise<{ ok: boolean; error?: string }>;
  minimizeToTray: () => Promise<{ ok: boolean }>;
  getConfig: () => Promise<AppConfig>;
  saveConfig: (partial: Partial<AppConfig>) => Promise<AppConfig>;
  pickExecutable: () => Promise<{ canceled: boolean; exePath?: string; exeName?: string; suggestedName?: string }>;
  scanProcesses: () => Promise<{ games: RunningProcessInfo[]; all: RunningProcessInfo[] }>;
  benchmarkDns: () => Promise<DnsBenchmarkResult[]>;
  applyDnsServer: (servers: string[]) => Promise<{ ok: boolean; message: string }>;
  getBoostState: () => Promise<BoostState>;
  startBoost: (params: { game: GameProfile | null; host: string; options: BoostOptions }) => Promise<BoostState>;
  stopBoost: () => Promise<BoostState>;
  rollbackAll: () => Promise<{ ok: boolean; message: string; state: BoostState }>;
  getBackupJson: () => Promise<BackupState | null>;
  scanBackground: () => Promise<BackgroundSnapshot>;
  toggleAppThrottle: (exeName: string, throttle: boolean) => Promise<{ ok: boolean; message: string }>;
  toggleServicePause: (serviceName: string, pause: boolean) => Promise<{ ok: boolean; message: string }>;
  // Packet / Connection Inspector + GeoIP Offline + ETW UDP + Windows Firewall
  inspectConnections: (exeName: string | null, options?: InspectOptions) => Promise<InspectorSnapshot>;
  blockRemoteIp: (params: {
    ipOrCidr: string;
    exeName?: string | null;
    exePath?: string | null;
    countryCode?: string | null;
  }) => Promise<{ ok: boolean; message: string }>;
  unblockFirewallRule: (ruleName: string) => Promise<{ ok: boolean; message: string }>;
  // WireGuard / WireSock EU Split Tunnel
  importWireGuardConf: () => Promise<{ canceled: boolean; profile?: WireGuardProfile; error?: string }>;
  clearWireGuardConf: () => Promise<{ ok: boolean }>;
  getTunnelState: () => Promise<TunnelState>;
  startTunnel: (params: {
    mode: TunnelMode;
    exeName: string | null;
    targetHost: string;
    extraIps?: string[];
  }) => Promise<{ ok: boolean; message: string; state: TunnelState }>;
  stopTunnel: () => Promise<{ ok: boolean; message: string; state: TunnelState }>;
  // Real-Time Monitoring & Traceroute
  startPingMonitor: (host: string) => Promise<{ ok: boolean; error?: string }>;
  stopPingMonitor: () => Promise<{ ok: boolean }>;
  resetComparison: () => Promise<{ ok: boolean }>;
  runTraceroute: (host: string) => Promise<{ ok: boolean; error?: string }>;
  cancelTraceroute: () => Promise<{ ok: boolean }>;
  getLogs: () => Promise<CommandLogEntry[]>;
  clearLogs: () => Promise<void>;
  onLog: (cb: (entry: CommandLogEntry) => void) => () => void;
  onPingSample: (cb: (data: { sample: PingSample; stats: PingStats; comparison: BeforeAfterComparison }) => void) => () => void;
  onTracerouteEvent: (cb: (ev: TracerouteEvent) => void) => () => void;
  onBoostStateChanged: (cb: (state: BoostState) => void) => () => void;
  onTunnelStateChanged: (cb: (state: TunnelState) => void) => () => void;
  onAutoRollbackNotice: (cb: (msg: string) => void) => () => void;
}

declare global {
  interface Window {
    netboostApi: NetBoostAPI;
  }
}
