// 长期记忆文档（memory.md）API 封装 + 每轮记忆维护逻辑。
//
// 记忆分两类（不要混）：
//  - 会话摘要：服务“这一场对话别断片”，存 session_summaries（每轮注入）
//  - 长期记忆：服务“跨对话记住这个人”，存 %APPDATA%\LocalMind\memory.md（单文件 markdown，用户可读可改）
//
// 本文件负责长期记忆的读写、注入文本，以及复用同一次模型调用同时产出
// 「会话摘要 + 新增长期事实」。模型调用失败一律不影响主流程（返回空结果）。

import { tauriInvoke } from './ipc';
import { ONLINE_MODEL_ID } from '@/config/models';

export interface MemoryDoc {
  path: string;
  content: string;
  char_count: number;
  inject_limit: number;
  compact_threshold: number;
}

export interface TurnInsight {
  summary: string;
  facts: string[];
}

/** 读取记忆文档（不存在时由 Rust 侧按模板创建）。 */
export async function fetchMemoryDoc(): Promise<MemoryDoc> {
  const r = await tauriInvoke<MemoryDoc>('get_memory_doc');
  if (!r.data) throw new Error('读取记忆文档失败：后端没有返回数据');
  return r.data;
}

/** 覆盖保存记忆文档（Rust 侧会先留一份 .bak）。返回字符数。 */
export async function saveMemoryDoc(content: string): Promise<number> {
  const r = await tauriInvoke<number>('save_memory_doc', { content });
  return r.data ?? 0;
}

/** 追加长期事实：去重 + 同一 turnId 只写一次。返回真正新增条数。 */
export async function appendMemoryFacts(turnId: string, facts: string[]): Promise<number> {
  if (!facts.length) return 0;
  const r = await tauriInvoke<number>('append_memory_facts', { turnId, facts });
  return r.data ?? 0;
}

/** 在资源管理器里定位记忆文档。 */
export async function openMemoryDoc(): Promise<void> {
  await tauriInvoke('open_memory_doc');
}

/** 从文档里提取已有的条目行（喂给模型，避免重复产出）。 */
function existingEntries(content: string, limit = 40): string {
  return content
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.startsWith('- '))
    .slice(-limit)
    .join('\n');
}

/** 解析模型返回的 {summary, facts}，容错 markdown 代码块与多余文字。 */
export function parseInsight(text: string, fallbackSource: string): TurnInsight {
  const cleaned = text.replace(/```json/gi, '').replace(/```/g, '').trim();
  const start = cleaned.indexOf('{');
  const end = cleaned.lastIndexOf('}');
  if (start >= 0 && end > start) {
    try {
      const obj = JSON.parse(cleaned.slice(start, end + 1));
      const summary = typeof obj.summary === 'string' ? obj.summary.trim() : '';
      const facts = Array.isArray(obj.facts)
        ? obj.facts
            .filter((f: unknown): f is string => typeof f === 'string' && f.trim().length > 0)
            .map((f: string) => f.trim())
            .slice(0, 3)
        : [];
      return { summary: summary || fallbackSource.substring(0, 100).trim(), facts };
    } catch {
      // 落到下面的兜底
    }
  }
  return { summary: (cleaned || fallbackSource).substring(0, 200).trim(), facts: [] };
}

/**
 * 复用本地 Agent 端点做一次轻量模型调用，返回完整文本。
 * 失败（没有可用凭据 / 超时 / Agent 未启动）返回空串，调用方自行兜底。
 */
