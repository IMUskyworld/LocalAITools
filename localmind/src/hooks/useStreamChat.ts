import { ONLINE_MODEL_ID, ONLINE_MODEL_LABEL, OFFLINE_MODEL_LABEL } from '@/config/models';
import { useCallback, useRef } from 'react';
import { useChatStore } from '@/stores/chatStore';
import { tauriInvoke } from '@/api/ipc';
import { runAgent, type AgentMessage } from '@/api/agent';
import { appendMemoryFacts, fetchMemoryDoc, generateTurnInsight, maybeCompactMemory } from '@/api/memory';
import { generateId } from '@/utils/helpers';
import type { ChatMessage } from '@/types/chat';

interface UseStreamChatReturn {
  sendMessage: (content: string) => Promise<void>;
  stopGeneration: () => void;
  isStreaming: boolean;
  error: string | null;
}

function messageOf(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === 'string') return error;
  return 'AI 调用失败';
}

export function useStreamChat(): UseStreamChatReturn {
  const currentSessionId = useChatStore((s) => s.currentSessionId);
  const mode = useChatStore((s) => s.mode);
  const isStreaming = useChatStore((s) => s.isStreaming);
  const error = useChatStore((s) => s.error);
  const addMessage = useChatStore((s) => s.addMessage);
  const updateMessage = useChatStore((s) => s.updateMessage);
  const setIsStreaming = useChatStore((s) => s.setIsStreaming);
  const setError = useChatStore((s) => s.setError);
  const getCurrentMessages = useChatStore((s) => s.getCurrentMessages);
  const beginTurn = useChatStore((s) => s.beginTurn);
  const completeTurn = useChatStore((s) => s.completeTurn);
  const failTurn = useChatStore((s) => s.failTurn);
  const selectedOllamaModel = useChatStore((s) => s.selectedOllamaModel);
  const attachments = useChatStore((s) => s.attachments);
  const addToolCall = useChatStore((s) => s.addToolCall);
  const addThinkingStep = useChatStore((s) => s.addThinkingStep);
  const requestConfirm = useChatStore((s) => s.requestConfirm);
  const clearToolCalls = useChatStore((s) => s.clearToolCalls);
  const clearThinkingSteps = useChatStore((s) => s.clearThinkingSteps);
  const abortRef = useRef<AbortController | null>(null);

  const sendMessage = useCallback(async (content: string) => {
    const sessionId = currentSessionId;
    if (!sessionId) return;

    clearToolCalls();
    clearThinkingSteps();
    setIsStreaming(true);
    setError(null);

    const controller = new AbortController();
    abortRef.current = controller;
    const activeModelLabel = mode === 'online'
      ? ONLINE_MODEL_LABEL
      : (selectedOllamaModel || OFFLINE_MODEL_LABEL);

    let turnId = '';
    let assistantId = '';
    let partialContent = '';

    try {
      const start = await beginTurn(sessionId, content, activeModelLabel);
      turnId = start.turnId;
      addMessage(sessionId, start.userMessage);

      assistantId = generateId();
      addMessage(sessionId, {
        id: assistantId,
        sessionId,
        role: 'assistant',
        content: '',
        timestamp: Date.now(),
        isStreaming: true,
        modelName: activeModelLabel,
      });

      // 加载 session summary（跨轮记忆注入）
      let sessionSummary = '';
      try {
        const sr: any = await tauriInvoke('get_session_summary', { sessionId });
        if (sr?.data) sessionSummary = sr.data;
      } catch {}

      // 长期记忆：跨会话的稳定事实（不存在时 Rust 侧会按模板创建；超长自动截断）
      let memoryText = '';
      try {
        memoryText = (await fetchMemoryDoc()).content || '';
      } catch (e) {
        console.warn('[memory] 读取长期记忆失败，本轮不注入', e);
      }

      const baseMessages = getCurrentMessages()
        .filter((message) => message.id !== assistantId)
        .map((message) => ({
          role: message.role as 'user' | 'assistant',
          content: message.content,
        }));
      const agentMessages: AgentMessage[] = withMemory(
        withSummary(withAttachments(baseMessages, attachments), sessionSummary),
        memoryText,
      );

      const result = await runAgent({
        mode,
        model: mode === 'online' ? ONLINE_MODEL_ID : selectedOllamaModel,
        messages: agentMessages,
        selectedAttachmentPaths: attachments.map((attachment) => attachment.path),
        signal: controller.signal,
        onToolCall: (log) => {
          addToolCall(log);
          // 工具调用写入审计日志（风险等级按工具取，原来写死 L2 会把
          // run_command / delete_path 这类高危操作记成中风险）
          tauriInvoke('save_audit_log', {
            sessionId,
            action: log.name,
            target: log.args?.substring?.(0, 200) || null,
            riskLevel: riskLevelOf(log.name),
            result: log.success ? 'success' : 'failed',
            detail: log.output?.substring?.(0, 500) || null,
          }).catch(() => {});
        },
        onThinking: addThinkingStep,
        // 高危工具（run_command / delete_path）需要用户确认。
        // 走 store + UI 卡片，而不是 window.confirm —— 后者在 Tauri WebView 里不可靠。
        onConfirm: requestConfirm,
      });
      partialContent = result.content;

      // 将工具调用嵌入消息内容，使跨轮对话保留完整上下文
      const contentWithTools = embedToolLogs(result.content, result.toolLogs);

      const saved = await completeTurn(
        turnId,
        contentWithTools,
        activeModelLabel,
        result.usage?.total_tokens ?? undefined,
      );

      // 每轮结束后的记忆维护（异步，不阻塞 UI）：
      // 一次调用同时产出「会话摘要」与「新的长期事实」——
      // 摘要进 session_summaries（服务本会话），事实去重后进记忆文档（服务跨会话）。
      // 注意必须喂「助手回复正文 + 工具结果」，不能喂用户输入（早期版本传错，摘要跑偏）。
      generateTurnInsight(contentWithTools, result.toolLogs, memoryText)
        .then((insight) => {
          const summary = insight.summary || contentWithTools.substring(0, 200).trim();
          if (summary) {
            tauriInvoke('save_session_summary', { sessionId, summary }).catch((err) =>
              console.warn('[memory] 会话摘要保存失败', err),
            );
          }
          if (insight.facts.length) {
            appendMemoryFacts(turnId, insight.facts)
              .then((added) => {
                if (added > 0) void maybeCompactMemory();
              })
              .catch((err) => console.warn('[memory] 长期记忆写入失败', err));
          }
        })
        .catch((err) => console.warn('[memory] 记忆维护失败', err));
      updateMessage(sessionId, assistantId, {
        id: saved.id,
        content: saved.content,
        timestamp: saved.timestamp,
        modelName: saved.modelName,
        isStreaming: false,
        usage: result.usage ?? undefined,
        latencyMs: result.elapsedMs || undefined,
        tokensPerSecond:
          result.usage && result.elapsedMs
            ? Math.round((result.usage.output_tokens / (result.elapsedMs / 1000)) * 10) / 10
            : undefined,
      });
    } catch (e: unknown) {
      const aborted = controller.signal.aborted;
      try {
        if (turnId) {
          await failTurn(
            turnId,
            aborted ? 'cancelled' : 'failed',
            aborted ? 'USER_CANCELLED' : 'AGENT_ERROR',
            aborted ? '用户停止生成' : messageOf(e),
          );
        }
      } catch (turnError) {
        setError(`保存 Turn 最终状态失败: ${messageOf(turnError)}`);
      }

      if (assistantId) {
        updateMessage(sessionId, assistantId, {
          content: aborted ? '（已停止）' : partialContent || `**错误**：${messageOf(e)}`,
          isStreaming: false,
        });
      }
      if (!aborted) setError(messageOf(e));
    } finally {
      if (abortRef.current === controller) abortRef.current = null;
      setIsStreaming(false);
    }
  }, [
    currentSessionId, mode, selectedOllamaModel, attachments,
    addMessage, updateMessage, setIsStreaming, setError, getCurrentMessages,
    beginTurn, completeTurn, failTurn, addToolCall, clearToolCalls,
    addThinkingStep, clearThinkingSteps, requestConfirm,
  ]);

  const stopGeneration = useCallback(() => {
    // 停止生成时若还有等待确认的高危操作，先按「拒绝」结掉：
    // 否则确认卡片会留在界面上，而且事后再点「确认执行」仍会把工具真的跑起来。
    useChatStore.getState().resolveConfirm(false);
    abortRef.current?.abort();
    abortRef.current = null;
    setIsStreaming(false);
  }, [setIsStreaming]);

  return { sendMessage, stopGeneration, isStreaming, error };
}

