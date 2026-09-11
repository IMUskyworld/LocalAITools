import React, { useEffect, useState } from 'react';
import { getRelayBaseUrl, getStoredDevice } from '@/api/account';
import { useAccountStore } from '@/stores/accountStore';

type Mode = 'login' | 'register';

export default function AccountPage() {
  const {
    status,
    user,
    devices,
    initialized,
    busy,
    error,
    initialize,
    login,
    register,
    logout,
    refreshDevices,
    removeDevice,
    clearError,
  } = useAccountStore();
  const [mode, setMode] = useState<Mode>('login');
  const [email, setEmail] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [formError, setFormError] = useState('');
  const currentDevice = getStoredDevice();

  useEffect(() => {
    void initialize();
  }, [initialize]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setFormError('');
    clearError();
    if (mode === 'register' && password !== confirmPassword) {
      setFormError('两次输入的密码不一致');
      return;
    }
    try {
      if (mode === 'login') {
        await login(email, password);
      } else {
        await register(email, displayName, password);
      }
      setPassword('');
      setConfirmPassword('');
    } catch {
      // Store exposes the user-facing error.
    }
  };

  if (!initialized || status === 'loading') {
    return (
      <div className="account-page">
        <div className="account-loading">
          <span className="account-spinner" />
          正在连接账号服务…
        </div>
      </div>
    );
  }

  return (
    <div className="account-page">
      <header className="account-page-header">
        <div>
          <h1 className="page-title">账号与设备</h1>
          <p>本地功始终无需登录；只有远程协同和设备发现需要账号。</p>
        </div>
        <span className={`account-mode-badge ${status === 'authenticated' ? 'online' : ''}`}>
          {status === 'authenticated' ? '账号模式' : '游客模式'}
        </span>
      </header>

      {(error || formError) && (
        <div className="account-alert error">{formError || error}</div>
      )}

      {status === 'guest' ? (
        <div className="account-guest-layout">
          <section className="account-card account-guest-intro">
            <span className="account-eyebrow">LOCAL FIRST</span>
            <h2>不登录也能完整使用本地能力</h2>
            <p>聊天、Agent、文件处理和文档生成不会被账号系统阻塞。登录后只增加“我的设备”与远程协同能力。</p>
            <ul>
              <li>同一账号用于识别设备归属</li>
              <li>同账号不会自动获得远程控制权</li>
              <li>电脑必须本机确认后才能接受手机任务</li>
            </ul>
          </section>

          <section className="account-card account-form-card">
            <div className="account-tabs">
              <button className={mode === 'login' ? 'active' : ''} onClick={() => setMode('login')}>
                登录
              </button>
              <button className={mode === 'register' ? 'active' : ''} onClick={() => setMode('register')}>
                注册
              </button>
            </div>
            <form className="account-form" onSubmit={submit}>
              {mode === 'register' && (
                <label>
                  <span>显示名称</span>
                  <input
                    value={displayName}
                    onChange={(event) => setDisplayName(event.target.value)}
                    placeholder="例如：小明的电脑"
                    maxLength={80}
                    required
                  />
                </label>
              )}
              <label>
                <span>邮箱</span>
                <input
                  type="email"
                  value={email}
                  onChange={(event) => setEmail(event.target.value)}
                  placeholder="name@example.com"
                  autoComplete="email"
                  required
                />
              </label>
              <label>
                <span>密码</span>
                <input
                  type="password"
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  placeholder="至少 8 位"
                  minLength={8}
                  autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
                  required
                />
              </label>
              {mode === 'register' && (
                <label>
                  <span>确认密码</span>
                  <input
                    type="password"
                    value={confirmPassword}
                    onChange={(event) => setConfirmPassword(event.target.value)}
                    minLength={8}
                    autoComplete="new-password"
                    required
                  />
                </label>
              )}
              <button className="account-primary" type="submit" disabled={busy}>
                {busy ? '请稍候…' : mode === 'login' ? '登录并登记此电脑' : '注册账号并登记此电脑'}
              </button>
            </form>
          </section>
        </div>
      ) : (
        <div className="account-grid">
          <section className="account-card account-profile-card">
            <div className="account-profile-avatar">
              {(user?.display_name || user?.email || 'L').slice(0, 1).toUpperCase()}
            </div>
            <div className="account-profile-copy">
              <strong>{user?.display_name}</strong>
              <span>{user?.email}</span>
            </div>
            <button className="account-secondary" onClick={() => void logout()} disabled={busy}>
              退出登录
            </button>
          </section>

          <section className="account-card account-devices-card">
            <div className="account-section-title">
              <div>
                <h2>我的设备</h2>
                <p>设备归属仅用于发现；远程控制仍需 Windows 本机确认。</p>
              </div>
              <button className="account-secondary" onClick={() => void refreshDevices()} disabled={busy}>
                刷新
              </button>
            </div>
            <div className="account-device-list">
              {devices.map((device) => {
                const isCurrent = device.id === currentDevice?.device_id;
                return (
                  <div className="account-device" key={device.id}>
                    <span className={`account-device-icon ${device.platform}`}>
                      {device.platform === 'windows' ? 'PC' : '手机'}
                    </span>
                    <div className="account-device-copy">
                      <strong>{device.device_name}</strong>
                      <small>
                        {device.platform === 'windows' ? 'Windows' : 'Android'}
                        {isCurrent ? ' · 当前设备' : ''}
                      </small>
                    </div>
                    <span className="account-device-state">已登记</span>
                    {!isCurrent && (
                      <button
                        className="account-remove"
                        onClick={() => void removeDevice(device.id)}
                        disabled={busy}
                        title="从账号中移除此设备"
                      >
                        移除
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          </section>

          <section className="account-card account-relay-card">
            <h2>协同服务</h2>
            <div className="account-relay-row">
              <span>Relay 地址</span>
              <code>{getRelayBaseUrl()}</code>
            </div>
            <div className="account-relay-row">
              <span>当前设备</span>
              <code>{currentDevice?.device_id || '正在登记'}</code>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}
