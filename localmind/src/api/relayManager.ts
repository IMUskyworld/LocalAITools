// Relay WSS 管理器 — 连接 Relay、接收远程命令、执行并回传状态
import { listen } from '@tauri-apps/api/event';
import { sendNotification, requestPermission } from '@tauri-apps/plugin-notification';
import { tauriInvoke } from '@/api/ipc';
import { runAgent, type AgentMessage } from '@/api/agent';
import { useChatStore } from '@/stores/chatStore';
import { generateId } from '@/utils/helpers';
import { ONLINE_MODEL_ID, ONLINE_MODEL_LABEL } from '@/config/models';

interface RelayCommandEnvelope {
  version: string;
  id: string;
  type: string;
  from_device_id: string;
  to_device_id?: string;
  tenant_id?: string;
  command_id?: string;
  intent_text?: string;
  action_type?: string;
  action_params?: Record<string, unknown>;
  state?: string;
  result_text?: string;
  timestamp: number;
}

let stopListening: (() => void) | null = null;

/** 启动 Relay WSS 连接并监听远程命令。在 App 组件 mount 时调用。 */
export async function startRelayListener(): Promise<void> {
  if (stopListening) return; // 已启动

  // 检查是否有设备凭据
  const stored: any = await tauriInvoke('get_api_key').catch(() => null);
  // 设备凭据存在时才连接（通过 account store 检查）
  const authRaw = localStorage.getItem('localmind-account-auth');
  const deviceRaw = localStorage.getItem('localmind-relay-device');
  if (!authRaw || !deviceRaw) {
    // 游客模式，不连接 WSS
    return;
  }

  const device = JSON.parse(deviceRaw);
  const baseUrl = localStorage.getItem('localmind-relay-url') || 'https://39.107.53.230';

  // 启动 Rust WSS 连接
  try {
    await tauriInvoke('connect_relay_wss', {
      baseUrl,
      deviceId: device.device_id,
      deviceToken: device.device_token,
    });
  } catch (e) {
    console.warn('[Relay] WSS connect failed:', e);
  }

  // 监听 WSS 重连状态
  listen<boolean>('relay-wss-connected', (event) => {
    const connected = event.payload;
    if (connected) {
      console.log('[Relay] WSS reconnected, pending messages will be flushed automatically');
    } else {
      console.log('[Relay] WSS disconnected, will auto-reconnect');
    }
  });

  // 监听远程命令事件
  stopListening = await listen<RelayCommandEnvelope>('relay-command', async (event) => {
    const env = event.payload;
    console.log('[Relay] received command:', env);

    if (env.type !== 'command' || env.action_type !== 'chat_task') {
      console.warn('[Relay] ignoring non-chat_task command:', env.action_type);
      return;
    }

    const intentText = env.intent_text || '(empty)';
    const commandId = env.command_id || env.id;
    const fromDeviceId = env.from_device_id;

    // 发送 Windows 系统通知
    try {
      await requestPermission();
      sendNotification({ title: 'LocalMind 远程命令', body: intentText.substring(0, 200) });
    } catch {}

    // 在当前 session 显示远程命令为系统消息
    const store = useChatStore.getState();
    const sessionId = store.currentSessionId;
    if (!sessionId) return;

    store.addMessage(sessionId, {
      id: generateId(),
      sessionId,
      role: 'system',
      content: `📱 收到远程命令（来自手机端）：${intentText}`,
      timestamp: Date.now(),
    });

    // 发送 running 状态
    try {
      await tauriInvoke('send_relay_state', {
        baseUrl,
        deviceId: device.device_id,
        deviceToken: device.device_token,
        toDeviceId: fromDeviceId,
        commandId,
        stateValue: 'running',
      });
    } catch (e) {
      console.warn('[Relay] failed to send running state:', e);
    }

    // 执行命令
    try {
      const result = await runAgent({
        mode: 'online',
        model: ONLINE_MODEL_ID,
        messages: [
          { role: 'user', content: intentText },
        ],
      });

      // 显示结果
      store.addMessage(sessionId, {
        id: generateId(),
        sessionId,
        role: 'assistant',
        content: result.content,
        timestamp: Date.now(),
        modelName: ONLINE_MODEL_LABEL,
      });

      // 发送 done 状态
      await tauriInvoke('send_relay_state', {
        baseUrl,
        deviceId: device.device_id,
        deviceToken: device.device_token,
        toDeviceId: fromDeviceId,
        commandId,
        stateValue: 'done',
        resultText: result.content.substring(0, 10000),
      });
    } catch (e: unknown) {
      const errorMsg = e instanceof Error ? e.message : String(e);

      store.addMessage(sessionId, {
        id: generateId(),
        sessionId,
        role: 'assistant',
        content: `❌ 远程命令执行失败：${errorMsg}`,
        timestamp: Date.now(),
      });

      // 发送 failed 状态
      try {
        await tauriInvoke('send_relay_state', {
          baseUrl,
          deviceId: device.device_id,
          deviceToken: device.device_token,
          toDeviceId: fromDeviceId,
          commandId,
          stateValue: 'failed',
          resultText: errorMsg,
        });
      } catch {}
    }
  });
}

/** 停止 Relay 监听（App unmount 时调用）。 */
export function stopRelayListener(): void {
  if (stopListening) {
    stopListening();
    stopListening = null;
  }
}
