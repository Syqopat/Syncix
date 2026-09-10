import * as vscode from 'vscode';
import { RpcManager } from '../rpc/manager';
import { getBaseUrl } from '../core/env';

export class DiagnosticsPanel {
    public static currentPanel: DiagnosticsPanel | undefined;
    private readonly _panel: vscode.WebviewPanel;
    private _disposables: vscode.Disposable[] = [];

    private constructor(panel: vscode.WebviewPanel, private rpcClient: RpcManager) {
        this._panel = panel;
        this._panel.onDidDispose(() => this.dispose(), null, this._disposables);

        this.rpcClient.onMessage((envelope) => {
            if (envelope.event_type === 'TELEMETRY_UPDATE') {
                this._panel.webview.postMessage(envelope.data);
            }
        });

        this._update();
    }

    public static createOrShow(rpcClient: RpcManager) {
        const column = vscode.ViewColumn.Active;

        if (DiagnosticsPanel.currentPanel) {
            DiagnosticsPanel.currentPanel._panel.reveal(column);
            return;
        }

        const panel = vscode.window.createWebviewPanel(
            'syncixDiagnostics',
            'Syncix Core Diagnostics',
            column,
            {
                enableScripts: true,
                retainContextWhenHidden: true,
            }
        );

        DiagnosticsPanel.currentPanel = new DiagnosticsPanel(panel, rpcClient);
    }

    private _update() {
        this._panel.webview.html = this._getHtmlForWebview();
    }

    private _getHtmlForWebview() {
        // Webview ownVersion surecinde calisir; core adresi HTML'e gomulur cunku
        // port artik sabit degil (core dolu portu atlayabiliyor).
        const base = getBaseUrl();
        return `
            <!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8">
                <style>
                    body { font-family: var(--vscode-font-family); padding: 20px; color: var(--vscode-foreground); }
                    .card { background: var(--vscode-editor-background); border: 1px solid var(--vscode-panel-border); padding: 15px; border-radius: 4px; margin-bottom: 10px; }
                    .header { font-size: 1.5em; margin-bottom: 15px; font-weight: bold; }
                    .metric-row { display: flex; justify-content: space-between; margin-bottom: 8px; font-size: 1.1em;}
                    .metric-val { font-family: 'Consolas', monospace; color: var(--vscode-terminal-ansiGreen); }
                    .error { color: var(--vscode-editorError-foreground); }
                </style>
            </head>
            <body>
                <div class="header">Syncix Health Monitor & Observability</div>
                
                <div class="card">
                    <h3>Core Engine Status</h3>
                    <div class="metric-row"><span>Status:</span> <span id="status" class="metric-val">Connecting...</span></div>
                    <div class="metric-row"><span>Uptime:</span> <span id="uptime" class="metric-val">0 s</span></div>
                    <div class="metric-row"><span>Memory Usage:</span> <span id="mem" class="metric-val">0 MB</span></div>
                </div>

                <div class="card">
                    <h3>Network & Throughput</h3>
                    <div class="metric-row"><span>Active Connections:</span> <span id="connections" class="metric-val">0</span></div>
                    <div class="metric-row"><span>Messages Processed:</span> <span id="messages" class="metric-val">0</span></div>
                    <div class="metric-row"><span>Chaos Engineering Mode:</span> <span id="chaos" class="metric-val">Disabled</span></div>
                </div>

                <script>
                    async function fetchHealth() {
                        try {
                            const res = await fetch('${base}/health');
                            if (!res.ok) throw new Error("HTTP " + res.status);
                            const data = await res.json();
                            
                            document.getElementById('status').innerText = data.status;
                            document.getElementById('status').className = 'metric-val';
                            document.getElementById('uptime').innerText = data.uptime_seconds + ' s';
                            document.getElementById('mem').innerText = data.memory_usage_mb + ' MB';
                            document.getElementById('connections').innerText = data.active_connections;
                            document.getElementById('messages').innerText = data.messages_processed;
                        } catch (e) {
                            document.getElementById('status').innerText = 'Offline (' + e.message + ')';
                            document.getElementById('status').className = 'metric-val error';
                        }
                    }

                    // Poll the health endpoint every 1 second
                    setInterval(fetchHealth, 1000);
                    fetchHealth();
                </script>
            </body>
            </html>
        `;
    }

    public dispose() {
        DiagnosticsPanel.currentPanel = undefined;
        this._panel.dispose();
        while (this._disposables.length) {
            const x = this._disposables.pop();
            if (x) x.dispose();
        }
    }
}
