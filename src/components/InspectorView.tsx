import React, { useState } from 'react';
import {
  AlertTriangle,
  Ban,
  Database,
  Globe,
  Loader2,
  Radar,
  Radio,
  RefreshCw,
  Shield,
  ShieldAlert,
  ShieldCheck,
  Trash2,
} from 'lucide-react';
import type { GameProfile, InspectorSnapshot } from '../types/ipc';

interface InspectorViewProps {
  selectedGame: GameProfile | null;
  snapshot: InspectorSnapshot | null;
  scanning: boolean;
  capturingEtw: boolean;
  inspectAll: boolean;
  allowHttpsFallback: boolean;
  busyIp: string | null;
  isAdmin: boolean;
  onToggleScope: (all: boolean) => void;
  onToggleHttpsFallback: (allow: boolean) => void;
  onScan: () => void;
  onCaptureUdpEtw: () => void;
  onBlockIp: (params: {
    ipOrCidr: string;
    exeName?: string | null;
    exePath?: string | null;
    countryCode?: string | null;
  }) => void;
  onUnblockRule: (ruleName: string) => void;
}

function toSlash24(ip: string): string {
  const parts = ip.trim().split('.');
  if (parts.length !== 4) return ip.trim();
  return `${parts[0]}.${parts[1]}.${parts[2]}.0/24`;
}

