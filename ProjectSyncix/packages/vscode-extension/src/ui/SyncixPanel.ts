import * as vscode from 'vscode';
import { StatusTracker } from '../state/StatusTracker';
import { panelHtml } from './panelHtml';

export const FEEDBACK_URL = 'https://github.com/Syqopat/Syncix/issues/new/choose';

/**
 * The Syncix view in the side bar: state, the live sync stream and the settings that
 * matter day to day, in the place the user already has open.
 *
 * It replaces a separate "diagnostics" editor tab that had to be opened by command and
 * an empty second view that nothing ever filled.
 */
export class SyncixPanel implements vscode.WebviewViewProvider {
    public static readonly viewId = 'syncixPanel';

    private view: vscode.WebviewView | undefined;

    constructor(
        private readonly extensionUri: vscode.Uri,
        private readonly tracker: StatusTracker
    ) {
        this.tracker.onChange((state) => {
            this.view?.webview.postMessage({ kind: 'state', state });
        });
    }

    public resolveWebviewView(view: vscode.WebviewView) {
        this.view = view;
        view.webview.options = { enableScripts: true, localResourceRoots: [this.extensionUri] };
        view.webview.html = panelHtml(nonce(), view.webview.cspSource);

        view.webview.onDidReceiveMessage((message) => this.handle(message));
        view.onDidDispose(() => {
            this.view = undefined;
        });
    }

    /** Brings the view into focus (what the "open panel" command does). */
    public static focus() {
        void vscode.commands.executeCommand(`${SyncixPanel.viewId}.focus`);
    }

    private handle(message: any) {
        switch (message?.kind) {
            case 'ready':
                this.postConfig();
                this.view?.webview.postMessage({ kind: 'state', state: this.tracker.current });
                break;
            case 'command':
                // Only the buttons the panel itself renders are routed, so a command name
                // that arrives from anywhere else cannot be run.
                if (ALLOWED_COMMANDS.includes(message.command)) {
                    void vscode.commands.executeCommand(message.command, message.arg);
                }
                break;
            case 'setAutoStart':
                void vscode.workspace
                    .getConfiguration('syncix')
                    .update('autoStartCore', message.value === true, vscode.ConfigurationTarget.Global)
                    .then(() => this.postConfig());
                break;
            case 'feedback':
                void vscode.env.openExternal(vscode.Uri.parse(FEEDBACK_URL));
                break;
        }
    }

    private postConfig() {
        this.view?.webview.postMessage({
            kind: 'config',
            autoStartCore: vscode.workspace.getConfiguration('syncix').get<boolean>('autoStartCore', true),
            feedbackUrl: FEEDBACK_URL,
        });
    }
}

const ALLOWED_COMMANDS = [
    'syncix.reconnect',
    'syncix.restartCore',
    'syncix.openWorkspaceFolder',
    'syncix.installPlugin',
    'syncix.selfTest',
    'syncix.installCliGlobal',
    'workbench.action.openSettings',
];

function nonce(): string {
    let text = '';
    const pool = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    for (let i = 0; i < 32; i++) text += pool.charAt(Math.floor(Math.random() * pool.length));
    return text;
}
