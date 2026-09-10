import React, { useState, useRef, useEffect, useCallback } from 'react';
import { useChatStore } from '@/stores/chatStore';
import { useStreamChat } from '@/hooks/useStreamChat';
import MessageBubble from '@/components/MessageBubble';
import { formatRelativeTime, truncate } from '@/utils/helpers';
import { pickAndReadFile, formatFileSize } from '@/api/files';
import type { ChatMessage } from '@/types/chat';

const EMPTY_MESSAGES: ChatMessage[] = [];

const QUICK_COMMANDS = [
  {
    label: '总结剪贴板',
    desc: '一键总结剪贴板内容',
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect x="8" y="2" width="8" height="4" rx="1" />
        <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
        <path d="m9 14 2 2 4-4" />
      </svg>
    ),
    prompt: '请总结当前剪贴板的内容',
  },
  {
    label: '整理桌面',
    desc: '自动分类桌面文件',
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2Z" />
      </svg>
    ),
    prompt: '请帮我整理桌面文件',
  },
  {
    label: '生成 PPT',
    desc: '制作演示文稿',
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect x="3" y="3" width="18" height="12" rx="2" />
        <path d="M3 11h18M12 3v8M8 21l4-4 4 4" />
      </svg>
    ),
    prompt: '请帮我生成一份 PPT 演示文稿',
  },
  {
    label: '翻译文本',
    desc: '中英互译 / 多语言',
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="10" />
        <path d="M12 2a15 15 0 0 1 0 20 15 15 0 0 1 0-20Z" />
        <path d="M2 12h20" />
      </svg>
    ),
    prompt: '请帮我翻译这段文本',
  },
  {
    label: '代码助手',
    desc: '写代码、找 Bug',
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="m8 6-5 6 5 6M16 6l5 6-5 6" />
      </svg>
    ),
    prompt: '请帮我写代码 / 排查 Bug',
  },
  {
    label: '打开应用',
    desc: '对话唤起桌面应用',
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91-.09z" />
        <path d="m12 15-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 0 1-4 2z" />
        <path d="M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0" />
        <path d="M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5" />
      </svg>
    ),
    prompt: '请帮我打开应用...',
  },
];

