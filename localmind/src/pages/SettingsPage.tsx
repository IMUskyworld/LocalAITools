import React, { useState, useEffect } from 'react';

type ThemeMode = 'light' | 'dark' | 'system';

export default function SettingsPage() {
  const [theme, setTheme] = useState<ThemeMode>(() => {
    return (localStorage.getItem('localmind-theme') as ThemeMode) || 'light';
  });

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

  return (
    <div className="settings-page">
      <h1 className="page-title">设置</h1>

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
