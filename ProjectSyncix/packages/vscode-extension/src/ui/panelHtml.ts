/**
 * Markup of the Syncix panel.
 *
 * It renders on the editor's own theme variables, so it follows every colour theme,
 * with one Syncix accent for the elements that belong to Syncix. The webview holds no
 * address and makes no request of its own: the extension posts the state in and the
 * webview posts the user's intent back out.
 */
export function panelHtml(nonce: string, cspSource: string): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy"
      content="default-src 'none'; style-src ${cspSource} 'nonce-${nonce}'; script-src 'nonce-${nonce}';">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<style nonce="${nonce}">
:root {
    --accent: #4c8df5;
    --ok: #3fb950;
    --warn: #d29922;
    --bad: #f85149;
    --gap: 10px;
}
body {
    font-family: var(--vscode-font-family);
    font-size: var(--vscode-font-size);
    color: var(--vscode-foreground);
    background: transparent;
    margin: 0;
    padding: 0 0 12px 0;
}
.tabs {
    display: flex;
    gap: 2px;
    position: sticky;
    top: 0;
    background: var(--vscode-sideBar-background);
    border-bottom: 1px solid var(--vscode-panel-border);
    z-index: 1;
}
.tab {
    flex: 1;
    padding: 7px 4px;
    text-align: center;
    cursor: pointer;
    border: none;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--vscode-descriptionForeground);
    font: inherit;
}
.tab:hover { color: var(--vscode-foreground); }
.tab.on {
    color: var(--vscode-foreground);
    border-bottom-color: var(--accent);
}
.page { display: none; padding: var(--gap); }
.page.on { display: block; }
.card {
    border: 1px solid var(--vscode-panel-border);
    border-radius: 4px;
    padding: 10px;
    margin-bottom: var(--gap);
    background: var(--vscode-editor-background);
}
.state { display: flex; align-items: center; gap: 8px; font-weight: 600; }
.dot { width: 9px; height: 9px; border-radius: 50%; background: var(--vscode-descriptionForeground); flex: none; }
.dot.ok { background: var(--ok); }
.dot.warn { background: var(--warn); }
.dot.bad { background: var(--bad); }
.sub { color: var(--vscode-descriptionForeground); margin-top: 4px; }
.row { display: flex; justify-content: space-between; gap: 8px; padding: 3px 0; }
.row .k { color: var(--vscode-descriptionForeground); }
.row .v { font-family: var(--vscode-editor-font-family); }
.actions { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 10px; }
button.act {
    font: inherit;
    padding: 4px 10px;
    border: 1px solid var(--vscode-button-secondaryBackground, var(--vscode-panel-border));
    border-radius: 3px;
    background: var(--vscode-button-secondaryBackground, transparent);
    color: var(--vscode-button-secondaryForeground, var(--vscode-foreground));
    cursor: pointer;
}
button.act:hover { background: var(--vscode-button-secondaryHoverBackground, var(--vscode-list-hoverBackground)); }
button.act.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #ffffff;
}
button.act.primary:hover { filter: brightness(1.1); }
h4 { margin: 0 0 6px 0; font-size: 1em; }
#stream { display: flex; flex-direction: column; gap: 1px; }
.line { display: flex; gap: 6px; padding: 2px 0; font-family: var(--vscode-editor-font-family); font-size: 0.92em; }
.line .arrow { width: 12px; flex: none; color: var(--accent); }
.line.out .arrow { color: var(--warn); }
.line.note .arrow { color: var(--vscode-descriptionForeground); }
.line .what { flex: none; }
.line .who { color: var(--vscode-descriptionForeground); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.line .at { margin-left: auto; color: var(--vscode-descriptionForeground); flex: none; }
.empty { color: var(--vscode-descriptionForeground); }
label.check { display: flex; align-items: center; gap: 8px; padding: 3px 0; }
code.link {
    display: block;
    margin-top: 6px;
    padding: 5px 6px;
    background: var(--vscode-textCodeBlock-background);
    border-radius: 3px;
    word-break: break-all;
    font-size: 0.9em;
}
</style>
</head>
<body>
<div class="tabs">
    <button class="tab on" data-page="status">Status</button>
    <button class="tab" data-page="live">Live</button>
    <button class="tab" data-page="settings">Settings</button>
</div>

<div class="page on" id="page-status">
    <div class="card">
        <div class="state"><span class="dot" id="dot"></span><span id="word">Checking...</span></div>
        <div class="sub" id="detail">&nbsp;</div>
        <div class="actions">
            <button class="act primary" data-cmd="syncix.reconnect">Reconnect</button>
            <button class="act" data-cmd="syncix.restartCore">Restart core</button>
            <button class="act" data-cmd="syncix.openWorkspaceFolder">Sync folder</button>
        </div>
    </div>
    <div class="card">
        <h4>Project</h4>
        <div class="row"><span class="k">Name</span><span class="v" id="project">-</span></div>
        <div class="row"><span class="k">Port</span><span class="v" id="port">-</span></div>
        <div class="row"><span class="k">Mode</span><span class="v" id="mode">-</span></div>
        <div class="row"><span class="k">Objects</span><span class="v" id="objects">-</span></div>
        <div class="row"><span class="k">Conflicts</span><span class="v" id="conflicts">-</span></div>
        <div class="row"><span class="k">Uptime</span><span class="v" id="uptime">-</span></div>
    </div>
    <div class="card">
        <h4>Versions</h4>
        <div class="row"><span class="k">Extension</span><span class="v" id="extver">-</span></div>
        <div class="row"><span class="k">Core</span><span class="v" id="corever">-</span></div>
        <div class="actions">
            <button class="act" data-cmd="syncix.installPlugin">Update Studio plugin</button>
            <button class="act" data-cmd="syncix.selfTest">Run self test</button>
        </div>
    </div>
</div>

<div class="page" id="page-live">
    <div class="card">
        <h4>Sync stream</h4>
        <div class="sub" id="throughput">&nbsp;</div>
        <div id="stream"><div class="empty">Nothing yet. Change something in Studio or save a file.</div></div>
    </div>
</div>

<div class="page" id="page-settings">
    <div class="card">
        <h4>Behaviour</h4>
        <label class="check"><input type="checkbox" id="autostart"> Start the core with the project</label>
        <div class="actions">
            <button class="act" data-cmd="workbench.action.openSettings" data-arg="syncix">All settings</button>
            <button class="act" data-cmd="syncix.installCliGlobal">Use the CLI elsewhere</button>
        </div>
    </div>
    <div class="card">
        <h4>Feedback</h4>
        <div class="sub">Something broken or missing? The issue form asks for the few details that make it reproducible.</div>
        <div class="actions"><button class="act primary" id="feedback">Send feedback</button></div>
        <code class="link" id="feedbackUrl"></code>
    </div>
</div>

<script nonce="${nonce}">
const vscodeApi = acquireVsCodeApi();

for (const tab of document.querySelectorAll('.tab')) {
    tab.addEventListener('click', () => {
        for (const other of document.querySelectorAll('.tab')) other.classList.toggle('on', other === tab);
        for (const page of document.querySelectorAll('.page')) {
            page.classList.toggle('on', page.id === 'page-' + tab.dataset.page);
        }
    });
}

for (const button of document.querySelectorAll('button.act[data-cmd]')) {
    button.addEventListener('click', () => {
        vscodeApi.postMessage({ kind: 'command', command: button.dataset.cmd, arg: button.dataset.arg });
    });
}

document.getElementById('feedback').addEventListener('click', () => {
    vscodeApi.postMessage({ kind: 'feedback' });
});

document.getElementById('autostart').addEventListener('change', (e) => {
    vscodeApi.postMessage({ kind: 'setAutoStart', value: e.target.checked });
});

const text = (id, value) => { document.getElementById(id).textContent = value; };

function duration(seconds) {
    if (!seconds) return '0s';
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = seconds % 60;
    return (h ? h + 'h ' : '') + (h || m ? m + 'm ' : '') + s + 's';
}

function clock(ms) {
    const d = new Date(ms);
    return String(d.getHours()).padStart(2, '0') + ':' + String(d.getMinutes()).padStart(2, '0') + ':'
        + String(d.getSeconds()).padStart(2, '0');
}

function render(s) {
    const dot = document.getElementById('dot');
    let word = 'In sync';
    let cls = 'dot ok';
    let detail = 'Studio and your editor agree.';
    if (!s.coreUp) {
        word = 'Core is down';
        cls = 'dot bad';
        detail = 'Nothing is syncing. Reconnect to start the core again.';
    } else if (s.syncSuspended) {
        word = 'Sync suspended';
        cls = 'dot bad';
        detail = 'This folder is bound to another place. Sync stays off until that is resolved.';
    } else if (!s.studioConnected) {
        word = 'Studio not connected';
        cls = 'dot warn';
        detail = 'The core runs, but nothing reaches Studio. Open Studio; the plugin connects itself.';
    } else if (s.conflicts > 0) {
        word = s.conflicts + ' conflict(s)';
        cls = 'dot warn';
        detail = 'Some changes were overwritten. The Live tab shows which.';
    }
    dot.className = cls;
    text('word', word);
    text('detail', detail);

    text('project', s.project || '-');
    text('port', s.port ? String(s.port) : '-');
    text('mode', s.mode || '-');
    text('objects', String(s.objectCount));
    text('conflicts', String(s.conflicts));
    text('uptime', duration(s.uptimeSeconds));
    text('extver', s.extensionVersion || '-');
    text('corever', s.coreVersion || '-');
    text('throughput', 'Queued ' + s.queued + '  -  merged ' + s.coalesced + '  -  loops ' + s.loops);

    const stream = document.getElementById('stream');
    if (!s.stream.length) {
        stream.innerHTML = '<div class="empty">Nothing yet. Change something in Studio or save a file.</div>';
        return;
    }
    stream.textContent = '';
    for (const entry of s.stream) {
        const line = document.createElement('div');
        line.className = 'line ' + entry.direction;
        const arrow = document.createElement('span');
        arrow.className = 'arrow';
        arrow.textContent = entry.direction === 'in' ? '<-' : entry.direction === 'out' ? '->' : '*';
        const what = document.createElement('span');
        what.className = 'what';
        what.textContent = entry.label;
        const who = document.createElement('span');
        who.className = 'who';
        who.textContent = entry.detail;
        const at = document.createElement('span');
        at.className = 'at';
        at.textContent = clock(entry.time);
        line.append(arrow, what, who, at);
        stream.append(line);
    }
}

window.addEventListener('message', (event) => {
    const message = event.data;
    if (message.kind === 'state') render(message.state);
    if (message.kind === 'config') {
        document.getElementById('autostart').checked = message.autoStartCore;
        text('feedbackUrl', message.feedbackUrl);
    }
});

vscodeApi.postMessage({ kind: 'ready' });
</script>
</body>
</html>`;
}
