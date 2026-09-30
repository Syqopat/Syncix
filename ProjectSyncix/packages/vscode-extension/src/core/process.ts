/**
 * The core process: where its binary is, starting it, stopping it and checking that its
 * version matches this extension's.
 */

import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { spawn } from 'child_process';
import * as env from './env';
import { findProjectRoot, findSyncWorkspaceRoot } from './project';

/** Core process started by the extension (for stopping and restarting). */
let coreProcessPid: number | undefined;

/** Extension path; resolveCorePaths uses it for the bundled binary. */
let extensionPath = '';

/**
 * The extension's own version.
 *
 * It used to be looked up by a hard-coded extension id. That id was never real (no
 * publisher was ever called "Syncix"), so the lookup always returned undefined and the
 * version-mismatch warning never fired.
 */
let extensionVersion = '';

/** The platform warning is shown once per session, not on every attempt. */
let platformWarningShown = false;

/** Reports a version mismatch once (so it does not misbehave silently). */
let versionWarningShown = false;

export function initProcessModule(context: vscode.ExtensionContext) {
    extensionPath = context.extensionPath;
    extensionVersion = context.extension.packageJSON?.version ?? '';
}

export function ownVersion(): string {
    return extensionVersion;
}

/**
 * Is the core engine up? Also updates the address: the port is no longer fixed,
 * the core may skip a taken port and move to the next one.
 */
export async function checkCoreHealth(): Promise<boolean> {
    const health = await env.refreshBaseUrl(findProjectRoot());
    if (!health) return false;
    warnVersionMismatch(health);
    return true;
}

function warnVersionMismatch(health: any) {
    if (versionWarningShown || !health?.version || !extensionVersion) return;
    const mm = (v: string) => v.split('.').slice(0, 2).join('.');
    if (mm(extensionVersion) !== mm(health.version)) {
        versionWarningShown = true;
        vscode.window.showWarningMessage(
            `Syncix version mismatch — extension ${extensionVersion}, core ${health.version}. ` +
            `The same major.minor is required; rebuild the core or update the extension.`
        );
    }
}

/**
 * Resolves the path of the core executable. Order:
 *   1) The user's setting (if any)
 *   2) The development build at the project root (when working inside the repository)
 *   3) The binary bundled with the extension (the end-user case; no repository needed)
 * The working directory is always the PARENT of the sync folder; the core expects
 * "../<sync_dir>", so the bundled binary finds the right folder too.
 */
export function resolveCorePaths(): { exePath: string; cwd: string } | undefined {
    const cfg = vscode.workspace.getConfiguration('syncix');
    const cfgExe = cfg.get<string>('coreExePath', '');
    const cfgCwd = cfg.get<string>('coreCwd', '');
    if (cfgExe && fs.existsSync(cfgExe) && cfgCwd && fs.existsSync(cfgCwd)) {
        return { exePath: cfgExe, cwd: cfgCwd };
    }

    const root = findProjectRoot();
    if (root) {
        const devCwd = path.join(root, 'packages', 'core-engine');
        const devExe = path.join(devCwd, 'target', 'release', env.coreBinaryName());
        if (fs.existsSync(devExe)) return { exePath: devExe, cwd: devCwd };
    }

    // Bundled binary: the one matching the platform is chosen (separate Windows/macOS folders).
    const bundled = env.bundledCorePaths(extensionPath).find((p) => fs.existsSync(p));
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

/** Starts the core engine automatically when it is not running (for convenience). */
export async function ensureCoreRunning(): Promise<void> {
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
 * This used to run `taskkill /IM syncix-core.exe /F`: killing by name shut down the
 * cores of ALL OPEN projects and only worked on Windows. Now only the process we
 * started ourselves is stopped, by PID.
 */
export function stopCore(): boolean {
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
