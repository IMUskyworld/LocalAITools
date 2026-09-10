// AI Agent 调用 — Python Agent 服务（Pydantic AI）SSE 客户端
// 前端只负责：拿端口 → POST /agent/stream → 解析 SSE 事件 → 分发回调 → 返回最终内容。
// Agent 循环（规划/执行/反思/重试）由 Python 端 Pydantic AI 承担。

import type { ThinkingStep } from '@/types/chat';
import { tauriInvoke } from '@/api/ipc';
import { getDesktopPath } from '@/api/tools';

// ========== 消息协议 ==========

export interface AgentMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

export interface ToolLog {
  name: string;
  args: string;
  output: string;
  success: boolean;
}

export interface AgentRunOptions {
  mode: 'online' | 'offline';
  model: string;              // 离线时的模型名
  messages: AgentMessage[];   // 已含附件注入的完整对话
  selectedAttachmentPaths?: string[]; // 本轮文件选择器明确选择的只读路径
  signal?: AbortSignal;       // 停止生成
  onToolCall?: (log: ToolLog) => void;
  onThinking?: (step: ThinkingStep) => void;  // 思考过程回调（规划/执行/反思）
}

export interface AgentRunResult {
  content: string;
  toolLogs: ToolLog[];
}

// ========== Python Agent 服务连接 ==========

interface AgentConfig {
  port: number;
  token: string;
}

/** 获取 Python Agent 服务的端口 + 会话 token（Rust 端懒启动） */
async function getAgentConfig(): Promise<AgentConfig> {
  const r: any = await tauriInvoke('get_agent_config');
  if (!r?.data?.port) {
    throw new Error(r?.error || 'Agent 服务未就绪');
  }
  return { port: r.data.port, token: r.data.token };
}

// ========== 主入口 ==========

export async function runAgent(opts: AgentRunOptions): Promise<AgentRunResult> {
  const config = await getAgentConfig();
  const desktopPath = await getDesktopPath();

  // 停止信号：外部 AbortController → 本地 AbortController
  const controller = new AbortController();
  if (opts.signal?.aborted) controller.abort();
  const onAbort = () => controller.abort();
  opts.signal?.addEventListener('abort', onAbort, { once: true });

  const toolLogs: ToolLog[] = [];
  let content = '';

  try {
    const res = await fetch(`http://127.0.0.1:${config.port}/agent/stream`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-LocalMind-Token': config.token,
      },
      // 注意：不传 API key——DeepSeek key 由 Rust 在启动 Agent 服务时通过环境变量注入，
      // 前端代码与 HTTP 请求体均不接触明文 key，避免打包后 JS bundle 泄露。
      body: JSON.stringify({
        mode: opts.mode,
        model: opts.model,
        desktop_path: desktopPath,
        messages: opts.messages,
        selected_attachment_paths: opts.selectedAttachmentPaths || [],
      }),
      signal: controller.signal,
    });

    if (!res.ok || !res.body) {
      throw new Error(`Agent 服务错误（HTTP ${res.status}）`);
    }

    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });

        // SSE 以空行分隔事件
        const events = buffer.split('\n\n');
        buffer = events.pop() || '';

        for (const ev of events) {
          const line = ev.trim();
          if (!line.startsWith('data: ')) continue;
          let obj: any;
          try {
            obj = JSON.parse(line.slice(6));
          } catch {
            continue; // 忽略无法解析的分块
          }

          switch (obj.type) {
            case 'delta':
              content += obj.text || '';
              break;
            case 'tool':
              if (obj.log) {
                const log = obj.log as ToolLog;
                toolLogs.push(log);
                opts.onToolCall?.(log);
              }
              break;
            case 'thinking':
              if (obj.step) opts.onThinking?.(obj.step as ThinkingStep);
              break;
            case 'done':
              content = obj.content ?? content;
              break;
            case 'error':
              throw new Error(obj.message || 'Agent 执行失败');
          }
        }
      }
    } finally {
      reader.releaseLock();
    }
  } finally {
    opts.signal?.removeEventListener('abort', onAbort);
  }

  return { content, toolLogs };
}
