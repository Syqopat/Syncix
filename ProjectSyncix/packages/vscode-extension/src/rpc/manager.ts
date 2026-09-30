import * as vscode from 'vscode';
import WebSocket from 'ws';
import { MessageEnvelope, PendingRequestQueue, RequestPriority } from './queue';
import { getWsUrl } from '../core/env';

export class RpcManager {
    private ws: WebSocket | null = null;
    private isConnected: boolean = false;
    // The port can change, so the address is resolved at connect time (see core/env.ts).
    private get uri(): string { return getWsUrl(); }
    
    private pendingQueue = new PendingRequestQueue();
    private activeRequests = new Map<string, { resolve: Function, reject: Function, timer: NodeJS.Timeout }>();

    private _onMessage = new vscode.EventEmitter<MessageEnvelope>();
    public readonly onMessage = this._onMessage.event;

    private _onConnectionChange = new vscode.EventEmitter<boolean>();
    public readonly onConnectionChange = this._onConnectionChange.event;

    constructor() {}

    public connect() {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            return;
        }

        this.ws = new WebSocket(this.uri);

        this.ws.on('open', () => {
            this.isConnected = true;
            this._onConnectionChange.fire(true);
            vscode.window.showInformationMessage('Syncix RPC: Connected');
            this.flushPendingQueue();
        });

        this.ws.on('message', (data: WebSocket.RawData) => {
            try {
                const text = data.toString('utf8');
                const envelope = JSON.parse(text) as MessageEnvelope;

                // Handle Response Matching
                if (envelope.request_id && this.activeRequests.has(envelope.request_id)) {
                    const req = this.activeRequests.get(envelope.request_id)!;
                    clearTimeout(req.timer);
                    req.resolve(envelope.data);
                    this.activeRequests.delete(envelope.request_id);
                } else {
                    // Push Event
                    this._onMessage.fire(envelope);
                }
            } catch (err) {
                console.error("RPC parse error:", err);
            }
        });

        this.ws.on('close', () => {
            const wasConnected = this.isConnected;
            this.isConnected = false;
            this._onConnectionChange.fire(false);
            // Only warn on a real disconnect; do not spam a notification
            // on every background reconnect attempt.
            if (wasConnected) {
                vscode.window.showWarningMessage('Syncix: offline (RPC disconnected)');
            }

            // Reconnect
            setTimeout(() => this.connect(), 5000);
        });

        this.ws.on('error', (err) => {
            console.error("RPC ws error:", err);
        });
    }

    private flushPendingQueue() {
        if (!this.isConnected || !this.ws) return;
        const validRequests = this.pendingQueue.dequeueAllValid();
        
        for (const req of validRequests) {
            this.sendEnvelopeRaw(req.envelope, req.resolve, req.reject);
        }
    }

    public send(eventType: string, data: any, priority: RequestPriority = RequestPriority.Normal): Promise<any> {
        const requestId = this.generateUuid();
        const envelope: MessageEnvelope = {
            event_type: eventType,
            api_version: 1,
            request_id: requestId,
            retry_count: 0,
            created_at: Date.now(),
            data
        };

        if (!this.isConnected) {
            return this.pendingQueue.enqueue(envelope, priority);
        }

        return new Promise((resolve, reject) => {
            this.sendEnvelopeRaw(envelope, resolve, reject);
        });
    }

    private sendEnvelopeRaw(envelope: MessageEnvelope, resolve?: Function, reject?: Function) {
        if (!this.ws || !this.isConnected) {
            if (reject) reject(new Error("Not connected"));
            return;
        }

        if (envelope.request_id && resolve && reject) {
            const timer = setTimeout(() => {
                this.activeRequests.delete(envelope.request_id!);
                // TODO: Implement Retry Logic instead of direct reject
                reject(new Error("RPC Timeout"));
            }, 10000); // 10s timeout

            this.activeRequests.set(envelope.request_id, { resolve, reject, timer });
        }

        this.ws.send(JSON.stringify(envelope));
    }

    private generateUuid(): string {
        return Math.random().toString(36).substring(2, 15) + Math.random().toString(36).substring(2, 15);
    }

    public dispose() {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
        this._onMessage.dispose();
        this._onConnectionChange.dispose();
    }
}
