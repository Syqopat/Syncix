import * as path from 'path';
import * as vscode from 'vscode';
import { StatusSnapshot, StatusTracker } from '../state/StatusTracker';
import { syncFolderPath } from '../core/project';

/**
 * The one line that ends the "is it working or not" worry.
 *
 * The three states are kept apart on purpose. The status bar used to look only at the
 * editor-core connection: with Studio closed it still said "Connected", so the user
 * believed everything was in sync while none of their changes reached Studio.
 */
export function createStatusBar(
    context: vscode.ExtensionContext,
    tracker: StatusTracker
): vscode.StatusBarItem {
    const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    item.command = 'syncix.showStatus';

    const paint = (s: StatusSnapshot) => {
        if (!s.coreUp) {
            item.text = '$(error) Syncix: core is down';
            item.tooltip = 'The Syncix core is not running. Click to start it and reconnect.';
            item.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
        } else if (!s.studioConnected) {
            item.text = '$(warning) Syncix: Studio not connected';
            item.tooltip =
                'The core is running but Roblox Studio is not connected.\n' +
                'Changes you make right now are NOT reaching Studio.\n' +
                'Open Studio; the Syncix plugin connects on its own.';
            item.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
        } else if (s.conflicts > 0) {
            item.text = `$(alert) Syncix: ${s.conflicts} conflict(s)`;
            item.tooltip =
                `${s.conflicts} change(s) conflicted and were overwritten.\n` +
                'Open the Syncix panel and check the Live tab.';
            item.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
        } else {
            item.text = `$(check) Syncix: connected (${s.objectCount})`;
            item.tooltip = `Syncix connected — ${s.objectCount} instances in sync.\nClick for status / reconnect.`;
            item.backgroundColor = undefined;
        }
    };

    paint(tracker.current);
    item.show();
    context.subscriptions.push(item, tracker.onChange(paint));
    return item;
}

/**
 * Save feedback.
 *
 * The editor never watched file saves: everything was left to the core's file watcher.
 * When you saved a .lua file there was no way to tell whether the change reached Studio
 * -- even with Studio closed, the save silently went nowhere.
 */
export function watchSaves(context: vscode.ExtensionContext, tracker: StatusTracker) {
    const syncFolder = syncFolderPath();
    if (!syncFolder) return;

    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument((savedDoc) => {
            const filePath = savedDoc.uri.fsPath;
            if (!filePath.startsWith(syncFolder)) return;

            const fileName = path.basename(filePath);
            if (!tracker.current.studioConnected) {
                vscode.window.showWarningMessage(
                    `Saved ${fileName}, but Roblox Studio is not connected — the change did not reach Studio.`
                );
                return;
            }
            tracker.note(`saved ${fileName}`);
            vscode.window.setStatusBarMessage(`$(check) ${fileName} → Studio`, 2500);
        })
    );
}
