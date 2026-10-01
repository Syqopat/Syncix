import { describe, expect, it, beforeEach } from 'vitest';
import { StatusTracker } from '../src/state/StatusTracker';
import { compareVersions } from '../src/core/install';
import { panelHtml } from '../src/ui/panelHtml';
import { reset } from './vscode.mock';

/** Just enough of RpcManager: the tracker only listens. */
function fakeRpc() {
    const message: ((value: any) => void)[] = [];
    const connection: ((value: boolean) => void)[] = [];
    return {
        onMessage: (listener: (value: any) => void) => {
            message.push(listener);
            return { dispose() {} };
        },
        onConnectionChange: (listener: (value: boolean) => void) => {
            connection.push(listener);
            return { dispose() {} };
        },
        send: () => Promise.resolve(),
        emit: (value: any) => message.forEach((l) => l(value)),
        setConnected: (value: boolean) => connection.forEach((l) => l(value)),
    };
}

beforeEach(reset);

describe('StatusTracker', () => {
    it('counts the objects a full sync reports', () => {
        const rpc = fakeRpc();
        const tracker = new StatusTracker(rpc as any, '0.1.7');

        rpc.emit({ event_type: 'FULL_SYNC', data: { nodes: [{}, {}, {}] } });
        expect(tracker.current.objectCount).toBe(3);

        rpc.emit({ event_type: 'INSTANCE_CREATED', data: { name: 'Box' } });
        expect(tracker.current.objectCount).toBe(4);

        rpc.emit({ event_type: 'INSTANCE_REMOVED', data: {} });
        expect(tracker.current.objectCount).toBe(3);
    });

    it('never counts below zero', () => {
        const rpc = fakeRpc();
        const tracker = new StatusTracker(rpc as any, '0.1.7');
        rpc.emit({ event_type: 'INSTANCE_REMOVED', data: {} });
        expect(tracker.current.objectCount).toBe(0);
    });

    it('keeps the newest events first and bounds the stream', () => {
        const rpc = fakeRpc();
        const tracker = new StatusTracker(rpc as any, '0.1.7');
        for (let i = 0; i < 200; i++) {
            rpc.emit({ event_type: 'PROPERTY_UPDATE', data: { name: `Part${i}`, property: 'Size' } });
        }
        const stream = tracker.current.stream;
        expect(stream.length).toBeLessThanOrEqual(60);
        expect(stream[0].detail).toBe('Part199.Size');
        expect(stream[0].direction).toBe('in');
    });

    it('records losing the core as a line of its own', () => {
        const rpc = fakeRpc();
        const tracker = new StatusTracker(rpc as any, '0.1.7');
        rpc.setConnected(true);
        rpc.setConnected(false);
        expect(tracker.current.coreUp).toBe(false);
        expect(tracker.current.stream[0].label).toContain('Lost the core');
    });
});

describe('compareVersions', () => {
    it('compares numerically, not as text', () => {
        expect(compareVersions('0.1.10', '0.1.9')).toBeGreaterThan(0);
        expect(compareVersions('0.1.7', '0.1.7')).toBe(0);
        expect(compareVersions('0.2.0', '0.10.0')).toBeLessThan(0);
    });

    it('treats a missing part as zero', () => {
        expect(compareVersions('1', '1.0.0')).toBe(0);
    });
});

describe('the panel page', () => {
    it('locks scripts to its own nonce and allows nothing else', () => {
        const html = panelHtml('abc123', 'vscode-resource://test');
        expect(html).toContain("script-src 'nonce-abc123'");
        expect(html).toContain("default-src 'none'");
        expect(html).not.toContain('unsafe-inline');
        // No address is baked into the page: the extension pushes the state in.
        expect(html).not.toContain('127.0.0.1');
        expect(html).not.toContain('fetch(');
    });

    it('offers the three views and the feedback button', () => {
        const html = panelHtml('n', 'c');
        for (const label of ['Status', 'Live', 'Settings', 'Send feedback']) {
            expect(html).toContain(label);
        }
    });
});
