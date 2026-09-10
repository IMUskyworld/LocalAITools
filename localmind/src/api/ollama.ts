// Ollama 本地推理 API 封装
// Ollama 默认监听 http://localhost:11434
// 聊天走原生 /api/chat（工具调用支持更好），检测优先走 Rust IPC（绕开 WebView2 访问 localhost 限制）

import { tauriInvoke } from '@/api/ipc';

const OLLAMA_BASE = 'http://localhost:11434';

export interface OllamaModel {
  name: string;
  size: number;        // 字节
  modifiedAt?: string;
}

export interface OllamaStatus {
  running: boolean;
  models: OllamaModel[];
  error?: string;
}

interface OllamaChatMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

/**
 * 检测 Ollama 是否运行，并拉取已安装模型列表
 * 优先走 Rust 端 IPC（check_ollama），避免 WebView2 对 localhost 的限制；
 * 若 IPC 不可用则回退到 fetch
 */
export async function checkOllama(timeoutMs = 3000): Promise<OllamaStatus> {
  // 先试 Rust IPC
  try {
    const result: any = await tauriInvoke('check_ollama');
    if (result?.data) {
      const d = result.data;
      return {
        running: d.running,
        models: (d.models || []).map((m: any) => ({
          name: m.name,
          size: m.size || 0,
          modifiedAt: m.modified_at || '',
        })),
        error: d.error || undefined,
      };
    }
  } catch (e) {
    // IPC 不可用，回退 fetch
  }

  // 回退：直接 fetch
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetch(`${OLLAMA_BASE}/api/tags`, { signal: controller.signal });
    clearTimeout(timer);
    if (!res.ok) {
      return { running: false, models: [], error: `Ollama 返回状态 ${res.status}` };
    }
    const data = await res.json();
    const models: OllamaModel[] = (data.models || []).map((m: any) => ({
      name: m.name,
      size: m.size || 0,
      modifiedAt: m.modified_at,
    }));
    return { running: true, models };
  } catch (e: any) {
    clearTimeout(timer);
    return {
      running: false,
      models: [],
      error: e?.name === 'AbortError' ? '连接超时' : e?.message || '无法连接',
    };
  }
}

/**
 * 流式聊天（Ollama OpenAI 兼容接口）
 * 返回 AbortController 用于停止生成
 */
export function streamOllamaChat(
  model: string,
  messages: OllamaChatMessage[],
  onDelta: (text: string) => void,
  onDone: (fullContent: string) => void,
  onError: (error: string) => void
): AbortController {
  const controller = new AbortController();

  const body = {
    model,
    messages,
    stream: true,
    options: { temperature: 0.7 },
  };

  (async () => {
    try {
      const res = await fetch(`${OLLAMA_BASE}/v1/chat/completions`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
        signal: controller.signal,
      });

      if (!res.ok) {
        const errText = await res.text().catch(() => 'Unknown error');
        onError(`Ollama 返回错误 ${res.status}: ${errText}`);
        return;
      }

      const reader = res.body?.getReader();
      if (!reader) {
        onError('无法读取响应');
        return;
      }

      const decoder = new TextDecoder();
      let buffer = '';
      let fullContent = '';

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });

        const lines = buffer.split('\n');
        buffer = lines.pop() || '';

        for (const line of lines) {
          const trimmed = line.trim();
          if (!trimmed.startsWith('data: ')) continue;
          const data = trimmed.slice(6);
          if (data === '[DONE]') {
            onDone(fullContent);
            return;
          }
          try {
            const parsed = JSON.parse(data);
            const delta = parsed.choices?.[0]?.delta?.content;
            if (delta) {
              fullContent += delta;
              onDelta(delta);
            }
          } catch {
            // skip malformed line
          }
        }
      }
      onDone(fullContent);
    } catch (e: any) {
      if (e?.name === 'AbortError') {
        onDone('');
      } else {
        onError(e?.message || 'Ollama 请求失败');
      }
    }
  })();

  return controller;
}

/**
 * 获取一个可展示的模型大小（MB/GB）
 */
export function formatModelSize(bytes: number): string {
  if (!bytes) return '';
  const mb = bytes / 1024 / 1024;
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${Math.round(mb)} MB`;
}
