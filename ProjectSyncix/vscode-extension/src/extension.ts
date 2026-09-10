import * as vscode from 'vscode';
import * as http from 'http';
import * as fs from 'fs';
import * as path from 'path';
import { spawn } from 'child_process';
import { RpcManager } from './rpc/manager';
import { WorkspaceExplorer } from './explorer/WorkspaceExplorer';
import { PropertyInspector } from './inspector/PropertyInspector';
import { EditorStateManager } from './state/EditorStateManager';

import { DiagnosticsPanel } from './diagnostics/DiagnosticsPanel';
import * as env from './core/env';

let rpcClient: RpcManager;

/** Core process started by the extension (for stopping and restarting). */
let coreProcessPid: number | undefined;

/**
 * Is the core engine up? Also updates the address: the port is no longer fixed,
 * the core may skip a taken port and move to the next one.
 */
async function checkCoreHealth(): Promise<boolean> {
    const health = await env.refreshBaseUrl(findProjectRoot());
    if (!health) return false;
    warnVersionMismatch(health);
    return true;
}

/** Reports a version mismatch once (so it does not misbehave silently). */
let versionWarningShown = false;
/**
 * The extension's own version; filled from context.extension in activate.
 *
 * It used to be looked up by a hard-coded extension id. That id was never
 * real (no publisher was ever called "Syncix"), so the lookup always
 * returned undefined and the version-mismatch warning never fired.
 * Reading it from the extension itself instead of hard-coding the id keeps it
 * correct even if the publisher or the name changes.
 */
let extensionVersion = '';
function warnVersionMismatch(health: any) {
    if (versionWarningShown || !health?.version) return;
    const ownVersion = extensionVersion;
    if (!ownVersion) return;
    const mm = (v: string) => v.split('.').slice(0, 2).join('.');
    if (mm(ownVersion) !== mm(health.version)) {
        versionWarningShown = true;
        vscode.window.showWarningMessage(
            `Syncix version mismatch — extension ${ownVersion}, core ${health.version}. ` +
            `The same major.minor is required; rebuild the core or update the extension.`
        );
    }
}

/**
 * Finds the project root (portability): searches upwards from the open folder for core-engine.
 * A configured value takes precedence; if there is none, undefined.
 */
function findProjectRoot(): string | undefined {
    const cfgRoot = vscode.workspace.getConfiguration('syncix').get<string>('coreCwd', '');
    if (cfgRoot && fs.existsSync(cfgRoot)) return path.dirname(cfgRoot);

    const folders = vscode.workspace.workspaceFolders;
    if (!folders) return undefined;
    for (const f of folders) {
        let dir = f.uri.fsPath;
        for (let i = 0; i < 5; i++) {
            if (fs.existsSync(path.join(dir, 'core-engine'))) return dir;
            const up = path.dirname(dir);
            if (up === dir) break;
            dir = up;
        }
    }
    return undefined;
}

/**
 * Resolves the path of the core executable. Order:
 *   1) The user's setting (if any)
 *   2) The development build at the project root (when working inside the repository)
 *   3) The binary bundled with the extension (the end-user case; no repository needed)
 * The working directory is always the PARENT of the sync folder; the core expects
 * "../<sync_dir>", so the bundled binary finds the right folder too.
 */
function resolveCorePaths(): { exePath: string; cwd: string } | undefined {
    const cfg = vscode.workspace.getConfiguration('syncix');
    const cfgExe = cfg.get<string>('coreExePath', '');
    const cfgCwd = cfg.get<string>('coreCwd', '');
    if (cfgExe && fs.existsSync(cfgExe) && cfgCwd && fs.existsSync(cfgCwd)) {
        return { exePath: cfgExe, cwd: cfgCwd };
    }

    const root = findProjectRoot();
    if (root) {
        const devCwd = path.join(root, 'core-engine');
        const devExe = path.join(devCwd, 'target', 'release', env.coreBinaryName());
        if (fs.existsSync(devExe)) return { exePath: devExe, cwd: devCwd };
    }

    // Bundled binary: the one matching the platform is chosen (separate Windows/macOS folders).
    const bundled = env.bundledCorePaths(context_extensionPath).find((p) => fs.existsSync(p));
    if (bundled) {
        const projRoot = root ?? findSyncWorkspaceRoot();
        if (projRoot) {
            const cwd = path.join(projRoot, '.syncix');
            try {
                fs.mkdirSync(cwd, { recursive: true });
                // Outside Windows, a file extracted from the package may not be executable.
                if (!env.isWindows()) fs.chmodSync(bundled, 0o755);
            } catch {
                /* ignore */
            }
            return { exePath: bundled, cwd };
        }
    }
    return undefined;
}

