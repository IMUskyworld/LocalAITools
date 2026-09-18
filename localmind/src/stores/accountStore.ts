import { create } from 'zustand';
import {
  AccountDevice,
  AccountUser,
  PairingRequest,
  RelayApiError,
  approvePairingRequest,
  claimCurrentDevice,
  clearStoredAuth,
  fetchCurrentUser,
  getStoredAuth,
  listAccountDevices,
  listPairingRequests,
  loginAccount,
  logoutAccount,
  refreshAccount,
  registerAccount,
  rejectPairingRequest,
  removeAccountDevice,
  storeAuth,
} from '@/api/account';
import { sendNotification, requestPermission } from '@tauri-apps/plugin-notification';
import { formatErrorMessage } from '@/utils/error-codes';

/** 已经弹过通知的申请 id，避免每轮都提醒。 */
const notifiedPairingRequestIds = new Set<string>();
let pairingWatcherTimer: number | null = null;

async function notifyPairingRequest(count: number): Promise<void> {
  try {
    await requestPermission();
    sendNotification({
      title: 'LocalMind 远控申请',
      body: count === 1
        ? '有设备申请远程控制本机，请在「账号」页批准或拒绝。'
        : `有 ${count} 台设备申请远程控制本机，请在「账号」页批准或拒绝。`,
    });
  } catch {
    // 通知不可用不影响主流程
  }
}

type AccountStatus = 'guest' | 'loading' | 'authenticated';

interface AccountState {
  status: AccountStatus;
  user: AccountUser | null;
  devices: AccountDevice[];
  /** 待本机批准的控制授权申请（别的设备想控制这台电脑） */
  pairingRequests: PairingRequest[];
  initialized: boolean;
  busy: boolean;
  error: string | null;
  initialize: () => Promise<void>;
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, displayName: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  refreshDevices: () => Promise<void>;
  removeDevice: (deviceId: string) => Promise<void>;
  refreshPairingRequests: () => Promise<void>;
  startPairingWatcher: () => void;
  stopPairingWatcher: () => void;
  approvePairing: (requestId: string) => Promise<void>;
  rejectPairing: (requestId: string) => Promise<void>;
  clearError: () => void;
}

