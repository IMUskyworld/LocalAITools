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
let initAttempted = false;

async function getInvoke() {
  if (!initAttempted) {
    initAttempted = true;
    try {
      const mod = await import('@tauri-apps/api/core');
      invokeFn = mod.invoke;
    } catch (e) {
      console.error('[IPC] @tauri-apps/api/core 不可用:', e);
    }
  }
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