export async function callAgentOnce(prompt: string, timeoutMs = 20000): Promise<string> {
  try {
    const config: any = await tauriInvoke('get_agent_config');
    if (!config?.data?.port) return '';
    const keyRes: any = await tauriInvoke('get_api_key');
    const auth = keyRes?.data || '';
    const endpoint = 'http://127.0.0.1:' + config.data.port + '/agent/stream';
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      'X-LocalMind-Token': config.data.token,
    };
    const payload = {
      mode: 'online',
      model: ONLINE_MODEL_ID,
      messages: [{ role: 'user', content: prompt }],
      token: auth,
    };
    const res = await fetch(endpoint, {
      method: 'POST',
      headers,
      body: JSON.stringify(payload),
      signal: AbortSignal.timeout(timeoutMs),
    });
    if (!res.ok || !res.body) return '';
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let acc = '';
    let done = '';
    let buf = '';
    while (true) {
      const { done: finished, value } = await reader.read();
      if (finished) break;
      buf += decoder.decode(value, { stream: true });
      const events = buf.split('\n\n');
      buf = events.pop() || '';
      for (const ev of events) {
        if (!ev.trim().startsWith('data: ')) continue;
        try {
          const obj = JSON.parse(ev.trim().slice(6));
          if (obj.type === 'delta') acc += obj.text || '';
          if (obj.type === 'done') done = obj.content ?? acc;
        } catch {
          // 单个事件解析失败不影响整体
        }
      }
    }
    reader.releaseLock();
    return String(done || acc || '').trim();
  } catch {
    return '';
  }
}

/** 组装「会话摘要 + 新增长期事实」的提示词。 */
export function buildInsightPrompt(
  content: string,
  toolLogs: { name: string; output: string; success: boolean }[],
  existingMemory: string,
): string {
  const toolInfo = toolLogs.map((l) => l.name + ': ' + (l.success ? 'OK' : 'FAIL')).join(', ');
  return [
    '你在维护一个桌面 AI 助手的两类记忆。请阅读本轮对话，只输出一个 JSON 对象，不要解释、不要 markdown 代码块：',
    '{"summary": "本轮对话的关键结果，不超过100字", "facts": ["值得长期记住的稳定事实或偏好"]}',
    'facts 的规则：',
    '- 只写跨会话仍然有用的稳定信息：用户偏好、长期项目/环境、身份、明确要求记住的事；',
    '- 不要写一次性任务细节、时间、寒暄、工具临时输出；',
    '- 每条是完整的中文短句，不超过30字，最多3条，没有就返回空数组；',
    '- 不要重复下面的已有记忆。',
    '已有长期记忆：',
    existingEntries(existingMemory) || '（暂无）',
    '本轮工具调用：' + (toolInfo || '无'),
    '助手回复：' + content.substring(0, 800),
  ].join('\n');
}

/**
 * 每轮结束后的记忆素材：一次调用同时拿到会话摘要与长期事实。
 * 刻意复用原来那次「摘要」调用，长期记忆不额外增加模型调用次数。
 */
export async function generateTurnInsight(
  content: string,
  toolLogs: { name: string; output: string; success: boolean }[],
  existingMemory: string,
): Promise<TurnInsight> {
  const text = await callAgentOnce(buildInsightPrompt(content, toolLogs, existingMemory));
  if (!text) return { summary: '', facts: [] };
  return parseInsight(text, content);
}

/** 让模型把记忆文档改写得更紧凑（失败返回空串，调用方保留原文）。 */
export async function rewriteMemoryDoc(content: string): Promise<string> {
  const prompt = [
    '下面是一个 AI 助手的长期记忆文档（markdown）。请把它整理得更紧凑、更清晰：',
    '- 合并重复或近义条目，删掉一次性或过期的信息；',
    '- 按「用户画像 / 偏好与习惯 / 项目与环境 / 其他」分组，用 markdown 小标题加短句条目；',
    '- 保留文件开头的说明段；总长度控制在 1200 字以内；',
    '- 只输出整理后的 markdown 正文，不要任何解释。',
    '',
    content,
  ].join('\n');
  const text = await callAgentOnce(prompt, 30000);
  return text.length >= 40 ? text : '';
}

/** 记忆文档超长时整理一次（异步执行，任何失败都保留原文）。 */
export async function maybeCompactMemory(): Promise<void> {
  try {
    const doc = await fetchMemoryDoc();
    if (doc.char_count <= doc.compact_threshold) return;
    const compacted = await rewriteMemoryDoc(doc.content);
    if (!compacted) return;
    await saveMemoryDoc(compacted);
    console.info('[memory] 记忆文档已整理：' + doc.char_count + ' → ' + compacted.length + ' 字符');
  } catch (e) {
    console.warn('[memory] 记忆整理失败，保留原文档', e);
  }
}
