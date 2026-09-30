/**
 * Where the project is: the repository root, the folder holding syncix.toml and the name
 * of the sync folder. Everything else asks here instead of guessing from folder names.
 */

import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

/**
 * Finds the project root (portability): searches upwards from the open folder for packages/core-engine.
 * A configured value takes precedence; if there is none, undefined.
 */
export function findProjectRoot(): string | undefined {
    const cfgRoot = vscode.workspace.getConfiguration('syncix').get<string>('coreCwd', '');
    if (cfgRoot && fs.existsSync(cfgRoot)) return path.dirname(cfgRoot);

    const folders = vscode.workspace.workspaceFolders;
    if (!folders) return undefined;
    for (const f of folders) {
        let dir = f.uri.fsPath;
        for (let i = 0; i < 5; i++) {
            if (fs.existsSync(path.join(dir, 'packages', 'core-engine'))) return dir;
            const up = path.dirname(dir);
            if (up === dir) break;
            dir = up;
        }
    }
    return undefined;
}

/** Finds the parent directory of the sync folder (based on syncix.toml). */
export function findSyncWorkspaceRoot(): string | undefined {
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

/** Path of syncix.toml in the workspace, or undefined. */
export function projectFile(): string | undefined {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) return undefined;
    for (const folder of folders) {
        const candidate = path.join(folder.uri.fsPath, 'syncix.toml');
        if (fs.existsSync(candidate)) return candidate;
    }
    return undefined;
}

/**
 * Is this a Syncix project?
 *
 * The criterion is the presence of syncix.toml. It used to look at the folder NAME
 * ("src_workspace", "projectsyncix") -- those were this repository's own old folder
 * names. The result: everything worked on our machine, while for anyone who named
 * their project "MyGame" the extension silently did nothing.
 */
export function isSyncixWorkspace(): boolean {
    return projectFile() !== undefined;
}

/**
 * NAME of the project's sync folder (`sync_dir` in syncix.toml).
 *
 * This name used to be hard-coded ("src_workspace") -- this repository's own folder
 * name. The result: for anyone who named their folder differently, the save feedback and
 * the "open sync folder" command silently pointed at the wrong path.
 */
export function syncFolderName(): string {
    const toml = projectFile();
    if (toml) {
        try {
            const m = /^\s*sync_dir\s*=\s*"([^"]+)"/m.exec(fs.readFileSync(toml, 'utf8'));
            if (m) return m[1];
        } catch {
            // unreadable: fall back to the default
        }
    }
    return 'src';
}

/** Absolute path of the sync folder, when there is a project. */
export function syncFolderPath(): string | undefined {
    const root = findSyncWorkspaceRoot() ?? findProjectRoot();
    return root ? path.join(root, syncFolderName()) : undefined;
}