export const useAccountStore = create<AccountState>((set, get) => ({
  status: 'guest',
  user: null,
  devices: [],
  pairingRequests: [],
  initialized: false,
  busy: false,
  error: null,

  initialize: async () => {
    if (get().initialized || get().status === 'loading') return;
    const stored = getStoredAuth();
    if (!stored) {
      set({ initialized: true, status: 'guest' });
      return;
    }

    set({ status: 'loading', error: null });
    try {
      let session = stored;
      let user: AccountUser;
      try {
        user = await fetchCurrentUser(session.tokens.access_token);
      } catch (error) {
        if (!(error instanceof RelayApiError) || error.code !== '403005') throw error;
        session = await refreshAccount(session.tokens.refresh_token);
        storeAuth(session);
        user = session.user;
      }
      await claimCurrentDevice(session.tokens.access_token);
      const devices = await listAccountDevices(session.tokens.access_token);
      set({ status: 'authenticated', user, devices, initialized: true, error: null });
    } catch (error) {
      // Relay 暂时不可达（relay_unreachable）不代表账号失效：保留本地会话，
      // 只提示错误，否则服务器抖动一次就会把用户静默登出。
      if (error instanceof RelayApiError && error.code === 'relay_unreachable') {
        set({
          status: 'guest',
          user: null,
          devices: [],
          initialized: true,
          error: messageFor(error),
        });
        return;
      }
      clearStoredAuth();
      set({
        status: 'guest',
        user: null,
        devices: [],
        initialized: true,
        error: messageFor(error),
      });
    }
  },

  login: async (email, password) => {
    set({ busy: true, error: null });
    try {
      const session = await loginAccount(email.trim(), password);
      storeAuth(session);
      await claimCurrentDevice(session.tokens.access_token);
      const devices = await listAccountDevices(session.tokens.access_token);
      set({ status: 'authenticated', user: session.user, devices, initialized: true });
    } catch (error) {
      clearStoredAuth();
      set({ status: 'guest', user: null, devices: [], error: messageFor(error) });
      throw error;
    } finally {
      set({ busy: false });
    }
  },

  register: async (email, displayName, password) => {
    set({ busy: true, error: null });
    try {
      const session = await registerAccount(email.trim(), displayName.trim(), password);
      storeAuth(session);
      await claimCurrentDevice(session.tokens.access_token);
      const devices = await listAccountDevices(session.tokens.access_token);
      set({ status: 'authenticated', user: session.user, devices, initialized: true });
    } catch (error) {
      clearStoredAuth();
      set({ status: 'guest', user: null, devices: [], error: messageFor(error) });
      throw error;
    } finally {
      set({ busy: false });
    }
  },

  logout: async () => {
    const stored = getStoredAuth();
    set({ busy: true, error: null });
    try {
      if (stored) await logoutAccount(stored.tokens.refresh_token);
    } catch {
      // Local logout must still succeed if the relay is temporarily unreachable.
    } finally {
      clearStoredAuth();
      set({
        status: 'guest',
        user: null,
        devices: [],
        busy: false,
        initialized: true,
      });
    }
  },

  refreshDevices: async () => {
    const stored = getStoredAuth();
    if (!stored) return;
    set({ busy: true, error: null });
    try {
      const devices = await withFreshAccessToken((token) => listAccountDevices(token));
      set({ devices });
    } catch (error) {
      set({ error: messageFor(error) });
      throw error;
    } finally {
      set({ busy: false });
    }
  },

  /** 拉取待本机批准的控制授权申请。 */
  refreshPairingRequests: async () => {
    const stored = getStoredAuth();
    if (!stored) return;
    try {
      const pending = await withFreshAccessToken((token) => listPairingRequests(token));
      set({ pairingRequests: pending.filter((r) => r.status === 'pending') });
    } catch {
      // 未配对 / Relay 抖动时不打扰用户，保持空列表
      set({ pairingRequests: [] });
    }
  },

  /** 批准授权：从此对方才能向本机下发受控任务。 */
  approvePairing: async (requestId) => {
    const stored = getStoredAuth();
    if (!stored) return;
    set({ busy: true, error: null });
    try {
      await withFreshAccessToken((token) => approvePairingRequest(token, requestId));
      const pending = await withFreshAccessToken((token) => listPairingRequests(token));
      set({ pairingRequests: pending.filter((r) => r.status === 'pending') });
    } catch (error) {
      set({ error: messageFor(error) });
      throw error;
    } finally {
      set({ busy: false });
    }
  },

  rejectPairing: async (requestId) => {
    const stored = getStoredAuth();
    if (!stored) return;
    set({ busy: true, error: null });
    try {
      await withFreshAccessToken((token) => rejectPairingRequest(token, requestId));
      const pending = await withFreshAccessToken((token) => listPairingRequests(token));
      set({ pairingRequests: pending.filter((r) => r.status === 'pending') });
    } catch (error) {
      set({ error: messageFor(error) });
      throw error;
    } finally {
      set({ busy: false });
    }
  },

  removeDevice: async (deviceId) => {
    const stored = getStoredAuth();
    if (!stored) return;
    set({ busy: true, error: null });
    try {
      await withFreshAccessToken((token) => removeAccountDevice(token, deviceId));
      const devices = await withFreshAccessToken((token) => listAccountDevices(token));
      set({ devices });
    } catch (error) {
      // 403007 = 该设备本来就不在这个账号里（列表是旧的，或对方已自行解绑）。
      // 这不是失败：刷新一次列表即可，不要把原始异常抛到界面上
      // （2026-09-18：此前会冒成「未处理的异步错误 RelayApiError」）。
      if (error instanceof RelayApiError && error.code === '403007') {
        try {
          const devices = await withFreshAccessToken((token) => listAccountDevices(token));
          set({ devices, error: null });
          return;
        } catch {
          // 落到下面的通用错误处理
        }
      }
      set({ error: messageFor(error) });
      throw error;
    } finally {
      set({ busy: false });
    }
  },

  /**
   * 全局轮询「待本机批准的控制授权申请」。
   *
   * 背景（2026-09-18 实测）：此前只在打开「账号」页时拉取一次，
   * 手机端发起申请后电脑端永远不会发现（除非退出再进那个页面）。
   * 现在 App 启动后每 5 秒轮询一次（窗口隐藏时跳过），新申请弹系统通知。
   */
  startPairingWatcher: () => {
    if (pairingWatcherTimer !== null) return;
    const tick = async () => {
      if (get().status !== 'authenticated') return;
      if (typeof document !== 'undefined' && document.hidden) return;
      try {
        await get().refreshPairingRequests();
      } catch {
        return;
      }
      const fresh = get().pairingRequests.filter((r) => !notifiedPairingRequestIds.has(r.id));
      if (fresh.length === 0) return;
      fresh.forEach((r) => notifiedPairingRequestIds.add(r.id));
      void notifyPairingRequest(fresh.length);
    };
    void tick();
    pairingWatcherTimer = window.setInterval(() => void tick(), 5000);
  },

  stopPairingWatcher: () => {
    if (pairingWatcherTimer !== null) {
      window.clearInterval(pairingWatcherTimer);
      pairingWatcherTimer = null;
    }
  },

  clearError: () => set({ error: null }),
}));

function messageFor(error: unknown): string {
  if (error instanceof RelayApiError) {
    // 已知错误码走共享错误码表；未登记的（如 relay_unreachable）回落到真实原因，
    // 避免把「未知错误」这种占位文案丢给用户而丢掉可诊断信息。
    return formatErrorMessage(error.code, error.message);
  }
  if (error instanceof Error) return error.message;
  return '账号服务暂时不可用';
}

/**
 * 用当前 access token 调 Relay；若已过期（403005）自动用 refresh token 换新并重存后重试一次。
 *
 * access token 只有 30 分钟有效期，只有 initialize() 会在启动时刷新。
 * 没有这层兜底时，软件连续运行超过 30 分钟后在账号页点「批准/刷新设备」会直接失败，
 * 用户只能重启客户端。
 */
async function withFreshAccessToken<T>(fn: (accessToken: string) => Promise<T>): Promise<T> {
  const stored = getStoredAuth();
  if (!stored) throw new Error('尚未登录账号');
  try {
    return await fn(stored.tokens.access_token);
  } catch (error) {
    if (!(error instanceof RelayApiError) || error.code !== '403005') throw error;
    const session = await refreshAccount(stored.tokens.refresh_token);
    storeAuth(session);
    return await fn(session.tokens.access_token);
  }
}
