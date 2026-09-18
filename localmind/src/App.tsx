import React, { useEffect } from 'react';
import { startRelayListener, stopRelayListener } from './api/relayManager';
import { Routes, Route, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import ChatPage from './pages/ChatPage';
import ModelsPage from './pages/ModelsPage';
import SettingsPage from './pages/SettingsPage';
import AccountPage from './pages/AccountPage';
import { useAccountStore } from './stores/accountStore';

function App() {
  useEffect(() => {
    // 启动 Relay WSS 连接（游客模式自动跳过）
    startRelayListener();
    return () => stopRelayListener();
  }, []);

  useEffect(() => {
    // 全局轮询「待本机批准的控制授权申请」+ 恢复账号会话。
    // 2026-09-18：此前只在打开「账号」页时拉一次，手机发起申请后
    // 电脑端永远发现不了（用户实测）；现在 App 一起来就轮询，新申请弹通知。
    const store = useAccountStore.getState();
    void store.initialize();
    store.startPairingWatcher();
    return () => useAccountStore.getState().stopPairingWatcher();
  }, []);

  useEffect(() => {
    const savedTheme = localStorage.getItem('localmind-theme') || 'light';

    if (savedTheme === 'system') {
      const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      document.documentElement.setAttribute('data-theme', prefersDark ? 'dark' : 'light');
    } else {
      document.documentElement.setAttribute('data-theme', savedTheme);
    }
  }, []);

  return (
    <Layout>
      <Routes>
        <Route path="/" element={<ChatPage />} />
        <Route path="/models" element={<ModelsPage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="/account" element={<AccountPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Layout>
  );
}

export default App;
