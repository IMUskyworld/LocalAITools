// Remote-related types

import type { CommandState, DeviceInfo } from './envelope';

export interface RemoteCommand {
  id: string;
  sessionId: string;
  envelopeId: string;
  intentText: string;
  actionType?: string;
  actionParams?: Record<string, unknown>;
  dangerLevel: 'normal' | 'dangerous';
  state: CommandState;
  fromDeviceName: string;
  fromDeviceId: string;
  timestamp: number;
  resultText?: string;
  errorCode?: string;
}

export interface PairingDevice {
  id: string;
  deviceId: string;
  deviceName: string;
  platform: 'windows' | 'android';
  model?: string;
  tenantId: string;
  pairedAt: number;
  lastOnline: number;
  isOnline: boolean;
}

export interface WhitelistAction {
  type: string;
  name: string;
  name_en: string;
  danger_level: 'normal' | 'dangerous';
  description: string;
  enabled: boolean;
}

export interface PairCodeInfo {
  code: string;
  expiresAt: number;
  qrDataUrl?: string;
}

export interface RemoteCommandLog {
  id: string;
  timestamp: number;
  sourceDevice: string;
  command: string;
  actionType?: string;
  status: 'success' | 'fail' | 'intercepted' | 'timeout';
  details?: string;
}
