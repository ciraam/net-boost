# NetBoost — Native Tauri v2 + Rust (windows-rs) Architecture & Parameters

Every runtime parameter shaping the native Windows network boost engine, ICMP telemetry, game watcher, live packet/connection inspector, EU split tunnel, System Tray mode, and automatic rollback:

## Native Win32 Core (Zero Polling Subprocesses in Gameplay)
- **Ping & Traceroute**: Native Win32 `IcmpSendEcho` (`iphlpapi`), `1000 ms` polling interval, `1500 ms` ping timeout, max `20` traceroute hops — *source: user request*
- **0% CPU Game Watcher**: Native Win32 `OpenProcess(PROCESS_SYNCHRONIZE)` + `WaitForMultipleObjects` kernel wait (instant game exit detection) — *source: user request*
- **Registry Tweaks (`winreg`)**: `TCPNoDelay = 1`, `TcpAckFrequency = 1`, `NetworkThrottlingIndex = 0xFFFFFFFF`, `Tcpip\QoS` `"Do not use NLA" = "1"`, strict interface GUID path validation — *source: user request*
- **Windows Firewall**: Native COM `INetFwPolicy2` / `INetFwRule` (`NetBoost_Block_*` Inbound + Outbound) — *source: user request*
- **Windows Services**: Native Win32 Service Control Manager (`OpenSCManagerW` / `ControlService` / `StartServiceW`) for `wuauserv`, `bits`, `DoSvc` — *source: user request*
- **System Tray**: Native Tauri v2 `TrayIconBuilder` (minimize to tray button + automatic minimize-to-tray on close while Boost/Tunnel is active) — *source: user request*

## Live Packet Inspector, Embedded Offline GeoIP & ETW UDP
- **Socket inspection**: Native Win32 `GetExtendedTcpTable` + `GetExtendedUdpTable` + `sysinfo` + on-demand native Windows ETW capture (`Microsoft-Windows-Kernel-Network`, `2500 ms` window, events `42, 43, 58, 59`) — *source: user request*
- **GeoIP lookup**: Embedded 4.5 MB IPv4 CIDR binary database (`O(log N)` search < 0.001 ms, zero rate-limit, zero third-party IP leak) + optional HTTPS fallback — *source: user request*

## WireGuard & WireSock EU Split Tunnel
- **DPAPI encryption**: `.conf` encrypted at rest via Win32 `CryptProtectData` / `CryptUnprotectData`, stale `tunnels/` wiped at startup — *source: user request*
- **Authenticode & ACLs**: Restricted strictly to `Program Files` binaries verified via Win32 `WinVerifyTrust`, temporary `.conf` locked with `icacls` NTFS ACLs — *source: user request*
- **Split modes**: WireSock (`AllowedApps = <game>.exe`) or WireGuard (`AllowedIPs = <gameIps>/32`) — *source: user request*

## Parallel DNS Benchmark, QoS & Rollback
- **DNS Benchmark**: Raw parallel Rust UDP RFC 1035 queries to `1.1.1.1`, `8.8.8.8`, `9.9.9.9` (`2000 ms` timeout) applied to live active adapter — *source: user request*
- **QoS DSCP**: `46` (game) and `8` (background apps) with `Do not use NLA = 1` — *source: user request*
- **Mandatory JSON backup & rollback**: Automatic full restore on game exit, app exit, or orphan backup detected on startup — *source: user request*
