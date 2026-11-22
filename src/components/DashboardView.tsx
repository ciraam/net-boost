import React from 'react';
import {
  Activity,
  ArrowDownRight,
  ArrowUpRight,
  CheckCircle2,
  ChevronRight,
  Clock,
  Globe,
  Loader2,
  MinusCircle,
  Play,
  RefreshCw,
  Terminal,
  XCircle,
} from 'lucide-react';
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import type {
  BeforeAfterComparison,
  BoostOptions,
  BoostState,
  CommandLogEntry,
  DnsBenchmarkResult,
  GameProfile,
  PingSample,
  PingStats,
  ServerPreset,
} from '../types/ipc';
import { BoostButton } from './BoostButton';
import type { NavTab } from './Sidebar';

interface DashboardViewProps {
  games: GameProfile[];
  presets: ServerPreset[];
  selectedGameId: string | null;
  selectedHost: string;
  boostOptions: BoostOptions;
  boostState: BoostState | null;
  pingHistory: PingSample[];
  pingStats: PingStats;
  comparison: BeforeAfterComparison;
  dnsResults: DnsBenchmarkResult[];
  dnsBenchmarking: boolean;
  logs: CommandLogEntry[];
  onSelectGame: (gameId: string | null) => void;
  onSelectHost: (host: string) => void;
  onUpdateOptions: (partial: Partial<BoostOptions>) => void;
  onToggleBoost: () => void;
  onRunDnsBenchmark: () => void;
  onApplyFastestDns: () => void;
  onResetComparison: () => void;
  onNavigate: (tab: NavTab) => void;
}

const OPTION_TOGGLES: Array<{ key: keyof BoostOptions; label: string }> = [
  { key: 'enableTunnel', label: 'Tunnel EU Split' },
  { key: 'enableQos', label: 'QoS DSCP 46' },
  { key: 'enableDns', label: 'DNS Auto + Flush' },
  { key: 'enableTcpIp', label: 'TCPNoDelay / Ack' },
  { key: 'disableEee', label: 'EEE Off' },
  { key: 'autoRollbackOnGameExit', label: 'Auto-Rollback' },
];

const STEP_TAGS: Record<string, string> = {
  backup: 'JSON',
  tunnel: 'SPLIT',
  qos: 'DSCP 46',
  dns: 'DNS',
  tcpip: 'TCP/IP',
  eee: 'EEE=0',
  background: 'DSCP 8',
};

