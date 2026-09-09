import * as vscode from 'vscode';
import { RpcManager } from '../rpc/manager';
import { EditorStateManager } from '../state/EditorStateManager';
import { ExplorerCache, NodeData } from './ExplorerCache';
import { MessageEnvelope, RequestPriority } from '../rpc/queue';

class SyncixTreeItem extends vscode.TreeItem {
    constructor(
        public readonly node: NodeData,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState
    ) {
        super(node.name, collapsibleState);
        this.id = node.id;
        this.description = node.className;
        this.contextValue = 'syncixNode';
        // Tıklanınca node'un diskteki karşılığını (varsa) aç
        this.command = {
            command: 'syncix.openNodeFile',
            title: 'Open Synced File',
            arguments: [node]
        };
    }
}

export class WorkspaceExplorer implements vscode.TreeDataProvider<SyncixTreeItem> {
    public readonly cache = new ExplorerCache();
    private _onDidChangeTreeData = new vscode.EventEmitter<SyncixTreeItem | undefined | null | void>();
    public readonly onDidChangeTreeData = this._onDidChangeTreeData.event;
    private searchQuery: string = "";
    private view?: vscode.TreeView<SyncixTreeItem>;
    /** Studio'dan gelen seçimi uygularken kendi olayımızı geri göndermemek için. */
    private studiodanGelenSecim = false;

    constructor(
        private context: vscode.ExtensionContext,
        private stateManager: EditorStateManager,
        private rpc: RpcManager
    ) {
        // Register Context Menu Commands (CRUD)
        vscode.commands.registerCommand('syncix.createInstance', async (item: SyncixTreeItem) => {
            const className = await vscode.window.showInputBox({ prompt: "Enter ClassName (e.g., Part)" });
            if (className) {
                rpc.send("CREATE_INSTANCE", { parentId: item.node.id, className }, RequestPriority.High);
            }
        });
        vscode.commands.registerCommand('syncix.deleteInstance', (item: SyncixTreeItem) => {
            rpc.send("DELETE_INSTANCE", { id: item.node.id }, RequestPriority.High);
        });
        vscode.commands.registerCommand('syncix.renameInstance', async (item: SyncixTreeItem) => {
            const newName = await vscode.window.showInputBox({ prompt: "Enter new name", value: item.node.name });
            if (newName) {
                rpc.send("RENAME_INSTANCE", { id: item.node.id, newName }, RequestPriority.High);
            }
        });

        // Node tıklanınca diskteki senkronize dosyayı aç (dosya adı: {isim}_{uuid'nin ilk 8 hanesi}.part.json)
        vscode.commands.registerCommand('syncix.openNodeFile', async (node: NodeData) => {
            const shortUuid = node.id.substring(0, 8).toLowerCase();
            const files = await vscode.workspace.findFiles(`**/*_${shortUuid}.part.json`, '**/node_modules/**', 1);
            if (files.length > 0) {
                const doc = await vscode.workspace.openTextDocument(files[0]);
                await vscode.window.showTextDocument(doc, { preview: true });
            } else {
                vscode.window.setStatusBarMessage(
                    `Syncix: ${node.name} için disk dosyası yok (şu an yalnızca Part sınıfı diske yazılıyor).`,
                    4000
                );
            }
        });

        vscode.commands.registerCommand('syncix.searchExplorer', async () => {
            const query = await vscode.window.showInputBox({ prompt: "Search by Name, ClassName, UUID or Tags..." });
            this.searchQuery = query ? query.toLowerCase() : "";
            this.refresh();
        });

        // Register Tree View
        const DRAG_MIME = 'application/vnd.code.tree.syncixexplorer';
        const view = vscode.window.createTreeView('syncixExplorer', {
            treeDataProvider: this,
            showCollapseAll: true,
            canSelectMany: true,
            dragAndDropController: { // Sürükle-bırak ile taşıma (reparent)
                dragMimeTypes: [DRAG_MIME],
                dropMimeTypes: [DRAG_MIME],
                handleDrag: (source, dataTransfer) => {
                    // Sürüklenen node id'lerini taşı
                    const ids = source.map(s => s.node.id);
                    dataTransfer.set(DRAG_MIME, new vscode.DataTransferItem(ids));
                },
                handleDrop: async (target, dataTransfer) => {
                    // Hedef yoksa (boşluğa bırakıldıysa) bir şey yapma
                    if (!target) return;
                    const item = dataTransfer.get(DRAG_MIME);
                    if (!item) return;
                    const draggedIds: string[] = item.value;
                    for (const id of draggedIds) {
                        if (id === target.node.id) continue; // kendi üstüne bırakma
                        this.rpc.send("REPARENT_INSTANCE", { id, newParentId: target.node.id }, RequestPriority.High);
                    }
                }
            }
        });

        this.context.subscriptions.push(view);

        // Bind RPC Events for Incremental Updates
        this.rpc.onMessage((msg: MessageEnvelope) => {
            switch(msg.event_type) {
                case "INSTANCE_CREATED":
                    this.cache.addNode(msg.data);
                    this.refresh(msg.data.parentId);
                    break;
                case "INSTANCE_UPDATED":
                    this.cache.updateNode(msg.data.id, msg.data);
                    this.refresh(msg.data.id);
                    break;
                case "INSTANCE_REMOVED":
                    const parent = this.cache.getNode(msg.data.id)?.parentId;
                    this.cache.removeNode(msg.data.id);
                    this.refresh(parent);
                    break;
                case "INSTANCE_MOVED":
                    this.cache.moveNode(msg.data.id, msg.data.newParentId);
                    this.refresh(msg.data.oldParentId);
                    this.refresh(msg.data.newParentId);
                    break;
                case "SELECTION":
                    void this.studioSeciminiGoster(msg.data?.ids ?? []);
                    break;
                case "TREE_UPDATED": // Bulk initial load
                case "FULL_SYNC":
                    this.cache.clear();
                    for (const node of msg.data.nodes) {
                        this.cache.addNode(node);
                    }
                    // Node sıralaması garantili olmadığından bağları sıfırdan kur
                    this.cache.rebuildChildLinks();
                    this.refresh();
                    break;
            }
        });

        // Update selection state
        view.onDidChangeSelection(e => {
            const ids = e.selection.map(item => item.node.id);
            this.stateManager.setSelectedNodes(ids);

            // Editörde seçileni Studio'da da seç.
            // Studio'dan gelen seçimi uygularken bu olay yeniden tetikleniyor;
            // bayrak olmasa iki taraf birbirini sonsuza kadar tetiklerdi.
            if (!this.studiodanGelenSecim) {
                this.rpc.send('SELECTION', { ids, source: 'editor' });
            }
        });

        this.view = view;
    }

