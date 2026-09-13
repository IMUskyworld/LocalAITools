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
import { formatErrorMessage } from '@/utils/error-codes';

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
      const devices = await listAccountDevices(stored.tokens.access_token);
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
      const pending = await listPairingRequests(stored.tokens.access_token);
      set({ pairingRequests: pending.filter((r) => r.status === 'pending') });
    } catch (error) {
      // 未配对时 Relay 可能返回空/错误，不打扰用户
      set({ pairingRequests: [] });
    }
  },

  /** 批准授权：从此对方才能向本机下发受控任务。 */
  approvePairing: async (requestId) => {
    const stored = getStoredAuth();
    if (!stored) return;
    set({ busy: true, error: null });
    try {
      await approvePairingRequest(stored.tokens.access_token, requestId);
      const pending = await listPairingRequests(stored.tokens.access_token);
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
      await rejectPairingRequest(stored.tokens.access_token, requestId);
      const pending = await listPairingRequests(stored.tokens.access_token);
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
      await removeAccountDevice(stored.tokens.access_token, deviceId);
      const devices = await listAccountDevices(stored.tokens.access_token);
      set({ devices });
    } catch (error) {
      set({ error: messageFor(error) });
      throw error;
    } finally {
      set({ busy: false });
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