export const InspectorView: React.FC<InspectorViewProps> = ({
  selectedGame,
  snapshot,
  scanning,
  capturingEtw,
  inspectAll,
  allowHttpsFallback,
  busyIp,
  isAdmin,
  onToggleScope,
  onToggleHttpsFallback,
  onScan,
  onCaptureUdpEtw,
  onBlockIp,
  onUnblockRule,
}) => {
  const [manualCidr, setManualCidr] = useState('');
  const [onlyNonEu, setOnlyNonEu] = useState(false);

  const connections = snapshot?.connections ?? [];
  const blockedRules = snapshot?.blockedRules ?? [];
  const filteredConns = onlyNonEu
    ? connections.filter((c) => !c.geo.isPrivate && !c.geo.isEu)
    : connections;

  const euCount = connections.filter((c) => !c.geo.isPrivate && c.geo.isEu).length;
  const nonEuCount = snapshot?.nonEuCount ?? 0;

  const handleManualBlock = (e: React.FormEvent) => {
    e.preventDefault();
    const clean = manualCidr.trim();
    if (!clean) return;
    onBlockIp({
      ipOrCidr: clean,
      exeName: selectedGame?.exeName ?? null,
      exePath: selectedGame?.exePath ?? null,
      countryCode: 'MANUEL',
    });
    setManualCidr('');
  };

  return (
    <div className="space-y-4">
      {!isAdmin && (
        <div className="p-3.5 rounded-xl bg-neon-amber/10 border border-neon-amber/40 flex items-center gap-3 text-xs text-neon-amber">
          <ShieldAlert className="w-4 h-4 shrink-0" />
          <span>
            Mode standard : l&apos;inspection TCP + GeoIP hors-ligne (0 ms) et le fallback <span className="font-mono">Get-NetUDPEndpoint</span> fonctionnent en direct. Relancez en Administrateur (UAC) pour activer la capture ETW des flux UDP distants du match et le blocage Pare-feu Windows.
          </span>
        </div>
      )}

      {/* Header & KPI Bar */}
      <div className="tactical-card rounded-2xl p-5 space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 className="text-sm font-bold text-text-primary flex items-center gap-2">
              <Radar className="w-4 h-4 text-neon-cyan" />
              Inspecteur de Paquets &amp; IP Distantes en Direct
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-neon-green/15 border border-neon-green/35 text-neon-green text-[10px] font-bold">
                <Database className="w-3 h-3" />
                GeoIP Hors-Ligne (0 ms)
              </span>
            </h2>
            <p className="text-xs text-text-muted mt-0.5">
              Base GeoIP locale intégrée (aucun rate-limit, aucune fuite d&apos;IP vers un tiers) + capture ETW Windows native (<span className="font-mono">Microsoft-Windows-Kernel-Network</span>) des flux UDP distants du jeu.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <div className="flex rounded-lg bg-bg-surface border border-border-subtle p-0.5 text-xs">
              <button
                onClick={() => onToggleScope(false)}
                disabled={!selectedGame}
                title={selectedGame ? `Filtrer sur ${selectedGame.exeName}` : 'Sélectionnez d\'abord un jeu .exe'}
                className={`px-2.5 py-1 rounded-md font-semibold cursor-pointer disabled:opacity-40 ${
                  !inspectAll && selectedGame ? 'bg-neon-cyan/15 text-neon-cyan' : 'text-text-muted'
                }`}
              >
                {selectedGame ? selectedGame.exeName : 'Jeu (.exe)'}
              </button>
              <button
                onClick={() => onToggleScope(true)}
                className={`px-2.5 py-1 rounded-md font-semibold cursor-pointer ${
                  inspectAll || !selectedGame ? 'bg-neon-cyan/15 text-neon-cyan' : 'text-text-muted'
                }`}
              >
                Toutes les connexions
              </button>
            </div>

            <button
              onClick={onCaptureUdpEtw}
              disabled={!selectedGame || capturingEtw || scanning}
              title={
                !selectedGame
                  ? 'Sélectionnez d\'abord un jeu (.exe) pour capturer ses flux UDP distants'
                  : !isAdmin
                    ? 'Droits Administrateur (UAC) requis pour la capture ETW Windows'
                    : `Capture ETW légère de 2.5s des paquets UDP Send/Recv de ${selectedGame.exeName}`
              }
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-neon-purple/15 hover:bg-neon-purple/25 border border-neon-purple/45 text-neon-purple text-xs font-bold cursor-pointer disabled:opacity-40 transition-colors"
            >
              {capturingEtw ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Radio className="w-3.5 h-3.5" />}
              {capturingEtw ? 'Capture ETW (2.5s)...' : 'Capturer UDP Match (ETW 2.5s)'}
            </button>

            <button
              onClick={onScan}
              disabled={scanning || capturingEtw}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-neon-cyan/15 hover:bg-neon-cyan/25 border border-neon-cyan/40 text-neon-cyan text-xs font-bold cursor-pointer disabled:opacity-50"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${scanning ? 'animate-spin' : ''}`} />
              Scanner {snapshot?.scannedAt ? `(${snapshot.scannedAt})` : ''}
            </button>
          </div>
        </div>

        {/* ETW capture status / technical note */}
        <div className="px-3.5 py-2.5 rounded-xl bg-bg-surface border border-neon-cyan/25 text-[11px] text-text-secondary leading-relaxed flex flex-wrap items-center justify-between gap-2">
          <div>
            <span className="text-neon-cyan font-bold">Flux UDP du jeu : </span>
            {snapshot?.etwUdpCaptured ? (
              <span>
                Capture ETW terminée : <span className="text-neon-purple font-bold">{snapshot.etwUdpCount ?? 0} flux UDP distant(s)</span> fusionné(s) avec les connexions TCP.
                {snapshot.etwNote ? ` ${snapshot.etwNote}` : ''}
              </span>
            ) : (
              <span>
                En scan rapide, <span className="font-mono text-text-primary">Get-NetUDPEndpoint</span> affiche uniquement les sockets UDP locaux (fallback sans Admin). En Administrateur, cliquez sur <span className="text-neon-purple font-semibold">Capturer UDP Match (ETW 2.5s)</span> pour extraire les vraies IP/ports distants UDP du serveur de jeu en cours de match.
                {snapshot?.etwNote ? ` (${snapshot.etwNote})` : ''}
              </span>
            )}
          </div>
        </div>

        {/* Summary KPI Strip */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3 font-mono">
          <div className="p-3 rounded-xl bg-bg-surface border border-border-subtle">
            <div className="text-[10px] font-sans uppercase text-text-muted">Connexions Actives</div>
            <div className="text-xl font-black text-text-primary mt-0.5">{connections.length}</div>
          </div>
          <div className="p-3 rounded-xl bg-bg-surface border border-border-subtle">
            <div className="text-[10px] font-sans uppercase text-text-muted">Routes Europe (UE)</div>
            <div className="text-xl font-black text-neon-green mt-0.5">{euCount}</div>
          </div>
          <div
            className={`p-3 rounded-xl border ${
              nonEuCount > 0 ? 'bg-neon-red/10 border-neon-red/40' : 'bg-bg-surface border-border-subtle'
            }`}
          >
            <div className="text-[10px] font-sans uppercase text-text-muted flex items-center gap-1">
              {nonEuCount > 0 && <AlertTriangle className="w-3 h-3 text-neon-red" />}
              Hors UE (Détours potentiels)
            </div>
            <div className={`text-xl font-black mt-0.5 ${nonEuCount > 0 ? 'text-neon-red' : 'text-text-secondary'}`}>
              {nonEuCount}
            </div>
          </div>
          <div className="p-3 rounded-xl bg-bg-surface border border-border-subtle">
            <div className="text-[10px] font-sans uppercase text-text-muted">Règles Pare-feu Actives</div>
            <div className="text-xl font-black text-neon-amber mt-0.5">{blockedRules.length}</div>
          </div>
        </div>

        {/* Filters + Quick Manual Block */}
        <div className="flex flex-wrap items-center justify-between gap-3 pt-1">
          <div className="flex flex-wrap items-center gap-4">
            <label className="flex items-center gap-2 text-xs text-text-secondary cursor-pointer select-none">
              <input
                type="checkbox"
                checked={onlyNonEu}
                onChange={(e) => setOnlyNonEu(e.target.checked)}
                className="accent-neon-red rounded"
              />
              Uniquement <span className="text-neon-red font-bold">HORS UE</span>
            </label>

            <label
              className="flex items-center gap-2 text-xs text-text-secondary cursor-pointer select-none"
              title="Par défaut 100% hors-ligne (zéro fuite d'IP). Activez uniquement si une IP est absente de la base locale."
            >
              <input
                type="checkbox"
                checked={allowHttpsFallback}
                onChange={(e) => onToggleHttpsFallback(e.target.checked)}
                className="accent-neon-cyan rounded"
              />
              Fallback HTTPS si IP inconnue
            </label>
          </div>

          <form onSubmit={handleManualBlock} className="flex items-center gap-2">
            <input
              type="text"
              value={manualCidr}
              onChange={(e) => setManualCidr(e.target.value)}
              placeholder="Bloquer IP ou plage (ex: 121.254.0.0/16)"
              className="w-64 bg-bg-surface border border-border-strong rounded-lg px-3 py-1.5 text-xs font-mono text-text-primary focus:outline-none focus:border-neon-red"
            />
            <button
              type="submit"
              disabled={!manualCidr.trim() || busyIp !== null}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-neon-red/15 hover:bg-neon-red/25 border border-neon-red/40 text-neon-red text-xs font-bold cursor-pointer disabled:opacity-40"
            >
              <Ban className="w-3.5 h-3.5" />
              Bloquer (Pare-feu)
            </button>
          </form>
        </div>

        {/* Live Connections Table */}
        <div className="overflow-x-auto rounded-xl border border-border-subtle max-h-[360px] overflow-y-auto">
          <table className="w-full text-left border-collapse text-xs">
            <thead className="sticky top-0 z-10">
              <tr className="bg-bg-surface text-[11px] uppercase tracking-wider text-text-muted border-b border-border-subtle">
                <th className="py-2.5 px-3">Proto / État</th>
                <th className="py-2.5 px-3">Processus (.exe)</th>
                <th className="py-2.5 px-3">IP Distante : Port</th>
                <th className="py-2.5 px-3">GeoIP (Pays / Ville)</th>
                <th className="py-2.5 px-3">Source / Info</th>
                <th className="py-2.5 px-3 text-right">Pare-feu Windows</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border-subtle/70">
              {filteredConns.length === 0 ? (
                <tr>
                  <td colSpan={6} className="py-8 text-center text-text-muted">
                    {scanning || capturingEtw
                      ? 'Analyse des connexions réseau en cours...'
                      : !inspectAll && selectedGame
                        ? `Aucune connexion distante active trouvée pour ${selectedGame.exeName}. Lancez le jeu, lancez une capture ETW UDP, ou basculez sur « Toutes les connexions ».`
                        : 'Aucune connexion correspondant au filtre.'}
                  </td>
                </tr>
              ) : (
                filteredConns.map((conn) => {
                  const isIpv4 = /^(\d{1,3}\.){3}\d{1,3}$/.test(conn.remoteAddress);
                  const cidr24 = isIpv4 ? toSlash24(conn.remoteAddress) : null;
                  const isBusyExact = busyIp === conn.remoteAddress;
                  const isBusyRange = cidr24 ? busyIp === cidr24 : false;

                  return (
                    <tr
                      key={conn.id}
                      className={`hover:bg-bg-surface/70 transition-colors ${
                        !conn.geo.isPrivate && !conn.geo.isEu ? 'bg-neon-red/[0.04]' : ''
                      }`}
                    >
                      <td className="py-2 px-3 font-mono">
                        <span
                          className={`px-1.5 py-0.5 rounded text-[10px] font-bold ${
                            conn.protocol === 'UDP'
                              ? 'bg-neon-purple/20 text-neon-purple'
                              : 'bg-neon-cyan/15 text-neon-cyan'
                          }`}
                        >
                          {conn.protocol}
                        </span>
                        <div className="text-[10px] text-text-muted mt-0.5">{conn.state}</div>
                      </td>
                      <td className="py-2 px-3">
                        <div className="font-mono font-semibold text-text-primary">{conn.exeName}</div>
                        <div className="text-[10px] text-text-muted font-mono">PID {conn.pid}</div>
                      </td>
                      <td className="py-2 px-3 font-mono">
                        <div className="text-text-primary font-semibold">
                          {conn.remoteAddress}
                          {conn.remotePort > 0 ? `:${conn.remotePort}` : ''}
                        </div>
                        <div className="text-[10px] text-text-muted">local :{conn.localPort}</div>
                      </td>
                      <td className="py-2 px-3">
                        <div className="flex items-center gap-1.5">
                          <Globe
                            className={`w-3.5 h-3.5 shrink-0 ${
                              conn.geo.isPrivate
                                ? 'text-text-muted'
                                : conn.geo.isEu
                                  ? 'text-neon-green'
                                  : 'text-neon-red'
                            }`}
                          />
                          <span className="font-semibold text-text-primary">
                            {conn.geo.country} ({conn.geo.countryCode})
                          </span>
                          {!conn.geo.isPrivate && (
                            <span
                              className={`px-1.5 py-0.5 rounded text-[9px] font-black uppercase ${
                                conn.geo.isEu
                                  ? 'bg-neon-green/15 text-neon-green'
                                  : 'bg-neon-red/20 text-neon-red'
                              }`}
                            >
                              {conn.geo.isEu ? 'UE' : 'HORS UE'}
                            </span>
                          )}
                        </div>
                        <div className="text-[11px] text-text-muted">{conn.geo.city}</div>
                      </td>
                      <td className="py-2 px-3 text-[11px] text-text-secondary max-w-[180px] truncate" title={conn.geo.as || conn.geo.isp}>
                        <span className="text-text-secondary">{conn.geo.isp}</span>
                        {conn.geo.as && <div className="text-[10px] font-mono text-text-muted">{conn.geo.as}</div>}
                      </td>
                      <td className="py-2 px-3 text-right">
                        {conn.geo.isPrivate || conn.remoteAddress === '*' ? (
                          <span className="text-[10px] text-text-muted">—</span>
                        ) : conn.isBlocked ? (
                          <span className="inline-flex items-center gap-1 px-2 py-1 rounded-lg bg-neon-red/15 text-neon-red text-[10px] font-bold">
                            <Ban className="w-3 h-3" />
                            BLOQUÉ
                          </span>
                        ) : (
                          <div className="inline-flex items-center gap-1.5">
                            <button
                              onClick={() =>
                                onBlockIp({
                                  ipOrCidr: conn.remoteAddress,
                                  exeName: conn.exeName,
                                  exePath: conn.exePath,
                                  countryCode: conn.geo.countryCode,
                                })
                              }
                              disabled={isBusyExact || isBusyRange}
                              className="inline-flex items-center gap-1 px-2 py-1 rounded-lg bg-neon-red/15 hover:bg-neon-red/25 border border-neon-red/40 text-neon-red text-[10px] font-bold cursor-pointer disabled:opacity-40"
                              title={`Bloquer l'IP exacte ${conn.remoteAddress}`}
                            >
                              {isBusyExact ? <Loader2 className="w-3 h-3 animate-spin" /> : <Ban className="w-3 h-3" />}
                              IP
                            </button>
                            {cidr24 && (
                              <button
                                onClick={() =>
                                  onBlockIp({
                                    ipOrCidr: cidr24,
                                    exeName: conn.exeName,
                                    exePath: conn.exePath,
                                    countryCode: conn.geo.countryCode,
                                  })
                                }
                                disabled={isBusyExact || isBusyRange}
                                className="inline-flex items-center gap-1 px-2 py-1 rounded-lg bg-neon-amber/15 hover:bg-neon-amber/25 border border-neon-amber/40 text-neon-amber text-[10px] font-bold cursor-pointer disabled:opacity-40"
                                title={`Bloquer toute la plage ${cidr24}`}
                              >
                                {isBusyRange ? <Loader2 className="w-3 h-3 animate-spin" /> : <Shield className="w-3 h-3" />}
                                /24
                              </button>
                            )}
                          </div>
                        )}
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Active Windows Firewall Block Rules Created by NetBoost */}
      <div className="tactical-card rounded-2xl p-5 space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <ShieldCheck className="w-4 h-4 text-neon-amber" />
            <h3 className="text-sm font-bold text-text-primary">
              Règles Pare-feu Windows Actives ({blockedRules.length})
            </h3>
          </div>
          <span className="text-[11px] text-text-muted">
            Toutes les règles <span className="font-mono text-neon-cyan">NetBoost_Block_*</span> sont supprimées automatiquement au Rollback et à la fermeture.
          </span>
        </div>

        {blockedRules.length === 0 ? (
          <div className="p-4 rounded-xl bg-bg-surface border border-dashed border-border-subtle text-center text-xs text-text-muted">
            Aucune règle de blocage active. Cliquez sur <span className="text-neon-red font-semibold">IP</span> ou <span className="text-neon-amber font-semibold">/24</span> sur une route hors UE pour bloquer ce serveur via <span className="font-mono">New-NetFirewallRule</span>.
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-2.5">
            {blockedRules.map((rule) => (
              <div
                key={rule.ruleName}
                className="p-3 rounded-xl bg-bg-surface border border-neon-red/35 flex items-center justify-between gap-2"
              >
                <div className="min-w-0">
                  <div className="font-mono text-xs font-bold text-neon-red truncate">{rule.remoteAddress}</div>
                  <div className="text-[11px] text-text-muted truncate">
                    {rule.exeName ? `App: ${rule.exeName}` : 'Toutes applis'}
                    {rule.countryCode ? ` • ${rule.countryCode}` : ''}
                  </div>
                </div>
                <button
                  onClick={() => onUnblockRule(rule.ruleName)}
                  className="shrink-0 flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-bg-elevated hover:bg-neon-green/15 border border-border-strong hover:border-neon-green/40 text-text-secondary hover:text-neon-green text-[11px] font-semibold cursor-pointer"
                  title="Supprimer cette règle Pare-feu"
                >
                  <Trash2 className="w-3 h-3" />
                  Débloquer
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
