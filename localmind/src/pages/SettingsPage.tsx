import React, { useState, useEffect } from 'react';
import { tauriInvoke } from '@/api/ipc';

type ThemeMode = 'light' | 'dark' | 'system';

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
