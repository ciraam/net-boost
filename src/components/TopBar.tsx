import React from 'react';
import {
  AlertTriangle,
  Crosshair,
  Minimize2,
  Moon,
  RotateCcw,
  Route,
  Server,
  Shield,
  ShieldAlert,
  Sun,
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
  theme: 'dark' | 'light';
  onToggleTheme: () => void;
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
  theme,
  onToggleTheme,
}) => {
  const isAdmin = systemInfo?.isAdmin ?? false;
  const hasBackup = boostState?.hasBackup || systemInfo?.hasBackup || false;
  const isBoosted = boostState?.active ?? false;
  const isTunnelActive = tunnelState?.active ?? false;

  return (
    <header className="h-14 shrink-0 craft-glass border-b border-border px-5 flex items-center justify-between gap-3 select-none">
      {/* Left: Target Telemetry Strip */}
      <div className="flex items-center gap-2 min-w-0">
        <div className="flex items-center gap-2 px-2.5 py-1.5 rounded-lg bg-card border border-border text-xs shadow-xs dark:shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]">
          <Crosshair className="w-3.5 h-3.5 text-primary shrink-0" />
          <span className="text-[10px] uppercase tracking-[0.08em] text-muted-foreground font-semibold">Cible</span>
          <span className="font-mono font-medium text-foreground truncate max-w-[165px] tabular-nums">
            {selectedGame ? selectedGame.exeName : 'GLOBAL'}
          </span>
        </div>

        <div className="flex items-center gap-2 px-2.5 py-1.5 rounded-lg bg-card border border-border text-xs metric-mono shadow-xs dark:shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]">
          <Server className="w-3.5 h-3.5 text-emerald-500 dark:text-emerald-400 shrink-0" />
          <span className="text-foreground font-medium">{selectedHost}</span>
        </div>

        {/* Live Session Pill */}
        <div
          className={`hidden 2xl:flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[10px] font-mono font-semibold uppercase tracking-[0.06em] border transition-colors duration-150 ${
            isBoosted
              ? 'bg-emerald-500/12 border-emerald-500/30 text-emerald-600 dark:text-emerald-400'
              : 'bg-secondary border-border text-muted-foreground'
          }`}
        >
          <span className={`w-1.5 h-1.5 rounded-full ${isBoosted ? 'bg-emerald-500 animate-status-beacon' : 'bg-muted-foreground/60'}`} />
          {isBoosted ? 'DSCP 46 • ACTIF' : 'STANDBY'}
        </div>
      </div>

      {/* Right: Tactical Status Pills & Controls */}
      <div className="flex items-center gap-2 shrink-0">
        {/* Tunnel EU Pill */}
        <div
          className={`hidden xl:flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-mono font-medium border transition-colors duration-150 ${
            isTunnelActive
              ? 'bg-sky-500/12 border-sky-500/30 text-sky-600 dark:text-sky-400'
              : 'bg-card border-border text-muted-foreground'
          }`}
          title={isTunnelActive ? `Tunnel actif : ${tunnelState?.endpoint}` : 'Tunnel EU Split inactif'}
        >
          <Route className={`w-3.5 h-3.5 ${isTunnelActive ? 'text-sky-500 dark:text-sky-400' : 'text-muted-foreground'}`} />
          <span>{isTunnelActive ? 'TUNNEL EU' : 'TUNNEL OFF'}</span>
        </div>

        {/* Admin UAC Badge / Action */}
        {isAdmin ? (
          <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-emerald-500/12 border border-emerald-500/30 text-emerald-600 dark:text-emerald-400 text-[11px] font-mono font-semibold">
            <Shield className="w-3.5 h-3.5" />
            ADMIN UAC
          </div>
        ) : (
          <button
            onClick={onRelaunchAdmin}
            title="Relancer NetBoost avec élévation Administrateur Windows (UAC) pour appliquer QoS, DNS, Registre, Tunnel et Pare-feu"
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-amber-500/12 hover:bg-amber-500/20 border border-amber-500/30 text-amber-600 dark:text-amber-400 text-xs font-medium transition-colors duration-150 cursor-pointer"
          >
            <ShieldAlert className="w-3.5 h-3.5" />
            Relancer en Admin
          </button>
        )}

        {/* Mandatory Rollback Button */}
        <button
          onClick={onRollback}
          disabled={busyRollback || boostState?.busy}
          className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors duration-150 cursor-pointer border disabled:opacity-50 ${
            hasBackup || isBoosted || isTunnelActive
              ? 'bg-destructive/12 hover:bg-destructive/20 border-destructive/35 text-destructive'
              : 'bg-card hover:bg-secondary border-border text-muted-foreground hover:text-foreground'
          }`}
        >
          {busyRollback ? (
            <AlertTriangle className="w-3.5 h-3.5 animate-spin" />
          ) : (
            <RotateCcw className="w-3.5 h-3.5" />
          )}
          <span>Rollback</span>
          {(hasBackup || isBoosted) && (
            <span className="w-1.5 h-1.5 rounded-full bg-destructive animate-status-beacon" />
          )}
        </button>

        {/* Theme Toggle (Dark / Light) */}
        <button
          onClick={onToggleTheme}
          title={theme === 'dark' ? 'Passer en mode clair' : 'Passer en mode sombre'}
          className="flex items-center justify-center w-8 h-8 rounded-lg bg-card hover:bg-secondary border border-border text-muted-foreground hover:text-foreground transition-colors duration-150 cursor-pointer"
        >
          {theme === 'dark' ? <Sun className="w-3.5 h-3.5" /> : <Moon className="w-3.5 h-3.5" />}
        </button>

        {/* System Tray Minimize Button */}
        <button
          onClick={onMinimizeToTray}
          title="Réduire dans la barre des tâches (System Tray) pour minimiser l'empreinte RAM/CPU en jeu"
          className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-card hover:bg-secondary border border-border text-muted-foreground hover:text-foreground text-xs font-mono font-medium transition-colors duration-150 cursor-pointer"
        >
          <Minimize2 className="w-3.5 h-3.5" />
          <span className="hidden 2xl:inline">Tray</span>
        </button>
      </div>
    </header>
  );
};
