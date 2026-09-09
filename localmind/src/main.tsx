import React from 'react';
import ReactDOM from 'react-dom/client';
import { HashRouter } from 'react-router-dom';
import App from './App';
import ErrorBoundary from './components/ErrorBoundary';
import './styles.css';

// Catch ALL errors including React render errors
window.addEventListener('error', (e) => {
  e.preventDefault();
  document.title = 'LocalMind 错误';
  document.body.innerHTML = `<pre style="padding:20px;background:#1E1E2E;color:#EF5350;font-family:monospace;font-size:13px;height:100vh;overflow:auto;white-space:pre-wrap;word-wrap:break-word;">
<h2 style="color:#FF6B6B">LocalMind 错误报告</h2>
<b>${e.message}</b>
${e.error?.stack ? '\n\n' + e.error.stack : ''}
${e.filename ? '\n\nat ' + e.filename + ':' + e.lineno + ':' + e.colno : ''}
</pre>`;
  return true;
}, true);

window.addEventListener('unhandledrejection', (e) => {
  e.preventDefault();
  document.title = 'LocalMind 错误';
  document.body.innerHTML = `<pre style="padding:20px;background:#1E1E2E;color:#EF5350;font-family:monospace;font-size:13px;height:100vh;overflow:auto;white-space:pre-wrap;word-wrap:break-word;">
<h2 style="color:#FF6B6B">未处理的异步错误</h2>
${e.reason?.toString() || JSON.stringify(e.reason, null, 2)}
${e.reason?.stack ? '\n\n' + e.reason.stack : ''}
</pre>`;
});

const root = document.getElementById('root');
if (!root) {
  document.body.innerHTML = '<h2>错误：未找到 #root 节点</h2>';
} else {
  ReactDOM.createRoot(root).render(
    <ErrorBoundary>
      <HashRouter>
        <App />
      </HashRouter>
    </ErrorBoundary>
  );
}