/** Extension path (filled in activate; resolveCorePaths uses it for the bundled binary). */
let context_extensionPath = '';

/** The platform warning is shown once per session, not on every attempt. */
let platformWarningShown = false;

/** Finds the parent directory of the sync folder (based on syncix.toml). */
function findSyncWorkspaceRoot(): string | undefined {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders) return undefined;
    for (const f of folders) {
        const p = f.uri.fsPath;
        if (fs.existsSync(path.join(p, 'syncix.toml'))) return p;
        // If the sync folder itself is open, the project root is one level up
        if (fs.existsSync(path.join(path.dirname(p), 'syncix.toml'))) return path.dirname(p);
    }
    return undefined;
}

/** Starts the core engine automatically when it is not running (for convenience). */
async function ensureCoreRunning(): Promise<void> {
    const cfg = vscode.workspace.getConfiguration('syncix');
    if (!cfg.get<boolean>('autoStartCore', true)) return;

    const paths = resolveCorePaths();
    if (!paths) {
        // Only the Windows binary is packaged in this release.
        //
        // Exiting silently was the worst: on macOS the extension installed, the tree
        // never opened, and the reason was written nowhere. Saying what is missing
        // does not make it work, but it can be diagnosed.
        if (!platformWarningShown) {
            platformWarningShown = true;
            const messageText = env.isWindows()
                ? 'Syncix could not find the core binary. Reinstall the extension, or set syncix.coreExePath.'
                : `Syncix ships a Windows core binary only, so it cannot start on ${process.platform}. ` +
                  'Build the core from source and point syncix.coreExePath at it.';
            vscode.window.showWarningMessage(messageText);
        }
        return;
    }
    const { exePath, cwd } = paths;

    if (await checkCoreHealth()) return; // already running

    try {
        const child = spawn(exePath, [], { cwd, detached: true, stdio: 'ignore' });
        child.on('error', (err) => {
            vscode.window.showErrorMessage(`Could not start the Syncix core: ${err.message}`);
        });
        // The PID is kept: stopping used to kill by name (taskkill /IM),
        // which also killed the cores of OTHER OPEN PROJECTS.
        coreProcessPid = child.pid;
        child.unref();

        // Wait for the core TO COME UP.
        //
        // Continuing without waiting, rpcClient.connect() would still go to the old address.
        // If another project's core holds 8080, that address points at it,
        // so the connection would go to the wrong game. refreshBaseUrl now
        // verifies the project, so the right address only exists once our own core
        // has started.
        const startIndex = Date.now();
        while (Date.now() - startIndex < 10_000) {
            await new Promise((r) => setTimeout(r, 300));
            if (await checkCoreHealth()) break;
        }

        vscode.window.setStatusBarMessage('Syncix: core engine started automatically.', 5000);
    } catch (err: any) {
        vscode.window.showErrorMessage(`Could not start the Syncix core: ${err?.message ?? err}`);
    }
}

/**
 * Stops the core started by this window.
 *
 * This used to run `taskkill /IM syncix-core.exe /F`: killing by name
 * shut down the cores of ALL OPEN projects and only worked on
 * Windows. Now only the process we started ourselves is stopped, by PID.
 */
function stopCore(): boolean {
    if (!coreProcessPid) return false;
    try {
        process.kill(coreProcessPid);
        coreProcessPid = undefined;
        return true;
    } catch {
        // The process may already be gone.
        coreProcessPid = undefined;
        return false;
    }
}

