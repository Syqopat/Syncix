import * as vscode from 'vscode';
import * as http from 'http';
import { RpcManager } from '../rpc/manager';
import { EditorStateManager } from '../state/EditorStateManager';
import { WorkspaceExplorer } from '../explorer/WorkspaceExplorer';
import { getBaseUrl } from '../core/env';

// Adres artik sabit degil: core dolu portu atlayabiliyor, tek kaynaktan okunur.
const BASE = () => getBaseUrl();

/** Küçük HTTP yardımcıları (core ile konuşur). */
function httpGetJson(path: string): Promise<any> {
    return new Promise((resolve, reject) => {
        const req = http.get(BASE() + path, (res) => {
            let body = '';
            res.on('data', (c) => (body += c));
            res.on('end', () => {
                try { resolve(JSON.parse(body)); } catch (e) { reject(e); }
            });
        });
        req.on('error', reject);
        req.setTimeout(3000, () => { req.destroy(); reject(new Error('timeout')); });
    });
}

function httpPostJson(path: string, data: any): Promise<void> {
    return new Promise((resolve, reject) => {
        const payload = JSON.stringify(data);
        const req = http.request(BASE() + path, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(payload) }
        }, (res) => { res.resume(); res.on('end', () => resolve()); });
        req.on('error', reject);
        req.setTimeout(3000, () => { req.destroy(); reject(new Error('timeout')); });
        req.write(payload);
        req.end();
    });
}

/**
 * Görsel Property Inspector.
 * Seçili objenin tüm property + attribute'larını uygun widget'larla gösterir
 * ve düzenlemeyi SET_PROPERTY / SET_ATTRIBUTE ile Studio'ya iletir.
 */
export class PropertyInspector {
    public static currentPanel: PropertyInspector | undefined;
    private panel: vscode.WebviewPanel | undefined;
    private currentId: string | undefined;
    private reloadTimer: NodeJS.Timeout | undefined;

    public static createOrShow(rpc: RpcManager, stateManager: EditorStateManager, explorer: WorkspaceExplorer) {
        if (!PropertyInspector.currentPanel) {
            PropertyInspector.currentPanel = new PropertyInspector(stateManager, rpc, explorer);
        }
        PropertyInspector.currentPanel.showPanel();
    }

    constructor(
        private stateManager: EditorStateManager,
        private rpc: RpcManager,
        private explorer: WorkspaceExplorer
    ) {
        // Seçim değişince Inspector'ı güncelle
        this.stateManager.onSelectionChanged((ids) => {
            if (this.panel && this.panel.visible && ids.length > 0) {
                this.loadProperties(ids[0]);
            }
        });

        // Studio'da bir şey değişince (canlı) gösterilen objeyi tazele
        this.rpc.onMessage((msg) => {
            if (!this.panel || !this.panel.visible || !this.currentId) return;
            if (msg.event_type === 'INSTANCE_UPDATED' && msg.data?.id === this.currentId) {
                this.scheduleReload();
            }
        });
    }

    private scheduleReload() {
        if (this.reloadTimer) clearTimeout(this.reloadTimer);
        this.reloadTimer = setTimeout(() => {
            if (this.currentId) this.loadProperties(this.currentId);
        }, 300);
    }

    private showPanel() {
        if (this.panel) {
            this.panel.reveal(vscode.ViewColumn.Two);
        } else {
            this.panel = vscode.window.createWebviewPanel(
                'syncixInspector', 'Syncix Inspector', vscode.ViewColumn.Two,
                { enableScripts: true, retainContextWhenHidden: true }
            );
            this.panel.onDidDispose(() => {
                this.panel = undefined;
                PropertyInspector.currentPanel = undefined;
            });
            this.panel.webview.onDidReceiveMessage(async (message) => {
                try {
                    if (message.command === 'set') {
                        await httpPostJson('/command', {
                            event_type: 'SET_PROPERTY',
                            data: { id: this.currentId, property: message.property, value: message.value }
                        });
                    } else if (message.command === 'setAttr') {
                        await httpPostJson('/command', {
                            event_type: 'SET_ATTRIBUTE',
                            data: { id: this.currentId, name: message.name, value: message.value }
                        });
                    }
                } catch (err: any) {
                    vscode.window.showErrorMessage('Syncix: could not send update — ' + (err?.message ?? err));
                }
            });
        }
        const sel = this.stateManager.selectedNodeIds;
        if (sel.length > 0) { this.loadProperties(sel[0]); }
        else { this.panel.webview.html = this.wrap('<p class="dim">Bir obje seç (soldaki Syncix ağacından).</p>'); }
    }

