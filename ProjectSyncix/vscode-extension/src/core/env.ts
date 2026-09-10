/**
 * Environment resolution: the core's address and platform-specific file paths.
 *
 * Why a separate module:
 *  - The address used to be hard-coded as 'http://127.0.0.1:8080' in four separate files.
 *    The port can change now (the core skips a taken port), so it has to be read
 *    from a single source.
 *  - Windows assumptions (.exe, powershell, taskkill) were scattered the same way.
 *    Roblox Studio also runs on macOS; these assumptions are gathered here in one place.
 */

import * as fs from 'fs';
import * as http from 'http';
import * as path from 'path';

const DEFAULT_PORT = 8080;
const PORT_RANGE = 10;

let baseUrl = `http://127.0.0.1:${DEFAULT_PORT}`;

/** HTTP address of the core (e.g. http://127.0.0.1:8081). */
export function getBaseUrl(): string {
    return baseUrl;
}

/** Core'un WebSocket RPC adresi. */
export function getWsUrl(): string {
    return baseUrl.replace(/^http/, 'ws') + '/rpc';
}

/** Is there a Syncix core on the given port? If so, returns its /health reply. */
export function probe(port: number, timeoutMs = 1200): Promise<any | undefined> {
    return new Promise((resolve) => {
        const req = http.get(`http://127.0.0.1:${port}/health`, (res) => {
            let bodyText = '';
            res.on('data', (c) => (bodyText += c));
            res.on('end', () => {
                try {
                    const j = JSON.parse(bodyText);
                    // Another program may be on the port; the Syncix signature is checked.
                    resolve(j && typeof j.status === 'string' ? j : undefined);
                } catch {
                    resolve(undefined);
                }
            });
        });
        req.on('error', () => resolve(undefined));
        req.setTimeout(timeoutMs, () => {
            req.destroy();
            resolve(undefined);
        });
    });
}

/** Port file the running core writes: <project>/.syncix/port */
export function readPortFile(projectRoot: string | undefined): number | undefined {
    if (!projectRoot) return undefined;
    try {
        const p = path.join(projectRoot, '.syncix', 'port');
        if (!fs.existsSync(p)) return undefined;
        const n = parseInt(fs.readFileSync(p, 'utf8').trim(), 10);
        return Number.isFinite(n) ? n : undefined;
    } catch {
        return undefined;
    }
}

/**
 * Do the two paths point at the same project?
 * Windows is case-insensitive; a trailing separator must not matter either.
 */
function isSameProject(a: string | undefined, b: string | undefined): boolean {
    if (!a || !b) return false;
    const fixUp = (x: string) => {
        const n = path.resolve(x).replace(/[\/]+$/, '');
        return isWindows() ? n.toLowerCase() : n;
    };
    return fixUp(a) === fixUp(b);
}

/**
 * Finds the running core and updates the address.
 * It checks the port file first (exact information), otherwise scans the range.
 * If nothing is found the address stays the same and undefined is returned.
 *
 * CRITICAL: the core found is verified to belong to THIS project.
 *
 * It used to check only "is there a healthy Syncix on the port". With two
 * projects open at once, the second connected to the first one's core: the editor
 * said "connected", the tree showed, but every change went to ANOTHER game.
 * A silent and dangerous situation; /health already carries the `root` field,
 * so verifying costs nothing.
 */
export async function refreshBaseUrl(projectRoot?: string): Promise<any | undefined> {
    // Without a known project root there is nothing to verify; the old behaviour is kept.
    const checkValue = (health: any) =>
        !projectRoot || isSameProject(health?.root, projectRoot);

    const fromFile = readPortFile(projectRoot);
    if (fromFile) {
        const health = await probe(fromFile);
        if (health && checkValue(health)) {
            baseUrl = `http://127.0.0.1:${fromFile}`;
            return health;
        }
    }

    for (let port = DEFAULT_PORT; port < DEFAULT_PORT + PORT_RANGE; port++) {
        const health = await probe(port);
        if (health && checkValue(health)) {
            baseUrl = `http://127.0.0.1:${port}`;
            return health;
        }
    }
    return undefined;
}

// ---------------------------------------------------------------------------
// Platform
// ---------------------------------------------------------------------------

export function isWindows(): boolean {
    return process.platform === 'win32';
}

/** Executable file name (with .exe on Windows). */
export function coreBinaryName(): string {
    return isWindows() ? 'syncix-core.exe' : 'syncix-core';
}

/**
 * Path of the bundled binary inside the extension.
 * Binaries are kept in separate folders per platform:
 *   resources/bin/win32-x64/syncix-core.exe
 *   resources/bin/darwin-arm64/syncix-core
 * The old single-file layout (resources/bin/syncix-core.exe) is supported too, so
 * old packages do not break.
 */
export function bundledCorePaths(extensionPath: string): string[] {
    const fileName = coreBinaryName();
    return [
        path.join(extensionPath, 'resources', 'bin', `${process.platform}-${process.arch}`, fileName),
        path.join(extensionPath, 'resources', 'bin', process.platform, fileName),
        path.join(extensionPath, 'resources', 'bin', fileName),
    ];
}

/** Roblox's plugin folder (per platform). */
export function robloxPluginsDir(): string | undefined {
    if (isWindows()) {
        const local = process.env.LOCALAPPDATA;
        return local ? path.join(local, 'Roblox', 'Plugins') : undefined;
    }
    if (process.platform === 'darwin') {
        const home = process.env.HOME;
        // On macOS Studio plugins live under the user's Documents folder.
        return home ? path.join(home, 'Documents', 'Roblox', 'Plugins') : undefined;
    }
    // There is no official Roblox Studio on Linux; plugin installation is skipped.
    return undefined;
}

