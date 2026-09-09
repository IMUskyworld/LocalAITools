import React, { useState } from 'react';
import type { ChatMessage } from '@/types/chat';
import { formatDate } from '@/utils/helpers';

interface MessageBubbleProps {
  message: ChatMessage;
}

export default function MessageBubble({ message }: MessageBubbleProps) {
  const isUser = message.role === 'user';
  const [copyBtnText, setCopyBtnText] = useState('复制');

  const handleCopy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopyBtnText('已复制!');
      setTimeout(() => setCopyBtnText('复制'), 2000);
    } catch {
      setCopyBtnText('复制失败');
    }
  };

  const renderContent = (content: string) => {
    // Simple markdown rendering: handle code blocks, bold, line breaks
    const parts = content.split(/(```[\s\S]*?```)/g);
    return parts.map((part, i) => {
      if (part.startsWith('```') && part.endsWith('```')) {
        const codeContent = part.slice(3, -3);
        const firstLineEnd = codeContent.indexOf('\n');
        const lang = firstLineEnd > 0 ? codeContent.slice(0, firstLineEnd).trim() : '';
        const code = firstLineEnd > 0 ? codeContent.slice(firstLineEnd + 1) : codeContent;

        return (
          <div key={i} className="code-block-wrapper">
            <div className="code-block-header">
              <span className="code-lang">{lang || 'code'}</span>
              <button
                className="code-copy-btn"
                onClick={() => handleCopy(code)}
              >
                {copyBtnText}
              </button>
            </div>
            <pre className="code-block"><code>{code}</code></pre>
          </div>
        );
      }
      // Render other content with basic markdown
      const rawLines = part.split('\n');
      const lines = rawLines.map((line, li) => {
        if (line.startsWith('**') && line.endsWith('**')) {
          return <strong key={li}>{line.slice(2, -2)}</strong>;
        }
        if (line.startsWith('#')) {
          const level = line.match(/^#+/)?.[0].length || 1;
          const text = line.replace(/^#+\s*/, '');
          const Tag = `h${Math.min(level, 6)}` as keyof JSX.IntrinsicElements;
          return <Tag key={li}>{text}</Tag>;
        }
        if (line.startsWith('- ')) {
          return <li key={li}>{line.slice(2)}</li>;
        }
        if (line.match(/^\d+\.\s/)) {
          return <li key={li}>{line.replace(/^\d+\.\s/, '')}</li>;
        }
        return (
          <span key={li}>
            {line}
            {li < rawLines.length - 1 && <br />}
          </span>
        );
      });
      return <div key={i} className="message-text">{lines}</div>;
    });
  };

  return (
    <div className={`message-bubble ${isUser ? 'user' : 'assistant'} ${message.isStreaming ? 'streaming' : ''}`}>
      {!isUser && (
        <div className="message-avatar">
          <span className="avatar-icon">M</span>
        </div>
      )}

      <div className="bubble-content">
        {!isUser && message.modelName && (
          <div className="bubble-model-name">{message.modelName}</div>
        )}

        <div className="bubble-text">
          {renderContent(message.content)}
        </div>

        <div className="bubble-meta">
          <span className="bubble-time">{formatDate(message.timestamp)}</span>
          {message.isStreaming && <span className="streaming-indicator">正在生成...</span>}
          {message.tokensPerSecond && (
            <span className="bubble-speed">{message.tokensPerSecond} tok/s</span>
          )}
          {message.latencyMs && (
            <span className="bubble-latency">{message.latencyMs}ms</span>
          )}
        </div>
      </div>

      {isUser && (
        <div className="message-avatar user-avatar">
          <span className="avatar-icon">U</span>
        </div>
      )}
    </div>
  );
}
