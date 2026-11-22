import React, { useCallback, useEffect, useState } from 'react';
import { AlertTriangle, Bell, RotateCcw, X } from 'lucide-react';
import { BackgroundView } from './components/BackgroundView';
import { ConsoleBackupView } from './components/ConsoleBackupView';
import { DashboardView } from './components/DashboardView';
import { InspectorView } from './components/InspectorView';
import { MonitoringView } from './components/MonitoringView';
import { Sidebar, type NavTab } from './components/Sidebar';
import { TargetingView } from './components/TargetingView';
import { TopBar } from './components/TopBar';
import { TunnelView } from './components/TunnelView';
import { netboostApi } from './lib/tauriApi';
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
  InspectorSnapshot,
  PingSample,
  PingStats,
  RunningProcessInfo,
  ServerPreset,
  SystemInfo,
  TracerouteHop,
  TunnelMode,
  TunnelState,
} from './types/ipc';

const EMPTY_STATS: PingStats = {
  sent: 0,
  received: 0,
  lost: 0,
  lossPct: 0,
  currentMs: null,
  avgMs: null,
  minMs: null,
  maxMs: null,
  jitterMs: 0,
};

const EMPTY_COMPARISON: BeforeAfterComparison = {
  before: { ...EMPTY_STATS },
  after: { ...EMPTY_STATS },
};

