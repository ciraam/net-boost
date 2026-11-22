import React from 'react';
import {
  Activity,
  Cpu,
  Crosshair,
  Gauge,
  Network,
  Radar,
  Route,
  ShieldCheck,
  Terminal,
  Wifi,
} from 'lucide-react';
import type { BoostState, SystemInfo, TunnelState } from '../types/ipc';

export type NavTab = 'dashboard' | 'targeting' | 'inspector' | 'tunnel' | 'background' | 'monitoring' | 'console';

interface SidebarProps {
  activeTab: NavTab;
  onSelectTab: (tab: NavTab) => void;
  systemInfo: SystemInfo | null;
  boostState: BoostState | null;
  tunnelState: TunnelState | null;
  nonEuCount: number;
}

const NAV_ITEMS: Array<{ id: NavTab; label: string; subtitle: string; icon: React.ComponentType<{ className?: string }> }> = [
  { id: 'dashboard', label: 'Boost & Cockpit', subtitle: 'Moteur réseau & KPIs', icon: Gauge },
  { id: 'targeting', label: 'Ciblage & Presets', subtitle: 'Jeux .exe & Serveurs', icon: Crosshair },
  { id: 'inspector', label: 'Inspecteur & Pare-feu', subtitle: 'GeoIP Offline & ETW', icon: Radar },
  { id: 'tunnel', label: 'Tunnel EU & Split', subtitle: 'WireSock / WireGuard', icon: Route },
  { id: 'background', label: 'Trafic Arrière-plan', subtitle: 'QoS DSCP 8 & MAJ', icon: Cpu },
  { id: 'monitoring', label: 'Monitoring & Route', subtitle: 'Ping Live & Traceroute', icon: Activity },
  { id: 'console', label: 'Console & Rollback', subtitle: 'Logs réels & JSON', icon: Terminal },
];

