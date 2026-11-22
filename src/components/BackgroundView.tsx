import React from 'react';
import {
  Cpu,
  DownloadCloud,
  Gauge,
  Loader2,
  Pause,
  Play,
  RefreshCw,
  ShieldAlert,
} from 'lucide-react';
import type { BackgroundSnapshot } from '../types/ipc';

interface BackgroundViewProps {
  snapshot: BackgroundSnapshot | null;
  scanning: boolean;
  busyKey: string | null;
  isAdmin: boolean;
  onScan: () => void;
  onToggleService: (serviceName: string, pause: boolean) => void;
  onToggleAppThrottle: (exeName: string, throttle: boolean) => void;
}

const CATEGORY_LABELS: Record<string, { label: string; color: string }> = {
  browser: { label: 'Navigateur', color: 'text-neon-cyan bg-neon-cyan/15' },
  launcher: { label: 'Launcher', color: 'text-neon-purple bg-neon-purple/15' },
  updater: { label: 'Sync / MAJ', color: 'text-neon-amber bg-neon-amber/15' },
  other: { label: 'Réseau actif', color: 'text-text-secondary bg-bg-elevated' },
};

export const BackgroundView: React.FC<BackgroundViewProps> = ({
  snapshot,
  scanning,
  busyKey,
  isAdmin,
  onScan,
  onToggleService,
  onToggleAppThrottle,
}) => {
  const services = snapshot?.services ?? [];
  const apps = snapshot?.apps ?? [];

  return (
    <div className="space-y-4">
      {!isAdmin && (
        <div className="p-3.5 rounded-xl bg-neon-amber/10 border border-neon-amber/40 flex items-center gap-3 text-xs text-neon-amber">
          <ShieldAlert className="w-4 h-4 shrink-0" />
          <span>
            Mode standard détecté : la lecture des processus/services est réelle, mais la mise en pause de Windows Update et le bridage QoS (DSCP 8) nécessitent de relancer l&apos;application en Administrateur (UAC).
          </span>
        </div>
      )}

      {/* Windows Update & Background Services */}
      <div className="rounded-2xl bg-bg-panel border border-border-subtle p-5 space-y-4">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-sm font-bold text-text-primary flex items-center gap-2">
              <DownloadCloud className="w-4 h-4 text-neon-amber" />
              Services Windows Gourmands (Windows Update / BITS / Delivery Optimization)
            </h2>
            <p className="text-xs text-text-muted mt-0.5">
              Mettez en pause les téléchargements Windows pendant la session de jeu. L&apos;état initial est sauvegardé dans le JSON de Rollback.
            </p>
          </div>
          <button
            onClick={onScan}
            disabled={scanning}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-bg-surface hover:bg-bg-elevated border border-border-strong text-xs font-semibold text-text-secondary hover:text-neon-cyan cursor-pointer"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${scanning ? 'animate-spin text-neon-cyan' : ''}`} />
            Actualiser {snapshot?.scannedAt ? `(${snapshot.scannedAt})` : ''}
          </button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
          {services.length === 0 ? (
            <div className="col-span-3 py-6 text-center text-xs text-text-muted rounded-xl bg-bg-surface border border-dashed border-border-subtle">
              Scan des services Windows en cours...
            </div>
          ) : (
            services.map((svc) => {
              const isRunning = svc.status.toLowerCase() === 'running';
              const isBusy = busyKey === `svc:${svc.name}`;
              return (
                <div
                  key={svc.name}
                  className="p-4 rounded-xl bg-bg-surface border border-border-subtle flex flex-col justify-between gap-3"
                >
                  <div>
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-mono text-xs font-bold text-neon-cyan">{svc.name}</span>
                      <span
                        className={`text-[10px] font-bold uppercase px-2 py-0.5 rounded-full ${
                          isRunning
                            ? 'bg-neon-amber/15 text-neon-amber'
                            : 'bg-neon-green/15 text-neon-green'
                        }`}
                      >
                        {svc.status}
                      </span>
                    </div>
                    <div className="text-xs font-semibold text-text-primary mt-1">{svc.displayName}</div>
                    <div className="text-[11px] text-text-muted mt-0.5">Démarrage : {svc.startType}</div>
                  </div>

                  <button
                    onClick={() => onToggleService(svc.name, isRunning)}
                    disabled={isBusy}
                    className={`w-full flex items-center justify-center gap-1.5 py-2 rounded-lg text-xs font-bold border transition-colors cursor-pointer disabled:opacity-50 ${
                      isRunning
                        ? 'bg-neon-amber/15 hover:bg-neon-amber/25 border-neon-amber/40 text-neon-amber'
                        : 'bg-neon-green/15 hover:bg-neon-green/25 border-neon-green/40 text-neon-green'
                    }`}
                  >
                    {isBusy ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin" />
                    ) : isRunning ? (
                      <Pause className="w-3.5 h-3.5" />
                    ) : (
                      <Play className="w-3.5 h-3.5" />
                    )}
                    {isRunning ? 'Mettre en pause' : 'Reprendre le service'}
                  </button>
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* Background Heavy Applications (Browsers, Launchers, Updaters) */}
      <div className="rounded-2xl bg-bg-panel border border-border-subtle p-5 space-y-4">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-sm font-bold text-text-primary flex items-center gap-2">
              <Cpu className="w-4 h-4 text-neon-cyan" />
              Processus Arrière-plan Actifs (Navigateurs, Launchers, Connexions TCP)
            </h2>
            <p className="text-xs text-text-muted mt-0.5">
              Applique une véritable politique Windows <span className="font-mono text-neon-cyan">New-NetQosPolicy -DSCPAction 8</span> (basse priorité) sans fermer l&apos;application.
            </p>
          </div>
          <span className="text-xs font-mono text-text-muted">{apps.length} app(s) détectée(s)</span>
        </div>

        <div className="overflow-x-auto rounded-xl border border-border-subtle">
          <table className="w-full text-left border-collapse text-xs">
            <thead>
              <tr className="bg-bg-surface text-[11px] uppercase tracking-wider text-text-muted border-b border-border-subtle">
                <th className="py-2.5 px-3">Application</th>
                <th className="py-2.5 px-3">Catégorie</th>
                <th className="py-2.5 px-3 text-right">Instances</th>
                <th className="py-2.5 px-3 text-right">RAM</th>
                <th className="py-2.5 px-3 text-right">Connexions TCP</th>
                <th className="py-2.5 px-3 text-right">Action QoS</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border-subtle/70">
              {apps.length === 0 ? (
                <tr>
                  <td colSpan={6} className="py-8 text-center text-text-muted">
                    Aucune application arrière-plan gourmande détectée. Cliquez sur « Actualiser ».
                  </td>
                </tr>
              ) : (
                apps.map((app) => {
                  const cat = CATEGORY_LABELS[app.category] ?? CATEGORY_LABELS.other;
                  const isBusy = busyKey === `app:${app.exeName}`;
                  return (
                    <tr key={app.exeName} className="hover:bg-bg-surface/60 transition-colors">
                      <td className="py-2.5 px-3">
                        <div className="font-semibold text-text-primary">{app.displayName}</div>
                        <div className="font-mono text-[11px] text-text-muted">{app.exeName}</div>
                      </td>
                      <td className="py-2.5 px-3">
                        <span className={`px-2 py-0.5 rounded text-[10px] font-bold uppercase ${cat.color}`}>
                          {cat.label}
                        </span>
                      </td>
                      <td className="py-2.5 px-3 text-right font-mono text-text-secondary">{app.instanceCount}</td>
                      <td className="py-2.5 px-3 text-right font-mono text-text-secondary">{app.totalMemoryMb} Mo</td>
                      <td className="py-2.5 px-3 text-right font-mono">
                        <span className={app.tcpConnections > 10 ? 'text-neon-amber font-bold' : 'text-text-secondary'}>
                          {app.tcpConnections}
                        </span>
                      </td>
                      <td className="py-2.5 px-3 text-right">
                        <button
                          onClick={() => onToggleAppThrottle(app.exeName, !app.throttledByApp)}
                          disabled={isBusy}
                          className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-bold border transition-all cursor-pointer disabled:opacity-50 ${
                            app.throttledByApp
                              ? 'bg-neon-green/15 hover:bg-neon-red/15 border-neon-green/40 text-neon-green hover:text-neon-red hover:border-neon-red/40'
                              : 'bg-bg-surface hover:bg-neon-cyan/15 border-border-strong hover:border-neon-cyan/50 text-text-secondary hover:text-neon-cyan'
                          }`}
                        >
                          {isBusy ? (
                            <Loader2 className="w-3 h-3 animate-spin" />
                          ) : (
                            <Gauge className="w-3 h-3" />
                          )}
                          {app.throttledByApp ? 'Bridé DSCP 8 (Retirer)' : 'Brider (DSCP 8)'}
                        </button>
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
