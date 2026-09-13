import React, { useState, useEffect, useCallback } from 'react';
import { tauriInvoke } from '@/api/ipc';

type ThemeMode = 'light' | 'dark' | 'system';


interface MemoryEntry { id: string; category: string; content: string; confidence: number; }
interface SessionSummary { session_id: string; session_title: string; summary: string; updated_at: number; }

/**
 * 记忆管理。
 *
 * 分两块展示：
 *  - 「会话摘要」：Agent 每轮结束后自动生成并写入 session_summaries 表，
 *    这是真正在用的"记忆文档"，也是上一轮对话被压缩成的内容。
 *  - 「长期记忆」：memory_entries 表，供未来跨会话事实沉淀使用。
 *
 * 之前这里只读 memory_entries（从未写入过数据），所以永远显示"暂无记忆条目"。
 */
function MemorySection() {
  const [tab, setTab] = useState<'summaries' | 'memories'>('summaries');
  const [summaries, setSummaries] = useState<SessionSummary[]>([]);
  const [memories, setMemories] = useState<MemoryEntry[]>([]);
  const [keyword, setKeyword] = useState('');
  const [loading, setLoading] = useState(false);

  const loadSummaries = useCallback(async () => {
    setLoading(true);
    try {
      const res: any = await tauriInvoke('list_session_summaries', { limit: 100 });
      setSummaries(res?.data || []);
    } catch { setSummaries([]); }
    setLoading(false);
  }, []);

  const loadMemories = useCallback(async () => {
    setLoading(true);
    try {
      const fn = keyword.trim() ? 'search_memories' : 'get_memories';
      const args = keyword.trim() ? { keyword: keyword.trim(), limit: 50 } : { limit: 50 };
      const res: any = await tauriInvoke(fn, args);
      setMemories(res?.data || []);
    } catch { setMemories([]); }
    setLoading(false);
  }, [keyword]);

  useEffect(() => {
    if (tab === 'summaries') void loadSummaries();
    else void loadMemories();
  }, [tab, loadSummaries, loadMemories]);

  const handleDeleteMemory = async (id: string) => {
    await tauriInvoke('delete_memory', { memoryId: id });
    setMemories((prev) => prev.filter((m) => m.id !== id));
  };

  return (
    <section className="settings-card">
      <h2>记忆管理</h2>
      <p className="settings-hint">
        Agent 的对话记忆。会话摘要由每轮对话自动生成，用于在后续轮次中回忆早期上下文。
      </p>

      <div className="theme-selector" style={{ marginBottom: 12 }}>
        <button
          className={`theme-btn ${tab === 'summaries' ? 'active' : ''}`}
          onClick={() => setTab('summaries')}
        >
          会话摘要
        </button>
        <button
          className={`theme-btn ${tab === 'memories' ? 'active' : ''}`}
          onClick={() => setTab('memories')}
        >
          长期记忆
        </button>
      </div>

      {tab === 'memories' && (
        <div className="api-key-input-row" style={{ marginBottom: 12 }}>
          <input
            type="text"
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            placeholder="搜索关键词..."
            className="api-key-input"
          />
          <button className="btn-primary" onClick={loadMemories} disabled={loading}>
            {loading ? '...' : '搜索'}
          </button>
        </div>
      )}

      {tab === 'summaries' && (
        <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: 8 }}>
          <button className="btn-primary" onClick={loadSummaries} disabled={loading}>
            {loading ? '...' : '刷新'}
          </button>
        </div>
      )}

      {tab === 'summaries' ? (
        summaries.length === 0 ? (
          <p className="settings-hint">暂无会话摘要。与 Agent 对话几轮后，这里会自动出现摘要。</p>
        ) : (
          <div style={{ maxHeight: 320, overflowY: 'auto' }}>
            {summaries.map((s) => (
              <div key={s.session_id} style={{ padding: '10px 0', borderBottom: '1px solid var(--border, #eee)' }}>
                <div style={{ fontSize: '0.78rem', color: 'var(--accent, #6c5ce7)', fontWeight: 600, marginBottom: 4 }}>
                  {s.session_title || '(未命名会话)'}
                  <span style={{ color: 'var(--text-secondary, #999)', fontWeight: 400, marginLeft: 8 }}>
                    {new Date(s.updated_at).toLocaleString()}
                  </span>
                </div>
                <div style={{ fontSize: '0.85rem', whiteSpace: 'pre-wrap' }}>{s.summary}</div>
              </div>
            ))}
          </div>
        )
      ) : memories.length === 0 ? (
        <p className="settings-hint">暂无长期记忆条目。</p>
      ) : (
        <div style={{ maxHeight: 320, overflowY: 'auto' }}>
          {memories.map((m) => (
            <div key={m.id} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start', padding: '8px 0', borderBottom: '1px solid var(--border, #eee)' }}>
              <div style={{ flex: 1 }}>
                <span style={{ fontSize: '0.75rem', color: 'var(--accent, #6c5ce7)', fontWeight: 600, marginRight: 8 }}>[{m.category}]</span>
                <span style={{ fontSize: '0.85rem' }}>{m.content}</span>
              </div>
              <button onClick={() => handleDeleteMemory(m.id)} style={{ background: 'none', border: 'none', color: '#e74c3c', cursor: 'pointer', fontSize: '0.8rem', marginLeft: 8 }}>删除</button>
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
  const [testStatus, setTestStatus] = useState<'idle' | 'testing' | 'ok' | 'fail'>('idle');
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

  const handleTestConnection = async () => {
    if (!apiKey.trim()) return;
    setTestStatus('testing');
    try {
      const res = await fetch('https://api.deepseek.com/chat/completions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: 'Bearer ' + apiKey.trim() },
        body: JSON.stringify({ model: 'deepseek-flash', messages: [{ role: 'user', content: 'hi' }], max_tokens: 1 }),
        signal: AbortSignal.timeout(10000),
      });
      setTestStatus(res.ok ? 'ok' : 'fail');
    } catch { setTestStatus('fail'); }
    setTimeout(() => setTestStatus('idle'), 5000);
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
          <button
            className="btn-primary"
            style={{ background: testStatus === 'fail' ? '#e74c3c' : '#4caf50', marginLeft: 8 }}
            onClick={handleTestConnection}
            disabled={testStatus === 'testing' || !apiKey.trim()}
          >
            {testStatus === 'testing' ? '测试中...' : testStatus === 'ok' ? '✓ 连通' : testStatus === 'fail' ? '✗ 失败' : '测试联通'}
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