    public inspect(node: any) {
        if (node?.id) { this.currentId = node.id; if (this.panel) this.loadProperties(node.id); }
    }

    private async loadProperties(id: string) {
        if (!this.panel) return;
        this.currentId = id;
        try {
            const o = await httpGetJson('/object?target=' + encodeURIComponent(id));
            if (o.error) { this.panel.webview.html = this.wrap(`<p class="err">${o.error}</p>`); return; }
            this.panel.webview.html = this.render(o);
        } catch (err: any) {
            this.panel.webview.html = this.wrap(`<p class="err">Yüklenemedi: ${err?.message ?? err}</p>`);
        }
    }

    /** PropertyValue serde biçiminden bir düzenleyici satırı üretir. */
    private renderField(name: string, pv: any, kind: 'prop' | 'attr'): string {
        const id = `${kind}_${name}`;
        // pv: {"Number":x} | {"Boolean":b} | {"String":s} | {"Vector3":{x,y,z}} | {"Color3":{r,g,b}} | {"UDim2":{xs,xo,ys,yo}}
        const key = pv && typeof pv === 'object' ? Object.keys(pv)[0] : typeof pv;
        const label = `<label title="${name}">${name}</label>`;
        const wrap = (inner: string) => `<div class="row" data-name="${name}" data-kind="${kind}" data-key="${key}">${label}<div class="ctrl">${inner}</div></div>`;

        if (key === 'Boolean') {
            return wrap(`<input type="checkbox" id="${id}" ${pv.Boolean ? 'checked' : ''} onchange="sendBool('${name}','${kind}',this.checked)">`);
        }
        if (key === 'Number') {
            return wrap(`<input type="number" step="any" id="${id}" value="${pv.Number}" onchange="sendNum('${name}','${kind}',this.value)">`);
        }
        if (key === 'String') {
            const s = String(pv.String).replace(/"/g, '&quot;');
            return wrap(`<input type="text" id="${id}" value="${s}" onchange="sendStr('${name}','${kind}',this.value)">`);
        }
        if (key === 'Vector3') {
            const v = pv.Vector3;
            return wrap(`<div class="vec">
                <input type="number" step="any" value="${v.x}" oninput="sendVec3('${name}','${kind}')" data-c="x">
                <input type="number" step="any" value="${v.y}" oninput="sendVec3('${name}','${kind}')" data-c="y">
                <input type="number" step="any" value="${v.z}" oninput="sendVec3('${name}','${kind}')" data-c="z">
            </div>`);
        }
        if (key === 'Color3') {
            const c = pv.Color3;
            const hex = rgbToHex(c.r, c.g, c.b);
            return wrap(`<input type="color" id="${id}" value="${hex}" onchange="sendColor('${name}','${kind}',this.value)">`);
        }
        if (key === 'UDim2') {
            const u = pv.UDim2;
            return wrap(`<div class="vec udim">
                <input type="number" step="any" value="${u.xs}" oninput="sendUDim2('${name}','${kind}')" data-c="xs" title="X Scale">
                <input type="number" step="any" value="${u.xo}" oninput="sendUDim2('${name}','${kind}')" data-c="xo" title="X Offset">
                <input type="number" step="any" value="${u.ys}" oninput="sendUDim2('${name}','${kind}')" data-c="ys" title="Y Scale">
                <input type="number" step="any" value="${u.yo}" oninput="sendUDim2('${name}','${kind}')" data-c="yo" title="Y Offset">
            </div>`);
        }
        return wrap(`<span class="dim">desteklenmeyen: ${key}</span>`);
    }

    private render(o: any): string {
        const props = o.properties || {};
        const attrs = o.attributes || {};
        const propRows = Object.keys(props).sort().map(k => this.renderField(k, props[k], 'prop')).join('');
        const attrRows = Object.keys(attrs).sort().map(k => this.renderField(k, attrs[k], 'attr')).join('');
        const breadcrumb = this.explorer.cache.getNodePath(o.syncix_id) || o.name;

        const body = `
            <div class="crumb">${breadcrumb}</div>
            <h2>${o.name} <span class="cls">${o.class_name}</span></h2>
            <div class="sec">Özellikler</div>
            ${propRows || '<p class="dim">(property yok)</p>'}
            <div class="sec">Attribute'lar</div>
            ${attrRows || '<p class="dim">(attribute yok)</p>'}
        `;
        return this.wrap(body);
    }

    private wrap(body: string): string {
        return `<!DOCTYPE html><html><head><style>
            body { font-family: var(--vscode-font-family); padding: 12px; color: var(--vscode-editor-foreground); }
            h2 { margin: 4px 0 12px; font-size: 15px; }
            .cls { font-size: 11px; color: var(--vscode-descriptionForeground); font-weight: normal; }
            .crumb { font-size: 11px; color: var(--vscode-descriptionForeground); margin-bottom: 6px; }
            .sec { margin: 14px 0 6px; font-size: 11px; text-transform: uppercase; letter-spacing: .5px; color: var(--vscode-descriptionForeground); border-bottom: 1px solid var(--vscode-panel-border); padding-bottom: 3px; }
            .row { display: flex; align-items: center; margin: 5px 0; gap: 8px; }
            .row label { flex: 0 0 42%; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
            .ctrl { flex: 1; }
            .ctrl input[type=text], .ctrl input[type=number] { width: 100%; box-sizing: border-box; background: var(--vscode-input-background); color: var(--vscode-input-foreground); border: 1px solid var(--vscode-input-border); padding: 3px 5px; border-radius: 3px; }
            .ctrl input[type=color] { width: 40px; height: 24px; background: none; border: 1px solid var(--vscode-input-border); }
            .vec { display: flex; gap: 4px; } .vec input { width: 25%; } .udim input { width: 25%; }
            .dim { color: var(--vscode-descriptionForeground); font-size: 12px; }
            .err { color: var(--vscode-errorForeground); }
        </style></head><body>
            ${body}
            <script>
                const vscode = acquireVsCodeApi();
                function rowVal(name, kind, sel){ return document.querySelector('.row[data-name="'+name+'"][data-kind="'+kind+'"] '+sel); }
                function sendBool(n,k,b){ vscode.postMessage({command: k==='attr'?'setAttr':'set', property:n, name:n, value:b}); }
                function sendNum(n,k,v){ vscode.postMessage({command: k==='attr'?'setAttr':'set', property:n, name:n, value:parseFloat(v)}); }
                function sendStr(n,k,v){ vscode.postMessage({command: k==='attr'?'setAttr':'set', property:n, name:n, value:v}); }
                function sendVec3(n,k){
                    const r = document.querySelector('.row[data-name="'+n+'"][data-kind="'+k+'"]');
                    const x=parseFloat(r.querySelector('[data-c=x]').value), y=parseFloat(r.querySelector('[data-c=y]').value), z=parseFloat(r.querySelector('[data-c=z]').value);
                    vscode.postMessage({command:k==='attr'?'setAttr':'set', property:n, name:n, value:{Vector3:{x,y,z}}});
                }
                function sendUDim2(n,k){
                    const r = document.querySelector('.row[data-name="'+n+'"][data-kind="'+k+'"]');
                    const xs=parseFloat(r.querySelector('[data-c=xs]').value), xo=parseFloat(r.querySelector('[data-c=xo]').value), ys=parseFloat(r.querySelector('[data-c=ys]').value), yo=parseFloat(r.querySelector('[data-c=yo]').value);
                    vscode.postMessage({command:k==='attr'?'setAttr':'set', property:n, name:n, value:{UDim2:{xs,xo,ys,yo}}});
                }
                function sendColor(n,k,hex){
                    const r=parseInt(hex.substr(1,2),16)/255, g=parseInt(hex.substr(3,2),16)/255, b=parseInt(hex.substr(5,2),16)/255;
                    vscode.postMessage({command:k==='attr'?'setAttr':'set', property:n, name:n, value:{Color3:{r,g,b}}});
                }
            </script>
        </body></html>`;
    }
}

function rgbToHex(r: number, g: number, b: number): string {
    const h = (n: number) => Math.max(0, Math.min(255, Math.round(n * 255))).toString(16).padStart(2, '0');
    return '#' + h(r) + h(g) + h(b);
}
