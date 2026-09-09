import React, { Component, ErrorInfo, ReactNode } from 'react';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export default class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('React Error Boundary caught:', error, info);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div style={{
          padding: '40px', fontFamily: 'monospace',
          background: '#1E1E2E', color: '#E0E0E0', height: '100vh',
          display: 'flex', flexDirection: 'column', justifyContent: 'center', alignItems: 'center'
        }}>
          <h2 style={{ color: '#EF5350', marginBottom: '16px' }}>⛔ 界面出错</h2>
          <div style={{
            background: '#2A2A3E', padding: '20px', borderRadius: '8px',
            maxWidth: '80vw', overflow: 'auto', whiteSpace: 'pre-wrap',
            fontFamily: 'monospace', fontSize: '13px'
          }}>
            {this.state.error?.toString()}
          </div>
          <p style={{ marginTop: '16px', color: '#999' }}>
            {this.state.error?.stack?.split('\n').slice(0,5).join('\n')}
          </p>
        </div>
      );
    }
    return this.props.children;
  }
}
