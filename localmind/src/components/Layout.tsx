import { ONLINE_MODEL_LABEL, OFFLINE_MODEL_LABEL } from '@/config/models';
import React, { useState } from 'react';
import { NavLink } from 'react-router-dom';
import ModeSwitch from './ModeSwitch';
import StatusIndicator from './StatusIndicator';
import { useChatStore } from '@/stores/chatStore';

interface LayoutProps { children: React.ReactNode; }

const navItems = [
  {
    path: '/',
    label: '对话',
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 11.5a8.38 8.38 0 0 1-8.5 8.5 8.5 8.5 0 0 1-3.8-.9L3 21l1.9-5.7a8.5 8.5 0 1 1 16.1-3.8Z" />
      </svg>
    ),
  },
  {
    path: '/models',
    label: '模型',
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect x="4" y="4" width="16" height="16" rx="3" />
        <path d="M9 9h6v6H9z" />
        <path d="M9 2v2M15 2v2M9 20v2M15 20v2M2 9h2M2 15h2M20 9h2M20 15h2" />
      </svg>
    ),
  },
  {
    path: '/account',
    label: '账号',
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="8" r="4" />
        <path d="M4 21a8 8 0 0 1 16 0" />
      </svg>
    ),
  },
  {
    path: '/settings',
    label: '设置',
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h.01a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51h.01a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v.01a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z" />
      </svg>
    ),
  },
];

export default function Layout({ children }: LayoutProps) {
  const mode = useChatStore((s) => s.mode);
  const latencyMs = useChatStore((s) => s.latencyMs);
  const ollamaRunning = useChatStore((s) => s.ollamaRunning);
  const [theme, setTheme] = useState<'light' | 'dark'>(() => {
    const saved = localStorage.getItem('localmind-theme');
    if (saved === 'dark' || saved === 'light') return saved;
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  });

  const applyTheme = (t: 'light' | 'dark') => {
    document.documentElement.setAttribute('data-theme', t);
    localStorage.setItem('localmind-theme', t);
  };

  const toggleTheme = () => {
    setTheme((prev) => {
      const next = prev === 'light' ? 'dark' : 'light';
      applyTheme(next);
      return next;
    });
  };

  const modelLabel = mode === 'online' ? ONLINE_MODEL_LABEL : ollamaRunning ? OFFLINE_MODEL_LABEL : '离线模式';

  return (
    <div className="app-layout">
      <aside className="app-sidebar">
        <div className="sidebar-logo">
          <span className="logo-icon">M</span>
          <span className="logo-text">LocalMind</span>
        </div>
        <nav className="sidebar-nav">
          {navItems.map((item) => (
            <NavLink
              key={item.path}
              to={item.path}
              end={item.path === '/'}
              className={({ isActive }) => `sidebar-nav-item ${isActive ? 'active' : ''}`}
            >
              <span className="nav-icon">{item.icon}</span>
              <span className="nav-label">{item.label}</span>
            </NavLink>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <ModeSwitch />
          <StatusIndicator />
        </div>
      </aside>
      <main className="app-main">
        <div className="topbar">
          <div className="topbar-left">
            <span className="topbar-title">LocalMind</span>
            <span className="topbar-divider">/</span>
            <span className="topbar-sub">{modelLabel}</span>
          </div>
          <div className="topbar-right">
            <div className="model-pill">
              <span className="model-pill-dot" />
              {modelLabel}
            </div>
            <div className="online-badge">
              <span className={`online-dot ${mode === 'online' ? 'on' : 'off'}`} />
              {mode === 'online' ? `在线 ${latencyMs || 0}ms` : '离线'}
            </div>
            <button
              className="icon-btn"
              onClick={toggleTheme}
              title="切换深色/浅色主题"
              aria-label="切换主题"
            >
              {theme === 'light' ? (
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79Z" />
                </svg>
              ) : (
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="12" cy="12" r="4" />
                  <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" />
                </svg>
              )}
            </button>
          </div>
        </div>
        {children}
        <div className="app-statusbar">
          <div className="statusbar-item">
            <span className={`statusbar-dot ${mode === 'online' ? 'ok' : 'off'}`} />
            <span>{mode === 'online' ? `已连接 · 在线 ${latencyMs || 0}ms` : '离线模式'}</span>
          </div>
          <div className="statusbar-item">
            <span className={`statusbar-dot ${ollamaRunning ? 'ok' : 'idle'}`} />
            <span>Ollama {ollamaRunning ? '就绪' : '未连接'}</span>
          </div>
          <div className="statusbar-item statusbar-right">
            <span className="statusbar-mode">
              {mode === 'online' ? `在线模式 · ${ONLINE_MODEL_LABEL}` : `离线模式 · ${OFFLINE_MODEL_LABEL}`}
            </span>
          </div>
        </div>
      </main>
    </div>
  );
}