// IPC 桥接层 — 懒加载 @tauri-apps/api/core 的 invoke
// 防止模块加载时直接依赖 Tauri API 导致崩溃

let invokeFn: any = null;
let initAttempted = false;

async function getInvoke(): Promise<any> {
  if (!initAttempted) {
    initAttempted = true;
    try {
      const mod = await import('@tauri-apps/api/core');
      invokeFn = mod.invoke;
    } catch (e) {
      console.warn('[IPC] @tauri-apps/api/core not available, using fallback:', e);
    }
  }
  return invokeFn;
}

export async function tauriInvoke(cmd: string, args?: Record<string, unknown>): Promise<any> {
  const fn = await getInvoke();
  if (!fn) {
    console.warn(`[IPC] invoke('${cmd}') skipped — not in Tauri context`);
    return null;
  }
  try {
    return await fn(cmd, args);
  } catch (e) {
    console.warn(`[IPC] invoke('${cmd}') failed:`, e);
    return null;
  }
}
