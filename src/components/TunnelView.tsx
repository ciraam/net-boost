import React from 'react';
import {
  CheckCircle2,
  FileCode2,
  FolderUp,
  Loader2,
  Power,
  Route,
  ShieldAlert,
  Trash2,
  XCircle,
  Zap,
} from 'lucide-react';
import type { GameProfile, TunnelMode, TunnelState, WireGuardProfile } from '../types/ipc';

interface TunnelViewProps {
  profile: WireGuardProfile | null;
  tunnelMode: TunnelMode;
  tunnelState: TunnelState | null;
  enableTunnelOnBoost: boolean;
  selectedGame: GameProfile | null;
  selectedHost: string;
  isAdmin: boolean;
  onImportConf: () => void;
  onClearConf: () => void;
  onSelectMode: (mode: TunnelMode) => void;
  onToggleEnableOnBoost: (enabled: boolean) => void;
  onStartTunnel: () => void;
  onStopTunnel: () => void;
}

export const TunnelView: React.FC<TunnelViewProps> = ({
  profile,
  tunnelMode,
  tunnelState,
  enableTunnelOnBoost,
  selectedGame,
  selectedHost,
  isAdmin,
  onImportConf,
  onClearConf,
  onSelectMode,
  onToggleEnableOnBoost,
  onStartTunnel,
  onStopTunnel,
}) => {
  const isActive = tunnelState?.active ?? false;
  const isBusy = tunnelState?.busy ?? false;
  const hasWireSock = Boolean(tunnelState?.clients.wiresockPath);
  const hasWireGuard = Boolean(tunnelState?.clients.wireguardPath);

  return (
    <div className="space-y-4">
      {!isAdmin && (
        <div className="p-3.5 rounded-xl bg-neon-amber/10 border border-neon-amber/40 flex items-center gap-3 text-xs text-neon-amber">
          <ShieldAlert className="w-4 h-4 shrink-0" />
          <span>
            Mode standard : vous pouvez importer votre fichier <span className="font-mono">.conf</span> WireGuard maintenant, mais l&apos;activation réelle du tunnel réseau WireSock / WireGuard requiert les droits Administrateur (UAC).
          </span>
        </div>
      )}

      <div className="grid grid-cols-12 gap-4">
        {/* Left: WireGuard .conf Importer & Relay Details */}
        <div className="col-span-12 lg:col-span-6 tactical-card rounded-2xl p-5 space-y-4">
          <div className="flex items-center justify-between gap-2">
            <div>
              <h2 className="text-sm font-bold text-text-primary flex items-center gap-2">
                <Route className="w-4 h-4 text-neon-cyan" />
                Configuration Relais EU WireGuard (.conf)
              </h2>
              <p className="text-xs text-text-muted mt-0.5">
                Importez la config <span className="font-mono">.conf</span> de votre VPS / VPN en Europe pour éviter les détours de peering FAI.
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2.5">
            <button
              onClick={onImportConf}
              disabled={isBusy}
              className="flex-1 flex items-center justify-center gap-2 py-2.5 px-4 rounded-xl bg-neon-cyan/15 hover:bg-neon-cyan/25 border border-neon-cyan/50 text-neon-cyan text-xs font-bold cursor-pointer transition-colors"
            >
              <FolderUp className="w-4 h-4" />
              {profile ? 'Remplacer le fichier .conf WireGuard' : 'Importer un fichier .conf WireGuard'}
            </button>

            {profile && (
              <button
                onClick={onClearConf}
                disabled={isBusy}
                className="p-2.5 rounded-xl bg-bg-surface hover:bg-neon-red/15 border border-border-strong hover:border-neon-red/40 text-text-muted hover:text-neon-red cursor-pointer"
                title="Supprimer le profil importé"
              >
                <Trash2 className="w-4 h-4" />
              </button>
            )}
          </div>

          {profile ? (
            <div className="rounded-xl bg-bg-surface border border-neon-cyan/30 p-4 space-y-2.5 text-xs">
              <div className="flex items-center justify-between border-b border-border-subtle pb-2">
                <span className="font-mono font-bold text-neon-cyan">{profile.fileName}</span>
                <span className="text-[10px] px-2 py-0.5 rounded bg-neon-green/15 text-neon-green font-bold">
                  {profile.isEncrypted ? 'CHIFFRÉ DPAPI WINDOWS' : 'PROFIL VALIDE'}
                </span>
              </div>
              <div className="grid grid-cols-2 gap-2 font-mono text-[11px]">
                <div>
                  <div className="text-[10px] font-sans text-text-muted">Relais Endpoint</div>
                  <div className="text-neon-green font-bold truncate" title={profile.endpoint}>
                    {profile.endpoint}
                  </div>
                </div>
                <div>
                  <div className="text-[10px] font-sans text-text-muted">Adresse Interface</div>
                  <div className="text-text-primary truncate">{profile.interfaceAddress}</div>
                </div>
                <div>
                  <div className="text-[10px] font-sans text-text-muted">DNS du Tunnel</div>
                  <div className="text-text-secondary truncate">{profile.dns}</div>
                </div>
                <div>
                  <div className="text-[10px] font-sans text-text-muted">Clé Publique Peer</div>
                  <div className="text-text-secondary truncate">{profile.publicKeyPreview}</div>
                </div>
              </div>
            </div>
          ) : (
            <div className="p-6 rounded-xl bg-bg-surface border border-dashed border-border-subtle text-center text-xs text-text-muted space-y-1.5">
              <FileCode2 className="w-6 h-6 mx-auto text-text-muted/70" />
              <div>Aucun fichier <span className="font-mono text-text-secondary">.conf</span> importé.</div>
              <div className="text-[11px]">
                La clé privée reste stockée localement et n&apos;est jamais affichée dans les logs.
              </div>
            </div>
          )}

          {/* Integration with Central BOOST button */}
          <label className="flex items-center justify-between p-3.5 rounded-xl bg-bg-surface border border-border-subtle cursor-pointer select-none">
            <div>
              <div className="text-xs font-bold text-text-primary">Activer le Tunnel EU avec le bouton BOOST</div>
              <div className="text-[11px] text-text-muted">
                Lance le Split Tunnel au clic sur BOOST et le coupe automatiquement à l&apos;arrêt / fermeture.
              </div>
            </div>
            <input
              type="checkbox"
              checked={enableTunnelOnBoost}
              onChange={(e) => onToggleEnableOnBoost(e.target.checked)}
              className="w-4 h-4 accent-neon-cyan cursor-pointer"
            />
          </label>
        </div>

        {/* Right: Split Tunneling Mode (WireSock vs WireGuard) & Controls */}
        <div className="col-span-12 lg:col-span-6 tactical-card rounded-2xl p-5 flex flex-col justify-between gap-4">
          <div className="space-y-3">
            <h2 className="text-sm font-bold text-text-primary">
              Moteur de Split Tunneling CLI (Ne router QUE le jeu)
            </h2>

            {/* Mode Cards */}
            <div className="grid grid-cols-1 gap-2.5">
              <button
                type="button"
                onClick={() => onSelectMode('wiresock')}
                disabled={isActive || isBusy}
                className={`p-3.5 rounded-xl border text-left transition-all cursor-pointer disabled:opacity-60 ${
                  tunnelMode === 'wiresock'
                    ? 'bg-neon-cyan/10 border-neon-cyan/60'
                    : 'bg-bg-surface border-border-subtle hover:border-border-strong'
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs font-bold text-text-primary">
                    WireSock CLI — Split par exécutable (<span className="font-mono text-neon-cyan">AllowedApps</span>)
                  </span>
                  {hasWireSock ? (
                    <span className="inline-flex items-center gap-1 text-[10px] font-bold text-neon-green">
                      <CheckCircle2 className="w-3.5 h-3.5" /> Signé Authenticode
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1 text-[10px] font-bold text-neon-amber">
                      <XCircle className="w-3.5 h-3.5" /> Non détecté (Program Files)
                    </span>
                  )}
                </div>
                <div className="text-[11px] text-text-muted mt-1">
                  Injecte <span className="font-mono text-text-secondary">AllowedApps = {selectedGame?.exeName ?? '<jeu>.exe'}</span> pour isoler uniquement le jeu dans le tunnel EU sans toucher à Discord/navigateurs.
                </div>
                {tunnelState?.clients.wiresockPath && (
                  <div className="text-[10px] font-mono text-text-muted mt-1 truncate">
                    {tunnelState.clients.wiresockPath}
                  </div>
                )}
              </button>

              <button
                type="button"
                onClick={() => onSelectMode('wireguard')}
                disabled={isActive || isBusy}
                className={`p-3.5 rounded-xl border text-left transition-all cursor-pointer disabled:opacity-60 ${
                  tunnelMode === 'wireguard'
                    ? 'bg-neon-cyan/10 border-neon-cyan/60'
                    : 'bg-bg-surface border-border-subtle hover:border-border-strong'
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs font-bold text-text-primary">
                    WireGuard CLI — Split par IP serveur (<span className="font-mono text-neon-cyan">AllowedIPs</span>)
                  </span>
                  {hasWireGuard ? (
                    <span className="inline-flex items-center gap-1 text-[10px] font-bold text-neon-green">
                      <CheckCircle2 className="w-3.5 h-3.5" /> Signé Authenticode
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1 text-[10px] font-bold text-neon-amber">
                      <XCircle className="w-3.5 h-3.5" /> Non détecté (Program Files)
                    </span>
                  )}
                </div>
                <div className="text-[11px] text-text-muted mt-1">
                  Réécrit <span className="font-mono text-text-secondary">AllowedIPs</span> sur l&apos;IP résolue de <span className="font-mono text-neon-green">{selectedHost}</span> + les IP distantes actives du jeu via <span className="font-mono">wireguard.exe /installtunnelservice</span>.
                </div>
                {tunnelState?.clients.wireguardPath && (
                  <div className="text-[10px] font-mono text-text-muted mt-1 truncate">
                    {tunnelState.clients.wireguardPath}
                  </div>
                )}
              </button>
            </div>
          </div>

          {/* Live Status & Direct Start/Stop Controls */}
          <div className="space-y-2.5 pt-2 border-t border-border-subtle">
            {tunnelState?.lastError && (
              <div className="p-2.5 rounded-lg bg-neon-red/10 border border-neon-red/40 text-[11px] text-neon-red">
                {tunnelState.lastError}
              </div>
            )}

            <div className="flex items-center justify-between gap-3">
              <div className="text-xs">
                <span className="text-text-muted">État du tunnel : </span>
                <span className={`font-bold ${isActive ? 'text-neon-green' : 'text-text-secondary'}`}>
                  {isActive ? `ACTIF (${tunnelState?.endpoint})` : 'ARRÊTÉ'}
                </span>
              </div>

              {isActive ? (
                <button
                  onClick={onStopTunnel}
                  disabled={isBusy}
                  className="flex items-center gap-2 px-4 py-2 rounded-xl bg-neon-red/15 hover:bg-neon-red/25 border border-neon-red/50 text-neon-red text-xs font-bold cursor-pointer disabled:opacity-50"
                >
                  {isBusy ? <Loader2 className="w-4 h-4 animate-spin" /> : <Power className="w-4 h-4" />}
                  Couper le Tunnel EU
                </button>
              ) : (
                <button
                  onClick={onStartTunnel}
                  disabled={!profile || isBusy}
                  className="flex items-center gap-2 px-4 py-2 rounded-xl bg-neon-green/15 hover:bg-neon-green/25 border border-neon-green/50 text-neon-green text-xs font-bold cursor-pointer disabled:opacity-40"
                >
                  {isBusy ? <Loader2 className="w-4 h-4 animate-spin" /> : <Zap className="w-4 h-4" />}
                  Démarrer le Tunnel EU
                </button>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Generated Split Tunnel Preview (PrivateKey masked) */}
      {tunnelState?.generatedPreview && (
        <div className="rounded-2xl bg-bg-panel border border-border-subtle p-4 space-y-2">
          <div className="text-xs font-bold uppercase tracking-wider text-text-secondary">
            Aperçu de la configuration Split Tunnel générée (Clé privée masquée)
          </div>
          <pre className="rounded-xl bg-bg-void border border-border-subtle p-3 font-mono text-[11px] text-neon-cyan/90 overflow-x-auto max-h-48">
            {tunnelState.generatedPreview}
          </pre>
        </div>
      )}
    </div>
  );
};
