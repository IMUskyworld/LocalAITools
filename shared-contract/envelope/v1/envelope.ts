// WSS 指令信封 v1 — TypeScript 类型定义（LocalMind 前端使用）
export interface CommandEnvelope {
  version: 'v1';
  id: string;
  type: EnvelopeType;
  from_device_id: string;
  to_device_id?: string;
  tenant_id?: string;
  command_id?: string;
  intent_text?: string;
  action_type?: string;
  action_params?: Record<string, unknown>;
  state?: CommandState;
  result_text?: string;
  error_code?: string;
  timestamp: number;
  pairing_code?: string;
  device_info?: DeviceInfo;
}

export type EnvelopeType =
  | 'command'
  | 'ack'
  | 'state'
  | 'pair'
  | 'pair_confirm'
  | 'heartbeat'
  | 'error';

export type CommandState =
  | 'sent'
  | 'delivered'
  | 'running'
  | 'done'
  | 'failed';

export interface DeviceInfo {
  device_name: string;
  platform: 'windows' | 'android';
  model?: string;
  os_version?: string;
}

export function createAck(original: CommandEnvelope): CommandEnvelope {
  return {
    version: 'v1',
    id: crypto.randomUUID(),
    type: 'ack',
    from_device_id: original.to_device_id!,
    to_device_id: original.from_device_id,
    tenant_id: original.tenant_id,
    command_id: original.command_id,
    timestamp: Date.now(),
  };
}

export function createHeartbeat(deviceId: string): CommandEnvelope {
  return {
    version: 'v1',
    id: crypto.randomUUID(),
    type: 'heartbeat',
    from_device_id: deviceId,
    timestamp: Date.now(),
  };
}
