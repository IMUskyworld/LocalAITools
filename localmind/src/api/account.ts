import { invoke } from '@tauri-apps/api/core';

export interface AccountUser {
  id: string;
  email: string;
  display_name: string;
  status: string;
  created_at: number;
  updated_at: number;
}

export interface AuthTokens {
  access_token: string;
  refresh_token: string;
  access_expires_at: number;
  refresh_expires_at: number;
}

export interface AuthSession {
  user: AccountUser;
  tokens: AuthTokens;
}

export interface DeviceCredentials {
  device_id: string;
  device_token: string;
  device_name: string;
  platform: 'windows';
  created_at: number;
}

export interface AccountDevice {
  id: string;
  device_name: string;
  platform: 'windows' | 'android';
  model: string;
  os_version: string;
  created_at: number;
  last_seen: number;
  enrolled_at: number;
}

const RELAY_URL_KEY = 'localmind-relay-url';
const DEVICE_KEY = 'localmind-relay-device';
const AUTH_KEY = 'localmind-account-auth';

export class RelayApiError extends Error {
  constructor(
    message: string,
    public readonly code: string,
    public readonly status: number,
  ) {
    super(message);
    this.name = 'RelayApiError';
  }
}

export function getRelayBaseUrl(): string {
  return (localStorage.getItem(RELAY_URL_KEY) || 'https://39.107.53.230').replace(/\/+$/, '');
}

export function setRelayBaseUrl(url: string): void {
  const normalized = url.trim().replace(/\/+$/, '');
  if (!normalized) {
    localStorage.removeItem(RELAY_URL_KEY);
    return;
  }
  localStorage.setItem(RELAY_URL_KEY, normalized);
}

export function getStoredDevice(): DeviceCredentials | null {
  return readJson<DeviceCredentials>(DEVICE_KEY);
}

export function getStoredAuth(): AuthSession | null {
  return readJson<AuthSession>(AUTH_KEY);
}

export function storeAuth(session: AuthSession): void {
  localStorage.setItem(AUTH_KEY, JSON.stringify(session));
}

export function clearStoredAuth(): void {
  localStorage.removeItem(AUTH_KEY);
}

export async function ensureDeviceCredentials(): Promise<DeviceCredentials> {
  const stored = getStoredDevice();
  if (stored?.device_id && stored.device_token) return stored;

  const platform = navigator.platform || 'Windows';
  const created = await relayRequest<Omit<DeviceCredentials, 'device_name' | 'platform'>>(
    '/v1/devices/register',
    {
      method: 'POST',
      body: JSON.stringify({
        device_name: 'LocalMind Windows',
        platform: 'windows',
        model: platform,
        os_version: '',
      }),
    },
  );
  const credentials: DeviceCredentials = {
    ...created,
    device_name: 'LocalMind Windows',
    platform: 'windows',
  };
  localStorage.setItem(DEVICE_KEY, JSON.stringify(credentials));
  return credentials;
}

export async function registerAccount(
  email: string,
  displayName: string,
  password: string,
): Promise<AuthSession> {
  return relayRequest<AuthSession>('/v1/auth/register', {
    method: 'POST',
    body: JSON.stringify({ email, display_name: displayName, password }),
  });
}

export async function loginAccount(email: string, password: string): Promise<AuthSession> {
  return relayRequest<AuthSession>('/v1/auth/login', {
    method: 'POST',
    body: JSON.stringify({ email, password }),
  });
}

export async function refreshAccount(refreshToken: string): Promise<AuthSession> {
  return relayRequest<AuthSession>('/v1/auth/refresh', {
    method: 'POST',
    body: JSON.stringify({ refresh_token: refreshToken }),
  });
}

export async function logoutAccount(refreshToken: string): Promise<void> {
  await relayRequest<void>('/v1/auth/logout', {
    method: 'POST',
    body: JSON.stringify({ refresh_token: refreshToken }),
  });
}

export async function fetchCurrentUser(accessToken: string): Promise<AccountUser> {
  return relayRequest<AccountUser>('/v1/auth/me', {}, accessToken);
}

export async function claimCurrentDevice(accessToken: string): Promise<AccountDevice> {
  const device = await ensureDeviceCredentials();
  return relayRequest<AccountDevice>(
    '/v1/account/devices/claim',
    {
      method: 'POST',
      headers: {
        'x-device-id': device.device_id,
        'x-device-token': device.device_token,
      },
    },
    accessToken,
  );
}

export async function listAccountDevices(accessToken: string): Promise<AccountDevice[]> {
  const device = await ensureDeviceCredentials();
  return relayRequest<AccountDevice[]>(
    '/v1/account/devices',
    {
      headers: {
        'x-device-id': device.device_id,
        'x-device-token': device.device_token,
      },
    },
    accessToken,
  );
}

export async function removeAccountDevice(accessToken: string, targetDeviceId: string): Promise<void> {
  const device = await ensureDeviceCredentials();
  await relayRequest<void>(
    `/v1/account/devices/${encodeURIComponent(targetDeviceId)}`,
    {
      method: 'DELETE',
      headers: {
        'x-device-id': device.device_id,
        'x-device-token': device.device_token,
      },
    },
    accessToken,
  );
}

// Relay 目前使用 Caddy internal CA 在纯 IP 上提供 TLS，WebView2 的系统信任链无法
// 通过校验，因此所有 Relay 请求都交给 Rust 侧的 relay_http_request 命令发起，
// 由 Rust 显式信任内置 CA。相关约束与校验见 src-tauri/src/relay_http.rs。
interface RelayHttpResponse {
  status: number;
  body: string;
}

async function relayRequest<T>(
  path: string,
  init: RequestInit = {},
  accessToken?: string,
): Promise<T> {
  const headers: Record<string, string> = {};
  new Headers(init.headers).forEach((value, key) => {
    headers[key] = value;
  });

  let response: RelayHttpResponse;
  try {
    response = await invoke<RelayHttpResponse>('relay_http_request', {
      baseUrl: getRelayBaseUrl(),
      method: (init.method || 'GET').toUpperCase(),
      path,
      body: typeof init.body === 'string' ? init.body : null,
      accessToken: accessToken ?? null,
      headers,
    });
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new RelayApiError(
      `无法连接 Relay 服务（${getRelayBaseUrl()}）：${detail}`,
      'relay_unreachable',
      0,
    );
  }

  const payload = response.body ? parseJsonBody(response.body) : null;
  if (response.status < 200 || response.status >= 300) {
    const code = payload?.error?.code || String(response.status);
    const message = payload?.error?.message || `Relay request failed (${response.status})`;
    throw new RelayApiError(message, code, response.status);
  }
  if (response.status === 204 || !response.body) return undefined as T;
  return payload as T;
}

function parseJsonBody(text: string): any {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function readJson<T>(key: string): T | null {
  try {
    const value = localStorage.getItem(key);
    return value ? (JSON.parse(value) as T) : null;
  } catch {
    return null;
  }
}
