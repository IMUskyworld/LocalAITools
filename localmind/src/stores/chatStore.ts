import { create } from 'zustand';
import type { ChatSession, ChatMessage, ChatMode, ThinkingStep } from '@/types/chat';
import { generateId } from '@/utils/helpers';
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
  // Ollama 离线推理相关
  ollamaRunning: boolean;
  ollamaModels: OllamaModel[];
  selectedOllamaModel: string;
  // 文件附件相关
  attachments: SelectedAttachment[];
  // 工具调用记录（当前会话）
  toolCalls: ToolLog[];
  // Agent 思考轨迹（规划/执行/反思）
  thinkingSteps: ThinkingStep[];
}

export interface ChatStoreActions {
  createSession: (title?: string) => Promise<string>;
  switchSession: (sessionId: string) => Promise<void>;
  renameSession: (sessionId: string, title: string) => Promise<void>;
  deleteSession: (sessionId: string) => Promise<void>;
  sendMessage: (sessionId: string, content: string) => Promise<void>;
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
  persistUserMessage: (sessionId: string, content: string) => Promise<ChatMessage>;
  persistAssistantMessage: (sessionId: string, content: string) => Promise<ChatMessage>;
  // Ollama 离线推理相关
  refreshOllama: () => Promise<OllamaModel[]>;
  setSelectedOllamaModel: (model: string) => void;
  getSelectedOllamaModel: () => string;
  // 文件附件
  addAttachment: (attach: SelectedAttachment) => void;
  removeAttachment: (id: string) => void;
  clearAttachments: () => void;
  // 工具调用
  addToolCall: (log: ToolLog) => void;
  clearToolCalls: () => void;
  addThinkingStep: (step: ThinkingStep) => void;
  clearThinkingSteps: () => void;
}

type ChatStore = ChatStoreState & ChatStoreActions;

const DEFAULT_TITLE = 'New Chat';

