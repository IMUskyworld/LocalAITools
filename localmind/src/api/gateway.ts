// DeepSeek 官方 API 客户端 - OpenAI-compatible /v1/chat/completions
// 在线模式直连 DeepSeek，不经过任何中间网关

import type { ChatMessage } from '@/types/chat';

const DEFAULT_GATEWAY_URL = 'https://api.deepseek.com/v1';

let gatewayUrl = DEFAULT_GATEWAY_URL;

export function setGatewayUrl(url: string) {
  gatewayUrl = url;
}

export function getGatewayUrl(): string {
  return gatewayUrl;
}

export interface ChatCompletionMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

function toGatewayMessages(messages: ChatMessage[]): ChatCompletionMessage[] {
  return messages.map((m) => ({
    role: m.role,
    content: m.content,
  }));
}

export async function checkHealth(): Promise<{ ok: boolean; latencyMs: number }> {
  const start = Date.now();
  try {
    const res = await fetch(`${gatewayUrl}/models`, { method: 'GET' });
    const latency = Date.now() - start;
    return { ok: res.ok, latencyMs: latency };
  } catch {
    return { ok: false, latencyMs: Date.now() - start };
  }
}

export async function streamChat(
  messages: ChatMessage[],
  token: string,
  onDelta: (text: string) => void,
  onDone: (fullContent: string) => void,
  onError: (error: string) => void
): Promise<AbortController> {
  const controller = new AbortController();
  const startTime = Date.now();

  const body = {
    model: 'deepseek-flash',
    messages: toGatewayMessages(messages),
    stream: true,
    max_tokens: 4096,
    temperature: 0.7,
  };

  (async () => {
    try {
      const res = await fetch(`${gatewayUrl}/chat/completions`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify(body),
        signal: controller.signal,
      });

      if (!res.ok) {
        const errText = await res.text().catch(() => '未知错误');
        onError(`网关错误 ${res.status}: ${errText}`);
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
          if (!trimmed || !trimmed.startsWith('data: ')) continue;
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
            // skip malformed JSON lines
          }
        }
      }
      onDone(fullContent);
    } catch (err: unknown) {
      if (err instanceof DOMException && err.name === 'AbortError') {
        onDone('');
      } else {
        const msg = err instanceof Error ? err.message : '未知错误';
        onError(msg);
      }
    }
  })();

  return controller;
}
