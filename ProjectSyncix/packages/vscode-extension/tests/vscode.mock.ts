/**
 * Enough of the `vscode` module for the extension's own logic to be testable.
 *
 * The real module only exists inside the editor, so every file that imports it was
 * untestable. Only what the tests touch is faked; anything else throws loudly rather
 * than quietly returning undefined.
 */

export const workspace = {
    folders: [] as { uri: { fsPath: string } }[],
    settings: new Map<string, unknown>(),

    get workspaceFolders() {
        return workspace.folders.length ? workspace.folders : undefined;
    },
    getConfiguration(section: string) {
        return {
            get<T>(key: string, fallback?: T): T | undefined {
                const value = workspace.settings.get(`${section}.${key}`);
                return (value === undefined ? fallback : value) as T | undefined;
            },
            update: async () => undefined,
        };
    },
    onDidSaveTextDocument: () => ({ dispose() {} }),
};

export class EventEmitter<T> {
    private listeners: ((value: T) => void)[] = [];

    readonly event = (listener: (value: T) => void) => {
        this.listeners.push(listener);
        return { dispose: () => undefined };
    };

    fire(value: T) {
        for (const listener of [...this.listeners]) listener(value);
    }

    dispose() {
        this.listeners = [];
    }
}

export class ThemeColor {
    constructor(public readonly id: string) {}
}

export const window = {
    messages: [] as string[],
    showWarningMessage(message: string) {
        window.messages.push(message);
    },
    showErrorMessage(message: string) {
        window.messages.push(message);
    },
    showInformationMessage(message: string) {
        window.messages.push(message);
    },
    setStatusBarMessage() {},
    createStatusBarItem() {
        return {
            text: '',
            tooltip: '',
            backgroundColor: undefined as ThemeColor | undefined,
            command: '',
            show() {},
            dispose() {},
        };
    },
};

export const commands = {
    executeCommand: async () => undefined,
    registerCommand: () => ({ dispose() {} }),
};

export const env = {
    opened: [] as string[],
    openExternal(uri: { toString(): string }) {
        env.opened.push(uri.toString());
        return Promise.resolve(true);
    },
    clipboard: { writeText: async () => undefined },
};

export const Uri = {
    parse: (value: string) => ({ toString: () => value }),
    file: (value: string) => ({ fsPath: value, toString: () => value }),
};

export const StatusBarAlignment = { Left: 1, Right: 2 };
export const ConfigurationTarget = { Global: 1, Workspace: 2 };
export const ViewColumn = { Active: -1 };
export const ProgressLocation = { Notification: 15 };

/** Puts the fake back to its starting state between tests. */
export function reset() {
    workspace.folders = [];
    workspace.settings = new Map();
    window.messages = [];
    env.opened = [];
}
