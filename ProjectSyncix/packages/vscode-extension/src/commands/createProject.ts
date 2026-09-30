import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { ensureCliInstalled, ensurePluginInstalled } from '../core/install';
import { ensureCoreRunning } from '../core/process';

/**
 * Starts a new Syncix project in the open folder.
 *
 * Without this command a new user was stuck in a closed loop: the extension only runs
 * when syncix.toml exists, syncix.toml could only be created by the CLI, and the CLI was
 * installed when the extension ran.
 */
export async function createProject(
    context: vscode.ExtensionContext,
    connect: () => void
): Promise<void> {
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
    if (!syncDir) return; // the user cancelled

    const sampleFile = path.join(context.extensionPath, 'resources', 'syncix.example.toml');
    let fileText: string;
    if (fs.existsSync(sampleFile)) {
        // The sample file explains every setting; it is given as is so a new user
        // can see what can be changed.
        fileText = fs
            .readFileSync(sampleFile, 'utf8')
            .replace(/^sync_dir = ".*"$/m, `sync_dir = "${syncDir.trim()}"`);
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
    connect();
    await vscode.commands.executeCommand('setContext', 'syncix.hasProject', true);

    const doc = await vscode.workspace.openTextDocument(tomlPath);
    await vscode.window.showTextDocument(doc);
    vscode.window.showInformationMessage(
        'Syncix project created. Open Roblox Studio — the plugin is installed and the core is running.'
    );
}
