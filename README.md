# NetBoost

Application desktop Windows (Tauri v2 + Rust `windows-rs` + React/TypeScript) dédiée à l'optimisation réseau, au split-tunneling et à la télémétrie en temps réel pour les jeux compétitifs.

Contrairement aux utilitaires génériques, NetBoost exécute des appels système Windows natifs réels (Win32 API, COM, Registre, ETW, ICMP) avec sauvegarde JSON obligatoire avant toute modification et restauration complète automatique.

---

## Architecture technique

- **Frontend** : React 19, TypeScript, Tailwind CSS v4, Recharts (`src/`)
- **Backend natif** : Rust / Tauri v2 (`src-tauri/src/`) via `windows` 0.58 et `winreg`
- **Exécution sans polling PowerShell en jeu** :
  - **Ping & Traceroute ICMP** : `IcmpCreateFile` / `IcmpSendEcho` (`iphlpapi.dll`), sans processus fils `ping.exe` ni `tracert.exe`.
  - **Surveillance fermeture du jeu** : `OpenProcess(PROCESS_SYNCHRONIZE)` + `WaitForMultipleObjects` (attente noyau à 0 % CPU).
  - **Registre TCP/IP & QoS** : lecture/écriture native via `winreg` avec validation stricte du GUID de l'interface réseau.
  - **Pare-feu Windows** : création/suppression de règles entrantes et sortantes via COM `INetFwPolicy2` / `INetFwRule`.
  - **Services Windows** : contrôle de `wuauserv`, `bits` et `dosvc` via `OpenSCManagerW` / `ControlService` / `StartServiceW`.
  - **Inspecteur de connexions & GeoIP** : sockets TCP/UDP via `GetExtendedTcpTable` / `GetExtendedUdpTable`, capture UDP ETW à la demande (`Microsoft-Windows-Kernel-Network`), base GeoIP IPv4 embarquée hors-ligne (`O(log N)`, aucune fuite d'IP par défaut).
  - **System Tray** : masquage de la fenêtre en barre des tâches lorsque le Boost ou le Tunnel est actif pour minimiser l'empreinte RAM/CPU de WebView2.

---

## Sécurité et réversibilité

- **Privilèges UAC** : détection native via `OpenProcessToken` / `TokenElevation` et relance UAC via `ShellExecuteW("runas")`.
- **Chiffrement au repos** : les profils WireGuard (`.conf`) sont chiffrés sur disque via Windows DPAPI (`CryptProtectData` / `CryptUnprotectData`) et ne sont jamais exposés en clair au frontend.
- **Binaires de tunnel vérifiés** : `wiresock-client.exe` et `wireguard.exe` sont strictement limités aux répertoires `Program Files` avec vérification de signature Authenticode (`WinVerifyTrust`). Les fichiers `.conf` temporaires reçoivent des ACL NTFS restreintes à l'utilisateur courant et `SYSTEM` (`icacls`), puis sont supprimés à l'arrêt et au démarrage.
- **Sauvegarde et Rollback atomique** : `backup-state.json` capture l'état initial (DNS primaire et adaptateurs secondaires, valeurs Registre, propriétés EEE, services, règles QoS et Pare-feu) avant toute action. Le rollback s'exécute manuellement, à la fermeture du jeu ciblé, à la fermeture de l'application ou en récupération de sauvegarde orpheline au démarrage.

---

## Prérequis

- **OS** : Windows 10 / 11 (x64), WebView2 Runtime (inclus par défaut sur Windows 11).
- **Build / Dev** :
  - Node.js 20+ et npm
  - Rust (toolchain stable via `rustup` + MSVC Build Tools)
- **Optionnel (Split Tunnel)** :
  - [WireSock VPN Client](https://www.wiresock.net/) (recommandé pour le split-tunneling par processus `AllowedApps`)
  - ou [WireGuard for Windows](https://www.wireguard.com/install/) (split-tunneling par IP `/32`)

---

## Installation et commandes

```bash
# Installer les dépendances frontend
npm install

# Vérification TypeScript
npm run typecheck

# Tests unitaires frontend (Vitest)
npm run test

# Tests unitaires backend Rust (DPAPI, GeoIP/ETW, sanitizers, parseur WireGuard)
cargo test --manifest-path src-tauri/Cargo.toml

# Lancer en mode développement
npm run tauri dev

# Compiler le binaire Windows de production
npm run build
```

Le binaire compilé est généré dans `src-tauri/target/release/`.

---

## Guide d'utilisation

### 1. Élévation Administrateur (UAC)
Les opérations système (QoS, DNS, Registre, EEE, Pare-feu, ETW UDP, Tunnel) nécessitent les droits Administrateur. Si l'application est lancée en utilisateur standard, cliquez sur le bouton d'élévation UAC dans la barre supérieure pour la relancer en mode Administrateur.

### 2. Ciblage du jeu et du serveur
Dans l'onglet **Ciblage** :
- Scannez les processus actifs pour détecter automatiquement votre jeu en cours d'exécution ou sélectionnez son fichier `.exe` manuellement.
- Choisissez un serveur de référence (ex. `1.1.1.1`, `fra-de-ping.vultr.com`, `par-fr-ping.vultr.com`) ou saisissez votre propre hôte/IP pour le monitoring et le split-tunnel.

### 3. Activation du Boost (Cockpit)
Dans le **Dashboard** :
- Cochez les modules souhaités :
  - **Tunnel EU Split** : active le tunnel WireSock/WireGuard configuré.
  - **QoS DSCP 46** : crée une politique `NetBoost_Game_*` (DSCP 46) et active `"Do not use NLA" = "1"` pour que Windows émette le marquage DSCP hors domaine Active Directory.
  - **DNS Auto + Flush** : benchmark en parallèle `1.1.1.1`, `8.8.8.8` et `9.9.9.9`, applique les deux plus rapides sur l'adaptateur actif et vide le cache DNS.
  - **TCPNoDelay / Ack** : applique `TCPNoDelay=1`, `TcpAckFrequency=1` et `NetworkThrottlingIndex=0xFFFFFFFF`.
  - **EEE Off** : désactive l'économie d'énergie Ethernet (Energy Efficient Ethernet) sur la carte réseau si exposée par le pilote.
  - **Auto-Rollback** : déclenche la restauration complète dès que le `.exe` du jeu se ferme.
- Cliquez sur **BOOST**. Une fois actif, fermer la fenêtre réduit l'application dans le **System Tray** sans interrompre la session.

### 4. Tunnel EU Split (WireSock / WireGuard)
Dans l'onglet **Tunnel EU** :
- Importez votre fichier `.conf` WireGuard. Il est immédiatement chiffré via DPAPI.
- Choisissez le mode :
  - **WireSock** : injecte `AllowedApps = <jeu>.exe` et exclut les plages LAN (`DisallowedIPs`) pour ne router que le trafic du jeu.
  - **WireGuard** : réécrit `AllowedIPs` avec les IPv4 `/32` résolues du serveur cible.

### 5. Inspecteur Réseau et Pare-feu
Dans l'onglet **Inspecteur** :
- Visualisez les connexions TCP/UDP actives du jeu (ou globales) avec géolocalisation hors-ligne instantanée (badge UE / Hors-UE).
- Lancez une **Capture UDP ETW** (2,5 s, mode Admin) pour identifier les IP distantes réelles des serveurs de jeu UDP.
- Bloquez une IP ou un bloc `/24` indésirable en un clic via le Pare-feu Windows (`NetBoost_Block_*`). Toutes les règles créées sont supprimées au rollback.

### 6. Trafic d'arrière-plan, Monitoring et Console
- **Arrière-plan** : mettez temporairement en pause `wuauserv`, `bits` ou `dosvc`, ou bridez des applications tierces (navigateurs, launchers) en basse priorité `DSCP 8`.
- **Monitoring** : suivez la latence, la gigue (jitter), la perte de paquets en temps réel avec comparaison **Avant / Après Boost**, ainsi que le traceroute visuel saut par saut.
- **Console & Backup** : consultez l'historique exact des opérations système exécutées et le contenu de `backup-state.json`, ou déclenchez un **Rollback complet** à tout moment.