// ========== 工具风险等级（与 localmind/scripts/tool_registry.py 对齐） ==========

const TOOL_RISK_LEVELS: Record<string, string> = {
  read_file: 'L0',
  list_dir: 'L0',
  read_clipboard: 'L1',
  open_app: 'L1',
  write_file: 'L2',
  move_file: 'L2',
  create_doc: 'L2',
  delete_path: 'L3',
  run_command: 'L4',
};

function riskLevelOf(toolName: string): string {
  return TOOL_RISK_LEVELS[toolName] ?? 'L2';
}

// ========== 长期记忆注入 ==========

/**
 * 把长期记忆作为 system 消息注入，放在会话摘要之前。
 * 提示词里明确要求"不要主动复述、也不要据此覆盖用户当下的明确要求"，
 * 避免记忆喧宾夺主。
 */
function withMemory(messages: AgentMessage[], memory: string): AgentMessage[] {
  const block = memory.trim();
  if (!block) return messages;
  const content =
    '【长期记忆】\n以下是此前对话中沉淀下来的、关于用户的长期事实与偏好，可作为背景参考：\n' +
    '（不要主动逐条复述；如果与用户当下的明确要求冲突，以当下要求为准）\n\n' +
    block;
  const first = messages[0];
  if (first?.role === 'system') {
    return [{ role: 'system', content: `${content}\n\n${first.content}` }, ...messages.slice(1)];
  }
  return [{ role: 'system', content }, ...messages];
}

