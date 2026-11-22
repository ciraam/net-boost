import React, { useState } from 'react';
import {
  FileJson,
  RefreshCw,
  RotateCcw,
  Terminal,
  Trash2,
} from 'lucide-react';
import type { BackupState, CommandLogEntry, LogCategory } from '../types/ipc';

interface ConsoleBackupViewProps {
  logs: CommandLogEntry[];
  backupJson: BackupState | null;
  backupPath: string;
  busyRollback: boolean;
  onClearLogs: () => void;
  onRefreshBackup: () => void;
  onRollback: () => void;
}

const CATEGORIES: Array<{ id: 'all' | LogCategory; label: string }> = [
  { id: 'all', label: 'Tous' },
  { id: 'tunnel', label: 'Tunnel EU' },
  { id: 'inspector', label: 'Inspecteur' },
  { id: 'firewall', label: 'Pare-feu' },
  { id: 'qos', label: 'QoS' },
  { id: 'dns', label: 'DNS' },
  { id: 'tcpip', label: 'TCP/IP' },
  { id: 'adapter', label: 'Adaptateur' },
  { id: 'background', label: 'Arrière-plan' },
  { id: 'rollback', label: 'Rollback' },
  { id: 'uac', label: 'UAC' },
  { id: 'monitor', label: 'Monitoring' },
];

export const ConsoleBackupView: React.FC<ConsoleBackupViewProps> = ({
  logs,
  backupJson,
  backupPath,
  busyRollback,
  onClearLogs,
  onRefreshBackup,
  onRollback,
}) => {
  const [filter, setFilter] = useState<'all' | LogCategory>('all');

  const filtered = [...logs]
    .filter((entry) => filter === 'all' || entry.category === filter)
    .reverse();

  return (
    <div className="grid grid-cols-12 gap-4">
      {/* Left: Full Command Console */}
      <div className="col-span-12 xl:col-span-8 rounded-2xl bg-bg-panel border border-border-subtle p-5 flex flex-col">
        <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
          <div className="flex items-center gap-2">
            <Terminal className="w-4 h-4 text-neon-green" />
            <h2 className="text-sm font-bold text-text-primary">
              Console des Commandes Exécutées ({filtered.length})
            </h2>
          </div>

          <button
            onClick={onClearLogs}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-bg-surface hover:bg-bg-elevated border border-border-strong text-xs text-text-secondary hover:text-neon-red cursor-pointer"
          >
            <Trash2 className="w-3.5 h-3.5" />
            Effacer
          </button>
        </div>

        {/* Category filter pills */}
        <div className="flex flex-wrap gap-1.5 mb-3">
          {CATEGORIES.map((cat) => (
            <button
              key={cat.id}
              onClick={() => setFilter(cat.id)}
              className={`px-2.5 py-1 rounded-lg text-[11px] font-semibold border cursor-pointer transition-colors ${
                filter === cat.id
                  ? 'bg-neon-cyan/15 border-neon-cyan/50 text-neon-cyan'
                  : 'bg-bg-surface border-border-subtle text-text-muted hover:text-text-secondary'
              }`}
            >
              {cat.label}
            </button>
          ))}
        </div>

        {/* Log Stream */}
        <div className="flex-1 rounded-xl bg-bg-void border border-border-subtle p-3.5 font-mono text-[11px] space-y-2.5 max-h-[540px] overflow-y-auto">
          {filtered.length === 0 ? (
            <div className="text-text-muted py-6 text-center">Aucune commande dans cette catégorie.</div>
          ) : (
            filtered.map((entry) => (
              <div key={entry.id} className="p-2.5 rounded-lg bg-bg-surface/70 border border-border-subtle/70 space-y-1">
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2 min-w-0">
                    <span className="text-text-muted shrink-0">[{entry.timestamp}]</span>
                    <span
                      className={`px-1.5 py-0.5 rounded text-[10px] font-bold uppercase shrink-0 ${
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
                    <span className="text-text-primary font-semibold break-all">{entry.command}</span>
                  </div>
                  <span className="text-[10px] text-text-muted shrink-0">
                    {entry.exitCode !== null ? `code=${entry.exitCode} • ${entry.durationMs}ms` : 'en cours...'}
                  </span>
                </div>

                {entry.stdout && (
                  <pre className="text-[11px] text-neon-green/90 whitespace-pre-wrap break-all pl-2 border-l-2 border-neon-green/40">
                    {entry.stdout}
                  </pre>
                )}
                {entry.stderr && (
                  <pre className="text-[11px] text-neon-red whitespace-pre-wrap break-all pl-2 border-l-2 border-neon-red/50">
                    {entry.stderr}
                  </pre>
                )}
              </div>
            ))
          )}
        </div>
      </div>

      {/* Right: JSON Backup & Rollback Inspector */}
      <div className="col-span-12 xl:col-span-4 rounded-2xl bg-bg-panel border border-border-subtle p-5 flex flex-col justify-between gap-4">
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <FileJson className="w-4 h-4 text-neon-cyan" />
              <h2 className="text-sm font-bold text-text-primary">Sauvegarde JSON (Rollback)</h2>
            </div>
            <button
              onClick={onRefreshBackup}
              className="p-1.5 rounded-lg bg-bg-surface border border-border-subtle text-text-muted hover:text-neon-cyan cursor-pointer"
              title="Recharger backup-state.json"
            >
              <RefreshCw className="w-3.5 h-3.5" />
            </button>
          </div>

          <div className="text-[11px] font-mono text-text-muted break-all p-2.5 rounded-lg bg-bg-surface border border-border-subtle">
            {backupPath || 'backup-state.json'}
          </div>

          {backupJson ? (
            <pre className="rounded-xl bg-bg-void border border-neon-green/30 p-3 font-mono text-[11px] text-text-secondary max-h-[380px] overflow-auto">
              {JSON.stringify(backupJson, null, 2)}
            </pre>
          ) : (
            <div className="p-6 rounded-xl bg-bg-surface border border-dashed border-border-subtle text-center text-xs text-text-muted">
              Aucune sauvegarde active : votre configuration réseau Windows n&apos;a pas été modifiée. Dès le premier clic sur <span className="text-neon-cyan font-semibold">BOOST</span> ou bridage, l&apos;état exact sera enregistré ici avant toute écriture.
            </div>
          )}
        </div>

        <button
          onClick={onRollback}
          disabled={busyRollback}
          className="w-full flex items-center justify-center gap-2 py-3 rounded-xl bg-neon-red/15 hover:bg-neon-red/25 border border-neon-red/50 text-neon-red text-xs font-bold cursor-pointer transition-colors disabled:opacity-50"
        >
          <RotateCcw className="w-4 h-4" />
          Restaurer la config d&apos;origine (Rollback)
        </button>
      </div>
    </div>
  );
};
