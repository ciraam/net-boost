import React from 'react';
import {
  Activity,
  Cpu,
  Crosshair,
  Gauge,
  Network,
  Radar,
  Route,
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
    <aside className="w-64 shrink-0 bg-sidebar/95 backdrop-blur-md border-r border-sidebar-border flex flex-col justify-between select-none">
      <div>
        {/* Brand Header (Craft.ro Indigo Badge) */}
        <div className="h-14 px-4 border-b border-sidebar-border flex items-center gap-3">
          <div className="relative w-8 h-8 rounded-lg bg-primary text-white flex items-center justify-center shadow-[0_0_16px_rgba(94,106,210,0.45),inset_0_1px_0_rgba(255,255,255,0.3)]">
            <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2.3">
              <path strokeLinecap="round" strokeLinejoin="round" d="M13 2L4 14h7l-1 8 9-12h-7l1-8z" />
            </svg>
            {boostState?.active && (
              <span className="absolute -top-0.5 -right-0.5 w-2 h-2 rounded-full bg-emerald-400 ring-2 ring-sidebar animate-status-beacon" />
            )}
          </div>
          <div>
            <div className="font-semibold tracking-[-0.02em] text-sm text-foreground flex items-center gap-1.5">
              NetBoost
              <span className="text-[10px] font-mono tabular-nums px-1.5 py-0.5 rounded-full bg-primary/12 text-primary border border-primary/25 font-medium">
                1.0
              </span>
            </div>
            <div className="text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
              Telemetry &amp; Router
            </div>
          </div>
        </div>

        {/* Navigation */}
        <nav className="p-2.5 space-y-0.5">
          <div className="px-2.5 pt-1 pb-1.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground/75">
            Modules
          </div>
          {NAV_ITEMS.map((item) => {
            const Icon = item.icon;
            const isActive = activeTab === item.id;
            return (
              <button
                key={item.id}
                onClick={() => onSelectTab(item.id)}
                className={`group relative w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-left transition-colors duration-150 cursor-pointer border ${
                  isActive
                    ? 'bg-primary/12 border-primary/25 text-foreground dark:shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]'
                    : 'bg-transparent border-transparent text-muted-foreground hover:bg-secondary/70 hover:text-foreground'
                }`}
              >
                {isActive && (
                  <span className="absolute left-0 top-2 bottom-2 w-0.5 rounded-r bg-primary" />
                )}
                <Icon className={`w-4 h-4 shrink-0 transition-transform duration-150 group-hover:scale-[1.04] ${isActive ? 'text-primary' : 'text-muted-foreground'}`} />
                <div className="min-w-0 flex-1">
                  <div className="text-[13px] font-medium tracking-[-0.01em] truncate flex items-center justify-between gap-1.5">
                    <span className="truncate">{item.label}</span>
                    {item.id === 'inspector' && nonEuCount > 0 && (
                      <span className="px-1.5 py-0.5 rounded-full bg-destructive/15 border border-destructive/30 text-destructive metric-mono text-[10px] font-semibold">
                        {nonEuCount}
                      </span>
                    )}
                    {item.id === 'tunnel' && tunnelState?.active && (
                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-status-beacon" />
                    )}
                  </div>
                  <div className="text-[11px] text-muted-foreground/80 truncate">{item.subtitle}</div>
                </div>
              </button>
            );
          })}
        </nav>
      </div>

      {/* Bottom Live Network Telemetry Card */}
      <div className="p-3 border-t border-sidebar-border">
        <div className="rounded-xl craft-card p-3 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground flex items-center gap-1.5">
              <Wifi className="w-3.5 h-3.5 text-primary" />
              Interface active
            </span>
            <span
              className={`w-2 h-2 rounded-full ${
                adapter ? 'bg-emerald-500' : 'bg-destructive'
              }`}
            />
          </div>

          {adapter ? (
            <div className="space-y-1 text-xs">
              <div className="font-medium text-foreground flex items-center justify-between gap-1">
                <span className="truncate">{adapter.name}</span>
                <span className="metric-mono text-[10px] px-1.5 py-0.5 rounded-full bg-emerald-500/12 border border-emerald-500/25 text-emerald-600 dark:text-emerald-400 shrink-0 font-medium">
                  {adapter.linkSpeed}
                </span>
              </div>
              <div className="text-[11px] text-muted-foreground truncate" title={adapter.description}>
                {adapter.description}
              </div>
              <div className="flex items-center justify-between pt-1.5 border-t border-border metric-mono text-[11px]">
                <span className="text-muted-foreground">IPv4</span>
                <span className="text-foreground font-medium">{adapter.ipv4 || '—'}</span>
              </div>
              <div className="flex items-center justify-between metric-mono text-[11px]">
                <span className="text-muted-foreground">DNS</span>
                <span className="text-muted-foreground truncate max-w-[130px]" title={adapter.currentDns.join(', ')}>
                  {adapter.currentDns[0] || (adapter.dnsIsDhcp ? 'DHCP' : '—')}
                </span>
              </div>
            </div>
          ) : (
            <div className="text-xs text-muted-foreground flex items-center gap-1.5">
              <Network className="w-3.5 h-3.5" />
              Scan interface...
            </div>
          )}
        </div>
      </div>
    </aside>
  );
};
