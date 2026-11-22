import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockInvoke = vi.hoisted(() => vi.fn());
const mockListen = vi.hoisted(() => vi.fn(async () => () => {}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: mockListen,
}));

import { netboostApi } from '../src/lib/tauriApi';

describe('Tauri v2 frontend bridge (tauriApi)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('invokes Tauri Rust commands with expected names and payloads', async () => {
    mockInvoke.mockResolvedValue({ ok: true });

    await netboostApi.getSystemInfo();
    expect(mockInvoke).toHaveBeenCalledWith('get_system_info');

    await netboostApi.minimizeToTray();
    expect(mockInvoke).toHaveBeenCalledWith('minimize_to_tray');

    await netboostApi.startPingMonitor('1.1.1.1');
    expect(mockInvoke).toHaveBeenCalledWith('start_ping_monitor', { host: '1.1.1.1' });

    await netboostApi.inspectConnections('cs2.exe', { captureUdpEtw: true, allowHttpsFallback: false });
    expect(mockInvoke).toHaveBeenCalledWith('inspect_connections', {
      exeName: 'cs2.exe',
      options: { captureUdpEtw: true, allowHttpsFallback: false },
    });

    await netboostApi.rollbackAll();
    expect(mockInvoke).toHaveBeenCalledWith('rollback_all');
  });

  it('subscribes cleanly to Tauri v2 push events', () => {
    const unsub = netboostApi.onPingSample(() => {});
    expect(mockListen).toHaveBeenCalledWith('event:ping', expect.any(Function));
    unsub();
  });
});
