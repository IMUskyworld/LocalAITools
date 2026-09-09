import React, { useState, useEffect } from 'react';
import { useChatStore } from '@/stores/chatStore';
import { checkOllama, formatModelSize } from '@/api/ollama';
import { tauriInvoke } from '@/api/ipc';

// 动态获取用户目录下的 Ollama 模型目录
let ollamaModelsDir = '';

async function resolveOllamaModelsDir(): Promise<string> {
  if (ollamaModelsDir) return ollamaModelsDir;
  try {
    const result = await tauriInvoke('get_common_paths');
    const lines: string[] = result?.data || [];
    const homeLine = lines.find((l) => l.startsWith('用户目录:'));
    const home = homeLine ? homeLine.replace('用户目录:', '').trim() : '';
    if (home) {
      ollamaModelsDir = `${home}\\.ollama\\models`;
      return ollamaModelsDir;
    }
  } catch {}
  return 'C:\\Users\\.ollama\\models';
}

export default function ModelsPage() {
  const mode = useChatStore((s) => s.mode);
  const switchMode = useChatStore((s) => s.switchMode);
  const ollamaRunning = useChatStore((s) => s.ollamaRunning);
  const ollamaModels = useChatStore((s) => s.ollamaModels);
  const refreshOllama = useChatStore((s) => s.refreshOllama);

  const [openResult, setOpenResult] = useState('');
  const [modelsDir, setModelsDir] = useState('');

  // 进入页面时刷新 Ollama 模型列表 + 解析模型目录
  useEffect(() => {
    refreshOllama();
    resolveOllamaModelsDir().then(setModelsDir);
  }, [refreshOllama]);

  // 打开 Ollama 模型目录
  const handleOpenFolder = async () => {
    const dir = modelsDir || (await resolveOllamaModelsDir());
    try {
      const result = await tauriInvoke('open_app', { target: dir });
      if (result?.data?.success) {
        setOpenResult('已打开模型目录');
      } else {
        setOpenResult(result?.data?.error || '打开失败');
      }
    } catch (e: any) {
      setOpenResult(e?.message || '打开失败');
    }
  };

  return (
    <div className="models-page">
      <h1 className="page-title">模型管理</h1>

      {/* 运行模式 */}
      <section className="model-card">
        <h2>运行模式</h2>
        <div className="mode-selector">
          <label className={`mode-option ${mode === 'online' ? 'selected' : ''}`}>
            <input
              type="radio"
              name="mode"
              checked={mode === 'online'}
              onChange={() => switchMode('online')}
            />
            <div className="mode-option-content">
              <span className="mode-option-icon">☁</span>
              <div>
                <strong>在线模式</strong>
                <p>直连 DeepSeek 云端模型，需要网络连接</p>
              </div>
            </div>
          </label>

          <label className={`mode-option ${mode === 'offline' ? 'selected' : ''}`}>
            <input
              type="radio"
              name="mode"
              checked={mode === 'offline'}
              onChange={() => switchMode('offline')}
            />
            <div className="mode-option-content">
              <span className="mode-option-icon">⛅</span>
              <div>
                <strong>离线模式</strong>
                <p>使用本地 Ollama 模型，无需网络，保护隐私</p>
              </div>
            </div>
          </label>
        </div>
      </section>

      {/* 本地模型（Ollama） */}
      <section className="model-card">
        <h2>本地模型（Ollama）</h2>
        <div className="ollama-models-status">
          <span className={`status-badge ${ollamaRunning ? 'running' : 'stopped'}`}>
            {ollamaRunning ? '● Ollama 运行中' : '○ Ollama 未运行'}
          </span>
          <button
            className="btn btn-secondary"
            onClick={() => refreshOllama()}
          >
            刷新
          </button>
        </div>

        {!ollamaRunning ? (
          <p className="ollama-empty-tip">
            Ollama 未运行。请先启动 Ollama（可在开始菜单打开），或参考「离线模式」的安装教程。
          </p>
        ) : ollamaModels.length === 0 ? (
          <p className="ollama-empty-tip">
            还没有下载模型。在 PowerShell 中运行 <code>ollama pull qwen2.5:7b</code> 下载模型，然后点「刷新」。
          </p>
        ) : (
          <div className="ollama-model-list">
            {ollamaModels.map((m) => {
              const info = getModelCapability(m.name);
              return (
                <div key={m.name} className="ollama-model-item">
                  <div className="ollama-model-main">
                    <span className="ollama-model-name">{m.name}</span>
                    <span className="ollama-model-size">{formatModelSize(m.size)}</span>
                    {m.name === useChatStore.getState().selectedOllamaModel && (
                      <span className="ollama-model-active">当前使用</span>
                    )}
                  </div>
                  <div className="ollama-model-caps">
                    {info.map((cap, i) => (
                      <span key={i} className={`ollama-cap ${cap.ok ? 'ok' : 'bad'}`}>
                        {cap.ok ? '✓' : '✗'} {cap.label}
                      </span>
                    ))}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {/* 存储位置（Ollama 模型目录） */}
      <section className="model-card">
        <h2>存储位置</h2>
        <div className="storage-location">
          <div className="storage-path">
            <code>{modelsDir || '正在获取模型目录...'}</code>
          </div>
          <div className="storage-actions">
            <button className="btn btn-secondary" onClick={handleOpenFolder}>
              打开文件夹
            </button>
          </div>
          {openResult && <div className="storage-result">{openResult}</div>}
        </div>
      </section>
    </div>
  );
}

// ========== 模型能力标注 ==========

interface ModelCap { label: string; ok: boolean }

/**
 * 根据模型名返回能力标注
 * 基于实际测试：qwen2.5:7b 可做聊天+文件+文档（偶发路径错误），
 * qwen2.5:0.5b 仅适合聊天
 */
function getModelCapability(modelName: string): ModelCap[] {
  const n = modelName.toLowerCase();
  if (n.includes('0.5b')) {
    return [
      { label: '聊天对话', ok: true },
      { label: '创建文件', ok: false },
      { label: '生成文档', ok: false },
      { label: '执行操作', ok: false },
    ];
  }
  // 7b 及以上
  if (n.includes('7b') || n.includes('8b') || n.includes('14b') || n.includes('32b') || n.includes('72b')) {
    return [
      { label: '聊天对话', ok: true },
      { label: '创建文件', ok: true },
      { label: '生成文档', ok: true },
      { label: '执行操作', ok: true },
    ];
  }
  // 未知模型默认按可用处理
  return [
    { label: '聊天对话', ok: true },
    { label: '创建文件', ok: true },
    { label: '生成文档', ok: true },
    { label: '执行操作', ok: true },
  ];
}
