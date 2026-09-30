import * as path from 'path';
import * as vscode from 'vscode';
import { RpcManager } from './rpc/manager';
import { WorkspaceExplorer } from './explorer/WorkspaceExplorer';
import { PropertyInspector } from './inspector/PropertyInspector';
import { EditorStateManager } from './state/EditorStateManager';
import { StatusTracker } from './state/StatusTracker';
import { SyncixPanel } from './ui/SyncixPanel';
import { createStatusBar, watchSaves } from './ui/statusBar';
import { createProject } from './commands/createProject';
import { importIntoStudio } from './commands/importIntoStudio';
import { ensureCliInstalled, ensurePluginInstalled } from './core/install';
import {
    ensureCoreRunning,
    initProcessModule,
    ownVersion,
    resolveCorePaths,
    stopCore,
} from './core/process';
import { findProjectRoot, isSyncixWorkspace, syncFolderPath } from './core/project';

let rpcClient: RpcManager;

export async function activate(context: vscode.ExtensionContext) {
    initProcessModule(context);

    // Auto-start and connect only in a Syncix workspace. In other projects everything
    // stays passive; connect by hand with "Syncix: Start" if wanted.
    const autoMode = isSyncixWorkspace();
    // The sidebar's welcome view looks at this context: without a project the user sees
    // a "create project" button instead of an empty tree.
    await vscode.commands.executeCommand('setContext', 'syncix.hasProject', autoMode);
    if (autoMode) {
        ensurePluginInstalled(context); // install or update the Studio plugin automatically
        ensureCliInstalled(context);    // "syncix" in the editor's terminals (the system PATH does not change)
        await ensureCoreRunning();
    }

    rpcClient = new RpcManager(() => findProjectRoot());

    const tracker = new StatusTracker(rpcClient, ownVersion());
    context.subscriptions.push(tracker, tracker.start());

    const panel = new SyncixPanel(context.extensionUri, tracker);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(SyncixPanel.viewId, panel, {
            webviewOptions: { retainContextWhenHidden: true },
        })
    );

    if (autoMode) {
        rpcClient.connect();
        createStatusBar(context, tracker);
        watchSaves(context, tracker);
    }

    const stateManager = new EditorStateManager(rpcClient);
    const savedSelection = context.workspaceState.get<string[]>('syncix.selectedNodes', []);
    if (savedSelection.length > 0) {
        stateManager.setSelectedNodes(savedSelection);
    }
    stateManager.onSelectionChanged((ids) => {
        context.workspaceState.update('syncix.selectedNodes', ids);
    });

    const workspaceExplorer = new WorkspaceExplorer(context, stateManager, rpcClient as any);
    const inspector = () => PropertyInspector.createOrShow(rpcClient, stateManager, workspaceExplorer);

    context.subscriptions.push(
        vscode.commands.registerCommand('syncix.start', () => rpcClient.connect()),
        vscode.commands.registerCommand('syncix.stop', () => {
            rpcClient.dispose();
            vscode.window.showInformationMessage('Syncix disconnected.');
        }),
        vscode.commands.registerCommand('syncix.inspectNode', (node) => {
            inspector();
            if (node) PropertyInspector.currentPanel?.inspect(node);
        }),
        vscode.commands.registerCommand('syncix.openInspector', inspector),
        vscode.commands.registerCommand('syncix.openPanel', () => SyncixPanel.focus()),
        vscode.commands.registerCommand('syncix.showStatus', () => showStatus(tracker)),
        vscode.commands.registerCommand('syncix.reconnect', async () => {
            await ensureCoreRunning();
            rpcClient.connect();
            vscode.window.setStatusBarMessage('Syncix: reconnecting...', 3000);
        }),
        vscode.commands.registerCommand('syncix.stopCore', () => {
            if (stopCore()) {
                vscode.window.showInformationMessage('Syncix core stopped.');
            } else {
                vscode.window.showWarningMessage(
                    'No Syncix core started by this window was found. ' +
                    'If another window started it, stop it from there.'
                );
            }
        }),
        vscode.commands.registerCommand('syncix.restartCore', async () => {
            stopCore();
            setTimeout(() => ensureCoreRunning().then(() => rpcClient.connect()), 1200);
            vscode.window.showInformationMessage('Restarting the Syncix core...');
        }),
        vscode.commands.registerCommand('syncix.initProject', () =>
            createProject(context, () => rpcClient.connect())
        ),
        vscode.commands.registerCommand('syncix.installPlugin', () => {
            ensurePluginInstalled(context);
            vscode.window.showInformationMessage(
                'Checked the Syncix plugin. If it changed, restart Roblox Studio.'
            );
        }),
        vscode.commands.registerCommand('syncix.refresh', () => {
            workspaceExplorer.refresh();
            rpcClient.send('GET_TREE', {});
        }),
        vscode.commands.registerCommand('syncix.openWorkspaceFolder', () => {
            const folder = syncFolderPath();
            if (folder) {
                vscode.commands.executeCommand('revealFileInOS', vscode.Uri.file(folder));
            }
        }),
        vscode.commands.registerCommand('syncix.importIntoStudio', (uri?: vscode.Uri) =>
            importIntoStudio(uri)
        ),
        vscode.commands.registerCommand('syncix.installCliGlobal', () => offerCliOnPath(context)),
        vscode.commands.registerCommand('syncix.selfTest', () => runSelfTest()),
        { dispose: () => rpcClient.dispose() }
    );
}

export function deactivate() {
    if (rpcClient) {
        rpcClient.dispose();
    }
}

function showStatus(tracker: StatusTracker) {
    const s = tracker.current;
    if (!s.coreUp) {
        vscode.window.showWarningMessage('The Syncix core is not running. Starting it...');
        void ensureCoreRunning().then(() => rpcClient.connect());
        return;
    }
    if (!s.studioConnected) {
        vscode.window.showWarningMessage(
            'Roblox Studio is not connected — your changes are not reaching Studio. Open Studio.'
        );
        return;
    }
    SyncixPanel.focus();
}

/**
 * The system PATH is a persistent user setting; the extension does NOT change it.
 * The folder is handed over with instructions and the decision stays the user's.
 */
async function offerCliOnPath(context: vscode.ExtensionContext) {
    const shim = ensureCliInstalled(context);
    if (!shim) {
        vscode.window.showErrorMessage(
            'Could not set up the CLI: core binary not found. Try Syncix: Restart Core first.'
        );
        return;
    }
    const dir = path.dirname(shim);
    const choice = await vscode.window.showInformationMessage(
        `"syncix" already works in this editor's terminals. Syncix does not change your system PATH. To use the command in other terminals too, add this folder to your PATH yourself:

${dir}`,
        { modal: true },
        'Copy Folder Path'
    );
    if (choice === 'Copy Folder Path') {
        await vscode.env.clipboard.writeText(dir);
        vscode.window.showInformationMessage(
            'Folder path copied. On Windows: Start, search "Edit environment variables for your account", select Path, New, paste.'
        );
    }
}

function runSelfTest() {
    const paths = resolveCorePaths();
    if (!paths) {
        vscode.window.showErrorMessage('Core binary not found.');
        return;
    }
    const terminal = vscode.window.createTerminal('Syncix Selftest');
    terminal.show();
    terminal.sendText(`"${paths.exePath}" selftest`);
}
