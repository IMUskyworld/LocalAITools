import { ONLINE_MODEL_LABEL, OFFLINE_MODEL_LABEL } from '@/config/models';
import { create } from 'zustand';
import type { ChatSession, ChatMessage, ChatMode, ThinkingStep } from '@/types/chat';
import { tauriInvoke } from '@/api/ipc';
import { checkOllama, type OllamaModel } from '@/api/ollama';
import type { SelectedAttachment } from '@/api/files';
import type { ToolLog } from '@/api/agent';

export interface ChatStoreState {
  sessions: ChatSession[];
  currentSessionId: string | null;
  messages: Record<string, ChatMessage[]>;
  mode: ChatMode;
  isStreaming: boolean;
  error: string | null;
  modelName: string;
  latencyMs: number;
  tokensPerSecond: number;
  ollamaRunning: boolean;
  ollamaModels: OllamaModel[];
  selectedOllamaModel: string;
  attachments: SelectedAttachment[];
  toolCalls: ToolLog[];
  thinkingSteps: ThinkingStep[];
}

export interface TurnStartResult {
  turnId: string;
  userMessage: ChatMessage;
}

export interface ChatStoreActions {
  createSession: (title?: string) => Promise<string>;
  switchSession: (sessionId: string) => Promise<void>;
  renameSession: (sessionId: string, title: string) => Promise<void>;
  deleteSession: (sessionId: string) => Promise<void>;
  stopGeneration: () => Promise<void>;
  switchMode: (mode: ChatMode) => Promise<void>;
  addMessage: (sessionId: string, message: ChatMessage) => void;
  updateMessage: (sessionId: string, messageId: string, updates: Partial<ChatMessage>) => void;
  loadSessions: () => Promise<void>;
  loadMessages: (sessionId: string) => Promise<void>;
  setIsStreaming: (streaming: boolean) => void;
  setError: (error: string | null) => void;
  setLatency: (ms: number) => void;
  setTokensPerSecond: (tps: number) => void;
  getCurrentSession: () => ChatSession | undefined;
  getCurrentMessages: () => ChatMessage[];
  beginTurn: (sessionId: string, content: string, modelLabel?: string) => Promise<TurnStartResult>;
  completeTurn: (turnId: string, content: string, modelLabel?: string) => Promise<ChatMessage>;
  failTurn: (
    turnId: string,
    status: 'failed' | 'cancelled' | 'interrupted',
    errorCode?: string,
    errorMessage?: string,
  ) => Promise<void>;
  refreshOllama: () => Promise<OllamaModel[]>;
  setSelectedOllamaModel: (model: string) => void;
  getSelectedOllamaModel: () => string;
  addAttachment: (attach: SelectedAttachment) => void;
  removeAttachment: (id: string) => void;
  clearAttachments: () => void;
  addToolCall: (log: ToolLog) => void;
  clearToolCalls: () => void;
  addThinkingStep: (step: ThinkingStep) => void;
  clearThinkingSteps: () => void;
}

type ChatStore = ChatStoreState & ChatStoreActions;

function messageFromDto(data: any): ChatMessage {
  return {
    id: data.id,
    sessionId: data.session_id,
    role: data.role,
    content: data.content,
    timestamp: data.created_at,
    modelName: data.model_label,
  };
}

function errorText(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === 'string') return error;
  return '操作失败';
}

function modelLabel(mode: ChatMode, selectedModel: string): string {
  return mode === 'online' ? ONLINE_MODEL_LABEL : (selectedModel || OFFLINE_MODEL_LABEL);
}

