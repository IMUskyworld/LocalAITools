import { ONLINE_MODEL_LABEL } from '@/config/models';
import React, { useState } from 'react';
import { useChatStore } from '@/stores/chatStore';
import type { ChatMode } from '@/types/chat';
import OllamaGuideDialog, { OllamaGuideMode } from '@/components/OllamaGuideDialog';

export default function ModeSwitch() {
  const currentMode = useChatStore((s) => s.mode);
  const switchMode = useChatStore((s) => s.switchMode);
  const refreshOllama = useChatStore((s) => s.refreshOllama);
  const ollamaRunning = useChatStore((s) => s.ollamaRunning);
  const ollamaModels = useChatStore((s) => s.ollamaModels);

  const [guideOpen, setGuideOpen] = useState(false);
  const [guideMode, setGuideMode] = useState<OllamaGuideMode>('info');

  const handleModeChange = async (mode: ChatMode) => {
    if (mode === 'online') {
      switchMode('online');
      return;
    }
    // 切到离线：先检测 Ollama
    await refreshOllama();
    if (!useChatStore.getState().ollamaRunning) {
      setGuideMode('not-installed');
      setGuideOpen(true);
      return;
    }
    if (useChatStore.getState().ollamaModels.length === 0) {
      setGuideMode('no-model');
      setGuideOpen(true);
      return;
    }
    switchMode('offline');
  };

  const handleRefresh = async () => {
    const models = await refreshOllama();
    if (useChatStore.getState().ollamaRunning && models.length > 0) {
      setGuideOpen(false);
      switchMode('offline');
    } else if (!useChatStore.getState().ollamaRunning) {
      setGuideMode('not-installed');
    } else {
      setGuideMode('no-model');
    }
  };

  return (
    <>
      <div className="mode-switch" role="radiogroup" aria-label="对话模式">
        <button
          className={`mode-btn ${currentMode === 'online' ? 'active' : ''}`}
          onClick={() => handleModeChange('online')}
          role="radio"
          aria-checked={currentMode === 'online'}
          title="使用云端 DeepSeek 模型"
        >
          <span className="mode-icon">☁</span>
          <span className="mode-label">在线</span>
          <span className="mode-sub-label">{ONLINE_MODEL_LABEL}</span>
        </button>
        <button
          className={`mode-btn ${currentMode === 'offline' ? 'active' : ''}`}
          onClick={() => handleModeChange('offline')}
          role="radio"
          aria-checked={currentMode === 'offline'}
          title="使用 Ollama 本地模型（离线）"
        >
          <span className="mode-icon">⛅</span>
          <span className="mode-label">离线</span>
          <span className="mode-sub-label">
            {ollamaRunning && ollamaModels.length > 0 ? '本地模型' : '需配置 Ollama'}
          </span>
        </button>
      </div>

      <OllamaGuideDialog
        open={guideOpen}
        mode={guideMode}
        onClose={() => setGuideOpen(false)}
        onRefresh={handleRefresh}
      />
    </>
  );
}
