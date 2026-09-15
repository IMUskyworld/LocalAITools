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

  // 只要【设备凭据】存在就连中继。
  //
  // 注意不能用账号会话来判断：Relay 的连接与命令路由只认设备 token，
  // 而账号会话是 30 分钟 access + 刷新失败即被清掉的。
  // 旧实现要求 localmind-account-auth 也存在 —— 一旦账号会话过期被清理
  // （打开「账号」页触发 initialize() 时最容易发生），电脑端从此不再连中继，
  // 界面上却一切正常，手机端则永远显示「电脑端不在线」。
  const deviceRaw = localStorage.getItem('localmind-relay-device');
  if (!deviceRaw) {
    // 从未登记过设备（游客模式），不连接 WSS
    console.log('[Relay] 没有设备凭据，跳过中继连接（游客模式）');
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
    // Relay 用 tenant_id 做路由校验，回传状态时必须原样带上，
    // 否则 process_state -> ensure_pair_route 会因"设备未配对"被拒。
    const tenantId = env.tenant_id;

    // 发送 Windows 系统通知
    try {
      await requestPermission();
      sendNotification({ title: 'LocalMind 远程命令', body: intentText.substring(0, 200) });
    } catch {}

    // 在当前 session 显示远程命令为系统消息。
    // 若当前没有会话（例如刚启动、上一轮被删），要【自动建一个】，
    // 否则旧实现会直接 return —— 手机端收不到任何状态，只能干等到超时，
    // 这正好是「发指令没反应」的一种成因。
    const store = useChatStore.getState();
    let sessionId = store.currentSessionId;
    if (!sessionId) {
      try {
        sessionId = await store.createSession('远程控制');
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        try {
          await tauriInvoke('send_relay_state', {
            baseUrl, deviceId: device.device_id, deviceToken: device.device_token,
            toDeviceId: fromDeviceId, tenantId, commandId,
            stateValue: 'failed', resultText: `电脑端无法创建会话：${msg}`,
          });
        } catch {}
        return;
      }
    }

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
        tenantId,
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
        tenantId,
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
          tenantId,
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
