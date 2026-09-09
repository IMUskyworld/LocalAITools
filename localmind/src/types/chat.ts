// Chat-related types

export type ChatMode = 'online' | 'offline';

export interface ChatSession {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  modelName?: string;
}

export interface ChatMessage {
  id: string;
  sessionId: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: number;
  modelName?: string;
  isStreaming?: boolean;
  tokensPerSecond?: number;
  latencyMs?: number;
}

export interface StreamEvent {
  type: 'delta' | 'done' | 'error';
  messageId: string;
  delta?: string;
  fullContent?: string;
  error?: string;
  tokensPerSecond?: number;
  latencyMs?: number;
}

export interface ChatState {
  messages: ChatMessage[];
  session: ChatSession;
  mode: ChatMode;
  isStreaming: boolean;
  error: string | null;
}

/** Agent 思考阶段 */
export type ThinkingPhase = 'planning' | 'executing' | 'reflecting' | 'retrying' | 'done';

/** Agent 思考步骤（规划/执行/反思） */
export interface ThinkingStep {
  id: string;
  phase: ThinkingPhase;
  label: string;          // "正在制定计划...", "执行: write_file", "反思: 参数错误"
  detail?: string;        // 可选富文本详情（计划步骤、反思建议）
  status: 'pending' | 'running' | 'success' | 'error';
  timestamp: number;
}
