// IPC 桥接层 — 对 Tauri invoke 做显式错误封装。
// 任何命令失败都不能再静默返回 null；调用方必须处理 IpcError。

export interface AppResponse<T = unknown> {
  success: boolean;
  data?: T;
  error?: string | null;
  error_code?: string | null;
}

export class IpcError extends Error {
  readonly command: string;
  readonly code: string;
  readonly cause?: unknown;

  constructor(command: string, code: string, message: string, cause?: unknown) {
    super(message);
    this.name = 'IpcError';
    this.command = command;
    this.code = code;
    this.cause = cause;
  }
}

let invokeFn: ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null = null;
// 用 Promise 而非 bool 记录初始化状态：
// 之前用 `initAttempted` 布尔标志时存在竞态 —— 首个调用者开始 await import，
// 第二个调用者看到标志已置位就立刻返回，此时 invokeFn 仍是 null，
// 于是抛出 IPC_UNAVAILABLE。启动阶段 loadSessions / createSession 同时触发，
// createSession 就是这样被静默失败的（错误随后又被 loadSessions 的 error:null 覆盖）。
let initPromise: Promise<void> | null = null;

async function getInvoke() {
  if (!initPromise) {
    initPromise = (async () => {
      try {
        const mod = await import('@tauri-apps/api/core');
        invokeFn = mod.invoke;
      } catch (e) {
        console.error('[IPC] @tauri-apps/api/core 不可用:', e);
      }
    })();
  }
  // 等待同一个初始化 Promise，保证并发调用都能拿到已就绪的 invoke
  await initPromise;
  return invokeFn;
}

function errorCodeFromPayload(payload: unknown): string {
  if (payload && typeof payload === 'object' && 'error_code' in payload) {
    const code = (payload as { error_code?: unknown }).error_code;
    if (typeof code === 'string' && code) return code;
  }
  return 'IPC_COMMAND_FAILED';
}

function errorMessage(error: unknown): string {
  if (typeof error === 'string' && error) return error;
  if (error instanceof Error && error.message) return error.message;
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === 'string' && message) return message;
  }
  return 'IPC 调用失败';
}

export async function tauriInvoke<T = any>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<AppResponse<T>> {
  const fn = await getInvoke();
  if (!fn) {
    throw new IpcError(cmd, 'IPC_UNAVAILABLE', `无法调用 ${cmd}：当前不在 Tauri 运行环境中`);
  }

  try {
    const result = await fn(cmd, args) as AppResponse<T>;
    if (result && typeof result === 'object' && result.success === false) {
      throw new IpcError(
        cmd,
        errorCodeFromPayload(result),
        result.error || `命令 ${cmd} 执行失败`,
      );
    }
    return result;
  } catch (error) {
    if (error instanceof IpcError) throw error;
    throw new IpcError(cmd, 'IPC_COMMAND_FAILED', `${cmd} 调用失败: ${errorMessage(error)}`, error);
  }
}