    /** Studio'dan gelen seçimi ağaçta göster. */
    private async studioSeciminiGoster(ids: string[]): Promise<void> {
        if (!this.view || ids.length === 0) return;

        // Yalnızca ağaçta karşılığı olanlar; silinmiş bir kimlik seçimi bozmasın.
        const ogeler = ids
            .map((id) => this.cache.getNode(id))
            .filter((n): n is NodeData => !!n)
            .map((n) => new SyncixTreeItem(n, vscode.TreeItemCollapsibleState.Collapsed));
        if (ogeler.length === 0) return;

        this.studiodanGelenSecim = true;
        try {
            // reveal ağacı gerekli yerlerde açar; select seçimi kurar.
            await this.view.reveal(ogeler[0], { select: true, focus: false, expand: 3 });
        } catch {
            // Görünmeyen bir düğüm reveal edilemeyebilir; seçim senkronu
            // bu yüzden hata vermemeli.
        } finally {
            this.studiodanGelenSecim = false;
        }
    }

    refresh(nodeId?: string | null): void {
        // Not: fire(element) VS Code tarafında referans eşleşmesi ister;
        // her seferinde yeni SyncixTreeItem üretmek hiçbir şeyi yenilemez.
        // Bu yüzden her zaman tam ağaç yenilenir (id bazlı expansion korunur).
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: SyncixTreeItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: SyncixTreeItem): Promise<SyncixTreeItem[]> {
        if (!this.stateManager.isConnected) return [];

        if (this.searchQuery) {
            if (element) return []; // Flat list under root when searching
            const matches = this.cache.searchNodes(this.searchQuery);
            return matches.map(c => new SyncixTreeItem(c, vscode.TreeItemCollapsibleState.None));
        }

        if (element) {
            // Tüm ağaç FULL_SYNC ile geldiği için lazy loading yok;
            // hiyerarşi doğrudan cache'ten okunur.
            const children = this.cache.getChildren(element.node.id);
            return children.map(c => new SyncixTreeItem(c, c.childrenIds.length > 0 ? vscode.TreeItemCollapsibleState.Collapsed : vscode.TreeItemCollapsibleState.None));
        } else {
            // Root: parent'ı olmayan node'lar (Roblox servisleri).
            // Roblox Explorer'daki sıralamaya yakın, sabit bir sıra uygulanır.
            const serviceOrder = ["Workspace", "Lighting", "ReplicatedFirst", "ReplicatedStorage", "ServerScriptService", "ServerStorage", "StarterGui", "StarterPack", "StarterPlayer", "Teams", "SoundService"];
            const roots = this.cache.getRootNodes().sort((a, b) => {
                const ia = serviceOrder.indexOf(a.name);
                const ib = serviceOrder.indexOf(b.name);
                return (ia === -1 ? 999 : ia) - (ib === -1 ? 999 : ib);
            });

            if (roots.length === 0) {
                // Henüz veri yoksa sunucudan tam ağacı iste (cevap FULL_SYNC push olarak gelir)
                this.rpc.send("GET_TREE", {}, RequestPriority.Normal);
                return [];
            }

            return roots.map(n => new SyncixTreeItem(
                n,
                n.childrenIds.length > 0 ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.Collapsed
            ));
        }
    }
}
