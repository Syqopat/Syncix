export interface NodeData {
    id: string;
    name: string;
    className: string;
    parentId: string | null;
    childrenIds: string[];
    isExpanded: boolean;
}

export class ExplorerCache {
    private nodes = new Map<string, NodeData>();
    private searchIndex = new Map<string, Set<string>>(); // Token -> Set of UUIDs

    private tokenize(text: string): string[] {
        return text.toLowerCase().split(/[\s_]+/).filter(t => t.length >= 2);
    }

    private indexNode(node: NodeData) {
        const tokens = [...this.tokenize(node.name), ...this.tokenize(node.className), node.id.toLowerCase()];
        for (const token of tokens) {
            if (!this.searchIndex.has(token)) {
                this.searchIndex.set(token, new Set());
            }
            this.searchIndex.get(token)!.add(node.id);
        }
    }

    private unindexNode(node: NodeData) {
        const tokens = [...this.tokenize(node.name), ...this.tokenize(node.className), node.id.toLowerCase()];
        for (const token of tokens) {
            this.searchIndex.get(token)?.delete(node.id);
        }
    }

    public addNode(node: NodeData) {
        this.nodes.set(node.id, node);
        this.indexNode(node);
        if (node.parentId) {
            const parent = this.nodes.get(node.parentId);
            if (parent && !parent.childrenIds.includes(node.id)) {
                parent.childrenIds.push(node.id);
            }
        }
    }

    public updateNode(id: string, updates: Partial<NodeData>) {
        const node = this.nodes.get(id);
        if (node) {
            this.unindexNode(node);
            Object.assign(node, updates);
            this.indexNode(node);
        }
    }

    public removeNode(id: string) {
        const node = this.nodes.get(id);
        if (node) {
            this.unindexNode(node);
            if (node.parentId) {
                const parent = this.nodes.get(node.parentId);
                if (parent) {
                    parent.childrenIds = parent.childrenIds.filter(childId => childId !== id);
                }
            }
            this.nodes.delete(id);
        }
    }

    public moveNode(id: string, newParentId: string) {
        const node = this.nodes.get(id);
        if (node) {
            if (node.parentId) {
                const oldParent = this.nodes.get(node.parentId);
                if (oldParent) {
                    oldParent.childrenIds = oldParent.childrenIds.filter(childId => childId !== id);
                }
            }
            node.parentId = newParentId;
            const newParent = this.nodes.get(newParentId);
            if (newParent && !newParent.childrenIds.includes(id)) {
                newParent.childrenIds.push(id);
            }
        }
    }

    public getNode(id: string): NodeData | undefined {
        return this.nodes.get(id);
    }

    public getNodePath(id: string): string {
        const node = this.nodes.get(id);
        if (!node) return "Unknown";
        
        let path = node.name;
        let current = node;
        while (current.parentId && current.parentId !== "Workspace") {
            const parent = this.nodes.get(current.parentId);
            if (!parent) break;
            path = parent.name + " > " + path;
            current = parent;
        }
        return "Workspace > " + path;
    }

    public getChildren(parentId: string): NodeData[] {
        const parent = this.nodes.get(parentId);
        if (!parent) return [];

        return parent.childrenIds
            .map(id => this.nodes.get(id))
            .filter((n): n is NodeData => n !== undefined);
    }

    /**
     * Returns the nodes without a parent (Roblox services).
     * They are shown at the root level of the Explorer.
     */
    public getRootNodes(): NodeData[] {
        const roots: NodeData[] = [];
        for (const node of this.nodes.values()) {
            if (!node.parentId) {
                roots.push(node);
            }
        }
        return roots;
    }

    /**
     * Rebuilds every parent-child link from scratch after a bulk load (FULL_SYNC).
     * When nodes arrive in no particular order (e.g. GET_TREE HashMap order),
     * the single linking in addNode is not enough; this function guarantees consistency.
     */
    public rebuildChildLinks() {
        for (const node of this.nodes.values()) {
            node.childrenIds = [];
        }
        for (const node of this.nodes.values()) {
            if (node.parentId) {
                const parent = this.nodes.get(node.parentId);
                if (parent && !parent.childrenIds.includes(node.id)) {
                    parent.childrenIds.push(node.id);
                }
            }
        }
    }

    public clear() {
        this.nodes.clear();
        this.searchIndex.clear();
    }

    public searchNodes(query: string): NodeData[] {
        const results: NodeData[] = [];
        const qTokens = this.tokenize(query);
        if (qTokens.length === 0 && query.length >= 36) { // Exact UUID
            const exactNode = this.nodes.get(query);
            if (exactNode) return [exactNode];
        }

        // Fast Inverted Index Search
        const matchingIds = new Set<string>();
        for (const token of qTokens) {
            for (const [key, ids] of this.searchIndex.entries()) {
                if (key.includes(token)) {
                    for (const id of ids) {
                        matchingIds.add(id);
                    }
                }
            }
        }

        for (const id of matchingIds) {
            const n = this.nodes.get(id);
            if (n) results.push(n);
        }
        return results;
    }
}