export const DashboardView: React.FC<DashboardViewProps> = ({
  games,
  presets,
  selectedGameId,
  selectedHost,
  boostOptions,
  boostState,
  pingHistory,
  pingStats,
  comparison,
  dnsResults,
  dnsBenchmarking,
  logs,
  onSelectGame,
  onSelectHost,
  onUpdateOptions,
  onToggleBoost,
  onRunDnsBenchmark,
  onApplyFastestDns,
  onResetComparison,
  onNavigate,
}) => {
  const selectedGame = games.find((g) => g.id === selectedGameId) ?? null;
  const fastestDns = dnsResults.find((d) => d.fastest) ?? null;
  const maxDnsMs = Math.max(...dnsResults.map((d) => d.avgMs ?? 0), 1);

  const comparisonChartData = [
    {
      metric: 'Ping Moy (ms)',
      Avant: comparison.before.avgMs ?? 0,
      Apres: comparison.after.avgMs ?? 0,
    },
    {
      metric: 'Jitter (ms)',
      Avant: comparison.before.jitterMs ?? 0,
      Apres: comparison.after.jitterMs ?? 0,
    },
    {
      metric: 'Perte (%)',
      Avant: comparison.before.lossPct ?? 0,
      Apres: comparison.after.lossPct ?? 0,
    },
  ];

  const pingDelta =
    comparison.before.avgMs !== null && comparison.after.avgMs !== null
      ? Math.round((comparison.after.avgMs - comparison.before.avgMs) * 10) / 10
      : null;

  const pingDeltaPct =
    pingDelta !== null && comparison.before.avgMs && comparison.before.avgMs > 0
      ? Math.round((pingDelta / comparison.before.avgMs) * 100)
      : null;

  const jitterDelta =
    comparison.before.sent > 0 && comparison.after.sent > 0
      ? Math.round((comparison.after.jitterMs - comparison.before.jitterMs) * 10) / 10
      : null;

  const recentLogs = [...logs].slice(-6).reverse();

  return (
    <div className="space-y-4">
      {/* Row 1: Central Boost Reactor + Animated Execution Pipeline & DNS Benchmark */}
      <div className="grid grid-cols-12 gap-4">
        {/* Hero Boost Panel */}
        <div className="col-span-12 lg:col-span-5 tactical-card rounded-2xl p-4 flex flex-col justify-between relative overflow-hidden">
          <div className="absolute -top-24 -right-24 w-60 h-60 rounded-full bg-neon-cyan/8 blur-3xl pointer-events-none" />
          <div className="absolute -bottom-24 -left-24 w-48 h-48 rounded-full bg-neon-green/6 blur-3xl pointer-events-none" />

          {/* Quick Target Pickers */}
          <div className="grid grid-cols-2 gap-2.5 z-10">
            <div>
              <label className="block text-[9px] font-mono font-bold uppercase tracking-[0.16em] text-text-muted mb-1">
                CIBLE EXÉCUTABLE (.EXE)
              </label>
              <select
                value={selectedGameId ?? ''}
                onChange={(e) => onSelectGame(e.target.value || null)}
                className="w-full bg-bg-void/90 border border-border-strong rounded-lg px-2.5 py-1.5 text-xs font-mono text-text-primary focus:outline-none focus:border-neon-cyan transition-colors"
              >
                <option value="">Mode Global (sans .exe)</option>
                {games.map((g) => (
                  <option key={g.id} value={g.id}>
                    {g.name} ({g.exeName})
                  </option>
                ))}
              </select>
            </div>

            <div>
              <label className="block text-[9px] font-mono font-bold uppercase tracking-[0.16em] text-text-muted mb-1">
                SERVEUR CIBLE
              </label>
              <select
                value={presets.some((p) => p.host === selectedHost) ? selectedHost : '__custom__'}
                onChange={(e) => {
                  if (e.target.value !== '__custom__') onSelectHost(e.target.value);
                }}
                className="w-full bg-bg-void/90 border border-border-strong rounded-lg px-2.5 py-1.5 text-xs font-mono text-text-primary focus:outline-none focus:border-neon-cyan transition-colors"
              >
                {presets.map((p) => (
                  <option key={p.id} value={p.host}>
                    [{p.region}] {p.name} ({p.host})
                  </option>
                ))}
                {!presets.some((p) => p.host === selectedHost) && (
                  <option value="__custom__">Custom ({selectedHost})</option>
                )}
              </select>
            </div>
          </div>

          {/* Central Tactical Reactor */}
          <div className="my-1 z-10">
            <BoostButton
              active={boostState?.active ?? false}
              busy={boostState?.busy ?? false}
              onToggle={onToggleBoost}
              targetLabel={
                selectedGame
                  ? `${selectedGame.exeName} → ${selectedHost}`
                  : `GLOBAL TELEMETRY → ${selectedHost}`
              }
            />
          </div>

          {/* Boost Module Toggles */}
          <div className="space-y-2 z-10">
            <div className="grid grid-cols-3 gap-1.5">
              {OPTION_TOGGLES.map((opt) => {
                const checked = boostOptions[opt.key];
                return (
                  <button
                    key={opt.key}
                    onClick={() => onUpdateOptions({ [opt.key]: !checked })}
                    disabled={boostState?.active || boostState?.busy}
                    className={`px-2 py-1.5 rounded-lg text-[10px] font-mono font-bold border transition-all duration-200 cursor-pointer disabled:opacity-60 truncate ${
                      checked
                        ? 'bg-neon-cyan/14 border-neon-cyan/50 text-neon-cyan shadow-[inset_0_0_12px_rgba(0,229,255,0.1)]'
                        : 'bg-bg-void/70 border-border-subtle text-text-muted hover:text-text-secondary hover:border-border-strong'
                    }`}
                  >
                    {opt.label}
                  </button>
                );
              })}
            </div>
            <div className="text-[10px] text-text-muted leading-snug">
              <span className="text-text-secondary font-semibold">Transparence :</span> TCPNoDelay/DNS optimisent TCP &amp; résolution (sans effet direct sur le ping UDP en match). Le routage en match agit via <span className="text-neon-cyan">Tunnel EU</span>, <span className="text-neon-green">QoS DSCP 46 (NLA=1)</span> et EEE Off.
            </div>
          </div>
        </div>

        {/* Right Column: Animated 7-Step Execution Pipeline + Parallel DNS Benchmark */}
        <div className="col-span-12 lg:col-span-7 flex flex-col gap-4">
          {/* Animated Tactical Pipeline */}
          <div className="tactical-card rounded-2xl p-4">
            <div className="flex items-center justify-between mb-2.5">
              <div className="flex items-center gap-2">
                <span className={`w-2 h-2 rounded-full ${boostState?.busy ? 'bg-neon-cyan animate-status-beacon' : boostState?.active ? 'bg-neon-green' : 'bg-text-muted'}`} />
                <h2 className="text-[11px] font-mono font-bold uppercase tracking-[0.16em] text-text-secondary">
                  PIPELINE SYSTÈME WINDOWS (7 ÉTAPES)
                </h2>
              </div>
              <span className="text-[10px] metric-mono text-text-muted">
                {boostState?.startedAt
                  ? `SESSION ${new Date(boostState.startedAt).toLocaleTimeString('fr-FR')}`
                  : 'BACKUP JSON OBLIGATOIRE'}
              </span>
            </div>

            <div className="grid grid-cols-2 gap-1.5">
              {(boostState?.steps ?? []).map((step, idx) => {
                const isRunning = step.status === 'running';
                const isApplied = step.status === 'applied';
                const isError = step.status === 'error';

                return (
                  <div
                    key={step.key}
                    className={`relative overflow-hidden flex items-start gap-2 p-2 rounded-xl border transition-all duration-300 ${
                      isRunning
                        ? 'bg-neon-cyan/10 border-neon-cyan/60 shadow-[0_0_16px_rgba(0,229,255,0.16)]'
                        : isApplied
                          ? 'bg-neon-green/[0.07] border-neon-green/40'
                          : isError
                            ? 'bg-neon-red/[0.08] border-neon-red/45'
                            : 'bg-bg-void/65 border-border-subtle'
                    }`}
                  >
                    {/* Animated shimmer sweep on currently executing step */}
                    {isRunning && (
                      <div className="pointer-events-none absolute inset-y-0 left-0 w-1/2 bg-gradient-to-r from-transparent via-neon-cyan/20 to-transparent animate-step-shimmer" />
                    )}

                    {/* Left status rail */}
                    <span
                      className={`absolute left-0 top-1.5 bottom-1.5 w-0.5 rounded-r ${
                        isRunning
                          ? 'bg-neon-cyan'
                          : isApplied
                            ? 'bg-neon-green'
                            : isError
                              ? 'bg-neon-red'
                              : 'bg-border-subtle'
                      }`}
                    />

                    <div className="mt-0.5 ml-1 shrink-0">
                      {isRunning && <Loader2 className="w-3.5 h-3.5 text-neon-cyan animate-spin" />}
                      {isApplied && <CheckCircle2 className="w-3.5 h-3.5 text-neon-green" />}
                      {isError && <XCircle className="w-3.5 h-3.5 text-neon-red" />}
                      {step.status === 'skipped' && <MinusCircle className="w-3.5 h-3.5 text-text-muted" />}
                      {step.status === 'idle' && <Clock className="w-3.5 h-3.5 text-text-muted/70" />}
                    </div>

                    <div className="min-w-0 flex-1">
                      <div className="flex items-center justify-between gap-1">
                        <span className="text-[11px] font-semibold text-text-primary truncate">
                          <span className="metric-mono text-[10px] text-text-muted mr-1">0{idx + 1}</span>
                          {step.label}
                        </span>
                        <span
                          className={`shrink-0 metric-mono text-[9px] font-bold px-1.5 py-0.2 rounded ${
                            isApplied
                              ? 'bg-neon-green/15 text-neon-green'
                              : isRunning
                                ? 'bg-neon-cyan/20 text-neon-cyan'
                                : isError
                                  ? 'bg-neon-red/20 text-neon-red'
                                  : 'bg-bg-elevated text-text-muted'
                          }`}
                        >
                          {STEP_TAGS[step.key] ?? 'SYS'}
                        </span>
                      </div>
                      <div
                        className={`text-[10px] metric-mono truncate mt-0.5 ${
                          isError
                            ? 'text-neon-red'
                            : isApplied
                              ? 'text-neon-green'
                              : isRunning
                                ? 'text-neon-cyan'
                                : 'text-text-muted'
                        }`}
                        title={step.detail}
                      >
                        {step.detail}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Parallel DNS Benchmark Card */}
          <div className="tactical-card rounded-2xl p-3.5 flex-1 flex flex-col justify-between">
            <div className="flex items-center justify-between gap-2 mb-2.5">
              <div className="flex items-center gap-2 min-w-0">
                <Globe className="w-4 h-4 text-neon-cyan shrink-0" />
                <h2 className="text-[11px] font-mono font-bold uppercase tracking-[0.14em] text-text-secondary truncate">
                  BENCHMARK DNS PARALLÈLE (1.1.1.1 / 8.8.8.8 / 9.9.9.9)
                </h2>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                {fastestDns && (
                  <button
                    onClick={onApplyFastestDns}
                    className="whitespace-nowrap px-2.5 py-1 rounded-lg bg-neon-green/15 hover:bg-neon-green/25 border border-neon-green/45 text-neon-green metric-mono text-[11px] font-bold cursor-pointer transition-all"
                  >
                    Appliquer {fastestDns.ip} + Flush
                  </button>
                )}
                <button
                  onClick={onRunDnsBenchmark}
                  disabled={dnsBenchmarking}
                  className="whitespace-nowrap flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-neon-cyan/15 hover:bg-neon-cyan/25 border border-neon-cyan/45 text-neon-cyan text-[11px] font-bold cursor-pointer transition-all disabled:opacity-50"
                >
                  {dnsBenchmarking ? <Loader2 className="w-3 h-3 animate-spin" /> : <Play className="w-3 h-3" />}
                  Tester DNS
                </button>
              </div>
            </div>

            <div className="grid grid-cols-3 gap-2.5">
              {(dnsResults.length > 0
                ? dnsResults
                : [
                    { ip: '1.1.1.1', name: 'Cloudflare', avgMs: null, minMs: null, maxMs: null, successCount: 0, totalCount: 4 },
                    { ip: '8.8.8.8', name: 'Google DNS', avgMs: null, minMs: null, maxMs: null, successCount: 0, totalCount: 4 },
                    { ip: '9.9.9.9', name: 'Quad9', avgMs: null, minMs: null, maxMs: null, successCount: 0, totalCount: 4 },
                  ]
              ).map((dns) => {
                const barPct = dns.avgMs !== null ? Math.max(12, Math.min(100, Math.round((dns.avgMs / maxDnsMs) * 100))) : 0;
                return (
                  <div
                    key={dns.ip}
                    className={`p-2.5 rounded-xl border transition-all ${
                      dns.fastest
                        ? 'bg-neon-green/10 border-neon-green/55 shadow-[0_0_16px_rgba(16,245,160,0.12)]'
                        : 'bg-bg-void/70 border-border-subtle'
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <span className="text-xs font-bold text-text-primary">{dns.name}</span>
                      {dns.fastest && (
                        <span className="metric-mono text-[9px] font-black px-1.5 py-0.5 rounded bg-neon-green/20 text-neon-green">
                          OPTIMAL
                        </span>
                      )}
                    </div>
                    <div className="text-[11px] metric-mono text-text-muted">{dns.ip}</div>
                    <div className="mt-1.5 flex items-baseline justify-between">
                      <span
                        className={`text-lg font-black metric-mono ${
                          dns.fastest ? 'text-neon-green' : dns.avgMs !== null ? 'text-neon-cyan' : 'text-text-muted'
                        }`}
                      >
                        {dns.avgMs !== null ? `${dns.avgMs} ms` : '—'}
                      </span>
                      <span className="text-[10px] text-text-muted metric-mono">
                        {dns.minMs !== null ? `min ${dns.minMs}` : 'idle'}
                      </span>
                    </div>
                    <div className="mt-1.5 h-1 w-full rounded-full bg-bg-elevated overflow-hidden">
                      <div
                        className={`h-full rounded-full transition-all duration-500 ${
                          dns.fastest ? 'bg-neon-green' : 'bg-neon-cyan/70'
                        }`}
                        style={{ width: `${barPct}%` }}
                      />
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>

      {/* Row 2: Live Ping/Jitter Telemetry Chart + High-Contrast Before/After Comparator */}
      <div className="grid grid-cols-12 gap-4">
        {/* Live Telemetry Chart */}
        <div className="col-span-12 lg:col-span-7 tactical-card rounded-2xl p-4">
          <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
            <div className="flex items-center gap-2">
              <Activity className="w-4 h-4 text-neon-cyan" />
              <h2 className="text-[11px] font-mono font-bold uppercase tracking-[0.14em] text-text-secondary">
                TÉLÉMÉTRIE ICMP LIVE — <span className="text-neon-cyan">{selectedHost}</span>
              </h2>
            </div>

            <div className="flex items-center gap-2 metric-mono text-xs">
              <div className="px-2.5 py-1 rounded-lg bg-bg-void/90 border border-neon-cyan/30">
                <span className="text-text-muted text-[10px] mr-1">PING</span>
                <span className="text-neon-cyan font-black">
                  {pingStats.currentMs !== null ? `${pingStats.currentMs} ms` : '—'}
                </span>
              </div>
              <div className="px-2.5 py-1 rounded-lg bg-bg-void/90 border border-neon-green/30">
                <span className="text-text-muted text-[10px] mr-1">JITTER</span>
                <span className="text-neon-green font-black">{pingStats.jitterMs} ms</span>
              </div>
              <div
                className={`px-2.5 py-1 rounded-lg bg-bg-void/90 border ${
                  pingStats.lossPct > 0 ? 'border-neon-red/50' : 'border-border-subtle'
                }`}
              >
                <span className="text-text-muted text-[10px] mr-1">LOSS</span>
                <span className={`font-black ${pingStats.lossPct > 0 ? 'text-neon-red' : 'text-neon-green'}`}>
                  {pingStats.lossPct}%
                </span>
              </div>
            </div>
          </div>

          <div className="h-40 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={pingHistory} margin={{ top: 8, right: 8, left: -18, bottom: 0 }}>
                <defs>
                  <linearGradient id="pingGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#00E5FF" stopOpacity={0.38} />
                    <stop offset="95%" stopColor="#00E5FF" stopOpacity={0.0} />
                  </linearGradient>
                  <linearGradient id="jitterGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#10F5A0" stopOpacity={0.3} />
                    <stop offset="95%" stopColor="#10F5A0" stopOpacity={0.0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="#1B293F" vertical={false} />
                <XAxis dataKey="timeLabel" stroke="#64748B" fontSize={10} tickLine={false} minTickGap={24} />
                <YAxis stroke="#64748B" fontSize={10} tickLine={false} unit="ms" />
                <Tooltip
                  contentStyle={{
                    backgroundColor: '#090E17',
                    borderColor: '#2A3F5F',
                    borderRadius: '10px',
                    fontSize: '12px',
                    fontFamily: 'var(--font-mono)',
                  }}
                />
                <Area
                  type="monotone"
                  dataKey="rttMs"
                  name="Ping (ms)"
                  stroke="#00E5FF"
                  strokeWidth={2.2}
                  fill="url(#pingGrad)"
                  isAnimationActive={false}
                  connectNulls={false}
                />
                <Area
                  type="monotone"
                  dataKey="jitterMs"
                  name="Jitter (ms)"
                  stroke="#10F5A0"
                  strokeWidth={1.7}
                  fill="url(#jitterGrad)"
                  isAnimationActive={false}
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </div>

        {/* High-Contrast Before / After Boost Comparison */}
        <div className="col-span-12 lg:col-span-5 tactical-card rounded-2xl p-4 flex flex-col justify-between">
          <div className="flex items-center justify-between mb-2">
            <h2 className="text-[11px] font-mono font-bold uppercase tracking-[0.14em] text-text-secondary">
              COMPARATIF AVANT / APRÈS BOOST
            </h2>
            <button
              onClick={onResetComparison}
              title="Réinitialiser les compteurs Avant / Après"
              className="flex items-center gap-1 text-[11px] font-mono text-text-muted hover:text-neon-cyan cursor-pointer transition-colors"
            >
              <RefreshCw className="w-3 h-3" />
              RESET
            </button>
          </div>

          <div className="h-24 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={comparisonChartData} margin={{ top: 6, right: 8, left: -20, bottom: 0 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="#1B293F" vertical={false} />
                <XAxis dataKey="metric" stroke="#94A3B8" fontSize={10} tickLine={false} />
                <YAxis stroke="#64748B" fontSize={10} tickLine={false} />
                <Tooltip
                  contentStyle={{
                    backgroundColor: '#090E17',
                    borderColor: '#2A3F5F',
                    borderRadius: '10px',
                    fontSize: '12px',
                    fontFamily: 'var(--font-mono)',
                  }}
                />
                <Bar dataKey="Avant" name="Avant Boost" fill="#64748B" radius={[4, 4, 0, 0]} isAnimationActive={false} />
                <Bar dataKey="Apres" name="Après Boost" fill="#10F5A0" radius={[4, 4, 0, 0]} isAnimationActive={false} />
              </BarChart>
            </ResponsiveContainer>
          </div>

          <div className="grid grid-cols-3 gap-2 pt-2.5 border-t border-border-subtle metric-mono">
            {/* BEFORE card - high-contrast slate */}
            <div className="p-2.5 rounded-xl bg-[#0D1524] border border-border-strong/80 text-center">
              <div className="text-[9px] font-bold uppercase tracking-wider text-text-secondary">
                AVANT ({comparison.before.sent})
              </div>
              <div className="text-base font-black text-text-primary mt-0.5">
                {comparison.before.avgMs !== null ? `${comparison.before.avgMs} ms` : '—'}
              </div>
              <div className="text-[10px] text-text-muted">
                JIT {comparison.before.jitterMs} ms
              </div>
            </div>

            {/* AFTER card - vivid emerald border */}
            <div className="p-2.5 rounded-xl bg-neon-green/[0.08] border border-neon-green/45 text-center shadow-[inset_0_0_16px_rgba(16,245,160,0.06)]">
              <div className="text-[9px] font-bold uppercase tracking-wider text-neon-green">
                APRÈS ({comparison.after.sent})
              </div>
              <div className="text-base font-black text-neon-green mt-0.5">
                {comparison.after.avgMs !== null ? `${comparison.after.avgMs} ms` : '—'}
              </div>
              <div className="text-[10px] text-neon-green/75">
                JIT {comparison.after.jitterMs} ms
              </div>
            </div>

            {/* REAL DELTA card */}
            <div
              className={`p-2.5 rounded-xl border text-center ${
                pingDelta === null
                  ? 'bg-bg-void/70 border-border-subtle'
                  : pingDelta <= 0
                    ? 'bg-neon-cyan/12 border-neon-cyan/50 shadow-[0_0_14px_rgba(0,229,255,0.12)]'
                    : 'bg-neon-amber/12 border-neon-amber/50'
              }`}
            >
              <div className="text-[9px] font-bold uppercase tracking-wider text-text-secondary">
                DELTA RÉEL
              </div>
              <div
                className={`text-base font-black mt-0.5 flex items-center justify-center gap-0.5 ${
                  pingDelta === null
                    ? 'text-text-muted'
                    : pingDelta <= 0
                      ? 'text-neon-cyan'
                      : 'text-neon-amber'
                }`}
              >
                {pingDelta !== null && pingDelta <= 0 && <ArrowDownRight className="w-3.5 h-3.5" />}
                {pingDelta !== null && pingDelta > 0 && <ArrowUpRight className="w-3.5 h-3.5" />}
                {pingDelta !== null ? `${pingDelta > 0 ? '+' : ''}${pingDelta} ms` : '—'}
              </div>
              <div className="text-[10px] text-text-muted">
                {pingDeltaPct !== null
                  ? `${pingDeltaPct > 0 ? '+' : ''}${pingDeltaPct}%`
                  : jitterDelta !== null
                    ? `JIT ${jitterDelta > 0 ? '+' : ''}${jitterDelta}`
                    : 'EN ATTENTE'}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Row 3: Compact Tactical Command Console */}
      <div className="tactical-card rounded-2xl p-4">
        <div className="flex items-center justify-between mb-2">
          <div className="flex items-center gap-2">
            <Terminal className="w-4 h-4 text-neon-green" />
            <h2 className="text-[11px] font-mono font-bold uppercase tracking-[0.14em] text-text-secondary">
              FLUX CONSOLE SYSTÈME (EXÉCUTION RÉELLE)
            </h2>
          </div>
          <button
            onClick={() => onNavigate('console')}
            className="flex items-center gap-1 text-xs font-mono text-neon-cyan hover:underline cursor-pointer"
          >
            Console complète &amp; Backup JSON
            <ChevronRight className="w-3.5 h-3.5" />
          </button>
        </div>

        <div className="rounded-xl bg-[#05080D] border border-border-subtle p-3 metric-mono text-[11px] space-y-1.5 max-h-36 overflow-y-auto">
          {recentLogs.length === 0 ? (
            <div className="text-text-muted">En attente de commandes système...</div>
          ) : (
            recentLogs.map((entry) => (
              <div key={entry.id} className="flex items-start gap-2 leading-relaxed">
                <span className="text-text-muted shrink-0">[{entry.timestamp}]</span>
                <span
                  className={`shrink-0 px-1.5 rounded text-[9px] font-bold uppercase ${
                    entry.status === 'ok'
                      ? 'bg-neon-green/15 text-neon-green'
                      : entry.status === 'error'
                        ? 'bg-neon-red/15 text-neon-red'
                        : entry.status === 'warn'
                          ? 'bg-neon-amber/15 text-neon-amber'
                          : 'bg-neon-cyan/15 text-neon-cyan'
                  }`}
                >
                  {entry.category}
                </span>
                <span className="text-text-primary truncate flex-1" title={entry.command}>
                  {entry.command}
                </span>
                {entry.stdout && (
                  <span className="text-text-muted truncate max-w-[300px]" title={entry.stdout}>
                    → {entry.stdout.split('\n')[0]}
                  </span>
                )}
                {entry.status === 'error' && entry.stderr && (
                  <span className="text-neon-red truncate max-w-[300px]" title={entry.stderr}>
                    ! {entry.stderr.split('\n')[0]}
                  </span>
                )}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
};