export const useChatStore = create<ChatStore>((set, get) => ({
  sessions: [],
  currentSessionId: null,
  messages: {},
  mode: 'online' as ChatMode,
  isStreaming: false,
  error: null,
  modelName: ONLINE_MODEL_LABEL,
  latencyMs: 0,
  tokensPerSecond: 0,
  ollamaRunning: false,
  ollamaModels: [],
  selectedOllamaModel: '',
  attachments: [],
  toolCalls: [],
  thinkingSteps: [],

  createSession: async (title?: string) => {
    try {
      const r = await tauriInvoke<any>('create_chat_session', { title: title || '' });
      if (!r.data) throw new Error('创建会话失败：后端没有返回会话数据');
      const session: ChatSession = {
        id: r.data.id,
        title: r.data.title,
        createdAt: r.data.created_at,
        updatedAt: r.data.updated_at,
      };
      set((s) => ({
        sessions: [...s.sessions, session],
        currentSessionId: session.id,
        messages: { ...s.messages, [session.id]: [] },
        toolCalls: [],
        thinkingSteps: [],
        error: null,
      }));
      return session.id;
    } catch (error) {
      set({ error: errorText(error) });
      throw error;
    }
  },

  switchSession: async (sessionId: string) => {
    set({ currentSessionId: sessionId, toolCalls: [], thinkingSteps: [] });
    await get().loadMessages(sessionId);
  },

  renameSession: async (sessionId: string, title: string) => {
    try {
      await tauriInvoke('rename_chat_session', { sessionId, title });
      set((s) => ({
        sessions: s.sessions.map((item) =>
          item.id === sessionId ? { ...item, title, updatedAt: Date.now() } : item
        ),
        error: null,
      }));
    } catch (error) {
      set({ error: errorText(error) });
      throw error;
    }
  },

  deleteSession: async (sessionId: string) => {
    try {
      await tauriInvoke('delete_chat_session', { sessionId });
      set((s) => {
        const sessions = s.sessions.filter((item) => item.id !== sessionId);
        const messages = { ...s.messages };
        delete messages[sessionId];
        return {
          sessions,
          messages,
          currentSessionId:
            s.currentSessionId === sessionId ? (sessions[sessions.length - 1]?.id ?? null) : s.currentSessionId,
          error: null,
        };
      });
    } catch (error) {
      set({ error: errorText(error) });
      throw error;
    }
  },

  stopGeneration: async () => {
    try {
      await tauriInvoke('stop_generation');
      set({ isStreaming: false, error: null });
    } catch (error) {
      set({ isStreaming: false, error: errorText(error) });
      throw error;
    }
  },

  switchMode: async (mode: ChatMode) => {
    try {
      const r = await tauriInvoke<any>('switch_mode', { mode });
      if (!r.data) throw new Error('切换模式失败：后端没有返回模式数据');
      set({
        mode: r.data.mode,
        modelName: r.data.current_model_label || (mode === 'online' ? ONLINE_MODEL_LABEL : OFFLINE_MODEL_LABEL),
        error: null,
      });
    } catch (error) {
      set({ error: errorText(error) });
      throw error;
    }
  },

  loadSessions: async () => {
    try {
      const r = await tauriInvoke<any[]>('list_sessions');
      const sessions: ChatSession[] = (r.data || []).map((item) => ({
        id: item.id,
        title: item.title,
        createdAt: item.created_at,
        updatedAt: item.updated_at,
      }));
      const currentSessionId = sessions.length > 0 && !get().currentSessionId ? sessions[0].id : get().currentSessionId;
      set({ sessions, currentSessionId, error: null });
    } catch (error) {
      set({ error: errorText(error) });
      throw error;
    }
  },

  loadMessages: async (sessionId: string) => {
    try {
      const r = await tauriInvoke<any[]>('list_messages', { sessionId });
      const messages: ChatMessage[] = (r.data || []).map(messageFromDto);
      set((s) => ({ messages: { ...s.messages, [sessionId]: messages }, error: null }));
    } catch (error) {
      set({ error: errorText(error) });
      throw error;
    }
  },

  addMessage: (sessionId: string, message: ChatMessage) => {
    set((state) => {
      const sessionMessages = state.messages[sessionId] || [];
      return {
        messages: { ...state.messages, [sessionId]: [...sessionMessages, message] },
        sessions: state.sessions.map((session) =>
          session.id === sessionId ? { ...session, updatedAt: Date.now() } : session
        ),
      };
    });
  },

  updateMessage: (sessionId: string, messageId: string, updates: Partial<ChatMessage>) => {
    set((state) => {
      const sessionMessages = state.messages[sessionId] || [];
      return {
        messages: {
          ...state.messages,
          [sessionId]: sessionMessages.map((message) =>
            message.id === messageId ? { ...message, ...updates } : message
          ),
        },
      };
    });
  },

  setIsStreaming: (streaming: boolean) => set({ isStreaming: streaming }),
  setError: (error: string | null) => set({ error }),
  setLatency: (ms: number) => set({ latencyMs: ms }),
  setTokensPerSecond: (tps: number) => set({ tokensPerSecond: tps }),

  getCurrentSession: () => {
    const state = get();
    return state.sessions.find((session) => session.id === state.currentSessionId);
  },

  getCurrentMessages: () => {
    const state = get();
    if (!state.currentSessionId) return [];
    return state.messages[state.currentSessionId] || [];
  },

  beginTurn: async (sessionId: string, content: string, label?: string) => {
    try {
      const r = await tauriInvoke<any>('turn_begin', {
        sessionId,
        content,
        modelLabel: label || modelLabel(get().mode, get().selectedOllamaModel),
      });
      if (!r.data) throw new Error('创建 Turn 失败：后端没有返回 Turn 数据');
      return {
        turnId: r.data.turn_id,
        userMessage: messageFromDto(r.data.user_message),
      };
    } catch (error) {
      set({ error: errorText(error) });
      throw error;
    }
  },

  completeTurn: async (turnId: string, content: string, label?: string) => {
    try {
      const r = await tauriInvoke<any>('turn_complete', {
        turnId,
        content,
        modelLabel: label || modelLabel(get().mode, get().selectedOllamaModel),
      });
      if (!r.data) throw new Error('完成 Turn 失败：后端没有返回消息数据');
      return messageFromDto(r.data);
    } catch (error) {
      set({ error: errorText(error) });
      throw error;
    }
  },

  failTurn: async (turnId, status, errorCode, errorMessage) => {
    try {
      await tauriInvoke('turn_fail', {
        turnId,
        status,
        errorCode: errorCode || 'AGENT_ERROR',
        errorMessage: errorMessage || '',
      });
    } catch (error) {
      set({ error: errorText(error) });
      throw error;
    }
  },

  refreshOllama: async () => {
    const status = await checkOllama();
    const models = status.models;
    set((s) => {
      let selected = s.selectedOllamaModel;
      if (models.length > 0 && !models.some((model) => model.name === selected)) {
        selected = models[0].name;
      }
      return { ollamaRunning: status.running, ollamaModels: models, selectedOllamaModel: selected };
    });
    return models;
  },

  setSelectedOllamaModel: (model: string) => set({ selectedOllamaModel: model, modelName: model }),
  getSelectedOllamaModel: () => get().selectedOllamaModel,

  addAttachment: (attach: SelectedAttachment) => set((s) => ({ attachments: [...s.attachments, attach] })),
  removeAttachment: (id: string) => set((s) => ({ attachments: s.attachments.filter((item) => item.id !== id) })),
  clearAttachments: () => set({ attachments: [] }),

  addToolCall: (log: ToolLog) => set((s) => ({ toolCalls: [...s.toolCalls, log] })),
  clearToolCalls: () => set({ toolCalls: [] }),
  addThinkingStep: (step: ThinkingStep) => set((s) => ({ thinkingSteps: [...s.thinkingSteps, step] })),
  clearThinkingSteps: () => set({ thinkingSteps: [] }),
}));