export default function ChatPage() {
  const sessions = useChatStore((s) => s.sessions);
  const currentSessionId = useChatStore((s) => s.currentSessionId);
  const messages = useChatStore((s) =>
    s.currentSessionId ? (s.messages[s.currentSessionId] || EMPTY_MESSAGES) : EMPTY_MESSAGES
  );
  const mode = useChatStore((s) => s.mode);
  const tokensPerSecond = useChatStore((s) => s.tokensPerSecond);
  const ollamaRunning = useChatStore((s) => s.ollamaRunning);
  const ollamaModels = useChatStore((s) => s.ollamaModels);
  const selectedOllamaModel = useChatStore((s) => s.selectedOllamaModel);
  const setSelectedOllamaModel = useChatStore((s) => s.setSelectedOllamaModel);
  const refreshOllama = useChatStore((s) => s.refreshOllama);
  const attachments = useChatStore((s) => s.attachments);
  const addAttachment = useChatStore((s) => s.addAttachment);
  const removeAttachment = useChatStore((s) => s.removeAttachment);
  const clearAttachments = useChatStore((s) => s.clearAttachments);
  const toolCalls = useChatStore((s) => s.toolCalls);
  const thinkingSteps = useChatStore((s) => s.thinkingSteps);
  const createSession = useChatStore((s) => s.createSession);
  const switchSession = useChatStore((s) => s.switchSession);
  const deleteSession = useChatStore((s) => s.deleteSession);
  const renameSession = useChatStore((s) => s.renameSession);
  const loadSessions = useChatStore((s) => s.loadSessions);

  const { sendMessage, stopGeneration, isStreaming, error } = useStreamChat();

  const [inputText, setInputText] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Load sessions from backend on mount
  useEffect(() => {
    loadSessions().catch(() => {});
  }, [loadSessions]);

  // Auto-scroll on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  // Create initial session if none exists after loading
  useEffect(() => {
    if (sessions.length === 0) {
      createSession().catch(() => {});
    }
  }, [sessions.length, createSession]);

  // 静默检测 Ollama，提前加载模型列表
  useEffect(() => {
    refreshOllama();
  }, [refreshOllama]);

  const handleSend = useCallback(() => {
    const text = inputText.trim();
    if (!text || !currentSessionId || isStreaming) return;

    // Rename session if first message
    const session = sessions.find((s) => s.id === currentSessionId);
    if (session && session.title === '新对话') {
      renameSession(currentSessionId, truncate(text, 30)).catch(() => {});
    }

    setInputText('');
    // 清空已发送的附件（内容已随消息进入上下文）
    if (attachments.length > 0) clearAttachments();
    sendMessage(text);
  }, [inputText, currentSessionId, isStreaming, sessions, renameSession, sendMessage, attachments.length, clearAttachments]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleQuickCommand = (prompt: string) => {
    setInputText(prompt);
    textareaRef.current?.focus();
  };

  // 选择并读取文件作为附件
  const handlePickFile = async () => {
    try {
      const attach = await pickAndReadFile();
      if (attach) {
        addAttachment(attach);
      }
    } catch (e: any) {
      useChatStore.getState().setError(e?.message || '选择文件失败');
    }
  };

  const filteredSessions = sessions.filter((s) =>
    s.title.toLowerCase().includes(searchQuery.toLowerCase())
  );

  return (
    <div className="chat-page">
      {/* Session List (Left panel within main) */}
      <div className="chat-session-list">
        <div className="session-list-header">
          <h3>对话列表</h3>
          <button
            className="new-chat-btn"
            onClick={() => { createSession().catch(() => {}); }}
            title="新建对话"
            aria-label="新建对话"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round">
              <path d="M12 5v14M5 12h14" />
            </svg>
          </button>
        </div>

        <div className="session-search">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
            <circle cx="11" cy="11" r="7" />
            <path d="m20 20-3.5-3.5" />
          </svg>
          <input
            type="text"
            placeholder="搜索对话..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>

        <div className="session-items">
          {filteredSessions.map((session) => (
            <div
              key={session.id}
              className={`session-item ${session.id === currentSessionId ? 'active' : ''}`}
              onClick={() => { switchSession(session.id).catch(() => {}); }}
            >
              <div className="session-item-title">{session.title}</div>
              <div className="session-item-meta">
                <span>{formatRelativeTime(session.updatedAt)}</span>
                <button
                  className="session-delete-btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    deleteSession(session.id).catch(() => {});
                  }}
                  title="删除对话"
                >
                  x
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Chat Area */}
      <div className="chat-main">
        {/* Messages */}
        <div className="chat-messages">
          {messages.length === 0 ? (
            <div className="chat-welcome">
              <div className="welcome-hero">
                <div className="welcome-logo">M</div>
                <h1>你好，我是 LocalMind</h1>
                <p>你的本地 AI 助手，随时随地为你服务。在线 / 离线双模式，对话、写文件、生成文档都能做。</p>
                <div className="welcome-features">
                  <div className="feature-item">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" /><path d="M7 11V7a5 5 0 0 1 10 0v4" /></svg>
                    隐私保护 · 数据本地处理
                  </div>
                  <div className="feature-item">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M13 2 3 14h9l-1 8 10-12h-9l1-8Z" /></svg>
                    快速响应 · 在线/离线双模式
                  </div>
                  <div className="feature-item">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M12 2v4M12 18v4M2 12h4M18 12h4M4.9 4.9l2.8 2.8M16.3 16.3l2.8 2.8M4.9 19.1l2.8-2.8M16.3 7.7l2.8-2.8" /></svg>
                    Agent 工具 · 写文件/生成文档
                  </div>
                </div>
              </div>
              <div className="quick-grid">
                {QUICK_COMMANDS.map((cmd) => (
                  <button
                    key={cmd.label}
                    className="qc-card"
                    onClick={() => handleQuickCommand(cmd.prompt)}
                  >
                    <span className="qc-icon">{cmd.icon}</span>
                    <span className="qc-text">
                      <span className="qc-label">{cmd.label}</span>
                      <span className="qc-desc">{cmd.desc}</span>
                    </span>
                  </button>
                ))}
              </div>
              <div className="welcome-mode">
                当前模式：{mode === 'online' ? '在线 DeepSeek-V4-Pro' : '离线 本地模型'}
              </div>
            </div>
          ) : (
            <>
              {messages.map((msg) => (
                <MessageBubble key={msg.id} message={msg} />
              ))}
            </>
          )}
          <div ref={messagesEndRef} />

          {error && (
            <div className="chat-error">
              <span>错误：{error}</span>
              <button onClick={() => useChatStore.getState().setError(null)}>x</button>
            </div>
          )}

          {/* AI 工具调用过程展示 */}
          {toolCalls.length > 0 && (
            <div className="tool-calls-panel">
              {toolCalls.map((tc, i) => (
                <div key={i} className={`tool-call-item ${tc.success ? '' : 'error'}`}>
                  <span className="tool-call-icon">{tc.success ? '🔧' : '⚠️'}</span>
                  <span className="tool-call-name">{toolLabel(tc.name)}</span>
                  <span className="tool-call-args">{tc.args}</span>
                  <span className="tool-call-result">
                    {tc.success ? '✓ 完成' : '✗ 失败'}
                  </span>
                  {tc.output && (
                    <details className="tool-call-output">
                      <summary>查看输出</summary>
                      <pre>{tc.output}</pre>
                    </details>
                  )}
                </div>
              ))}
            </div>
          )}

          {/* Agent 思考过程展示（规划/反思） */}
          {thinkingSteps.length > 0 && (
            <div className="thinking-panel">
              {thinkingSteps.map((step, i) => (
                <div key={i} className={`thinking-step ${step.phase} ${step.status}`}>
                  <span className="thinking-icon">
                    {step.phase === 'planning' ? '📋' :
                     step.phase === 'executing' ? '🔧' :
                     step.phase === 'reflecting' ? '🤔' :
                     step.phase === 'retrying' ? '🔄' : '✅'}
                  </span>
                  <span className="thinking-label">{step.label}</span>
                  {step.detail && (
                    <details className="thinking-detail">
                      <summary>详情</summary>
                      <pre>{step.detail}</pre>
                    </details>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Input Area */}
        <div className="chat-input-area">
          <div className="chat-input-wrapper">
            <button
              className="attach-btn"
              onClick={handlePickFile}
              title="添加文件附件"
              aria-label="添加文件附件"
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="m21.4 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48" />
              </svg>
            </button>



            {/* 附件 chip 列表 */}
            {attachments.length > 0 && (
              <div className="attachment-list">
                {attachments.map((a) => (
                  <div key={a.id} className="attachment-chip" title={a.path}>
                    <span className="attachment-icon">📄</span>
                    <span className="attachment-name">{a.file_name}</span>
                    <span className="attachment-size">{formatFileSize(a.size_bytes)}</span>
                    <button
                      className="attachment-remove"
                      onClick={() => removeAttachment(a.id)}
                      title="移除附件"
                    >
                      ×
                    </button>
                  </div>
                ))}
              </div>
            )}

            <textarea
              ref={textareaRef}
              className="chat-input"
              placeholder="输入消息，Enter 发送，Shift+Enter 换行"
              value={inputText}
              onChange={(e) => setInputText(e.target.value)}
              onKeyDown={handleKeyDown}
              rows={1}
            />

            {isStreaming ? (
              <button
                className="stop-btn"
                onClick={stopGeneration}
                title="停止生成"
                aria-label="停止生成"
              >
                <svg viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="2" /></svg>
              </button>
            ) : (
              <button
                className="send-btn"
                onClick={handleSend}
                disabled={!inputText.trim()}
                title="发送"
                aria-label="发送"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M12 19V5M5 12l7-7 7 7" />
                </svg>
              </button>
            )}
          </div>
          <div className="composer-hint">
            <span>{mode === 'online' ? 'DeepSeek-V4-Pro' : '本地模型'}</span>
            <span className="composer-hint-sep" />
            <span>{mode === 'online' ? '在线 · 隐私本地处理' : '离线 · 本地推理'}</span>
            <span className="composer-hint-sep" />
            <span>Enter 发送 · Shift+Enter 换行</span>
          </div>

          {/* 离线模式：Ollama 模型切换下拉 + 推理速度 */}
          {mode === 'offline' && (
            <div className="offline-controls">
              {ollamaRunning && ollamaModels.length > 0 ? (
                <>
                  <label className="ollama-model-label">模型</label>
                  <select
                    className="ollama-model-select"
                    value={selectedOllamaModel}
                    onChange={(e) => setSelectedOllamaModel(e.target.value)}
                    title="选择 Ollama 本地模型"
                  >
                    {ollamaModels.map((m) => (
                      <option key={m.name} value={m.name}>{m.name}</option>
                    ))}
                  </select>
                  <button
                    className="ollama-refresh-btn"
                    onClick={() => refreshOllama()}
                    title="刷新模型列表"
                  >
                    ↻
                  </button>
                  {tokensPerSecond > 0 && (
                    <span className="offline-speed">速度：{tokensPerSecond} tok/s</span>
                  )}
                </>
              ) : (
                <span className="offline-speed offline-speed-warn">
                  Ollama 未连接，点击左侧「离线」查看安装教程
                </span>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// 工具名 → 中文可读标签
function toolLabel(name: string): string {
  const map: Record<string, string> = {
    write_file: '写入文件',
    append_file: '追加文件',
    read_file: '读取文件',
    list_dir: '浏览目录',
    delete_path: '删除文件/目录',
    rename_path: '重命名/移动',
    run_command: '执行命令',
    open_app: '打开程序',
  };
  return map[name] || name;
}
