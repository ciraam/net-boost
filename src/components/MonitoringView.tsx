import React, { useState } from 'react';
import {
  Activity,
  Loader2,
  Play,
  RefreshCw,
  Route,
  Square,
} from 'lucide-react';
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import type { BeforeAfterComparison, PingSample, PingStats, TracerouteHop } from '../types/ipc';

interface MonitoringViewProps {
  selectedHost: string;
  pingHistory: PingSample[];
  pingStats: PingStats;
  comparison: BeforeAfterComparison;
  tracerouteHops: TracerouteHop[];
  tracerouteRunning: boolean;
  tracerouteHost: string;
  onChangeHost: (host: string) => void;
  onResetComparison: () => void;
  onRunTraceroute: (host: string) => void;
  onCancelTraceroute: () => void;
}

export const MonitoringView: React.FC<MonitoringViewProps> = ({
  selectedHost,
  pingHistory,
  pingStats,
  comparison,
  tracerouteHops,
  tracerouteRunning,
  tracerouteHost,
  onChangeHost,
  onResetComparison,
  onRunTraceroute,
  onCancelTraceroute,
}) => {
  const [hostInput, setHostInput] = useState(selectedHost);

  const handleApplyHost = (e: React.FormEvent) => {
    e.preventDefault();
    if (hostInput.trim()) {
      onChangeHost(hostInput.trim());
    }
  };

  const hopChartData = tracerouteHops.map((h) => ({
    label: `#${h.hop}`,
    avgMs: h.avgMs ?? 0,
    ip: h.ip ?? 'Timeout (*)',
    timedOut: h.timedOut,
  }));

  return (
    <div className="space-y-4">
      {/* Live Ping & Jitter Telemetry Expanded */}
      <div className="rounded-2xl bg-bg-panel border border-border-subtle p-5 space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <Activity className="w-4 h-4 text-neon-cyan" />
            <h2 className="text-sm font-bold text-text-primary">Monitoring ICMP Continu (Ping / Jitter / Perte)</h2>
          </div>

          <form onSubmit={handleApplyHost} className="flex items-center gap-2">
            <input
              type="text"
              value={hostInput}
              onChange={(e) => setHostInput(e.target.value)}
              placeholder="IP ou domaine cible..."
              className="w-56 bg-bg-surface border border-border-strong rounded-lg px-3 py-1.5 text-xs font-mono text-text-primary focus:outline-none focus:border-neon-cyan"
            />
            <button
              type="submit"
              className="px-3 py-1.5 rounded-lg bg-neon-cyan/15 hover:bg-neon-cyan/25 border border-neon-cyan/40 text-neon-cyan text-xs font-bold cursor-pointer"
            >
              Cibler
            </button>
            <button
              type="button"
              onClick={onResetComparison}
              className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-bg-surface border border-border-strong text-xs text-text-secondary hover:text-text-primary cursor-pointer"
            >
              <RefreshCw className="w-3.5 h-3.5" />
              Reset
            </button>
          </form>
        </div>

        {/* KPI Cards */}
        <div className="grid grid-cols-2 md:grid-cols-6 gap-2.5 font-mono">
          <div className="p-3 rounded-xl bg-bg-surface border border-border-subtle">
            <div className="text-[10px] font-sans uppercase text-text-muted">Actuel</div>
            <div className="text-xl font-black text-neon-cyan mt-0.5">
              {pingStats.currentMs !== null ? `${pingStats.currentMs} ms` : '—'}
            </div>
          </div>
          <div className="p-3 rounded-xl bg-bg-surface border border-border-subtle">
            <div className="text-[10px] font-sans uppercase text-text-muted">Moyenne</div>
            <div className="text-xl font-black text-text-primary mt-0.5">
              {pingStats.avgMs !== null ? `${pingStats.avgMs} ms` : '—'}
            </div>
          </div>
          <div className="p-3 rounded-xl bg-bg-surface border border-border-subtle">
            <div className="text-[10px] font-sans uppercase text-text-muted">Min / Max</div>
            <div className="text-base font-bold text-text-secondary mt-1">
              {pingStats.minMs !== null ? `${pingStats.minMs} / ${pingStats.maxMs}` : '—'}
            </div>
          </div>
          <div className="p-3 rounded-xl bg-bg-surface border border-border-subtle">
            <div className="text-[10px] font-sans uppercase text-text-muted">Jitter</div>
            <div className="text-xl font-black text-neon-green mt-0.5">{pingStats.jitterMs} ms</div>
          </div>
          <div className="p-3 rounded-xl bg-bg-surface border border-border-subtle">
            <div className="text-[10px] font-sans uppercase text-text-muted">Perte Paquets</div>
            <div className={`text-xl font-black mt-0.5 ${pingStats.lossPct > 0 ? 'text-neon-red' : 'text-neon-green'}`}>
              {pingStats.lossPct}%
            </div>
          </div>
          <div className="p-3 rounded-xl bg-bg-surface border border-border-subtle">
            <div className="text-[10px] font-sans uppercase text-text-muted">Avant / Après</div>
            <div className="text-xs font-bold text-text-secondary mt-1.5">
              {comparison.before.avgMs ?? '—'} → <span className="text-neon-green">{comparison.after.avgMs ?? '—'} ms</span>
            </div>
          </div>
        </div>

        {/* Full Live Chart */}
        <div className="h-56 w-full">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={pingHistory} margin={{ top: 8, right: 12, left: -14, bottom: 0 }}>
              <defs>
                <linearGradient id="monPingGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#00E5FF" stopOpacity={0.35} />
                  <stop offset="95%" stopColor="#00E5FF" stopOpacity={0.0} />
                </linearGradient>
                <linearGradient id="monJitGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#00FF94" stopOpacity={0.25} />
                  <stop offset="95%" stopColor="#00FF94" stopOpacity={0.0} />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="#1E2A3D" vertical={false} />
              <XAxis dataKey="timeLabel" stroke="#64748B" fontSize={10} minTickGap={25} />
              <YAxis stroke="#64748B" fontSize={10} unit="ms" />
              <Tooltip
                contentStyle={{
                  backgroundColor: '#0D121B',
                  borderColor: '#2B3B55',
                  borderRadius: '10px',
                  fontSize: '12px',
                }}
              />
              <Area type="monotone" dataKey="rttMs" name="Ping (ms)" stroke="#00E5FF" strokeWidth={2} fill="url(#monPingGrad)" isAnimationActive={false} connectNulls={false} />
              <Area type="monotone" dataKey="jitterMs" name="Jitter (ms)" stroke="#00FF94" strokeWidth={1.6} fill="url(#monJitGrad)" isAnimationActive={false} />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* Visual Traceroute Tool */}
      <div className="rounded-2xl bg-bg-panel border border-border-subtle p-5 space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 className="text-sm font-bold text-text-primary flex items-center gap-2">
              <Route className="w-4 h-4 text-neon-green" />
              Outil Traceroute Visuel — <span className="font-mono text-neon-cyan">{tracerouteHost || selectedHost}</span>
            </h2>
            <p className="text-xs text-text-muted mt-0.5">
              Exécute réellement <span className="font-mono">tracert.exe -d -h 20 -w 1000</span> vers le serveur cible pour identifier chaque saut réseau.
            </p>
          </div>

          <div className="flex items-center gap-2">
            {tracerouteRunning ? (
              <button
                onClick={onCancelTraceroute}
                className="flex items-center gap-1.5 px-3.5 py-2 rounded-xl bg-neon-red/15 hover:bg-neon-red/25 border border-neon-red/40 text-neon-red text-xs font-bold cursor-pointer"
              >
                <Square className="w-3.5 h-3.5" />
                Arrêter Traceroute
              </button>
            ) : (
              <button
                onClick={() => onRunTraceroute(selectedHost)}
                className="flex items-center gap-1.5 px-3.5 py-2 rounded-xl bg-neon-green/15 hover:bg-neon-green/25 border border-neon-green/40 text-neon-green text-xs font-bold cursor-pointer"
              >
                <Play className="w-3.5 h-3.5" />
                Lancer Traceroute vers {selectedHost}
              </button>
            )}
          </div>
        </div>

        {/* Latency per hop bar chart */}
        {tracerouteHops.length > 0 && (
          <div className="h-44 w-full rounded-xl bg-bg-surface border border-border-subtle p-3">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={hopChartData} margin={{ top: 8, right: 8, left: -16, bottom: 0 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="#1E2A3D" vertical={false} />
                <XAxis dataKey="label" stroke="#94A3B8" fontSize={10} />
                <YAxis stroke="#64748B" fontSize={10} unit="ms" />
                <Tooltip
                  contentStyle={{
                    backgroundColor: '#0D121B',
                    borderColor: '#2B3B55',
                    borderRadius: '10px',
                    fontSize: '12px',
                  }}
                />
                <Bar dataKey="avgMs" name="Latence Moyenne (ms)" radius={[4, 4, 0, 0]} isAnimationActive={false}>
                  {hopChartData.map((entry, idx) => (
                    <Cell
                      key={`hop-${idx}`}
                      fill={
                        entry.timedOut
                          ? '#475569'
                          : entry.avgMs > 90
                            ? '#FF3B5C'
                            : entry.avgMs > 40
                              ? '#FFB800'
                              : '#00E5FF'
                      }
                    />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </div>
        )}

        {/* Hop-by-hop visual node table */}
        <div className="overflow-x-auto rounded-xl border border-border-subtle">
          <table className="w-full text-left border-collapse text-xs font-mono">
            <thead>
              <tr className="bg-bg-surface font-sans text-[11px] uppercase tracking-wider text-text-muted border-b border-border-subtle">
                <th className="py-2 px-3">Saut</th>
                <th className="py-2 px-3">Adresse IP Routeur</th>
                <th className="py-2 px-3 text-right">RTT 1</th>
                <th className="py-2 px-3 text-right">RTT 2</th>
                <th className="py-2 px-3 text-right">RTT 3</th>
                <th className="py-2 px-3 text-right">Moyenne</th>
                <th className="py-2 px-3 text-right">État</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border-subtle/70">
              {tracerouteHops.length === 0 ? (
                <tr>
                  <td colSpan={7} className="py-8 text-center font-sans text-text-muted">
                    {tracerouteRunning ? (
                      <span className="inline-flex items-center gap-2 text-neon-cyan">
                        <Loader2 className="w-4 h-4 animate-spin" />
                        Découverte des sauts réseau en cours...
                      </span>
                    ) : (
                      'Cliquez sur « Lancer Traceroute » pour cartographier la route réelle vers le serveur cible.'
                    )}
                  </td>
                </tr>
              ) : (
                tracerouteHops.map((hop) => (
                  <tr key={hop.hop} className="hover:bg-bg-surface/60">
                    <td className="py-2 px-3 font-bold text-neon-cyan">#{hop.hop}</td>
                    <td className="py-2 px-3 text-text-primary">{hop.ip ?? '* * * (Délai dépassé)'}</td>
                    <td className="py-2 px-3 text-right text-text-secondary">{hop.rtt1 !== null ? `${hop.rtt1} ms` : '*'}</td>
                    <td className="py-2 px-3 text-right text-text-secondary">{hop.rtt2 !== null ? `${hop.rtt2} ms` : '*'}</td>
                    <td className="py-2 px-3 text-right text-text-secondary">{hop.rtt3 !== null ? `${hop.rtt3} ms` : '*'}</td>
                    <td className="py-2 px-3 text-right font-bold text-text-primary">
                      {hop.avgMs !== null ? `${hop.avgMs} ms` : '—'}
                    </td>
                    <td className="py-2 px-3 text-right font-sans">
                      {hop.timedOut ? (
                        <span className="px-2 py-0.5 rounded bg-bg-elevated text-text-muted text-[10px] font-bold">
                          MASQUÉ / *
                        </span>
                      ) : (hop.avgMs ?? 0) > 90 ? (
                        <span className="px-2 py-0.5 rounded bg-neon-red/15 text-neon-red text-[10px] font-bold">
                          LATENCE ÉLEVÉE
                        </span>
                      ) : (hop.avgMs ?? 0) > 40 ? (
                        <span className="px-2 py-0.5 rounded bg-neon-amber/15 text-neon-amber text-[10px] font-bold">
                          MODÉRÉ
                        </span>
                      ) : (
                        <span className="px-2 py-0.5 rounded bg-neon-green/15 text-neon-green text-[10px] font-bold">
                          RAPIDE
                        </span>
                      )}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
