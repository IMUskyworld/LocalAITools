import React, { useState } from 'react';
import type { ChatMessage } from '@/types/chat';
import { formatDate } from '@/utils/helpers';

interface MessageBubbleProps {
  message: ChatMessage;
}

/** 解析工具调用标记，返回结构化数据 */
function parseToolBlocks(content: string): Array<{ type: 'text'; value: string } | { type: 'tool'; name: string; args: string; result: string; failed: boolean }> {
  const blocks: Array<{ type: 'text'; value: string } | { type: 'tool'; name: string; args: string; result: string; failed: boolean }> = [];
  content = content.replace('[最终回复]', '');
  const regex = /\[调用工具:(\S+?)\]\s*(.*?)\n\[工具结果:\1\]\s*(.*?)(?=\n\[调用工具:|\n*$)/gs;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(content)) !== null) {
    if (match.index > lastIndex) {
      blocks.push({ type: 'text', value: content.slice(lastIndex, match.index) });
    }
    const resultRaw = match[3].trim();
    const failed = resultRaw.startsWith('(失败) ');
    blocks.push({
      type: 'tool',
      name: match[1],
      args: match[2].trim(),
      result: failed ? resultRaw.slice(4) : resultRaw,
      failed,
    });
    lastIndex = match.index + match[0].length;
  }
  if (lastIndex < content.length) {
    blocks.push({ type: 'text', value: content.slice(lastIndex) });
  }
  return blocks;
}

function ToolCallCard({ name, args, result, failed }: { name: string; args: string; result: string; failed: boolean }) {
  const [expanded, setExpanded] = useState(false);
  const argsDisplay = (() => {
    try { return JSON.stringify(JSON.parse(args), null, 2); } catch { return args; }
  })();

  return (
    <div className={`tool-call-card ${failed ? 'tool-failed' : 'tool-success'}`}>
      <div className="tool-call-header" onClick={() => setExpanded(!expanded)}>
        <span className="tool-call-icon">{failed ? '❌' : '🔧'}</span>
        <span className="tool-call-name">{name}</span>
        <span className="tool-call-toggle">{expanded ? '▲' : '▼'}</span>
      </div>
      {expanded && (
        <div className="tool-call-detail">
          <div className="tool-call-section">
            <div className="tool-call-label">参数</div>
            <pre className="tool-call-args">{argsDisplay}</pre>
          </div>
          <div className="tool-call-section">
            <div className="tool-call-label">{failed ? '错误' : '结果'}</div>
            <pre className="tool-call-result">{result}</pre>
          </div>
        </div>
      )}
    </div>
  );
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
    const blocks = parseToolBlocks(content);
    return blocks.map((block, i) => {
      if (block.type === 'tool') {
        return <ToolCallCard key={i} name={block.name} args={block.args} result={block.result} failed={block.failed} />;
      }
      // text block - existing markdown rendering
      const parts = block.value.split(/(```[\s\S]*?```)/g);
      return parts.map((part, pi) => {
        if (part.startsWith('```') && part.endsWith('```')) {
          const codeContent = part.slice(3, -3);
          const firstLineEnd = codeContent.indexOf('\n');
          const lang = firstLineEnd > 0 ? codeContent.slice(0, firstLineEnd).trim() : '';
          const code = firstLineEnd > 0 ? codeContent.slice(firstLineEnd + 1) : codeContent;
          return (
            <div key={`${i}-${pi}`} className="code-block-wrapper">
              <div className="code-block-header">
                <span className="code-lang">{lang || 'code'}</span>
                <button className="code-copy-btn" onClick={() => handleCopy(code)}>{copyBtnText}</button>
              </div>
              <pre className="code-block"><code>{code}</code></pre>
            </div>
          );
        }
        const rawLines = part.split('\n');
        const lines = rawLines.map((line, li) => {
          if (line.startsWith('**') && line.endsWith('**')) return <strong key={li}>{line.slice(2, -2)}</strong>;
          if (line.startsWith('#')) {
            const level = line.match(/^#+/)?.[0].length || 1;
            const text = line.replace(/^#+\s*/, '');
            const Tag = `h${Math.min(level, 6)}` as keyof JSX.IntrinsicElements;
            return <Tag key={li}>{text}</Tag>;
          }
          if (line.startsWith('- ')) return <li key={li}>{line.slice(2)}</li>;
          if (line.match(/^\d+\.\s/)) return <li key={li}>{line.replace(/^\d+\.\s/, '')}</li>;
          return <span key={li}>{line}{li < rawLines.length - 1 && <br />}</span>;
        });
        return <div key={`${i}-${pi}`} className="message-text">{lines}</div>;
      });
    });
  };

  return (
    <div className={`message-bubble ${isUser ? 'user' : 'assistant'} ${message.isStreaming ? 'streaming' : ''}`}>
      {!isUser && (
        <div className="message-avatar"><span className="avatar-icon">M</span></div>
      )}
      <div className="bubble-content">
        {!isUser && message.modelName && <div className="bubble-model-name">{message.modelName}</div>}
        <div className="bubble-text">{renderContent(message.content)}</div>
        <div className="bubble-meta">
          <span className="bubble-time">{formatDate(message.timestamp)}</span>
          {message.isStreaming && <span className="streaming-indicator">正在生成...</span>}
          {message.tokensPerSecond && <span className="bubble-speed">{message.tokensPerSecond} tok/s</span>}
          {message.latencyMs && <span className="bubble-latency">{(message.latencyMs / 1000).toFixed(1)}s</span>}
          {message.usage && message.usage.total_tokens > 0 && (
            <span
              className="bubble-tokens"
              title={
                `输入 ${message.usage.input_tokens} tok · 输出 ${message.usage.output_tokens} tok` +
                (message.usage.cache_read_tokens > 0 ? ` · 缓存命中 ${message.usage.cache_read_tokens} tok` : '') +
                (message.usage.requests > 1 ? ` · ${message.usage.requests} 次请求` : '') +
                (message.usage.tool_calls > 0 ? ` · ${message.usage.tool_calls} 次工具调用` : '')
              }
            >
              Σ {message.usage.total_tokens.toLocaleString()} tok
              <span className="token-split">
                （↑{message.usage.input_tokens.toLocaleString()} ↓{message.usage.output_tokens.toLocaleString()}）
              </span>
            </span>
          )}
        </div>
      </div>
      {isUser && (
        <div className="message-avatar user-avatar"><span className="avatar-icon">U</span></div>
      )}
    </div>
  );
}