/** Is the open folder a Syncix workspace? (no auto-start in unrelated projects) */
/**
 * Is this a Syncix project?
 *
 * The criterion is the presence of syncix.toml. It used to look at the folder NAME
 * ("src_workspace", "projectsyncix") — those were this repository's own old folder
 * names. The result: everything worked on our machine, while for anyone who named
 * their project "MyGame" the extension silently did nothing.
 * A good example of why testing an install only on your own machine is not enough.
 */
async function isSyncixWorkspace(): Promise<boolean> {
    return projectFile() !== undefined;
}

/** Path of syncix.toml in the workspace, or undefined. */
function projectFile(): string | undefined {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) return undefined;
    for (const folder of folders) {
        const candidate = path.join(folder.uri.fsPath, 'syncix.toml');
        if (fs.existsSync(candidate)) return candidate;
    }
    return undefined;
}

/**
 * NAME of the project's sync folder (`sync_dir` in syncix.toml).
 *
 * This name used to be hard-coded ("src_workspace") — this repository's own folder
 * name. The result: for anyone who named their folder differently, the save feedback and
 * the "open sync folder" command silently pointed at the wrong path.
 */
function syncFolderName(): string {
    const toml = projectFile();
    if (toml) {
        try {
            const m = /^\s*sync_dir\s*=\s*"([^"]+)"/m.exec(fs.readFileSync(toml, 'utf8'));
            if (m) return m[1];
        } catch {
            // okunamiyorsa varsayilana dus
        }
    }
    return 'src';
}

/**
 * Starts a new Syncix project in the open folder.
 *
 * Without this command a new user was stuck in a closed loop: the extension
 * only runs when syncix.toml exists, syncix.toml could only be created by the
 * CLI, and the CLI was installed when the extension ran.
 */
async function createProject(context: vscode.ExtensionContext): Promise<void> {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) {
        vscode.window.showErrorMessage('Open a folder first, then create the Syncix project inside it.');
        return;
    }

    const rootPath = folders[0].uri.fsPath;
    const tomlPath = path.join(rootPath, 'syncix.toml');
    if (fs.existsSync(tomlPath)) {
        vscode.window.showInformationMessage('This folder already has a syncix.toml.');
        return;
    }

    const syncDir = await vscode.window.showInputBox({
        prompt: 'Folder that will mirror the Roblox tree',
        value: 'src',
        validateInput: (v) => (v && v.trim().length > 0 ? undefined : 'A name is required'),
    });
    if (!syncDir) return;   // the user cancelled

    const sampleFile = path.join(context.extensionPath, 'resources', 'syncix.example.toml');
    let fileText: string;
    if (fs.existsSync(sampleFile)) {
        // The sample file explains every setting; it is given as is so a new user
        // can see what can be changed.
        fileText = fs.readFileSync(sampleFile, 'utf8').replace(
            /^sync_dir = ".*"$/m,
            `sync_dir = "${syncDir.trim()}"`
        );
    } else {
        fileText = `[files]
sync_dir = "${syncDir.trim()}"
`;
    }

    try {
        fs.writeFileSync(tomlPath, fileText);
        fs.mkdirSync(path.join(rootPath, syncDir.trim()), { recursive: true });
    } catch (err: any) {
        vscode.window.showErrorMessage(`Could not create the project: ${err.message}`);
        return;
    }

    // The project exists now; run the setup steps right away so the user
    // does not have to reload the window.
    ensurePluginInstalled(context);
    ensureCliInstalled(context);
    await ensureCoreRunning();
    rpcClient?.connect();
    await vscode.commands.executeCommand('setContext', 'syncix.hasProject', true);

    const doc = await vscode.workspace.openTextDocument(tomlPath);
    await vscode.window.showTextDocument(doc);
    vscode.window.showInformationMessage(
        'Syncix project created. Open Roblox Studio — the plugin is installed and the core is running.'
    );
}

/**
 * Installs the Studio plugin (the .rbxm bundled with the extension) into the Roblox
 * Plugins folder automatically. It copies only when the content differs, and then says
 * "restart Studio". So the user never installs the plugin by hand and versions never mix.
 */
