import { ONLINE_MODEL_ID, ONLINE_MODEL_LABEL, OFFLINE_MODEL_LABEL } from '@/config/models';
import { useCallback, useRef } from 'react';
import { useChatStore } from '@/stores/chatStore';
import { runAgent, type AgentMessage } from '@/api/agent';
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

      const baseMessages = getCurrentMessages()
        .filter((message) => message.id !== assistantId)
        .map((message) => ({
          role: message.role as 'user' | 'assistant',
          content: message.content,
        }));
      const agentMessages: AgentMessage[] = withAttachments(baseMessages, attachments);

      const result = await runAgent({
        mode,
        model: mode === 'online' ? ONLINE_MODEL_ID : selectedOllamaModel,
        messages: agentMessages,
        selectedAttachmentPaths: attachments.map((attachment) => attachment.path),
        signal: controller.signal,
        onToolCall: addToolCall,
        onThinking: addThinkingStep,
      });
      partialContent = result.content;

      const saved = await completeTurn(turnId, result.content, activeModelLabel);
      updateMessage(sessionId, assistantId, {
        id: saved.id,
        content: saved.content,
        timestamp: saved.timestamp,
        modelName: saved.modelName,
        isStreaming: false,
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
    addThinkingStep, clearThinkingSteps,
  ]);

  const stopGeneration = useCallback(() => {
    abortRef.current?.abort();
    abortRef.current = null;
    setIsStreaming(false);
  }, [setIsStreaming]);

  return { sendMessage, stopGeneration, isStreaming, error };
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
