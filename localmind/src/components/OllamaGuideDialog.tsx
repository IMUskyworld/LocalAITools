import React from 'react';

export type OllamaGuideMode = 'not-installed' | 'no-model' | 'info';

interface OllamaGuideDialogProps {
  open: boolean;
  mode: OllamaGuideMode;
  onClose: () => void;
  onRefresh: () => void;
}

/**
 * Ollama 安装 / 部署模型教程弹窗
 * 点击离线模式时，根据 Ollama 状态展示对应引导
 */
export default function OllamaGuideDialog({
  open,
  mode,
  onClose,
  onRefresh,
}: OllamaGuideDialogProps) {
  if (!open) return null;

  const title =
    mode === 'not-installed'
      ? '需要先安装 Ollama'
      : mode === 'no-model'
      ? 'Ollama 已安装，但还没有模型'
      : 'Ollama 离线模型使用教程';

  const bannerText =
    mode === 'not-installed'
      ? '检测到你的电脑还没有安装 Ollama。离线模式依赖 Ollama 在本地运行大模型，请按以下步骤完成安装。'
      : mode === 'no-model'
      ? '检测到 Ollama 正在运行，但你还没有下载任何模型。请拉取一个模型后即可使用离线模式。'
      : '以下是 Ollama 本地模型的使用说明。';

  return (
    <div
      className="ollama-guide-overlay"
      onClick={onClose}
      style={{
        position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.5)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        zIndex: 1000,
      }}
    >
      <div
        className="ollama-guide-dialog"
        onClick={(e) => e.stopPropagation()}
        style={{
          background: 'var(--bg-surface, #fff)', color: 'var(--text-primary, #1a1a1a)',
          borderRadius: '12px', maxWidth: '640px', width: '90%',
          maxHeight: '85vh', overflow: 'auto', padding: '24px',
          boxShadow: '0 8px 30px rgba(0,0,0,0.25)',
        }}
      >
        {/* 头部 */}
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
          <h2 style={{ fontSize: '20px', margin: 0, color: 'var(--accent-primary, #0066CC)' }}>
            {title}
          </h2>
          <button
            onClick={onClose}
            style={{
              border: 'none', background: 'none', fontSize: '22px', cursor: 'pointer',
              color: 'var(--text-secondary, #666)', lineHeight: 1,
            }}
            title="关闭"
          >
            ✕
          </button>
        </div>

        <p style={{ marginBottom: '20px', lineHeight: 1.7, color: 'var(--text-secondary, #666)' }}>
          {bannerText}
        </p>

        {/* 教程步骤 */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: '18px' }}>
          <Step n={1} title="下载并安装 Ollama">
            <p>打开浏览器访问 <strong>https://ollama.com/download</strong>，下载 Windows 版安装包，双击一路「下一步」完成安装。</p>
          </Step>

          <Step n={2} title="验证 Ollama 是否运行">
            <p>打开 PowerShell，输入：</p>
            <CodeLine>ollama list</CodeLine>
            <p>如果显示模型列表（或空列表），说明 Ollama 已正常运行。</p>
          </Step>

          <Step n={3} title="拉取一个本地模型">
            <p>按你的电脑内存选择模型（越大越聪明，但越吃配置）：</p>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '14px' }}>
              <thead>
                <tr style={{ textAlign: 'left' }}>
                  <th style={{ padding: '6px 8px', borderBottom: '1px solid var(--border, #e0e0e0)' }}>电脑配置</th>
                  <th style={{ padding: '6px 8px', borderBottom: '1px solid var(--border, #e0e0e0)' }}>推荐命令</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td style={{ padding: '6px 8px', borderBottom: '1px solid var(--border-light, #f0f0f0)' }}>入门（8GB 内存）</td>
                  <td style={{ padding: '6px 8px', borderBottom: '1px solid var(--border-light, #f0f0f0)' }}><code>ollama pull qwen2.5:3b</code></td>
                </tr>
                <tr>
                  <td style={{ padding: '6px 8px', borderBottom: '1px solid var(--border-light, #f0f0f0)' }}>推荐（16GB 内存）</td>
                  <td style={{ padding: '6px 8px', borderBottom: '1px solid var(--border-light, #f0f0f0)' }}><code>ollama pull qwen2.5:7b</code></td>
                </tr>
                <tr>
                  <td style={{ padding: '6px 8px' }}>更强（32GB 内存）</td>
                  <td style={{ padding: '6px 8px' }}><code>ollama pull qwen2.5:14b</code></td>
                </tr>
              </tbody>
            </table>
            <p>例如执行 <CodeLine inline>ollama pull qwen2.5:7b</CodeLine>，等待下载完成（7b 约 4.7GB）。</p>
          </Step>

          <Step n={4} title="回到 LocalMind 使用">
            <p>点击下方「刷新模型」按钮，下拉框选择刚下载的模型，即可开始离线聊天。断网也能用！</p>
          </Step>
        </div>

        {/* 操作按钮 */}
        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px', marginTop: '24px' }}>
          <button
            onClick={onClose}
            style={{
              padding: '8px 18px', borderRadius: '6px', cursor: 'pointer',
              border: '1px solid var(--border, #e0e0e0)', background: 'transparent',
              color: 'var(--text-secondary, #666)',
            }}
          >
            稍后再说
          </button>
          <button
            onClick={onRefresh}
            style={{
              padding: '8px 18px', borderRadius: '6px', cursor: 'pointer',
              border: 'none', background: 'var(--accent-primary, #0066CC)',
              color: '#fff', fontWeight: 600,
            }}
          >
            刷新模型
          </button>
        </div>
      </div>
    </div>
  );
}

function Step({ n, title, children }: { n: number; title: string; children: React.ReactNode }) {
  return (
    <div style={{ display: 'flex', gap: '14px' }}>
      <div
        style={{
          width: '28px', height: '28px', borderRadius: '50%', flexShrink: 0,
          background: 'var(--accent-light, #e6f0fa)', color: 'var(--accent-primary, #0066CC)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          fontSize: '14px', fontWeight: 700,
        }}
      >
        {n}
      </div>
      <div>
        <div style={{ fontWeight: 600, marginBottom: '6px' }}>{title}</div>
        <div style={{ lineHeight: 1.7, color: 'var(--text-secondary, #666)', fontSize: '14px' }}>
          {children}
        </div>
      </div>
    </div>
  );
}

function CodeLine({ children, inline }: { children: React.ReactNode; inline?: boolean }) {
  return (
    <code
      style={{
        display: inline ? 'inline' : 'block',
        background: 'var(--bg-input, #f0f0f0)', padding: inline ? '2px 6px' : '8px 12px',
        borderRadius: '4px', fontSize: '13px', fontFamily: 'monospace',
        margin: inline ? '0 2px' : '6px 0',
      }}
    >
      {children}
    </code>
  );
}