function ensurePluginInstalled(context: vscode.ExtensionContext) {
    try {
        const src = path.join(context.extensionPath, 'resources', 'SyncixPlugin.rbxm');
        if (!fs.existsSync(src)) return;

        // The plugin folder depends on the platform (Windows: LOCALAPPDATA, macOS: Documents).
        // There is no official Studio on Linux, so undefined is returned and installation is skipped.
        const pluginsDir = env.robloxPluginsDir();
        if (!pluginsDir) return;
        if (!fs.existsSync(pluginsDir)) {
            fs.mkdirSync(pluginsDir, { recursive: true });
        }
        const dest = path.join(pluginsDir, 'SyncixPlugin.rbxm');

        const srcBuf = fs.readFileSync(src);
        let needsCopy = true;
        if (fs.existsSync(dest)) {
            needsCopy = !srcBuf.equals(fs.readFileSync(dest));
        }
        if (needsCopy) {
            fs.writeFileSync(dest, srcBuf);
            vscode.window.showInformationMessage(
                'The Syncix Studio plugin was updated. Restart Roblox Studio for it to take effect.'
            );
        }
    } catch (err: any) {
        console.error('Syncix plugin install error:', err);
    }
}

/**
 * Adds the `syncix` command to the editor's own terminals.
 *
 * This function used to write a .cmd into the user's home folder and change the
 * user's PATH in the registry with a hidden, detached shell process
 * — without asking, on every project open. That was a persistent change to
 * the system without consent.
 *
 * Now the shortcut is written to the extension's own storage folder and added to PATH
 * only through the VS Code API, for terminals the editor opens.
 * The registry, the home folder and the system PATH are not
 * touched; nothing is left behind when the extension is uninstalled.
 */
function ensureCliInstalled(context: vscode.ExtensionContext): string | undefined {
    try {
        // The CLI is not a separate script but the core binary itself: `syncix-core <command>`
        // acts as a client, so value conversion cannot drift between the CLI and the core.
        const paths = resolveCorePaths();
        if (!paths) return undefined;
        const exe = paths.exePath;

        const binDir = path.join(context.globalStorageUri.fsPath, 'bin');
        fs.mkdirSync(binDir, { recursive: true });

        let shim: string;
        if (env.isWindows()) {
            shim = path.join(binDir, 'syncix.cmd');
            const desired = `@echo off

"${exe}" %*

`;
            const current = fs.existsSync(shim) ? fs.readFileSync(shim, 'utf8') : '';
            if (current !== desired) fs.writeFileSync(shim, desired, 'ascii');
        } else {
            shim = path.join(binDir, 'syncix');
            const desired = `#!/bin/sh
exec "${exe}" "$@"
`;
            const current = fs.existsSync(shim) ? fs.readFileSync(shim, 'utf8') : '';
            if (current !== desired) {
                fs.writeFileSync(shim, desired, 'utf8');
                fs.chmodSync(shim, 0o755);
            }
        }

        // Only the editor's terminals are affected; the system PATH does not change.
        context.environmentVariableCollection.prepend('PATH', binDir + path.delimiter);
        return shim;
    } catch (err) {
        console.error('Syncix CLI setup error:', err);
        return undefined;
    }
}

