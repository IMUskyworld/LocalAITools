import React from 'react';
import { useChatStore } from '@/stores/chatStore';

export default function StatusIndicator() {
  const mode = useChatStore((s) => s.mode);
  const latencyMs = useChatStore((s) => s.latencyMs);
  const tokensPerSecond = useChatStore((s) => s.tokensPerSecond);
  const error = useChatStore((s) => s.error);

  let statusClass = 'status-dot';
  let statusText = '';

  if (error) {
    statusClass += ' error';
    statusText = '连接异常';
  } else if (mode === 'online') {
    statusClass += ' online';
    statusText = `在线 ${latencyMs}ms`;
  } else {
    statusClass += ' offline';
    statusText = `离线 ${tokensPerSecond > 0 ? tokensPerSecond + ' tok/s' : ''}`;
  }

  return (
    <div className="status-indicator" title={statusText}>
      <span className={statusClass} />
      <span className="status-text">{statusText}</span>
    </div>
  );
}