export const useChatStore = create<ChatStore>((set, get) => ({
  sessions: [],
  currentSessionId: null,
  messages: {},
  mode: 'online' as ChatMode,
  isStreaming: false,
  error: null,
  modelName: 'Deepseek-V4-Pro',
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
      const r: any = await tauriInvoke('create_chat_session', { title: title || '' });
      if (r?.data) {
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
        }));
        return session.id;
      }
    } catch (e) {
      console.warn('create_chat_session failed, falling back to local:', e);
    }
    const id = generateId();
    const session: ChatSession = { id, title: title || DEFAULT_TITLE, createdAt: Date.now(), updatedAt: Date.now() };
    set((s) => ({ sessions: [...s.sessions, session], currentSessionId: id, messages: { ...s.messages, [id]: [] }, toolCalls: [], thinkingSteps: [] }));
    return id;
  },

  switchSession: async (sessionId: string) => {
    set({ currentSessionId: sessionId, toolCalls: [], thinkingSteps: [] });
    await get().loadMessages(sessionId);
  },

  renameSession: async (sessionId: string, title: string) => {
    try {
      await tauriInvoke('rename_chat_session', { sessionId, title });
    } catch (e) { console.warn('rename_chat_session failed:', e); }
    set((s) => ({
      sessions: s.sessions.map((s2) =>
        s2.id === sessionId ? { ...s2, title, updatedAt: Date.now() } : s2
      ),
    }));
  },

  deleteSession: async (sessionId: string) => {
    try {
      await tauriInvoke('delete_chat_session', { sessionId });
    } catch (e) { console.warn('delete_chat_session failed:', e); }
    set((s) => {
      const ns = s.sessions.filter((s2) => s2.id !== sessionId);
      const nm = { ...s.messages }; delete nm[sessionId];
      return {
        sessions: ns,
        messages: nm,
        currentSessionId:
          s.currentSessionId === sessionId ? (ns[ns.length - 1]?.id ?? null) : s.currentSessionId,
      };
    });
  },

  sendMessage: async (sessionId: string, content: string) => {
    try {
      const r: any = await tauriInvoke('send_message', {
        sessionId: sessionId,
        content,
      });
      if (r?.data) {
        const aiMsg: ChatMessage = {
          id: r.data.id,
          sessionId: r.data.session_id,
          role: r.data.role,
          content: r.data.content,
          timestamp: r.data.created_at,
        };
        set((s) => ({
          messages: { ...s.messages, [sessionId]: [...(s.messages[sessionId] || []), aiMsg] },
        }));
      }
    } catch (e) {
      console.warn('send_message failed:', e);
    }
  },

  stopGeneration: async () => {
    try { await tauriInvoke('stop_generation'); } catch (e) { console.warn('stop_generation failed:', e); }
    set({ isStreaming: false });
  },

  switchMode: async (mode: ChatMode) => {
    try {
      const r: any = await tauriInvoke('switch_mode', { mode });
      if (r?.data) {
        set({
          mode: r.data.mode,
          modelName: r.data.current_model_label || (mode === 'online' ? 'Deepseek-V4-Pro' : 'Local Model'),
          error: null,
        });
        return;
      }
    } catch (e: any) {
      set({ error: e?.toString() || 'Switch failed' });
    }
    set({ mode, modelName: mode === 'online' ? 'Deepseek-V4-Pro' : 'Local Model' });
  },

  loadSessions: async () => {
    try {
      const r: any = await tauriInvoke('list_sessions');
      if (r?.data) {
        const sessions: ChatSession[] = r.data.map((s: any) => ({
          id: s.id,
          title: s.title,
          createdAt: s.created_at,
          updatedAt: s.updated_at,
        }));
        set({ sessions });
      }
    } catch (e) { console.warn('list_sessions failed:', e); }
  },

  loadMessages: async (sessionId: string) => {
    try {
      const r: any = await tauriInvoke('list_messages', { sessionId });
      if (r?.data) {
        const msgs: ChatMessage[] = r.data.map((m: any) => ({
          id: m.id,
          sessionId: m.session_id,
          role: m.role,
          content: m.content,
          timestamp: m.created_at,
          modelName: m.model_label,
        }));
        set((s) => ({ messages: { ...s.messages, [sessionId]: msgs } }));
      }
    } catch (e) { console.warn('list_messages failed:', e); }
  },

  addMessage: (sessionId: string, message: ChatMessage) => {
    set((state) => {
      const sessionMessages = state.messages[sessionId] || [];
      return {
        messages: {
          ...state.messages,
          [sessionId]: [...sessionMessages, message],
        },
        sessions: state.sessions.map((s) =>
          s.id === sessionId ? { ...s, updatedAt: Date.now() } : s
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
          [sessionId]: sessionMessages.map((m) =>
            m.id === messageId ? { ...m, ...updates } : m
          ),
        },
      };
    });
  },

  setIsStreaming: (streaming: boolean) => {
    set({ isStreaming: streaming });
  },

  setError: (error: string | null) => {
    set({ error });
  },

  setLatency: (ms: number) => {
    set({ latencyMs: ms });
  },

  setTokensPerSecond: (tps: number) => {
    set({ tokensPerSecond: tps });
  },

  getCurrentSession: () => {
    const state = get();
    return state.sessions.find((s) => s.id === state.currentSessionId);
  },

  getCurrentMessages: () => {
    const state = get();
    if (!state.currentSessionId) return [];
    return state.messages[state.currentSessionId] || [];
  },

  persistUserMessage: async (sessionId: string, content: string): Promise<ChatMessage> => {
    const modelLabel = get().mode === 'online' ? 'Deepseek-V4-Pro' : 'Local Model';
    try {
      const r: any = await tauriInvoke('append_message', {
        sessionId,
        role: 'user',
        content,
        modelLabel,
      });
      if (r?.data) {
        return {
          id: r.data.id,
          sessionId: r.data.session_id,
          role: r.data.role,
          content: r.data.content,
          timestamp: r.data.created_at,
          modelName: r.data.model_label,
        };
      }
    } catch (e) { console.warn('append_message (user) failed:', e); }
    return {
      id: generateId(),
      sessionId,
      role: 'user',
      content,
      timestamp: Date.now(),
    };
  },

  persistAssistantMessage: async (sessionId: string, content: string): Promise<ChatMessage> => {
    const modelLabel = get().mode === 'online' ? 'Deepseek-V4-Pro' : 'Local Model';
    try {
      const r: any = await tauriInvoke('append_message', {
        sessionId,
        role: 'assistant',
        content,
        modelLabel,
      });
      if (r?.data) {
        return {
          id: r.data.id,
          sessionId: r.data.session_id,
          role: r.data.role,
          content: r.data.content,
          timestamp: r.data.created_at,
          modelName: r.data.model_label,
        };
      }
    } catch (e) { console.warn('append_message (assistant) failed:', e); }
    return {
      id: generateId(),
      sessionId,
      role: 'assistant',
      content,
      timestamp: Date.now(),
    };
  },

  // ===== Ollama 离线推理 =====

  refreshOllama: async () => {
    const status = await checkOllama();
    const models = status.models;
    set((s) => {
      // 保持当前选中模型有效；若无效或为空则选第一个
      let selected = s.selectedOllamaModel;
      if (models.length > 0 && !models.some((m) => m.name === selected)) {
        selected = models[0].name;
      }
      return {
        ollamaRunning: status.running,
        ollamaModels: models,
        selectedOllamaModel: selected,
      };
    });
    return models;
  },

  setSelectedOllamaModel: (model: string) => {
    set({ selectedOllamaModel: model, modelName: model });
  },

  getSelectedOllamaModel: () => get().selectedOllamaModel,

  // ===== 文件附件 =====

  addAttachment: (attach: SelectedAttachment) => {
    set((s) => ({ attachments: [...s.attachments, attach] }));
  },

  removeAttachment: (id: string) => {
    set((s) => ({ attachments: s.attachments.filter((a) => a.id !== id) }));
  },

  clearAttachments: () => {
    set({ attachments: [] });
  },

  // ===== 工具调用 =====

  addToolCall: (log: ToolLog) => {
    set((s) => ({ toolCalls: [...s.toolCalls, log] }));
  },

  clearToolCalls: () => {
    set({ toolCalls: [] });
  },

  // ===== 思考轨迹 =====

  addThinkingStep: (step: ThinkingStep) => {
    set((s) => ({ thinkingSteps: [...s.thinkingSteps, step] }));
  },

  clearThinkingSteps: () => {
    set({ thinkingSteps: [] });
  },
}));