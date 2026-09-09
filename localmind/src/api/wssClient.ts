// WebSocket client for relay-server connection (WSS)
// Used for remote control pairing and command relay

import type { CommandEnvelope } from '@/types/envelope';

type MessageHandler = (envelope: CommandEnvelope) => void;
type ConnectionHandler = (connected: boolean) => void;

export interface WSSClientOptions {
  url: string;
  authTicket: string;
  deviceId: string;
  onMessage: MessageHandler;
  onConnectionChange: ConnectionHandler;
  onError: (error: string) => void;
}

export class WSSClient {
  private ws: WebSocket | null = null;
  private options: WSSClientOptions;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10;
  private baseDelay = 1000;
  private destroyed = false;

  constructor(options: WSSClientOptions) {
    this.options = options;
  }

  connect(): void {
    if (this.ws?.readyState === WebSocket.OPEN || this.ws?.readyState === WebSocket.CONNECTING) {
      return;
    }
    this.destroyed = false;
    this.reconnectAttempts = 0;

    const wsUrl = `${this.options.url}?auth=${encodeURIComponent(this.options.authTicket)}&device_id=${encodeURIComponent(this.options.deviceId)}`;
    this.ws = new WebSocket(wsUrl);

    this.ws.onopen = () => {
      this.reconnectAttempts = 0;
      this.options.onConnectionChange(true);
      this.startHeartbeat();
    };

    this.ws.onclose = () => {
      this.options.onConnectionChange(false);
      this.stopHeartbeat();
      this.scheduleReconnect();
    };

    this.ws.onerror = () => {
      this.options.onError('WebSocket connection error');
    };

    this.ws.onmessage = (event: MessageEvent) => {
      try {
        const envelope: CommandEnvelope = JSON.parse(event.data as string);
        this.options.onMessage(envelope);
      } catch {
        this.options.onError('Failed to parse WSS message');
      }
    };
  }

  disconnect(): void {
    this.destroyed = true;
    this.stopHeartbeat();
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.close();
      this.ws = null;
    }
    this.options.onConnectionChange(false);
  }

  send(envelope: CommandEnvelope): boolean {
    if (this.ws?.readyState !== WebSocket.OPEN) {
      this.options.onError('WebSocket not connected');
      return false;
    }
    this.ws.send(JSON.stringify(envelope));
    return true;
  }

  private startHeartbeat(): void {
    this.stopHeartbeat();
    this.heartbeatTimer = setInterval(() => {
      const heartbeat: CommandEnvelope = {
        version: 'v1',
        id: crypto.randomUUID(),
        type: 'heartbeat',
        from_device_id: this.options.deviceId,
        timestamp: Date.now(),
      };
      this.send(heartbeat);
    }, 30000);
  }

  private stopHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  private scheduleReconnect(): void {
    if (this.destroyed || this.reconnectAttempts >= this.maxReconnectAttempts) {
      return;
    }
    const delay = this.baseDelay * Math.pow(2, this.reconnectAttempts);
    this.reconnectAttempts++;
    this.reconnectTimer = setTimeout(() => {
      if (!this.destroyed) {
        this.connect();
      }
    }, Math.min(delay, 30000)); // cap at 30s
  }
}
