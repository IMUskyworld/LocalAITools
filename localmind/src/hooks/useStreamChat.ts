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
  const persistUserMessage = useChatStore((s) => s.persistUserMessage);
  const persistAssistantMessage = useChatStore((s) => s.persistAssistantMessage);
  const selectedOllamaModel = useChatStore((s) => s.selectedOllamaModel);
  const attachments = useChatStore((s) => s.attachments);
  const addToolCall = useChatStore((s) => s.addToolCall);
  const addThinkingStep = useChatStore((s) => s.addThinkingStep);
  const clearToolCalls = useChatStore((s) => s.clearToolCalls);
  const clearThinkingSteps = useChatStore((s) => s.clearThinkingSteps);
  // 当前请求的取消控制器（停止按钮真正中止 Agent 请求）
  const abortRef = useRef<AbortController | null>(null);

  const sendMessage = useCallback(async (content: string) => {
    const sessionId = currentSessionId;
    if (!sessionId) return;

    // 清空上次的工具调用记录 + 思考轨迹
    clearToolCalls();
    clearThinkingSteps();

    // 显示用户消息
    const userMsg: ChatMessage = {
      id: generateId(), sessionId, role: 'user', content, timestamp: Date.now(),
    };
    addMessage(sessionId, userMsg);
    persistUserMessage(sessionId, content).catch(() => {});

    setIsStreaming(true);
    setError(null);

    // 记录取消控制器，停止按钮可中止本次请求
    const controller = new AbortController();
    abortRef.current = controller;

    // AI 消息占位
    const assistantId = generateId();
    const activeModelLabel = mode === 'online'
      ? 'Deepseek-V4-Pro'
      : (selectedOllamaModel || '本地模型');
    addMessage(sessionId, {
      id: assistantId, sessionId, role: 'assistant', content: '',
      timestamp: Date.now(), isStreaming: true, modelName: activeModelLabel,
    });

    try {
      // 组装历史消息（含附件注入）
      const baseMessages = getCurrentMessages()
        .filter((m) => m.id !== assistantId)
        .map((m) => ({ role: m.role as 'user' | 'assistant', content: m.content }));
      const agentMessages: AgentMessage[] = withAttachments(baseMessages, attachments);

      // 走 agent 循环（AI 可调工具操作电脑）
      const result = await runAgent({
        mode,
        model: selectedOllamaModel,
        messages: agentMessages,
        signal: controller.signal,
        onToolCall: (log) => {
          addToolCall(log);
        },
        onThinking: (step) => {
          addThinkingStep(step);
        },
      });

      updateMessage(sessionId, assistantId, {
        content: result.content,
        isStreaming: false,
      });
      persistAssistantMessage(sessionId, result.content).catch(() => {});
    } catch (e: any) {
      if (controller.signal.aborted) {
        // 用户主动停止：不显示错误，占位标记
        updateMessage(sessionId, assistantId, {
          content: '（已停止）',
          isStreaming: false,
        });
      } else {
        const errMsg = e?.message || 'AI 调用失败';
        updateMessage(sessionId, assistantId, {
          content: '**错误**：' + errMsg,
          isStreaming: false,
        });
        setError(errMsg);
      }
    } finally {
      if (abortRef.current === controller) abortRef.current = null;
      setIsStreaming(false);
    }
  }, [
    currentSessionId, mode, selectedOllamaModel, attachments,
    addMessage, updateMessage, setIsStreaming, setError,
    getCurrentMessages, persistUserMessage, persistAssistantMessage,
    addToolCall, clearToolCalls, addThinkingStep, clearThinkingSteps,
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

  const parts = attachments.map((a) => {
    const header = `【附件：${a.file_name}${a.truncated ? '（内容过长已截断）' : ''}】`;
    return `${header}\n\`\`\`\n${a.content}\n\`\`\``;
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