// ========== Session Summary 注入 ==========

function withSummary(messages: AgentMessage[], summary: string): AgentMessage[] {
  if (!summary) return messages;
  const summaryBlock =
    '【历史对话摘要】\n' +
    '以下是之前对话的摘要，请在回答时参考这些上下文：\n' +
    summary;
  if (messages.length > 0 && messages[0].role === 'system') {
    return [
      { role: 'system', content: `${messages[0].content}\n\n${summaryBlock}` },
      ...messages.slice(1),
    ];
  }
  return [{ role: 'system', content: summaryBlock }, ...messages];
}

// ========== 工具调用嵌入（跨轮记忆修复） ==========

const FINAL_REPLY_MARKER = '[最终回复]';

function embedToolLogs(content: string, toolLogs: { name: string; args: string; output: string; success: boolean }[]): string {
  if (!toolLogs || toolLogs.length === 0) return content;
  const blocks = toolLogs.map((log) => {
    const argsStr = typeof log.args === 'string' ? log.args : JSON.stringify(log.args);
    return `[调用工具:${log.name}] ${argsStr}\n[工具结果:${log.name}] ${log.success ? '' : '(失败) '}${log.output}`;
  });
  // 用显式分隔符标记「工具块结束、最终回复开始」，
  // 否则 Python 侧无法区分最后一个工具的输出和助手正文。
  return blocks.join('\n') + '\n' + FINAL_REPLY_MARKER + '\n' + content;
}

// ========== 附件注入 ==========

interface ConvMsg {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

function withAttachments(base: ConvMsg[], attachments: any[]): AgentMessage[] {
  if (!attachments || attachments.length === 0) return base as AgentMessage[];

  const parts = attachments.map((attachment) => {
    const header = `【附件：${attachment.file_name}${attachment.truncated ? '（内容过长已截断）' : ''}】`;
    return `${header}\n\`\`\`\n${attachment.content}\n\`\`\``;
  });
  const attachmentContext =
    '以下是用户附加的文件内容，请基于这些内容回答用户的问题：\n\n' +
    parts.join('\n\n');

  const hasSystem = base.length > 0 && base[0].role === 'system';
  if (hasSystem) {
    return [
      { role: 'system', content: `${base[0].content}\n\n${attachmentContext}` },
      ...base.slice(1),
    ];
  }
  return [{ role: 'system', content: attachmentContext }, ...base];
}
