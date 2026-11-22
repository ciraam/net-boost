import React, { useState } from 'react';
import {
  Check,
  Crosshair,
  Edit3,
  FolderOpen,
  Gamepad2,
  Plus,
  RefreshCw,
  Search,
  Server,
  Trash2,
  X,
} from 'lucide-react';
import type { GameProfile, RunningProcessInfo, ServerPreset } from '../types/ipc';
import { netboostApi } from '../lib/tauriApi';

interface TargetingViewProps {
  games: GameProfile[];
  presets: ServerPreset[];
  selectedGameId: string | null;
  selectedHost: string;
  detectedGames: RunningProcessInfo[];
  allProcesses: RunningProcessInfo[];
  scanningProcs: boolean;
  onScanProcesses: () => void;
  onSelectGame: (gameId: string | null) => void;
  onSelectHost: (host: string) => void;
  onAddGame: (game: GameProfile) => void;
  onRemoveGame: (gameId: string) => void;
  onSavePresets: (presets: ServerPreset[]) => void;
}

export const TargetingView: React.FC<TargetingViewProps> = ({
  games,
  presets,
  selectedGameId,
  selectedHost,
  detectedGames,
  allProcesses,
  scanningProcs,
  onScanProcesses,
  onSelectGame,
  onSelectHost,
  onAddGame,
  onRemoveGame,
  onSavePresets,
}) => {
  // Manual Game Add form
  const [gameName, setGameName] = useState('');
  const [exeName, setExeName] = useState('');
  const [exePath, setExePath] = useState('');
  const [gameServer, setGameServer] = useState(selectedHost || '1.1.1.1');

  // Process Explorer state
  const [showAllProcs, setShowAllProcs] = useState(false);
  const [procFilter, setProcFilter] = useState('');

  // Preset edit/add state
  const [editingPresetId, setEditingPresetId] = useState<string | null>(null);
  const [presetDraft, setPresetDraft] = useState<ServerPreset | null>(null);
  const [newPresetOpen, setNewPresetOpen] = useState(false);
  const [newPreset, setNewPreset] = useState<Omit<ServerPreset, 'id'>>({
    name: '',
    region: 'UE',
    host: '',
    description: '',
  });

  const handleBrowseExe = async () => {
    const picked = await netboostApi.pickExecutable();
    if (!picked.canceled && picked.exeName) {
      setExeName(picked.exeName);
      setExePath(picked.exePath ?? picked.exeName);
      if (!gameName && picked.suggestedName) {
        setGameName(picked.suggestedName);
      }
    }
  };

  const handleAddManualGame = (e: React.FormEvent) => {
    e.preventDefault();
    const cleanExe = exeName.trim();
    if (!cleanExe) return;
    const finalExe = cleanExe.toLowerCase().endsWith('.exe') ? cleanExe : `${cleanExe}.exe`;
    const newGame: GameProfile = {
      id: `game-${Date.now()}`,
      name: gameName.trim() || finalExe.replace(/\.exe$/i, ''),
      exeName: finalExe,
      exePath: exePath.trim() || finalExe,
      serverHost: gameServer.trim() || '1.1.1.1',
      addedManually: true,
    };
    onAddGame(newGame);
    onSelectGame(newGame.id);
    onSelectHost(newGame.serverHost);
    setGameName('');
    setExeName('');
    setExePath('');
  };

  const handleTargetRunningProc = (proc: RunningProcessInfo) => {
    const existing = games.find((g) => g.exeName.toLowerCase() === proc.exeName.toLowerCase());
    if (existing) {
      onSelectGame(existing.id);
      return;
    }
    const created: GameProfile = {
      id: `game-${Date.now()}`,
      name: proc.windowTitle || proc.name,
      exeName: proc.exeName,
      exePath: proc.exePath,
      serverHost: selectedHost || '1.1.1.1',
      addedManually: false,
      isRunning: true,
      pid: proc.pid,
    };
    onAddGame(created);
    onSelectGame(created.id);
  };

  const handleStartEditPreset = (p: ServerPreset) => {
    setEditingPresetId(p.id);
    setPresetDraft({ ...p });
  };

  const handleConfirmEditPreset = () => {
    if (!presetDraft || !presetDraft.host.trim()) return;
    const updated = presets.map((p) => (p.id === presetDraft.id ? presetDraft : p));
    onSavePresets(updated);
    if (editingPresetId && presets.find((p) => p.id === editingPresetId)?.host === selectedHost) {
      onSelectHost(presetDraft.host.trim());
    }
    setEditingPresetId(null);
    setPresetDraft(null);
  };

  const handleDeletePreset = (id: string) => {
    onSavePresets(presets.filter((p) => p.id !== id));
  };

  const handleCreatePreset = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newPreset.name.trim() || !newPreset.host.trim()) return;
    const created: ServerPreset = {
      id: `preset-${Date.now()}`,
      name: newPreset.name.trim(),
      region: newPreset.region,
      host: newPreset.host.trim(),
      description: newPreset.description.trim() || 'Serveur personnalisé',
    };
    onSavePresets([...presets, created]);
    onSelectHost(created.host);
    setNewPreset({ name: '', region: 'UE', host: '', description: '' });
    setNewPresetOpen(false);
  };

  const shownProcesses = (showAllProcs ? allProcesses : detectedGames).filter((p) => {
    if (!procFilter.trim()) return true;
    const q = procFilter.toLowerCase();
    return (
      p.exeName.toLowerCase().includes(q) ||
      p.name.toLowerCase().includes(q) ||
      p.windowTitle.toLowerCase().includes(q)
    );
  });

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-12 gap-4">
        {/* Left: Manual Game Addition & Saved Library */}
        <div className="col-span-12 lg:col-span-6 rounded-2xl bg-bg-panel border border-border-subtle p-5 space-y-4">
          <div className="flex items-center gap-2">
            <Crosshair className="w-4 h-4 text-neon-cyan" />
            <h2 className="text-sm font-bold text-text-primary">Ajout manuel d&apos;un jeu (.exe) &amp; serveur</h2>
          </div>

          <form onSubmit={handleAddManualGame} className="space-y-3">
            <div className="grid grid-cols-2 gap-2.5">
              <div>
                <label className="block text-[11px] text-text-muted mb-1">Nom du jeu</label>
                <input
                  type="text"
                  value={gameName}
                  onChange={(e) => setGameName(e.target.value)}
                  placeholder="ex: Valorant, CS2..."
                  className="w-full bg-bg-surface border border-border-strong rounded-lg px-3 py-2 text-xs text-text-primary focus:outline-none focus:border-neon-cyan"
                />
              </div>
              <div>
                <label className="block text-[11px] text-text-muted mb-1">IP ou Domaine serveur à tester</label>
                <input
                  type="text"
                  value={gameServer}
                  onChange={(e) => setGameServer(e.target.value)}
                  placeholder="ex: 1.1.1.1 ou fra-de-ping.vultr.com"
                  className="w-full bg-bg-surface border border-border-strong rounded-lg px-3 py-2 text-xs font-mono text-text-primary focus:outline-none focus:border-neon-cyan"
                />
              </div>
            </div>

            <div>
              <label className="block text-[11px] text-text-muted mb-1">Exécutable (.exe)</label>
              <div className="flex gap-2">
                <input
                  type="text"
                  value={exeName}
                  onChange={(e) => setExeName(e.target.value)}
                  placeholder="ex: VALORANT-Win64-Shipping.exe"
                  className="flex-1 bg-bg-surface border border-border-strong rounded-lg px-3 py-2 text-xs font-mono text-text-primary focus:outline-none focus:border-neon-cyan"
                />
                <button
                  type="button"
                  onClick={handleBrowseExe}
                  className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-bg-elevated hover:bg-border-strong border border-border-strong text-xs font-semibold text-text-primary cursor-pointer"
                >
                  <FolderOpen className="w-3.5 h-3.5 text-neon-cyan" />
                  Parcourir
                </button>
              </div>
              {exePath && exePath !== exeName && (
                <div className="mt-1 text-[10px] font-mono text-text-muted truncate" title={exePath}>
                  {exePath}
                </div>
              )}
            </div>

            <button
              type="submit"
              disabled={!exeName.trim()}
              className="w-full flex items-center justify-center gap-2 py-2 rounded-xl bg-neon-cyan/15 hover:bg-neon-cyan/25 border border-neon-cyan/50 text-neon-cyan text-xs font-bold transition-colors cursor-pointer disabled:opacity-40"
            >
              <Plus className="w-4 h-4" />
              Ajouter et cibler ce jeu
            </button>
          </form>

          {/* Saved Games List */}
          <div className="pt-3 border-t border-border-subtle space-y-2">
            <div className="text-xs font-bold uppercase tracking-wider text-text-muted">
              Jeux configurés ({games.length})
            </div>
            {games.length === 0 ? (
              <div className="text-xs text-text-muted py-4 text-center rounded-xl bg-bg-surface border border-dashed border-border-subtle">
                Aucun jeu enregistré. Ajoutez un .exe ci-dessus ou cliquez sur un processus actif à droite.
              </div>
            ) : (
              <div className="space-y-2 max-h-52 overflow-y-auto pr-1">
                {games.map((g) => {
                  const active = g.id === selectedGameId;
                  return (
                    <div
                      key={g.id}
                      onClick={() => {
                        onSelectGame(g.id);
                        onSelectHost(g.serverHost);
                      }}
                      className={`p-3 rounded-xl border flex items-center justify-between gap-2 cursor-pointer transition-all ${
                        active
                          ? 'bg-neon-cyan/10 border-neon-cyan/50'
                          : 'bg-bg-surface border-border-subtle hover:border-border-strong'
                      }`}
                    >
                      <div className="min-w-0">
                        <div className="text-xs font-bold text-text-primary flex items-center gap-2">
                          <span className="truncate">{g.name}</span>
                          {active && (
                            <span className="text-[10px] px-1.5 py-0.5 rounded bg-neon-cyan/20 text-neon-cyan">
                              CIBLÉ
                            </span>
                          )}
                        </div>
                        <div className="text-[11px] font-mono text-text-muted truncate">
                          {g.exeName} → {g.serverHost}
                        </div>
                      </div>
                      <button
                        onClick={(ev) => {
                          ev.stopPropagation();
                          onRemoveGame(g.id);
                        }}
                        className="p-1.5 rounded-lg text-text-muted hover:text-neon-red hover:bg-neon-red/10 cursor-pointer"
                        title="Supprimer"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>

        {/* Right: Auto-Detected Running Games & Process Explorer */}
        <div className="col-span-12 lg:col-span-6 rounded-2xl bg-bg-panel border border-border-subtle p-5 flex flex-col">
          <div className="flex items-center justify-between gap-2 mb-3">
            <div className="flex items-center gap-2">
              <Gamepad2 className="w-4 h-4 text-neon-green" />
              <h2 className="text-sm font-bold text-text-primary">Détection automatique des jeux actifs</h2>
            </div>
            <button
              onClick={onScanProcesses}
              disabled={scanningProcs}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-bg-surface hover:bg-bg-elevated border border-border-strong text-xs text-text-secondary hover:text-neon-cyan cursor-pointer"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${scanningProcs ? 'animate-spin text-neon-cyan' : ''}`} />
              Scanner
            </button>
          </div>

          <div className="flex items-center gap-2 mb-3">
            <div className="flex rounded-lg bg-bg-surface border border-border-subtle p-0.5 text-xs">
              <button
                onClick={() => setShowAllProcs(false)}
                className={`px-2.5 py-1 rounded-md font-semibold cursor-pointer ${
                  !showAllProcs ? 'bg-neon-green/15 text-neon-green' : 'text-text-muted'
                }`}
              >
                Jeux détectés ({detectedGames.length})
              </button>
              <button
                onClick={() => setShowAllProcs(true)}
                className={`px-2.5 py-1 rounded-md font-semibold cursor-pointer ${
                  showAllProcs ? 'bg-neon-cyan/15 text-neon-cyan' : 'text-text-muted'
                }`}
              >
                Tous les .exe ({allProcesses.length})
              </button>
            </div>

            <div className="relative flex-1">
              <Search className="w-3.5 h-3.5 text-text-muted absolute left-2.5 top-1/2 -translate-y-1/2" />
              <input
                type="text"
                value={procFilter}
                onChange={(e) => setProcFilter(e.target.value)}
                placeholder="Filtrer processus..."
                className="w-full bg-bg-surface border border-border-subtle rounded-lg pl-8 pr-2.5 py-1.5 text-xs text-text-primary focus:outline-none focus:border-neon-cyan"
              />
            </div>
          </div>

          <div className="flex-1 overflow-y-auto max-h-80 space-y-1.5 pr-1">
            {shownProcesses.length === 0 ? (
              <div className="py-10 text-center text-xs text-text-muted rounded-xl bg-bg-surface border border-dashed border-border-subtle px-4">
                {showAllProcs
                  ? 'Aucun processus correspondant au filtre.'
                  : 'Aucun jeu connu en cours d\'exécution détecté pour le moment. Lancez un jeu ou basculez sur « Tous les .exe » pour cibler n\'importe quel processus actif.'}
              </div>
            ) : (
              shownProcesses.map((proc) => (
                <div
                  key={`${proc.pid}-${proc.exeName}`}
                  className="p-2.5 rounded-xl bg-bg-surface border border-border-subtle hover:border-border-strong flex items-center justify-between gap-2"
                >
                  <div className="min-w-0">
                    <div className="text-xs font-semibold text-text-primary flex items-center gap-2">
                      <span className="truncate font-mono">{proc.exeName}</span>
                      {proc.isLikelyGame && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-neon-green/15 text-neon-green font-sans font-bold">
                          JEU
                        </span>
                      )}
                    </div>
                    <div className="text-[11px] text-text-muted truncate">
                      PID {proc.pid} • {proc.memoryMb} Mo RAM {proc.windowTitle ? `• ${proc.windowTitle}` : ''}
                    </div>
                  </div>
                  <button
                    onClick={() => handleTargetRunningProc(proc)}
                    className="shrink-0 px-2.5 py-1.5 rounded-lg bg-neon-cyan/15 hover:bg-neon-cyan/25 border border-neon-cyan/40 text-neon-cyan text-[11px] font-semibold cursor-pointer"
                  >
                    Cibler
                  </button>
                </div>
              ))
            )}
          </div>
        </div>
      </div>

      {/* Bottom: Editable Network Test Presets (Cloudflare, EU, NA) */}
      <div className="rounded-2xl bg-bg-panel border border-border-subtle p-5 space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Server className="w-4 h-4 text-neon-cyan" />
            <h2 className="text-sm font-bold text-text-primary">
              Presets de Test Réseau Modifiables (Cloudflare, Serveurs UE / NA)
            </h2>
          </div>
          <button
            onClick={() => setNewPresetOpen((v) => !v)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-neon-cyan/15 hover:bg-neon-cyan/25 border border-neon-cyan/40 text-neon-cyan text-xs font-semibold cursor-pointer"
          >
            <Plus className="w-3.5 h-3.5" />
            Nouveau Preset
          </button>
        </div>

        {newPresetOpen && (
          <form onSubmit={handleCreatePreset} className="p-3.5 rounded-xl bg-bg-surface border border-neon-cyan/40 grid grid-cols-12 gap-2.5 items-end">
            <div className="col-span-3">
              <label className="block text-[10px] text-text-muted mb-1">Nom</label>
              <input
                type="text"
                value={newPreset.name}
                onChange={(e) => setNewPreset({ ...newPreset, name: e.target.value })}
                placeholder="ex: Serveur Valorant Paris"
                className="w-full bg-bg-void border border-border-strong rounded-lg px-2.5 py-1.5 text-xs text-text-primary"
              />
            </div>
            <div className="col-span-2">
              <label className="block text-[10px] text-text-muted mb-1">Région</label>
              <select
                value={newPreset.region}
                onChange={(e) => setNewPreset({ ...newPreset, region: e.target.value as ServerPreset['region'] })}
                className="w-full bg-bg-void border border-border-strong rounded-lg px-2.5 py-1.5 text-xs text-text-primary"
              >
                <option value="Cloudflare">Cloudflare</option>
                <option value="UE">UE</option>
                <option value="NA">NA</option>
                <option value="DNS">DNS</option>
                <option value="Custom">Custom</option>
              </select>
            </div>
            <div className="col-span-3">
              <label className="block text-[10px] text-text-muted mb-1">IP / Domaine</label>
              <input
                type="text"
                value={newPreset.host}
                onChange={(e) => setNewPreset({ ...newPreset, host: e.target.value })}
                placeholder="ex: 1.1.1.1"
                className="w-full bg-bg-void border border-border-strong rounded-lg px-2.5 py-1.5 text-xs font-mono text-text-primary"
              />
            </div>
            <div className="col-span-3">
              <label className="block text-[10px] text-text-muted mb-1">Description</label>
              <input
                type="text"
                value={newPreset.description}
                onChange={(e) => setNewPreset({ ...newPreset, description: e.target.value })}
                placeholder="Notes..."
                className="w-full bg-bg-void border border-border-strong rounded-lg px-2.5 py-1.5 text-xs text-text-primary"
              />
            </div>
            <div className="col-span-1">
              <button
                type="submit"
                className="w-full py-1.5 rounded-lg bg-neon-green/20 border border-neon-green/50 text-neon-green text-xs font-bold cursor-pointer"
              >
                OK
              </button>
            </div>
          </form>
        )}

        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-3">
          {presets.map((preset) => {
            const isSelected = selectedHost === preset.host;
            const isEditing = editingPresetId === preset.id && presetDraft;

            if (isEditing) {
              return (
                <div key={preset.id} className="p-3 rounded-xl bg-bg-surface border border-neon-cyan space-y-2">
                  <input
                    type="text"
                    value={presetDraft.name}
                    onChange={(e) => setPresetDraft({ ...presetDraft, name: e.target.value })}
                    className="w-full bg-bg-void border border-border-strong rounded px-2 py-1 text-xs text-text-primary"
                  />
                  <div className="flex gap-1.5">
                    <select
                      value={presetDraft.region}
                      onChange={(e) => setPresetDraft({ ...presetDraft, region: e.target.value as ServerPreset['region'] })}
                      className="bg-bg-void border border-border-strong rounded px-1.5 py-1 text-xs text-text-primary"
                    >
                      <option value="Cloudflare">Cloudflare</option>
                      <option value="UE">UE</option>
                      <option value="NA">NA</option>
                      <option value="DNS">DNS</option>
                      <option value="Custom">Custom</option>
                    </select>
                    <input
                      type="text"
                      value={presetDraft.host}
                      onChange={(e) => setPresetDraft({ ...presetDraft, host: e.target.value })}
                      className="flex-1 min-w-0 bg-bg-void border border-border-strong rounded px-2 py-1 text-xs font-mono text-text-primary"
                    />
                  </div>
                  <div className="flex justify-end gap-1.5">
                    <button
                      onClick={() => {
                        setEditingPresetId(null);
                        setPresetDraft(null);
                      }}
                      className="p-1 rounded bg-bg-elevated text-text-muted hover:text-text-primary cursor-pointer"
                    >
                      <X className="w-3.5 h-3.5" />
                    </button>
                    <button
                      onClick={handleConfirmEditPreset}
                      className="p-1 rounded bg-neon-green/20 text-neon-green cursor-pointer"
                    >
                      <Check className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>
              );
            }

            return (
              <div
                key={preset.id}
                onClick={() => onSelectHost(preset.host)}
                className={`p-3.5 rounded-xl border transition-all cursor-pointer flex flex-col justify-between ${
                  isSelected
                    ? 'bg-neon-cyan/10 border-neon-cyan/60 shadow-[0_0_16px_rgba(0,229,255,0.12)]'
                    : 'bg-bg-surface border-border-subtle hover:border-border-strong'
                }`}
              >
                <div>
                  <div className="flex items-center justify-between gap-2 mb-1">
                    <span className="text-[10px] font-bold uppercase tracking-wider px-1.5 py-0.5 rounded bg-bg-elevated text-neon-cyan">
                      {preset.region}
                    </span>
                    <div className="flex items-center gap-1">
                      <button
                        onClick={(ev) => {
                          ev.stopPropagation();
                          handleStartEditPreset(preset);
                        }}
                        className="p-1 rounded text-text-muted hover:text-neon-cyan cursor-pointer"
                        title="Modifier"
                      >
                        <Edit3 className="w-3 h-3" />
                      </button>
                      <button
                        onClick={(ev) => {
                          ev.stopPropagation();
                          handleDeletePreset(preset.id);
                        }}
                        className="p-1 rounded text-text-muted hover:text-neon-red cursor-pointer"
                        title="Supprimer"
                      >
                        <Trash2 className="w-3 h-3" />
                      </button>
                    </div>
                  </div>
                  <div className="text-xs font-bold text-text-primary truncate">{preset.name}</div>
                  <div className="text-[11px] font-mono text-neon-green truncate mt-0.5">{preset.host}</div>
                </div>
                <div className="text-[10px] text-text-muted mt-2 line-clamp-2">{preset.description}</div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};