export async function activate(context: vscode.ExtensionContext) {
    console.log('Syncix Extension Activated');
    context_extensionPath = context.extensionPath;
    extensionVersion = context.extension.packageJSON?.version ?? '';

    // 0. Auto-start and connect the core only in a Syncix workspace.
    // In other projects everything stays passive; connect by hand with "Syncix: Start" if wanted.
    const autoMode = await isSyncixWorkspace();
    // The sidebar's welcome view looks at this context: without a project
    // the user sees a "create project" button instead of an empty tree.
    await vscode.commands.executeCommand('setContext', 'syncix.hasProject', autoMode);
    if (autoMode) {
        ensurePluginInstalled(context); // install or update the Studio plugin automatically
        ensureCliInstalled(context);    // "syncix" in the editor's terminals (the system PATH does not change)
        await ensureCoreRunning();
    }

    // 1. Dependency Injection: RPC Client
    rpcClient = new RpcManager();

    // Auto-connect on startup
    if (autoMode) {
        rpcClient.connect();
    }

    // 1b. Visual status bar — ends the "is it working or not" worry.
    if (autoMode) {
        const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
        statusBar.command = 'syncix.showStatus';
        let objectCount = 0;
        let connected = false;

        // Studio's status is tracked SEPARATELY.
        //
        // The status bar used to look only at the editor-core connection: with Studio
        // closed it still said "Connected ✓". The user thought everything was in sync
        // while none of their changes reached Studio. A silent and
        // misleading state; now the three states are shown separately.
        let studioConnected = false;
        let conflictCount = 0;

        const updateStatusBar = () => {
            if (!connected) {
                statusBar.text = `$(error) Syncix: core is down`;
                statusBar.tooltip = 'The Syncix core is not running. Click to start it and reconnect.';
                statusBar.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
                return;
            }
            if (!studioConnected) {
                statusBar.text = `$(warning) Syncix: Studio not connected`;
                statusBar.tooltip =
                    'The core is running but Roblox Studio is not connected.\n' +
                    'Changes you make right now are NOT reaching Studio.\n' +
                    'Open Studio; the Syncix plugin connects on its own.';
                statusBar.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
                return;
            }
            if (conflictCount > 0) {
                statusBar.text = `$(alert) Syncix: ${conflictCount} conflict(s)`;
                statusBar.tooltip =
                    `${conflictCount} change(s) conflicted and were overwritten.\n` +
                    'See the "Recent changes" tab in the Syncix panel inside Studio.';
                statusBar.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
                return;
            }
            statusBar.text = `$(check) Syncix: connected (${objectCount})`;
            statusBar.tooltip = `Syncix connected — ${objectCount} instances in sync.\nClick for status / reconnect.`;
            statusBar.backgroundColor = undefined;
        };

        // Poll /health regularly to track Studio's status and the conflict count.
        const pollHealth = async () => {
            try {
                const health = await env.probe(
                    parseInt(new URL(env.getBaseUrl()).port || '8080', 10),
                    1200
                );
                if (health) {
                    studioConnected = health.studio_connected === true;
                    conflictCount = health.conflicts ?? 0;
                    if (typeof health.object_count === 'number') {
                        objectCount = health.object_count;
                    }
                } else {
                    studioConnected = false;
                }
            } catch {
                studioConnected = false;
            }
            updateStatusBar();
        };
        const healthTimer = setInterval(pollHealth, 5000);
        context.subscriptions.push({ dispose: () => clearInterval(healthTimer) });
        pollHealth();
        updateStatusBar();
        statusBar.show();
        context.subscriptions.push(statusBar);

        rpcClient.onConnectionChange((c) => { connected = c; updateStatusBar(); });
        rpcClient.onMessage((msg) => {
            switch (msg.event_type) {
                case 'FULL_SYNC':
                case 'TREE_UPDATED':
                    objectCount = msg.data?.nodes?.length ?? objectCount;
                    break;
                case 'INSTANCE_CREATED':
                    objectCount++;
                    break;
                case 'INSTANCE_REMOVED':
                    objectCount = Math.max(0, objectCount - 1);
                    break;
            }
            updateStatusBar();
        });

        // SAVE FEEDBACK
        //
        // The editor never watched file saves: everything was left to the core's file
        // watcher. When you saved a .lua file there was no way to tell whether the change
        // reached Studio — even with Studio closed, the save
        // silently went nowhere. Now every save reports its status.
        const syncRoot = findSyncWorkspaceRoot() ?? findProjectRoot();
        const saveWatcher = vscode.workspace.onDidSaveTextDocument((savedDoc) => {
            if (!syncRoot) return;
            const filePath = savedDoc.uri.fsPath;
            const syncFolder = path.join(syncRoot, syncFolderName());
            if (!filePath.startsWith(syncFolder)) return;

            const fileName = path.basename(filePath);
            if (!studioConnected) {
                vscode.window.showWarningMessage(
                    `Saved ${fileName}, but Roblox Studio is not connected — the change did not reach Studio.`
                );
                return;
            }
            vscode.window.setStatusBarMessage(`$(check) ${fileName} → Studio`, 2500);
        });
        context.subscriptions.push(saveWatcher);

        const showStatusCmd = vscode.commands.registerCommand('syncix.showStatus', () => {
            if (!connected) {
                vscode.window.showWarningMessage('The Syncix core is not running. Starting it...');
                ensureCoreRunning().then(() => rpcClient.connect());
                return;
            }
            if (!studioConnected) {
                vscode.window.showWarningMessage(
                    'Roblox Studio is not connected — your changes are not reaching Studio. Open Studio.'
                );
                return;
            }
            const conflictNote = conflictCount > 0 ? `  —  ${conflictCount} conflict(s)` : '';
            vscode.window.showInformationMessage(
                `Syncix connected  —  ${objectCount} instances in sync${conflictNote}`
            );
        });
        context.subscriptions.push(showStatusCmd);
    }

    // 2. State Management & Recovery
    const stateManager = new EditorStateManager(rpcClient);
    
    // Restore selection from previous session
    const savedSelection = context.workspaceState.get<string[]>('syncix.selectedNodes', []);
    if (savedSelection.length > 0) {
        stateManager.setSelectedNodes(savedSelection);
    }

    // Save selection changes
    stateManager.onSelectionChanged((ids) => {
        context.workspaceState.update('syncix.selectedNodes', ids);
    });

    // 3. Commands Registration
    const startCmd = vscode.commands.registerCommand('syncix.start', () => {
        rpcClient.connect();
    });

    const stopCmd = vscode.commands.registerCommand('syncix.stop', () => {
        rpcClient.dispose();
        vscode.window.showInformationMessage('Syncix disconnected.');
    });

    // 4. Register Tree Data Provider
    const workspaceExplorer = new WorkspaceExplorer(context, stateManager, rpcClient as any);

    const inspectNodeCmd = vscode.commands.registerCommand('syncix.inspectNode', (node) => {
        PropertyInspector.createOrShow(rpcClient, stateManager, workspaceExplorer);
        if (node) {
            PropertyInspector.currentPanel?.inspect(node);
        }
    });

    const diagnosticsCmd = vscode.commands.registerCommand('syncix.openDiagnostics', () => {
        DiagnosticsPanel.createOrShow(rpcClient);
    });

    // ── Command palette (Ctrl+Shift+P) actions ──
    const cfg = vscode.workspace.getConfiguration('syncix');
    const projRoot = findProjectRoot() ?? '';
    const syncDir = projRoot ? path.join(projRoot, syncFolderName()) : '';

    const paletteCmds = [
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
        vscode.commands.registerCommand('syncix.initProject', () => createProject(context)),
        vscode.commands.registerCommand('syncix.installPlugin', () => {
            ensurePluginInstalled(context);
            vscode.window.showInformationMessage('Checked the Syncix plugin. If it changed, restart Roblox Studio.');
        }),
        vscode.commands.registerCommand('syncix.openInspector', () => {
            PropertyInspector.createOrShow(rpcClient, stateManager, workspaceExplorer);
        }),
        vscode.commands.registerCommand('syncix.refresh', () => {
            workspaceExplorer.refresh();
            rpcClient.send('GET_TREE', {});
        }),
        vscode.commands.registerCommand('syncix.openWorkspaceFolder', () => {
            if (syncDir) {
                vscode.commands.executeCommand('revealFileInOS', vscode.Uri.file(syncDir));
            }
        }),
        vscode.commands.registerCommand('syncix.installCliGlobal', async () => {
            // The system PATH is a persistent user setting; the extension does NOT CHANGE it.
            // We give the folder and say how to add it; the decision is the user's.
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
        }),
        vscode.commands.registerCommand('syncix.selfTest', async () => {
            const paths = resolveCorePaths();
            if (!paths) {
                vscode.window.showErrorMessage('Core binary not found.');
                return;
            }
            const terminal = vscode.window.createTerminal('Syncix Selftest');
            terminal.show();
            terminal.sendText(`"${paths.exePath}" selftest`);
        }),
    ];

    context.subscriptions.push(startCmd, stopCmd, inspectNodeCmd, diagnosticsCmd, ...paletteCmds);
    context.subscriptions.push({ dispose: () => rpcClient.dispose() });
}

export function deactivate() {
    if (rpcClient) {
        rpcClient.dispose();
    }
}
