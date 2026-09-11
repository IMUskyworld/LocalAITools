import { create } from 'zustand';
import {
  AccountDevice,
  AccountUser,
  RelayApiError,
  claimCurrentDevice,
  clearStoredAuth,
  fetchCurrentUser,
  getStoredAuth,
  listAccountDevices,
  loginAccount,
  logoutAccount,
  refreshAccount,
  registerAccount,
  removeAccountDevice,
  storeAuth,
} from '@/api/account';
import { getErrorInfo } from '@/utils/error-codes';

type AccountStatus = 'guest' | 'loading' | 'authenticated';

interface AccountState {
  status: AccountStatus;
  user: AccountUser | null;
  devices: AccountDevice[];
  initialized: boolean;
  busy: boolean;
  error: string | null;
  initialize: () => Promise<void>;
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, displayName: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  refreshDevices: () => Promise<void>;
  removeDevice: (deviceId: string) => Promise<void>;
  clearError: () => void;
}

export const useAccountStore = create<AccountState>((set, get) => ({
  status: 'guest',
  user: null,
  devices: [],
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
    const info = getErrorInfo(error.code);
    return info.localMessage || error.message;
  }
  if (error instanceof Error) return error.message;
  return '账号服务暂时不可用';
}
