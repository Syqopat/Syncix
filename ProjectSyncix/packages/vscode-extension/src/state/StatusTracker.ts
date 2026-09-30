import * as vscode from 'vscode';
import { RpcManager } from '../rpc/manager';
import * as env from '../core/env';

/** One line of the live sync stream shown in the Syncix panel. */
export interface StreamEntry {
    direction: 'in' | 'out' | 'note';
    label: string;
    detail: string;
    time: number;
}

/** Everything the status bar and the Syncix panel show about the current sync. */
export interface StatusSnapshot {
    coreUp: boolean;
    studioConnected: boolean;
    syncSuspended: boolean;
    objectCount: number;
    conflicts: number;
    project: string;
    port: number;
    coreVersion: string;
    extensionVersion: string;
    uptimeSeconds: number;
    mode: string;
    queued: number;
    coalesced: number;
    loops: number;
    stream: StreamEntry[];
}

const STREAM_LIMIT = 60;

/**
 * Single source of truth for "is it working right now".
 *
 * The status bar and the panel used to answer that question separately: the status bar
 * polled /health while the panel's webview fetched the same endpoint on its own timer
 * with the address baked into its HTML. Two pollers meant two different answers on
 * screen at the same time. One tracker polls, everything else listens.
 */
export class StatusTracker {
    private readonly _onChange = new vscode.EventEmitter<StatusSnapshot>();
    public readonly onChange = this._onChange.event;

    private snapshot: StatusSnapshot = {
        coreUp: false,
        studioConnected: false,
        syncSuspended: false,
        objectCount: 0,
        conflicts: 0,
        project: '',
        port: 0,
        coreVersion: '',
        extensionVersion: '',
        uptimeSeconds: 0,
        mode: '',
        queued: 0,
        coalesced: 0,
        loops: 0,
        stream: [],
    };

    private timer: NodeJS.Timeout | undefined;

    constructor(private readonly rpc: RpcManager, extensionVersion: string) {
        this.snapshot.extensionVersion = extensionVersion;

        this.rpc.onConnectionChange((connected) => {
            this.snapshot.coreUp = connected;
            this.note(connected ? 'Connected to the core' : 'Lost the core connection');
        });

        this.rpc.onMessage((message) => this.absorb(message));
    }

    public get current(): StatusSnapshot {
        return this.snapshot;
    }

    /** Starts polling /health. Returns a disposable that stops it. */
    public start(intervalMs = 2500): vscode.Disposable {
        void this.poll();
        this.timer = setInterval(() => void this.poll(), intervalMs);
        return { dispose: () => this.stop() };
    }

    public stop() {
        if (this.timer) clearInterval(this.timer);
        this.timer = undefined;
    }

    public dispose() {
        this.stop();
        this._onChange.dispose();
    }

    /** Adds a line to the stream that did not come from the core. */
    public note(text: string) {
        this.push({ direction: 'note', label: text, detail: '', time: Date.now() });
    }

    private async poll() {
        const port = parseInt(new URL(env.getBaseUrl()).port || '8080', 10);
        let health: any;
        try {
            health = await env.probe(port, 1200);
        } catch {
            health = undefined;
        }
        if (!health) {
            this.snapshot.studioConnected = false;
            this.snapshot.coreUp = false;
            this.fire();
            return;
        }
        this.snapshot.coreUp = true;
        this.snapshot.studioConnected = health.studio_connected === true;
        this.snapshot.syncSuspended = health.sync_suspended === true;
        this.snapshot.conflicts = health.conflicts ?? 0;
        this.snapshot.project = health.project ?? '';
        this.snapshot.port = health.port ?? port;
        this.snapshot.coreVersion = health.version ?? '';
        this.snapshot.uptimeSeconds = health.uptime_seconds ?? 0;
        this.snapshot.mode = health.config?.mode ?? '';
        this.snapshot.queued = health.plugin_queued ?? 0;
        this.snapshot.coalesced = health.plugin_coalesced ?? 0;
        this.snapshot.loops = health.loops_detected ?? 0;
        if (typeof health.object_count === 'number') {
            this.snapshot.objectCount = health.object_count;
        }
        this.fire();
    }

    private absorb(message: any) {
        const data = message?.data ?? {};
        switch (message?.event_type) {
            case 'FULL_SYNC':
            case 'TREE_UPDATED':
                this.snapshot.objectCount = data.nodes?.length ?? this.snapshot.objectCount;
                break;
            case 'INSTANCE_CREATED':
                this.snapshot.objectCount++;
                break;
            case 'INSTANCE_REMOVED':
                this.snapshot.objectCount = Math.max(0, this.snapshot.objectCount - 1);
                break;
        }
        this.push({
            direction: 'in',
            label: String(message?.event_type ?? 'EVENT'),
            detail: describe(data),
            time: Date.now(),
        });
    }

    private push(entry: StreamEntry) {
        this.snapshot.stream = [entry, ...this.snapshot.stream].slice(0, STREAM_LIMIT);
        this.fire();
    }

    private fire() {
        this._onChange.fire(this.snapshot);
    }
}

/** Short, readable summary of an event payload for the stream. */
function describe(data: any): string {
    if (!data || typeof data !== 'object') return '';
    if (data.property) {
        return `${data.name ?? data.uuid ?? ''}.${data.property}`.replace(/^\./, '');
    }
    if (Array.isArray(data.nodes)) return `${data.nodes.length} objects`;
    if (data.name) return String(data.name);
    if (data.path) return String(data.path);
    return '';
}