export const Sidebar: React.FC<SidebarProps> = ({
  activeTab,
  onSelectTab,
  systemInfo,
  boostState,
  tunnelState,
  nonEuCount,
}) => {
  const adapter = systemInfo?.activeAdapter;

  return (
    <aside className="w-64 shrink-0 bg-[#090E17]/92 backdrop-blur-md border-r border-border-subtle flex flex-col justify-between select-none">
      <div>
        {/* Brand Header */}
        <div className="px-4 py-4 border-b border-border-subtle flex items-center gap-3">
          <div className="relative w-10 h-10 rounded-xl bg-gradient-to-br from-neon-cyan/20 via-bg-panel to-neon-green/15 border border-neon-cyan/45 flex items-center justify-center shadow-[0_0_20px_rgba(0,229,255,0.2)]">
            <svg viewBox="0 0 24 24" className="w-5 h-5 text-neon-cyan" fill="none" stroke="currentColor" strokeWidth="2.3">
              <path strokeLinecap="round" strokeLinejoin="round" d="M13 2L4 14h7l-1 8 9-12h-7l1-8z" />
            </svg>
            {boostState?.active && (
              <span className="absolute -top-1 -right-1 w-2.5 h-2.5 rounded-full bg-neon-green shadow-[0_0_8px_#10F5A0] animate-status-beacon" />
            )}
          </div>
          <div>
            <div className="font-black tracking-[0.14em] text-sm text-text-primary flex items-center gap-1">
              NET<span className="text-neon-cyan">BOOST</span>
            </div>
            <div className="text-[9px] font-mono uppercase tracking-[0.22em] text-text-muted font-semibold">
              TACTICAL TELEMETRY
            </div>
          </div>
        </div>

        {/* Navigation */}
        <nav className="p-2.5 space-y-1">
          {NAV_ITEMS.map((item) => {
            const Icon = item.icon;
            const isActive = activeTab === item.id;
            return (
              <button
                key={item.id}
                onClick={() => onSelectTab(item.id)}
                className={`relative w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-left transition-all duration-200 cursor-pointer border ${
                  isActive
                    ? 'bg-neon-cyan/10 border-neon-cyan/40 text-text-primary shadow-[inset_0_0_18px_rgba(0,229,255,0.08)]'
                    : 'bg-transparent border-transparent text-text-secondary hover:bg-bg-panel/70 hover:border-border-subtle hover:text-text-primary'
                }`}
              >
                {isActive && (
                  <span className="absolute left-0 top-2 bottom-2 w-0.5 rounded-r bg-neon-cyan shadow-[0_0_8px_#00E5FF]" />
                )}
                <Icon className={`w-4 h-4 shrink-0 ${isActive ? 'text-neon-cyan' : 'text-text-muted'}`} />
                <div className="min-w-0 flex-1">
                  <div className="text-xs font-semibold truncate flex items-center justify-between gap-1.5">
                    <span className="truncate">{item.label}</span>
                    {item.id === 'inspector' && nonEuCount > 0 && (
                      <span className="px-1.5 py-0.5 rounded bg-neon-red/20 border border-neon-red/35 text-neon-red metric-mono text-[9px] font-black">
                        {nonEuCount}
                      </span>
                    )}
                    {item.id === 'tunnel' && tunnelState?.active && (
                      <span className="w-2 h-2 rounded-full bg-neon-green shadow-[0_0_8px_#10F5A0] animate-status-beacon" />
                    )}
                  </div>
                  <div className="text-[10px] text-text-muted truncate">{item.subtitle}</div>
                </div>
              </button>
            );
          })}
        </nav>
      </div>

      {/* Bottom Live Network Telemetry Card */}
      <div className="p-3 border-t border-border-subtle space-y-2">
        <div className="rounded-xl bg-bg-surface/90 border border-border-subtle p-3 space-y-2 shadow-[inset_0_1px_0_rgba(255,255,255,0.03)]">
          <div className="flex items-center justify-between">
            <span className="text-[9px] font-mono font-bold uppercase tracking-[0.16em] text-text-muted flex items-center gap-1.5">
              <Wifi className="w-3.5 h-3.5 text-neon-cyan" />
              INTERFACE ACTIVE
            </span>
            <span
              className={`w-2 h-2 rounded-full ${
                adapter ? 'bg-neon-green shadow-[0_0_8px_#10F5A0]' : 'bg-neon-red'
              }`}
            />
          </div>

          {adapter ? (
            <div className="space-y-1 text-xs">
              <div className="font-semibold text-text-primary flex items-center justify-between gap-1">
                <span className="truncate">{adapter.name}</span>
                <span className="metric-mono text-[10px] px-1.5 py-0.5 rounded bg-bg-elevated text-neon-green shrink-0">
                  {adapter.linkSpeed}
                </span>
              </div>
              <div className="text-[10px] text-text-muted truncate" title={adapter.description}>
                {adapter.description}
              </div>
              <div className="flex items-center justify-between pt-1.5 border-t border-border-subtle/70 metric-mono text-[11px]">
                <span className="text-text-muted">IPv4</span>
                <span className="text-neon-cyan font-semibold">{adapter.ipv4 || '—'}</span>
              </div>
              <div className="flex items-center justify-between metric-mono text-[11px]">
                <span className="text-text-muted">DNS</span>
                <span className="text-text-secondary truncate max-w-[130px]" title={adapter.currentDns.join(', ')}>
                  {adapter.currentDns[0] || (adapter.dnsIsDhcp ? 'DHCP' : '—')}
                </span>
              </div>
            </div>
          ) : (
            <div className="text-xs text-text-muted flex items-center gap-1.5">
              <Network className="w-3.5 h-3.5" />
              Scan interface...
            </div>
          )}
        </div>

        <div className="px-3 py-2 rounded-lg bg-bg-void/90 border border-border-subtle flex items-center justify-between text-[10px] font-mono">
          <span className="text-text-muted flex items-center gap-1.5 uppercase tracking-wider">
            <ShieldCheck className={`w-3.5 h-3.5 ${boostState?.hasBackup ? 'text-neon-green' : 'text-text-muted'}`} />
            BACKUP JSON
          </span>
          <span className={`font-bold ${boostState?.hasBackup ? 'text-neon-green' : 'text-text-muted'}`}>
            {boostState?.hasBackup ? 'ACTIF' : 'VIDE'}
          </span>
        </div>
      </div>
    </aside>
  );
};
