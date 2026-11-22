import React from 'react';
import {
  AlertTriangle,
  Crosshair,
  Minimize2,
  RotateCcw,
  Route,
  Server,
  Shield,
  ShieldAlert,
} from 'lucide-react';
import type { BoostState, GameProfile, SystemInfo, TunnelState } from '../types/ipc';

interface TopBarProps {
  systemInfo: SystemInfo | null;
  boostState: BoostState | null;
  tunnelState: TunnelState | null;
  selectedGame: GameProfile | null;
  selectedHost: string;
  onRelaunchAdmin: () => void;
  onRollback: () => void;
  onMinimizeToTray: () => void;
  busyRollback: boolean;
}

export const TopBar: React.FC<TopBarProps> = ({
  systemInfo,
  boostState,
  tunnelState,
  selectedGame,
  selectedHost,
  onRelaunchAdmin,
  onRollback,
  onMinimizeToTray,
  busyRollback,
}) => {
  const isAdmin = systemInfo?.isAdmin ?? false;
  const hasBackup = boostState?.hasBackup || systemInfo?.hasBackup || false;
  const isBoosted = boostState?.active ?? false;
  const isTunnelActive = tunnelState?.active ?? false;

  return (
    <header className="h-15 shrink-0 bg-[#090E17]/90 backdrop-blur-md border-b border-border-subtle px-5 flex items-center justify-between gap-3 select-none">
      {/* Left: Target Telemetry Strip */}
      <div className="flex items-center gap-2.5 min-w-0">
        <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-bg-surface/90 border border-border-subtle text-xs">
          <Crosshair className="w-3.5 h-3.5 text-neon-cyan shrink-0" />
          <span className="text-[10px] uppercase tracking-wider text-text-muted font-bold">Cible</span>
          <span className="font-mono font-semibold text-text-primary truncate max-w-[165px]">
            {selectedGame ? selectedGame.exeName : 'GLOBAL'}
          </span>
        </div>

        <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-bg-surface/90 border border-border-subtle text-xs metric-mono">
          <Server className="w-3.5 h-3.5 text-neon-green shrink-0" />
          <span className="text-neon-cyan font-semibold">{selectedHost}</span>
        </div>

        {/* Live Session Badge */}
        <div
          className={`hidden 2xl:flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[10px] font-mono font-bold uppercase tracking-wider border transition-all ${
            isBoosted
              ? 'bg-neon-green/12 border-neon-green/45 text-neon-green shadow-[0_0_12px_rgba(16,245,160,0.18)]'
              : 'bg-bg-surface/80 border-border-subtle text-text-muted'
          }`}
        >
          <span className={`w-1.5 h-1.5 rounded-full ${isBoosted ? 'bg-neon-green animate-status-beacon' : 'bg-text-muted'}`} />
          {isBoosted ? 'DSCP 46 • ACTIF' : 'STANDBY'}
        </div>
      </div>

      {/* Right: Tactical Status Badges (Tunnel / Admin UAC / Rollback) */}
      <div className="flex items-center gap-2 shrink-0">
        {/* Tunnel EU Badge */}
        <div
          className={`hidden xl:flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-[11px] font-mono font-semibold border transition-all ${
            isTunnelActive
              ? 'bg-neon-cyan/12 border-neon-cyan/45 text-neon-cyan shadow-[0_0_14px_rgba(0,229,255,0.16)]'
              : 'bg-bg-surface/70 border-border-subtle text-text-muted'
          }`}
          title={isTunnelActive ? `Tunnel actif : ${tunnelState?.endpoint}` : 'Tunnel EU Split inactif'}
        >
          <Route className={`w-3.5 h-3.5 ${isTunnelActive ? 'text-neon-cyan' : 'text-text-muted'}`} />
          <span className={`w-1.5 h-1.5 rounded-full ${isTunnelActive ? 'bg-neon-green animate-status-beacon' : 'bg-text-muted/60'}`} />
          <span>{isTunnelActive ? 'TUNNEL EU' : 'TUNNEL OFF'}</span>
        </div>

        {/* Admin UAC Badge */}
        {isAdmin ? (
          <div className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-neon-green/12 border border-neon-green/40 text-neon-green text-[11px] font-mono font-bold shadow-[0_0_12px_rgba(16,245,160,0.12)]">
            <span className="w-1.5 h-1.5 rounded-full bg-neon-green animate-status-beacon" />
            <Shield className="w-3.5 h-3.5" />
            ADMIN UAC
          </div>
        ) : (
          <button
            onClick={onRelaunchAdmin}
            title="Relancer NetBoost avec élévation Administrateur Windows (UAC) pour appliquer QoS, DNS, Registre, Tunnel et Pare-feu"
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-neon-amber/15 hover:bg-neon-amber/25 border border-neon-amber/50 text-neon-amber text-xs font-semibold transition-all hover:shadow-[0_0_14px_rgba(255,184,0,0.2)] cursor-pointer"
          >
            <span className="w-1.5 h-1.5 rounded-full bg-neon-amber animate-status-beacon" />
            <ShieldAlert className="w-3.5 h-3.5" />
            Relancer en Admin (UAC)
          </button>
        )}

        {/* Mandatory Rollback Button + Pending State Indicator */}
        <button
          onClick={onRollback}
          disabled={busyRollback || boostState?.busy}
          className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-semibold transition-all cursor-pointer border disabled:opacity-50 ${
            hasBackup || isBoosted || isTunnelActive
              ? 'bg-neon-red/15 hover:bg-neon-red/25 border-neon-red/55 text-neon-red shadow-[0_0_18px_rgba(255,59,92,0.22)]'
              : 'bg-bg-surface/90 hover:bg-bg-elevated border-border-strong text-text-secondary hover:text-text-primary'
          }`}
        >
          {busyRollback ? (
            <AlertTriangle className="w-3.5 h-3.5 animate-spin" />
          ) : (
            <RotateCcw className="w-3.5 h-3.5" />
          )}
          <span>Restaurer (Rollback)</span>
          {(hasBackup || isBoosted) && (
            <span className="w-1.5 h-1.5 rounded-full bg-neon-red animate-status-beacon" />
          )}
        </button>

        {/* System Tray Minimize Button */}
        <button
          onClick={onMinimizeToTray}
          title="Réduire dans la barre des tâches (System Tray) pour minimiser l'empreinte RAM/CPU en jeu"
          className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-bg-surface/90 hover:bg-bg-elevated border border-border-subtle hover:border-neon-cyan/45 text-text-secondary hover:text-neon-cyan text-xs font-mono font-semibold transition-all cursor-pointer"
        >
          <Minimize2 className="w-3.5 h-3.5" />
          <span className="hidden 2xl:inline">Tray</span>
        </button>
      </div>
    </header>
  );
};