const DEFAULT_OPTIONS: BoostOptions = {
  enableTunnel: false,
  enableQos: true,
  enableDns: true,
  enableTcpIp: true,
  disableEee: true,
  pauseWindowsUpdate: false,
  throttleBackgroundApps: false,
  autoRollbackOnGameExit: true,
};

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<NavTab>('dashboard');
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [config, setConfig] = useState<AppConfig>({
    games: [],
    presets: [],
    selectedGameId: null,
    selectedHost: '1.1.1.1',
    boostOptions: DEFAULT_OPTIONS,
    tunnelMode: 'wiresock',
    wireguardProfile: null,
  });
  const [boostState, setBoostState] = useState<BoostState | null>(null);
  const [tunnelState, setTunnelState] = useState<TunnelState | null>(null);
  const [backupJson, setBackupJson] = useState<BackupState | null>(null);
  const [logs, setLogs] = useState<CommandLogEntry[]>([]);

  // Live Ping & Comparison
  const [pingHistory, setPingHistory] = useState<PingSample[]>([]);
  const [pingStats, setPingStats] = useState<PingStats>(EMPTY_STATS);
  const [comparison, setComparison] = useState<BeforeAfterComparison>(EMPTY_COMPARISON);

  // DNS Benchmark
  const [dnsResults, setDnsResults] = useState<DnsBenchmarkResult[]>([]);
  const [dnsBenchmarking, setDnsBenchmarking] = useState(false);

  // Running processes & active games
  const [detectedGames, setDetectedGames] = useState<RunningProcessInfo[]>([]);
  const [allProcesses, setAllProcesses] = useState<RunningProcessInfo[]>([]);
  const [scanningProcs, setScanningProcs] = useState(false);

  // Live Packet / Connection Inspector + GeoIP Offline + ETW UDP + Firewall
  const [inspectorSnapshot, setInspectorSnapshot] = useState<InspectorSnapshot | null>(null);
  const [scanningInspector, setScanningInspector] = useState(false);
  const [capturingEtw, setCapturingEtw] = useState(false);
  const [inspectAll, setInspectAll] = useState(true);
  const [allowHttpsFallback, setAllowHttpsFallback] = useState(false);
  const [busyFirewallIp, setBusyFirewallIp] = useState<string | null>(null);

  // Background traffic
  const [backgroundSnapshot, setBackgroundSnapshot] = useState<BackgroundSnapshot | null>(null);
  const [scanningBg, setScanningBg] = useState(false);
  const [busyBgKey, setBusyBgKey] = useState<string | null>(null);

  // Traceroute
  const [tracerouteHops, setTracerouteHops] = useState<TracerouteHop[]>([]);
  const [tracerouteRunning, setTracerouteRunning] = useState(false);
  const [tracerouteHost, setTracerouteHost] = useState('');

  // Global status / toast
  const [busyRollback, setBusyRollback] = useState(false);
  const [notice, setNotice] = useState<{ text: string; tone: 'info' | 'ok' | 'warn' } | null>(null);

  const showNotice = useCallback((text: string, tone: 'info' | 'ok' | 'warn' = 'info') => {
    setNotice({ text, tone });
  }, []);

  const refreshSystemAndBackup = useCallback(async () => {
    const [info, bState, tState, bJson] = await Promise.all([
      netboostApi.getSystemInfo(),
      netboostApi.getBoostState(),
      netboostApi.getTunnelState(),
      netboostApi.getBackupJson(),
    ]);
    setSystemInfo(info);
    setBoostState(bState);
    setTunnelState(tState);
    setBackupJson(bJson);
  }, []);

  const handleScanProcesses = useCallback(async () => {
    setScanningProcs(true);
    try {
      const res = await netboostApi.scanProcesses();
      setDetectedGames(res.games);
      setAllProcesses(res.all);
    } finally {
      setScanningProcs(false);
    }
  }, []);

  const handleScanBackground = useCallback(async () => {
    setScanningBg(true);
    try {
      const snap = await netboostApi.scanBackground();
      setBackgroundSnapshot(snap);
    } finally {
      setScanningBg(false);
    }
  }, []);

  const handleScanInspector = useCallback(
    async (exeOverride?: string | null, opts?: { captureUdpEtw?: boolean; allowHttpsFallback?: boolean }) => {
      if (opts?.captureUdpEtw) setCapturingEtw(true);
      else setScanningInspector(true);
      try {
        const snap = await netboostApi.inspectConnections(exeOverride ?? null, {
          captureUdpEtw: opts?.captureUdpEtw ?? false,
          allowHttpsFallback: opts?.allowHttpsFallback ?? allowHttpsFallback,
        });
        setInspectorSnapshot(snap);
        if (opts?.captureUdpEtw && snap.etwNote) {
          showNotice(snap.etwNote, snap.etwUdpCaptured ? 'info' : 'warn');
        }
      } finally {
        setCapturingEtw(false);
        setScanningInspector(false);
      }
    },
    [allowHttpsFallback, showNotice],
  );

  const handleRunDnsBenchmark = useCallback(async () => {
    setDnsBenchmarking(true);
    try {
      const results = await netboostApi.benchmarkDns();
      setDnsResults(results);
    } finally {
      setDnsBenchmarking(false);
    }
  }, []);

  // Bootstrap real system telemetry and listeners
  useEffect(() => {
    let mounted = true;

    const unsubLog = netboostApi.onLog((entry) => {
      setLogs((prev) => {
        const idx = prev.findIndex((e) => e.id === entry.id);
        if (idx >= 0) {
          const copy = [...prev];
          copy[idx] = entry;
          return copy;
        }
        const next = [...prev, entry];
        return next.length > 400 ? next.slice(next.length - 400) : next;
      });
    });

    const unsubPing = netboostApi.onPingSample(({ sample, stats, comparison: comp }) => {
      setPingHistory((prev) => {
        const next = [...prev, sample];
        return next.length > 60 ? next.slice(next.length - 60) : next;
      });
      setPingStats(stats);
      setComparison(comp);
    });

    const unsubTrace = netboostApi.onTracerouteEvent((ev) => {
      if (ev.type === 'start') {
        setTracerouteHost(ev.host);
        setTracerouteHops([]);
        setTracerouteRunning(true);
      } else if (ev.type === 'hop' && ev.hop) {
        setTracerouteHops((prev) => [...prev, ev.hop!]);
      } else if (ev.type === 'done' || ev.type === 'error') {
        setTracerouteRunning(false);
      }
    });

    const unsubBoost = netboostApi.onBoostStateChanged((state) => {
      setBoostState(state);
    });

    const unsubTunnel = netboostApi.onTunnelStateChanged((state) => {
      setTunnelState(state);
    });

    const unsubNotice = netboostApi.onAutoRollbackNotice((msg) => {
      showNotice(msg, 'warn');
      void refreshSystemAndBackup();
    });

    (async () => {
      const [cfg, bState, initialLogs, bJson] = await Promise.all([
        netboostApi.getConfig(),
        netboostApi.getBoostState(),
        netboostApi.getLogs(),
        netboostApi.getBackupJson(),
      ]);

      if (!mounted) return;
      setConfig(cfg);
      setBoostState(bState);
      setLogs(initialLogs);
      setBackupJson(bJson);

      const initialGame = cfg.games.find((g) => g.id === cfg.selectedGameId) ?? null;
      const initialScopeAll = !initialGame;
      setInspectAll(initialScopeAll);

      // Fire all real system probes in parallel without blocking one another
      void netboostApi.startPingMonitor(cfg.selectedHost || '1.1.1.1');
      void handleRunDnsBenchmark();

      netboostApi.getSystemInfo().then((info) => {
        if (mounted) setSystemInfo(info);
      });
      netboostApi.getTunnelState().then((tState) => {
        if (mounted) setTunnelState(tState);
      });

      void handleScanProcesses();
      void handleScanBackground();
      void handleScanInspector(initialScopeAll ? null : initialGame?.exeName ?? null);
    })();

    return () => {
      mounted = false;
      unsubLog();
      unsubPing();
      unsubTrace();
      unsubBoost();
      unsubTunnel();
      unsubNotice();
    };
  }, [handleRunDnsBenchmark, handleScanBackground, handleScanInspector, handleScanProcesses, refreshSystemAndBackup, showNotice]);

  const selectedGame = config.games.find((g) => g.id === config.selectedGameId) ?? null;

  // Periodic background refresh for active tab
  useEffect(() => {
    if (activeTab === 'targeting') {
      const id = setInterval(() => void handleScanProcesses(), 8000);
      return () => clearInterval(id);
    }
    if (activeTab === 'inspector') {
      const id = setInterval(() => {
        void handleScanInspector(inspectAll || !selectedGame ? null : selectedGame.exeName);
      }, 6000);
      return () => clearInterval(id);
    }
    return undefined;
  }, [activeTab, handleScanInspector, handleScanProcesses, inspectAll, selectedGame]);

  const updateConfig = async (partial: Partial<AppConfig>) => {
    const saved = await netboostApi.saveConfig(partial);
    setConfig(saved);
    return saved;
  };

  const handleSelectGame = async (gameId: string | null) => {
    const found = config.games.find((g) => g.id === gameId);
    const nextHost = found?.serverHost || config.selectedHost;
    await updateConfig({ selectedGameId: gameId, selectedHost: nextHost });
    if (nextHost !== config.selectedHost) {
      setPingHistory([]);
      await netboostApi.startPingMonitor(nextHost);
    }
    if (found) {
      setInspectAll(false);
      void handleScanInspector(found.exeName);
    }
  };

  const handleSelectHost = async (host: string) => {
    const clean = host.trim();
    if (!clean) return;
    const res = await netboostApi.startPingMonitor(clean);
    if (!res.ok) {
      showNotice(res.error || `Adresse cible invalide : ${clean}`, 'warn');
      return;
    }
    await updateConfig({ selectedHost: clean });
    setPingHistory([]);
    setPingStats(EMPTY_STATS);
    setComparison(EMPTY_COMPARISON);
  };

  const handleUpdateOptions = async (partial: Partial<BoostOptions>) => {
    await updateConfig({
      boostOptions: { ...config.boostOptions, ...partial },
    });
  };

  const handleAddGame = async (game: GameProfile) => {
    const exists = config.games.some((g) => g.exeName.toLowerCase() === game.exeName.toLowerCase());
    const nextGames = exists
      ? config.games.map((g) => (g.exeName.toLowerCase() === game.exeName.toLowerCase() ? game : g))
      : [...config.games, game];
    await updateConfig({ games: nextGames });
  };

  const handleRemoveGame = async (gameId: string) => {
    const nextGames = config.games.filter((g) => g.id !== gameId);
    const nextSel = config.selectedGameId === gameId ? null : config.selectedGameId;
    await updateConfig({ games: nextGames, selectedGameId: nextSel });
  };

  const handleSavePresets = async (presets: ServerPreset[]) => {
    await updateConfig({ presets });
  };

  const handleToggleBoost = async () => {
    if (boostState?.busy) return;
    if (boostState?.active) {
      const next = await netboostApi.stopBoost();
      setBoostState(next);
      showNotice('Boost arrêté, tunnel coupé et configuration Windows d\'origine restaurée (Rollback).', 'ok');
      await refreshSystemAndBackup();
      void handleScanBackground();
      void handleScanInspector(inspectAll || !selectedGame ? null : selectedGame.exeName);
      return;
    }

    const next = await netboostApi.startBoost({
      game: selectedGame,
      host: config.selectedHost || '1.1.1.1',
      options: config.boostOptions,
    });
    setBoostState(next);
    await refreshSystemAndBackup();
    void handleScanBackground();

    if (next.active) {
      showNotice('Boost réseau Windows appliqué ! Le monitoring mesure maintenant les performances « Après Boost ».', 'ok');
    } else if (!systemInfo?.isAdmin) {
      showNotice('Élévation Administrateur (UAC) requise pour écrire les règles Tunnel / QoS / DNS / Registre / EEE.', 'warn');
    } else {
      showNotice('Consultez la console de logs pour le détail de l\'exécution.', 'warn');
    }
  };

  const handleRollback = async () => {
    setBusyRollback(true);
    try {
      const res = await netboostApi.rollbackAll();
      setBoostState(res.state);
      showNotice(res.message, res.ok ? 'ok' : 'warn');
      await refreshSystemAndBackup();
      void handleScanBackground();
      void handleScanInspector(inspectAll || !selectedGame ? null : selectedGame.exeName);
    } finally {
      setBusyRollback(false);
    }
  };

  const handleApplyFastestDns = async () => {
    const sorted = dnsResults.filter((d) => d.avgMs !== null).sort((a, b) => (a.avgMs ?? 9999) - (b.avgMs ?? 9999));
    if (sorted.length === 0) return;
    const servers = sorted.slice(0, 2).map((d) => d.ip);
    const res = await netboostApi.applyDnsServer(servers);
    showNotice(res.message, res.ok ? 'ok' : 'warn');
    await refreshSystemAndBackup();
  };

  const handleToggleService = async (serviceName: string, pause: boolean) => {
    setBusyBgKey(`svc:${serviceName}`);
    try {
      const res = await netboostApi.toggleServicePause(serviceName, pause);
      showNotice(res.message, res.ok ? 'ok' : 'warn');
      await Promise.all([handleScanBackground(), refreshSystemAndBackup()]);
    } finally {
      setBusyBgKey(null);
    }
  };

  const handleToggleAppThrottle = async (exeName: string, throttle: boolean) => {
    setBusyBgKey(`app:${exeName}`);
    try {
      const res = await netboostApi.toggleAppThrottle(exeName, throttle);
      showNotice(res.message, res.ok ? 'ok' : 'warn');
      await Promise.all([handleScanBackground(), refreshSystemAndBackup()]);
    } finally {
      setBusyBgKey(null);
    }
  };

  const handleBlockRemoteIp = async (params: {
    ipOrCidr: string;
    exeName?: string | null;
    exePath?: string | null;
    countryCode?: string | null;
  }) => {
    setBusyFirewallIp(params.ipOrCidr);
    try {
      const res = await netboostApi.blockRemoteIp(params);
      showNotice(res.message, res.ok ? 'ok' : 'warn');
      await Promise.all([
        handleScanInspector(inspectAll || !selectedGame ? null : selectedGame.exeName),
        refreshSystemAndBackup(),
      ]);
    } finally {
      setBusyFirewallIp(null);
    }
  };

  const handleUnblockFirewallRule = async (ruleName: string) => {
    const res = await netboostApi.unblockFirewallRule(ruleName);
    showNotice(res.message, res.ok ? 'ok' : 'warn');
    await Promise.all([
      handleScanInspector(inspectAll || !selectedGame ? null : selectedGame.exeName),
      refreshSystemAndBackup(),
    ]);
  };

  const handleImportWireGuardConf = async () => {
    const res = await netboostApi.importWireGuardConf();
    if (res.canceled) return;
    if (res.error) {
      showNotice(res.error, 'warn');
      return;
    }
    if (res.profile) {
      setConfig((prev) => ({ ...prev, wireguardProfile: res.profile! }));
      showNotice(`Profil WireGuard importé : ${res.profile.fileName} (${res.profile.endpoint})`, 'ok');
    }
  };

  const handleClearWireGuardConf = async () => {
    await netboostApi.clearWireGuardConf();
    setConfig((prev) => ({ ...prev, wireguardProfile: null }));
    showNotice('Profil WireGuard supprimé.', 'info');
    await refreshSystemAndBackup();
  };

  const handleSelectTunnelMode = async (mode: TunnelMode) => {
    await updateConfig({ tunnelMode: mode });
  };

  const handleStartTunnel = async () => {
    const activeGameIps =
      selectedGame && inspectorSnapshot
        ? inspectorSnapshot.connections
            .filter((c) => !c.geo.isPrivate && c.exeName.toLowerCase() === selectedGame.exeName.toLowerCase())
            .map((c) => c.remoteAddress)
        : [];
    const res = await netboostApi.startTunnel({
      mode: config.tunnelMode,
      exeName: selectedGame?.exeName ?? null,
      targetHost: config.selectedHost,
      extraIps: activeGameIps,
    });
    setTunnelState(res.state);
    showNotice(res.message, res.ok ? 'ok' : 'warn');
    await refreshSystemAndBackup();
  };

  const handleStopTunnel = async () => {
    const res = await netboostApi.stopTunnel();
    setTunnelState(res.state);
    showNotice(res.message, res.ok ? 'ok' : 'warn');
    await refreshSystemAndBackup();
  };

  const handleResetComparison = async () => {
    await netboostApi.resetComparison();
    setPingHistory([]);
    setPingStats(EMPTY_STATS);
    setComparison(EMPTY_COMPARISON);
  };

  const handleRelaunchAdmin = async () => {
    const res = await netboostApi.relaunchAsAdmin();
    if (!res.ok && res.error) {
      showNotice(res.error, 'warn');
    }
  };

  return (
    <div className="h-screen w-screen flex tactical-bg text-text-primary overflow-hidden">
      <Sidebar
        activeTab={activeTab}
        onSelectTab={setActiveTab}
        systemInfo={systemInfo}
        boostState={boostState}
        tunnelState={tunnelState}
        nonEuCount={inspectorSnapshot?.nonEuCount ?? 0}
      />

      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        <TopBar
          systemInfo={systemInfo}
          boostState={boostState}
          tunnelState={tunnelState}
          selectedGame={selectedGame}
          selectedHost={config.selectedHost}
          onRelaunchAdmin={handleRelaunchAdmin}
          onRollback={handleRollback}
          onMinimizeToTray={() => void netboostApi.minimizeToTray()}
          busyRollback={busyRollback}
        />

        {systemInfo?.hasOrphanBackup && !boostState?.active && (
          <div className="px-6 py-2.5 text-xs font-medium flex items-center justify-between gap-3 border-b bg-neon-red/12 border-neon-red/40 text-neon-red">
            <div className="flex items-center gap-2 min-w-0">
              <AlertTriangle className="w-4 h-4 shrink-0" />
              <span className="truncate">
                Sauvegarde réseau orpheline détectée ({systemInfo.backupTimestamp ? new Date(systemInfo.backupTimestamp).toLocaleString('fr-FR') : 'session précédente'}) suite à une fermeture anormale. Restaurez votre configuration d&apos;origine.
              </span>
            </div>
            <button
              onClick={handleRollback}
              disabled={busyRollback}
              className="shrink-0 inline-flex items-center gap-1.5 px-3 py-1 rounded-lg bg-neon-red/20 hover:bg-neon-red/30 border border-neon-red/60 text-neon-red font-bold cursor-pointer disabled:opacity-50"
            >
              <RotateCcw className="w-3.5 h-3.5" />
              Restaurer maintenant
            </button>
          </div>
        )}

        {notice && (
          <div
            className={`px-6 py-2.5 text-xs font-medium flex items-center justify-between gap-3 border-b ${
              notice.tone === 'ok'
                ? 'bg-neon-green/10 border-neon-green/30 text-neon-green'
                : notice.tone === 'warn'
                  ? 'bg-neon-amber/10 border-neon-amber/30 text-neon-amber'
                  : 'bg-neon-cyan/10 border-neon-cyan/30 text-neon-cyan'
            }`}
          >
            <div className="flex items-center gap-2 min-w-0">
              <Bell className="w-3.5 h-3.5 shrink-0" />
              <span className="truncate">{notice.text}</span>
            </div>
            <button onClick={() => setNotice(null)} className="opacity-70 hover:opacity-100 cursor-pointer">
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        <main className="flex-1 overflow-y-auto p-5">
          {activeTab === 'dashboard' && (
            <DashboardView
              games={config.games}
              presets={config.presets}
              selectedGameId={config.selectedGameId}
              selectedHost={config.selectedHost}
              boostOptions={config.boostOptions}
              boostState={boostState}
              pingHistory={pingHistory}
              pingStats={pingStats}
              comparison={comparison}
              dnsResults={dnsResults}
              dnsBenchmarking={dnsBenchmarking}
              logs={logs}
              onSelectGame={handleSelectGame}
              onSelectHost={handleSelectHost}
              onUpdateOptions={handleUpdateOptions}
              onToggleBoost={handleToggleBoost}
              onRunDnsBenchmark={handleRunDnsBenchmark}
              onApplyFastestDns={handleApplyFastestDns}
              onResetComparison={handleResetComparison}
              onNavigate={setActiveTab}
            />
          )}

          {activeTab === 'targeting' && (
            <TargetingView
              games={config.games}
              presets={config.presets}
              selectedGameId={config.selectedGameId}
              selectedHost={config.selectedHost}
              detectedGames={detectedGames}
              allProcesses={allProcesses}
              scanningProcs={scanningProcs}
              onScanProcesses={handleScanProcesses}
              onSelectGame={handleSelectGame}
              onSelectHost={handleSelectHost}
              onAddGame={handleAddGame}
              onRemoveGame={handleRemoveGame}
              onSavePresets={handleSavePresets}
            />
          )}

          {activeTab === 'inspector' && (
            <InspectorView
              selectedGame={selectedGame}
              snapshot={inspectorSnapshot}
              scanning={scanningInspector}
              capturingEtw={capturingEtw}
              inspectAll={inspectAll}
              allowHttpsFallback={allowHttpsFallback}
              busyIp={busyFirewallIp}
              isAdmin={systemInfo?.isAdmin ?? false}
              onToggleScope={(all) => {
                setInspectAll(all);
                void handleScanInspector(all || !selectedGame ? null : selectedGame.exeName);
              }}
              onToggleHttpsFallback={(allow) => {
                setAllowHttpsFallback(allow);
                void handleScanInspector(inspectAll || !selectedGame ? null : selectedGame.exeName, {
                  allowHttpsFallback: allow,
                });
              }}
              onScan={() => void handleScanInspector(inspectAll || !selectedGame ? null : selectedGame.exeName)}
              onCaptureUdpEtw={() => {
                if (!selectedGame) return;
                setInspectAll(false);
                void handleScanInspector(selectedGame.exeName, { captureUdpEtw: true });
              }}
              onBlockIp={handleBlockRemoteIp}
              onUnblockRule={handleUnblockFirewallRule}
            />
          )}

          {activeTab === 'tunnel' && (
            <TunnelView
              profile={config.wireguardProfile}
              tunnelMode={config.tunnelMode}
              tunnelState={tunnelState}
              enableTunnelOnBoost={config.boostOptions.enableTunnel}
              selectedGame={selectedGame}
              selectedHost={config.selectedHost}
              isAdmin={systemInfo?.isAdmin ?? false}
              onImportConf={handleImportWireGuardConf}
              onClearConf={handleClearWireGuardConf}
              onSelectMode={handleSelectTunnelMode}
              onToggleEnableOnBoost={(enabled) => void handleUpdateOptions({ enableTunnel: enabled })}
              onStartTunnel={handleStartTunnel}
              onStopTunnel={handleStopTunnel}
            />
          )}

          {activeTab === 'background' && (
            <BackgroundView
              snapshot={backgroundSnapshot}
              scanning={scanningBg}
              busyKey={busyBgKey}
              isAdmin={systemInfo?.isAdmin ?? false}
              onScan={handleScanBackground}
              onToggleService={handleToggleService}
              onToggleAppThrottle={handleToggleAppThrottle}
            />
          )}

          {activeTab === 'monitoring' && (
            <MonitoringView
              selectedHost={config.selectedHost}
              pingHistory={pingHistory}
              pingStats={pingStats}
              comparison={comparison}
              tracerouteHops={tracerouteHops}
              tracerouteRunning={tracerouteRunning}
              tracerouteHost={tracerouteHost}
              onChangeHost={handleSelectHost}
              onResetComparison={handleResetComparison}
              onRunTraceroute={async (h) => {
                const res = await netboostApi.runTraceroute(h);
                if (!res.ok) showNotice(res.error || 'Adresse Traceroute invalide.', 'warn');
              }}
              onCancelTraceroute={() => void netboostApi.cancelTraceroute()}
            />
          )}

          {activeTab === 'console' && (
            <ConsoleBackupView
              logs={logs}
              backupJson={backupJson}
              backupPath={systemInfo?.backupPath || boostState?.backupPath || ''}
              busyRollback={busyRollback}
              onClearLogs={async () => {
                await netboostApi.clearLogs();
                setLogs([]);
              }}
              onRefreshBackup={refreshSystemAndBackup}
              onRollback={handleRollback}
            />
          )}
        </main>
      </div>
    </div>
  );
};

export default App;
