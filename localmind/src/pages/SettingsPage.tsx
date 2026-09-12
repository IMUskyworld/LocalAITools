import React, { useState, useEffect, useCallback } from 'react';
import { tauriInvoke } from '@/api/ipc';

type ThemeMode = 'light' | 'dark' | 'system';


interface MemoryEntry { id: string; category: string; content: string; confidence: number; }

function MemorySection() {
  const [memories, setMemories] = useState<MemoryEntry[]>([]);
  const [keyword, setKeyword] = useState("");
  const [loading, setLoading] = useState(false);
  const load = useCallback(async () => {
    setLoading(true);
    try {
      const fn = keyword.trim() ? "search_memories" : "get_memories";
      const args = keyword.trim() ? { keyword: keyword.trim(), limit: 50 } : { limit: 50 };
      const res: any = await tauriInvoke(fn, args);
      setMemories(res?.data || []);
    } catch { setMemories([]); }
    setLoading(false);
  }, [keyword]);
  useEffect(() => { load(); }, []);
  const handleDelete = async (id: string) => {
    await tauriInvoke("delete_memory", { memoryId: id });
    setMemories((prev) => prev.filter((m) => m.id !== id));
  };
  return (
    <section className="settings-card">
      <h2>记忆管理</h2>
      <p className="settings-hint">Agent 的长期记忆条目，可搜索和删除。</p>
      <div className="api-key-input-row" style={{ marginBottom: 12 }}>
        <input type="text" value={keyword} onChange={(e) => setKeyword(e.target.value)} placeholder="搜索关键词..." className="api-key-input" />
        <button className="btn-primary" onClick={load} disabled={loading}>{loading ? "..." : "搜索"}</button>
      </div>
      {memories.length === 0 ? (
        <p className="settings-hint">暂无记忆条目。</p>
      ) : (
        <div style={{ maxHeight: 300, overflowY: "auto" }}>
          {memories.map((m) => (
            <div key={m.id} style={{ display: "flex", justifyContent: "space-between", alignItems: "start", padding: "8px 0", borderBottom: "1px solid var(--border, #eee)" }}>
              <div style={{ flex: 1 }}>
                <span style={{ fontSize: "0.75rem", color: "var(--accent, #6c5ce7)", fontWeight: 600, marginRight: 8 }}>[{m.category}]</span>
                <span style={{ fontSize: "0.85rem" }}>{m.content}</span>
              </div>
              <button onClick={() => handleDelete(m.id)} style={{ background: "none", border: "none", color: "#e74c3c", cursor: "pointer", fontSize: "0.8rem", marginLeft: 8 }}>删除</button>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

export default function SettingsPage() {
  const [theme, setTheme] = useState<ThemeMode>(() => {
    return (localStorage.getItem('localmind-theme') as ThemeMode) || 'light';
  });
  const [apiKey, setApiKey] = useState('');
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [keyStatus, setKeyStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');

  // 加载已保存的 API key
  useEffect(() => {
    tauriInvoke('get_api_key').then((res: any) => {
      if (res?.data) setApiKey(String(res.data));
    });
  }, []);

  const handleThemeChange = (newTheme: ThemeMode) => {
    setTheme(newTheme);
    localStorage.setItem('localmind-theme', newTheme);
    applyTheme(newTheme);
  };

  const applyTheme = (t: ThemeMode) => {
    let resolved: 'light' | 'dark';
    if (t === 'system') {
      resolved = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    } else {
      resolved = t;
    }
    document.documentElement.setAttribute('data-theme', resolved);
  };

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const handleSaveKey = async () => {
    setKeyStatus('saving');
    try {
      await tauriInvoke('set_api_key', { key: apiKey.trim() });
      setKeyStatus('saved');
      setApiKeySaved(true);
      setTimeout(() => setKeyStatus('idle'), 2000);
    } catch {
      setKeyStatus('error');
      setTimeout(() => setKeyStatus('idle'), 3000);
    }
  };

  return (
    <div className="settings-page">
      <h1 className="page-title">设置</h1>

      {/* API Key */}
      <section className="settings-card">
        <h2>DeepSeek API Key</h2>
        <p className="settings-hint">
          请前往 <a href="https://platform.deepseek.com" target="_blank" rel="noopener noreferrer">DeepSeek 开放平台</a> 获取 API Key。
          Key 仅保存在本地数据库，不会上传到任何服务器。
        </p>
        <div className="settings-field api-key-field">
          <div className="api-key-input-row">
            <input
              type={showKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => { setApiKey(e.target.value); setApiKeySaved(false); }}
              placeholder="sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
              className="api-key-input"
            />
            <button
              className="btn-icon"
              onClick={() => setShowKey(!showKey)}
              title={showKey ? '隐藏' : '显示'}
            >
              {showKey ? '🙈' : '👁'}
            </button>
          </div>
          <button
            className="btn-primary"
            onClick={handleSaveKey}
            disabled={keyStatus === 'saving' || apiKey.trim() === ''}
          >
            {keyStatus === 'saving' ? '保存中...' : keyStatus === 'saved' ? '✓ 已保存' : keyStatus === 'error' ? '保存失败' : '保存'}
          </button>
        </div>
        {!apiKey.trim() && (
          <p className="settings-warning">
            ⚠ 未填写 API Key，在线模式将无法使用。
          </p>
        )}
      </section>

      {/* Appearance */}
      <section className="settings-card">
        <h2>外观</h2>
        <div className="settings-field">
          <label>主题</label>
          <div className="theme-selector">
            {(['light', 'dark', 'system'] as const).map((t) => (
              <button
                key={t}
                className={`theme-btn ${theme === t ? 'active' : ''}`}
                onClick={() => handleThemeChange(t)}
              >
                {t === 'light' && '☀ 浅色'}
                {t === 'dark' && '🌙 深色'}
                {t === 'system' && '💻 跟随系统'}
              </button>
            ))}
          </div>
        </div>
      </section>

      {/* Memory Management */}
      <MemorySection />

      {/* About */}
      <section className="settings-card">
        <h2>关于</h2>
        <div className="about-info">
          <div className="about-row">
            <span className="about-label">应用名称</span>
            <span className="about-value">LocalMind</span>
          </div>
          <div className="about-row">
            <span className="about-label">版本号</span>
            <span className="about-value">0.1.0</span>
          </div>
          <div className="about-row">
            <span className="about-label">运行平台</span>
            <span className="about-value">Windows</span>
          </div>
        </div>
      </section>
    </div>
  );
}
