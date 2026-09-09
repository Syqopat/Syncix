import * as vscode from 'vscode';
import { RpcManager } from '../rpc/manager';
import { MessageEnvelope, RequestPriority } from '../rpc/queue';

export class EditorStateManager {
    private rpc: RpcManager;
    
    // States
    public isConnected: boolean = false;
    public selectedNodeIds: string[] = [];
    public pendingPatchesCount: number = 0;

    // Events
    private _onSelectionChanged = new vscode.EventEmitter<string[]>();
    public readonly onSelectionChanged = this._onSelectionChanged.event;

    private _onStateChanged = new vscode.EventEmitter<void>();
    public readonly onStateChanged = this._onStateChanged.event;

    constructor(rpcManager: RpcManager) {
        this.rpc = rpcManager;

        // Bind RPC Connection State
        this.rpc.onConnectionChange((connected) => {
            this.isConnected = connected;
            this._onStateChanged.fire();
        });

        // Listen for Patches or Global Events
        this.rpc.onMessage((msg: MessageEnvelope) => {
            if (msg.event_type === "SYNC_STATUS_CHANGED") {
                this.pendingPatchesCount = msg.data.pending_count || 0;
                this._onStateChanged.fire();
            }
        });
    }

    public setSelectedNodes(ids: string[], source: 'vscode' | 'rpc' = 'vscode') {
        // Compare arrays
        const same = this.selectedNodeIds.length === ids.length && 
                     this.selectedNodeIds.every((val, index) => val === ids[index]);
                     
        if (!same) {
            this.selectedNodeIds = ids;
            this._onSelectionChanged.fire(ids);
            this._onStateChanged.fire();
            
            // Sync selection to Studio only if the source was VS Code
            if (source === 'vscode') {
                this.rpc.send("SELECTION_CHANGED", { ids: ids }, RequestPriority.Low);
            }
        }
    }

    public dispose() {
        this._onSelectionChanged.dispose();
        this._onStateChanged.dispose();
    }
}
