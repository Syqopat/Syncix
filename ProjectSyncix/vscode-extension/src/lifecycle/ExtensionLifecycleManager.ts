import * as vscode from 'vscode';
import { RpcManager } from '../rpc/manager';
import { EditorStateManager } from '../state/EditorStateManager';
import { WorkspaceExplorer } from '../explorer/WorkspaceExplorer';

export class ExtensionLifecycleManager {
    private rpcManager: RpcManager;
    private stateManager: EditorStateManager;
    private explorer: WorkspaceExplorer;
    private disposables: vscode.Disposable[] = [];

    constructor(context: vscode.ExtensionContext) {
        // 1. Initialize RPC Layer
        this.rpcManager = new RpcManager();
        this.disposables.push(this.rpcManager as any);

        // 2. Initialize State Manager
        this.stateManager = new EditorStateManager(this.rpcManager);
        this.disposables.push(this.stateManager as any);

        // 3. Initialize Workspace Explorer Cache & UI
        this.explorer = new WorkspaceExplorer(context, this.stateManager, this.rpcManager);
        this.disposables.push(this.explorer as any);

        // Bind Context
        context.subscriptions.push(this);
    }

    public activate() {
        console.log("Syncix Extension Lifecycle: Activating...");
        
        // Register Commands
        vscode.commands.registerCommand('syncix.connect', () => {
            this.rpcManager.connect();
        });

        // Auto-connect on startup
        this.rpcManager.connect();
    }

    public dispose() {
        console.log("Syncix Extension Lifecycle: Disposing resources...");
        for (const d of this.disposables) {
            if (d && typeof d.dispose === 'function') {
                d.dispose();
            }
        }
    }
}
