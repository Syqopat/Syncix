/**
 * Local setup: the Studio plugin bundled with the extension and the `syncix` command in
 * the editor's terminals. Neither touches the system beyond what it has to.
 */

import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import * as env from './env';
import { resolveCorePaths } from './process';

/** Compares dotted versions numerically: <0 if a is older, 0 if equal, >0 if newer. */
export function compareVersions(a: string, b: string): number {
    const pa = a.split('.').map((n) => parseInt(n, 10) || 0);
    const pb = b.split('.').map((n) => parseInt(n, 10) || 0);
    for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
        const d = (pa[i] ?? 0) - (pb[i] ?? 0);
        if (d !== 0) return d;
    }
    return 0;
}

/**
 * Installs the Studio plugin (the .rbxm bundled with the extension) into the Roblox
 * Plugins folder automatically. It copies only when the content differs, and then says
 * "restart Studio". So the user never installs the plugin by hand and versions never mix.
 */
export function ensurePluginInstalled(context: vscode.ExtensionContext) {
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
        // Which extension version wrote the installed plugin. Studio only loads .rbxm,
        // .rbxmx and .lua files, so it ignores this one.
        const versionFile = path.join(pluginsDir, 'SyncixPlugin.version');
        const mine: string = context.extension.packageJSON?.version ?? '';

        const srcBuf = fs.readFileSync(src);
        let needsCopy = true;
        if (fs.existsSync(dest)) {
            needsCopy = !srcBuf.equals(fs.readFileSync(dest));
        }
        // An older Syncix in another editor (or an older copy in this one) used to put
        // its own plugin back over a newer one on every start, so Studio silently ran
        // old code. A plugin written by a newer version is left alone.
        if (needsCopy && mine && fs.existsSync(versionFile)) {
            const installed = fs.readFileSync(versionFile, 'utf8').trim();
            if (compareVersions(installed, mine) > 0) {
                console.log(
                    `Syncix: the Studio plugin from ${installed} is newer than this extension (${mine}); left as is.`
                );
                return;
            }
        }
        if (needsCopy) {
            fs.writeFileSync(dest, srcBuf);
            if (mine) fs.writeFileSync(versionFile, mine);
            vscode.window.showInformationMessage(
                'The Syncix Studio plugin was updated. Restart Roblox Studio for it to take effect.'
            );
        } else if (mine && !fs.existsSync(versionFile)) {
            fs.writeFileSync(versionFile, mine);
        }
    } catch (err: any) {
        console.error('Syncix plugin install error:', err);
    }
}

/**
 * Adds the `syncix` command to the editor's own terminals.
 *
 * This function used to write a .cmd into the user's home folder and change the user's
 * PATH in the registry with a hidden, detached shell process -- without asking, on every
 * project open. That was a persistent change to the system without consent.
 *
 * Now the shortcut is written to the extension's own storage folder and added to PATH
 * only through the VS Code API, for terminals the editor opens. The registry, the home
 * folder and the system PATH are not touched; nothing is left behind when the extension
 * is uninstalled.
 */
export function ensureCliInstalled(context: vscode.ExtensionContext): string | undefined {
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
