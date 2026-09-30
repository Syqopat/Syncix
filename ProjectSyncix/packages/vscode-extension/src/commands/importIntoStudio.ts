import * as http from 'http';
import * as vscode from 'vscode';
import { spawn } from 'child_process';
import * as env from '../core/env';
import { findProjectRoot } from '../core/project';
import { resolveCorePaths } from '../core/process';

/**
 * "Syncix: Import into Studio" on a folder or a model file in the Explorer.
 *
 * A model kept outside the sync folder (what the assetkit tool writes, or a .rbxmx) is
 * brought into the place. Copying such a folder into the sync folder cannot work: the
 * watcher reads one file at a time and the identities in them belong to the place they
 * came from. The CLI reads the whole tree in one go and creates it with fresh identities,
 * so the same folder can be imported twice and gives two copies.
 */
export async function importIntoStudio(uri?: vscode.Uri) {
    let target = uri?.fsPath;
    if (!target) {
        const picked = await vscode.window.showOpenDialog({
            canSelectFiles: true,
            canSelectFolders: true,
            openLabel: 'Import into Studio',
            title: 'Choose a model folder, a .json or a .rbxmx file',
        });
        target = picked?.[0]?.fsPath;
    }
    if (!target) return;

    const parent = await pickImportParent();
    if (!parent) return; // the user closed the list

    const paths = resolveCorePaths();
    if (!paths) {
        vscode.window.showErrorMessage('Syncix: the core was not found; try Syncix: Restart Core first.');
        return;
    }
    const output = vscode.window.createOutputChannel('Syncix Import');
    output.show(true);
    output.appendLine(`syncix import "${target}" ${parent}`);

    await vscode.window.withProgress(
        { location: vscode.ProgressLocation.Notification, title: `Syncix: importing into ${parent}...` },
        () =>
            new Promise<void>((resolve) => {
                const child = spawn(paths.exePath, ['import', target as string, parent], { cwd: paths.cwd });
                // The colours the CLI writes for a terminal are noise in an output channel.
                const write = (data: Buffer) =>
                    output.append(data.toString().replace(/\u001b\[[0-9;]*m/g, ''));
                child.stdout.on('data', write);
                child.stderr.on('data', write);
                child.on('error', (err) => {
                    output.appendLine(String(err));
                    resolve();
                });
                child.on('close', (code) => {
                    if (code === 0) {
                        vscode.window.showInformationMessage(`Syncix: imported into ${parent}.`);
                    } else {
                        vscode.window.showErrorMessage(
                            'Syncix: the import did not finish; see the Syncix Import output.'
                        );
                    }
                    resolve();
                });
            })
    );
}

/** Where the import should land: the services of the place, Workspace first. */
async function pickImportParent(): Promise<string | undefined> {
    let services: string[] = [];
    try {
        services = serviceNames(await readTree());
    } catch {
        // No core, no list: Workspace is the default anyway, and a name can be typed.
    }
    if (services.length === 0) services = ['Workspace'];

    const typed = 'Somewhere else (type a name or id)...';
    const choice = await vscode.window.showQuickPick([...services, typed], {
        title: 'Syncix: where should it be created?',
        placeHolder: 'Workspace',
    });
    if (!choice) return undefined;
    if (choice !== typed) return choice;
    return vscode.window.showInputBox({
        title: 'Syncix: where should it be created?',
        prompt: 'A name or a short id, as syncix import takes it',
        value: 'Workspace',
    });
}

/** The tree from the core. The project token is required; without it the call is refused. */
function readTree(): Promise<any[]> {
    const token = env.readTokenFile(findProjectRoot());
    const options: http.RequestOptions = token ? { headers: { 'X-Syncix-Token': token } } : {};
    return new Promise((resolve, reject) => {
        const req = http.get(env.getBaseUrl() + '/tree', options, (res) => {
            let body = '';
            res.on('data', (c) => (body += c));
            res.on('end', () => {
                try {
                    resolve(JSON.parse(body));
                } catch (e) {
                    reject(e);
                }
            });
        });
        req.on('error', reject);
        req.setTimeout(3000, () => {
            req.destroy();
            reject(new Error('timeout'));
        });
    });
}

function serviceNames(tree: any[]): string[] {
    return tree
        .filter((n) => !n.parentId && n.name)
        .map((n) => String(n.name))
        .sort((a, b) => (a === 'Workspace' ? -1 : b === 'Workspace' ? 1 : a.localeCompare(b)));
}
