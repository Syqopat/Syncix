import * as vscode from 'vscode';
import * as http from 'http';
import * as fs from 'fs';
import * as path from 'path';
import { spawn } from 'child_process';
import { RpcManager } from './rpc/manager';
import { WorkspaceExplorer } from './explorer/WorkspaceExplorer';
import { PropertyInspector } from './inspector/PropertyInspector';
import { EditorStateManager } from './state/EditorStateManager';

import { DiagnosticsPanel } from './diagnostics/DiagnosticsPanel';
import * as env from './core/env';

let rpcClient: RpcManager;

/** Extension tarafından başlatılan core süreci (durdurma/yeniden başlatma için). */
let coreProcessPid: number | undefined;

/**
 * Core Engine ayakta mı? Aynı zamanda adresi günceller: port artık sabit değil,
 * core dolu portu atlayıp bir sonrakine geçebiliyor.
 */
async function checkCoreHealth(): Promise<boolean> {
    const health = await env.refreshBaseUrl(findProjectRoot());
    if (!health) return false;
    warnVersionMismatch(health);
    return true;
}

/** Sürüm uyuşmazlığını bir kez bildirir (sessizce garip davranmasın). */
let versionWarningShown = false;
/**
 * Eklentinin ownVersion sürümü; activate'te context.extension'dan doldurulur.
 *
 * Eskiden getExtension('Syncix.syncix-vscode') ile aranıyordu. O kimlik hiçbir
 * zaman gerçek değildi (yayıncı hiç "Syncix" olmadı), arama her seferinde
 * undefined döndü ve sürüm uyuşmazlığı uyarısı bugüne kadar hiç tetiklenmedi.
 * Kimliği koda yazmak yerine eklentinin kendisinden okuyoruz; yayıncı ya da ad
 * değişse de doğru kalıyor.
 */
let extensionVersion = '';
function warnVersionMismatch(health: any) {
    if (versionWarningShown || !health?.version) return;
    const ownVersion = extensionVersion;
    if (!ownVersion) return;
    const mm = (v: string) => v.split('.').slice(0, 2).join('.');
    if (mm(ownVersion) !== mm(health.version)) {
        versionWarningShown = true;
        vscode.window.showWarningMessage(
            `Syncix version mismatch — extension ${ownVersion}, core ${health.version}. ` +
            `The same major.minor is required; rebuild the core or update the extension.`
        );
    }
}

/**
 * Proje kökünü bulur (taşınabilirlik): açık klasörden yukarı doğru core-engine arar.
 * Ayar verilmişse o öncelikli; hiçbiri yoksa undefined.
 */
function findProjectRoot(): string | undefined {
    const cfgRoot = vscode.workspace.getConfiguration('syncix').get<string>('coreCwd', '');
    if (cfgRoot && fs.existsSync(cfgRoot)) return path.dirname(cfgRoot);

    const folders = vscode.workspace.workspaceFolders;
    if (!folders) return undefined;
    for (const f of folders) {
        let dir = f.uri.fsPath;
        for (let i = 0; i < 5; i++) {
            if (fs.existsSync(path.join(dir, 'core-engine'))) return dir;
            const up = path.dirname(dir);
            if (up === dir) break;
            dir = up;
        }
    }
    return undefined;
}

/**
 * Core exe'nin yolunu çözer. Sıra:
 *   1) Kullanıcı ayarı (varsa)
 *   2) Proje kökündeki geliştirme derlemesi (repo içinde çalışırken)
 *   3) Extension'a gömülü binary (son kullanıcı senaryosu — repo gerekmez)
 * Çalışma dizini her zaman sync klasörünün ÜST dizini olur; core "../<sync_dir>"
 * beklediği için gömülü binary çalışırken de doğru klasörü bulur.
 */
function resolveCorePaths(): { exePath: string; cwd: string } | undefined {
    const cfg = vscode.workspace.getConfiguration('syncix');
    const cfgExe = cfg.get<string>('coreExePath', '');
    const cfgCwd = cfg.get<string>('coreCwd', '');
    if (cfgExe && fs.existsSync(cfgExe) && cfgCwd && fs.existsSync(cfgCwd)) {
        return { exePath: cfgExe, cwd: cfgCwd };
    }

    const root = findProjectRoot();
    if (root) {
        const devCwd = path.join(root, 'core-engine');
        const devExe = path.join(devCwd, 'target', 'release', env.coreBinaryName());
        if (fs.existsSync(devExe)) return { exePath: devExe, cwd: devCwd };
    }

    // Gömülü binary: platforma uygun olanı seçilir (Windows/macOS ayrı klasörler).
    const bundled = env.bundledCorePaths(context_extensionPath).find((p) => fs.existsSync(p));
    if (bundled) {
        const projRoot = root ?? findSyncWorkspaceRoot();
        if (projRoot) {
            const cwd = path.join(projRoot, '.syncix');
            try {
                fs.mkdirSync(cwd, { recursive: true });
                // Windows dışında paketten çıkan dosya çalıştırılabilir olmayabilir.
                if (!env.isWindows()) fs.chmodSync(bundled, 0o755);
            } catch {
                /* yoksay */
            }
            return { exePath: bundled, cwd };
        }
    }
    return undefined;
}

/** Extension yolu (activate'te doldurulur; resolveCorePaths gömülü binary için kullanır). */
let context_extensionPath = '';

/** Platform uyarısı oturumda bir kez gösterilir; her denemede tekrarlanmamalı. */
let platformWarningShown = false;

/** Sync klasörünün üst dizinini bulur (syncix.toml'a göre). */
function findSyncWorkspaceRoot(): string | undefined {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders) return undefined;
    for (const f of folders) {
        const p = f.uri.fsPath;
        if (fs.existsSync(path.join(p, 'syncix.toml'))) return p;
        // Sync klasörünün kendisi açıksa proje kökü bir üst dizindir
        if (fs.existsSync(path.join(path.dirname(p), 'syncix.toml'))) return path.dirname(p);
    }
    return undefined;
}

/** Core Engine çalışmıyorsa otomatik başlatır (kullanım basitleştirme). */
async function ensureCoreRunning(): Promise<void> {
    const cfg = vscode.workspace.getConfiguration('syncix');
    if (!cfg.get<boolean>('autoStartCore', true)) return;

    const paths = resolveCorePaths();
    if (!paths) {
        // Bu surumde yalnizca Windows binary'si paketleniyor.
        //
        // Sessizce cikmak en kotusuydu: macOS'ta eklenti kuruluyor, agac
        // acilmiyor, hicbir yerde sebep yazmiyordu. Neyin eksik oldugunu
        // soylemek, calismiyor olmaktan daha iyi degil ama tesis edilebilir.
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

    if (await checkCoreHealth()) return; // Zaten çalışıyor

    try {
        const child = spawn(exePath, [], { cwd, detached: true, stdio: 'ignore' });
        child.on('error', (err) => {
            vscode.window.showErrorMessage(`Could not start the Syncix core: ${err.message}`);
        });
        // PID saklanıyor: durdurma işlemi eskiden isimden öldürüyordu (taskkill /IM),
        // bu da AÇIK OLAN DİĞER PROJELERİN core'unu da kapatıyordu.
        coreProcessPid = child.pid;
        child.unref();

        // Core'un AYAGA KALKMASINI bekle.
        //
        // Beklemeden devam edilirse rpcClient.connect() hala eski adrese gider.
        // Baska bir projenin core'u 8080'i tutuyorsa bu adres onu gosteriyor
        // demektir; yani baglanti yanlis oyuna kurulur. refreshBaseUrl artik
        // projeyi dogruladigi icin, dogru adres ancak ownVersion core'umuz acildiktan
        // sonra olusuyor.
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
 * Bu pencerenin başlattığı core'u durdurur.
 *
 * Eskiden burada `taskkill /IM syncix-core.exe /F` çalışıyordu: isimden öldürdüğü
 * için AÇIK OLAN BÜTÜN projelerin core'unu kapatıyordu ve yalnızca Windows'ta
 * çalışıyordu. Artık yalnızca ownVersion başlattığımız süreç, PID ile durduruluyor.
 */
function stopCore(): boolean {
    if (!coreProcessPid) return false;
    try {
        process.kill(coreProcessPid);
        coreProcessPid = undefined;
        return true;
    } catch {
        // Süreç zaten ölmüş olabilir.
        coreProcessPid = undefined;
        return false;
    }
}

/** Açık klasör Syncix çalışma alanı mı? (alakasız projelerde otomatik başlatma yapılmaz) */
/**
 * Bir Syncix projesi mi?
 *
 * Ölçüt syncix.toml'un varlığı. Daha önce klasör ADINA bakılıyordu
 * ("src_workspace", "projectsyncix") — bunlar bu deponun ownVersion eski klasör
 * isimleriydi. Sonucu şuydu: ownVersion makinemizde her şey çalışıyor, projesine
 * "MyGame" adını veren herkeste eklenti sessizce hiçbir şey yapmıyordu.
 * Kurulumu ownVersion makinende denemenin neden yetmediğinin iyi bir örneği.
 */
async function isSyncixWorkspace(): Promise<boolean> {
    return projectFile() !== undefined;
}

/** Çalışma alanındaki syncix.toml'un yolu, yoksa undefined. */
function projectFile(): string | undefined {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) return undefined;
    for (const folder of folders) {
        const candidate = path.join(folder.uri.fsPath, 'syncix.toml');
        if (fs.existsSync(candidate)) return candidate;
    }
    return undefined;
}

/**
 * Projenin sync klasörünün ADI (syncix.toml'daki `sync_dir`).
 *
 * Daha önce bu ad kodda sabitti ("src_workspace") — bu deponun ownVersion klasör
 * adı. Sonuç: klasörüne başka bir ad veren herkeste kayıt geri bildirimi ve
 * "sync klasörünü aç" komutu sessizce yanlış yolu gösteriyordu.
 */
function syncFolderName(): string {
    const toml = projectFile();
    if (toml) {
        try {
            const m = /^\s*sync_dir\s*=\s*"([^"]+)"/m.exec(fs.readFileSync(toml, 'utf8'));
            if (m) return m[1];
        } catch {
            // okunamiyorsa varsayilana dus
        }
    }
    return 'src';
}

/**
 * Açık klasörde yeni bir Syncix projesi başlatır.
 *
 * Bu komut olmadan yeni kullanıcı kapalı bir döngüde kalıyordu: eklenti
 * yalnızca syncix.toml varsa çalışıyor, syncix.toml'u da yalnızca CLI
 * yaratabiliyor, CLI ise eklenti çalışınca kuruluyordu.
 */
async function createProject(context: vscode.ExtensionContext): Promise<void> {
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
    if (!syncDir) return;   // kullanıcı vazgeçti

    const sampleFile = path.join(context.extensionPath, 'resources', 'syncix.example.toml');
    let fileText: string;
    if (fs.existsSync(sampleFile)) {
        // Örnek dosya her ayarı açıklıyor; yeni kullanıcının neyi
        // değiştirebileceğini görmesi için olduğu gibi veriliyor.
        fileText = fs.readFileSync(sampleFile, 'utf8').replace(
            /^sync_dir = ".*"$/m,
            `sync_dir = "${syncDir.trim()}"`
        );
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

    // Proje artık var; kurulum adımlarını hemen çalıştır ki kullanıcı
    // pencereyi yeniden yüklemek zorunda kalmasın.
    ensurePluginInstalled(context);
    ensureCliInstalled(context);
    await ensureCoreRunning();
    rpcClient?.connect();
    await vscode.commands.executeCommand('setContext', 'syncix.hasProject', true);

    const doc = await vscode.workspace.openTextDocument(tomlPath);
    await vscode.window.showTextDocument(doc);
    vscode.window.showInformationMessage(
        'Syncix project created. Open Roblox Studio — the plugin is installed and the core is running.'
    );
}

/**
 * Studio plugini'ni (extension'a gömülü .rbxm) otomatik olarak Roblox Plugins
 * klasörüne kurar. Yalnızca içerik farklıysa kopyalar ve o zaman "Studio'yu yeniden
 * başlat" der. Böylece kullanıcı plugini elle kurmaz, sürüm karmaşası yaşamaz.
 */
function ensurePluginInstalled(context: vscode.ExtensionContext) {
    try {
        const src = path.join(context.extensionPath, 'resources', 'SyncixPlugin.rbxm');
        if (!fs.existsSync(src)) return;

        // Plugin klasörü platforma göre değişir (Windows: LOCALAPPDATA, macOS: Documents).
        // Linux'ta resmi Studio olmadığı için undefined döner ve kurulum atlanır.
        const pluginsDir = env.robloxPluginsDir();
        if (!pluginsDir) return;
        if (!fs.existsSync(pluginsDir)) {
            fs.mkdirSync(pluginsDir, { recursive: true });
        }
        const dest = path.join(pluginsDir, 'SyncixPlugin.rbxm');

        const srcBuf = fs.readFileSync(src);
        let needsCopy = true;
        if (fs.existsSync(dest)) {
            needsCopy = !srcBuf.equals(fs.readFileSync(dest));
        }
        if (needsCopy) {
            fs.writeFileSync(dest, srcBuf);
            vscode.window.showInformationMessage(
                'The Syncix Studio plugin was updated. Restart Roblox Studio for it to take effect.'
            );
        }
    } catch (err: any) {
        console.error('Syncix plugin install error:', err);
    }
}

/**
 * `syncix` komutunu editörün ownVersion terminallerine ekler.
 *
 * Eskiden bu fonksiyon kullanıcının ev klasörüne bir .cmd yazıyor ve gizli,
 * ayrık bir kabuk süreciyle kullanıcının PATH'ini kayıt defterinde
 * değiştiriyordu — kimse sormadan, her proje açılışında. Bu, rıza dışı
 * kalıcı bir sistem değişikliğiydi.
 *
 * Artık kısayol eklentinin ownVersion depolama klasörüne yazılıyor ve PATH'e
 * yalnızca VS Code API'si üzerinden, editörün açtığı terminaller için
 * ekleniyor. Kayıt defterine, kullanıcı klasörüne ya da sistem PATH'ine
 * dokunulmuyor; eklenti kaldırılınca geride iz kalmıyor.
 */
function ensureCliInstalled(context: vscode.ExtensionContext): string | undefined {
    try {
        // CLI ayrı bir betik değil, core binary'sinin kendisi: `syncix-core <komut>`
        // istemci gibi çalışır, değer dönüşümü CLI ile core arasında ayrışamaz.
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

        // Yalnızca editörün terminalleri etkilenir; sistem PATH'i değişmez.
        context.environmentVariableCollection.prepend('PATH', binDir + path.delimiter);
        return shim;
    } catch (err) {
        console.error('Syncix CLI setup error:', err);
        return undefined;
    }
}

export async function activate(context: vscode.ExtensionContext) {
    console.log('Syncix Extension Activated');
    context_extensionPath = context.extensionPath;
    extensionVersion = context.extension.packageJSON?.version ?? '';

    // 0. Yalnızca Syncix çalışma alanında otomatik core başlat + bağlan.
    // Diğer projelerde her şey pasif kalır; istenirse "Syncix: Start" komutu ile elle bağlanılır.
    const autoMode = await isSyncixWorkspace();
    // Kenar çubuğundaki karşılama ekranı bu bağlama bakıyor: proje yoksa
    // kullanıcıya boş bir ağaç değil, "proje oluştur" düğmesi gösterilir.
    await vscode.commands.executeCommand('setContext', 'syncix.hasProject', autoMode);
    if (autoMode) {
        ensurePluginInstalled(context); // Studio plugini'ni otomatik kur/güncelle
        ensureCliInstalled(context);    // "syncix" editörün terminallerinde (sistem PATH'i değişmez)
        await ensureCoreRunning();
    }

    // 1. Dependency Injection: RPC Client
    rpcClient = new RpcManager();

    // Auto-connect on startup
    if (autoMode) {
        rpcClient.connect();
    }

    // 1b. Görsel durum çubuğu — "acaba çalışıyor mu" derdini bitirir.
    if (autoMode) {
        const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
        statusBar.command = 'syncix.showStatus';
        let objectCount = 0;
        let connected = false;

        // Studio'nun durumu AYRI izlenir.
        //
        // Eskiden durum çubuğu yalnızca editör-core bağlantısına bakıyordu: Studio
        // kapalıyken bile "Bağlı ✓" yazıyordu. Kullanıcı her şeyin senkron olduğunu
        // sanarken yaptığı hiçbir değişiklik Studio'ya ulaşmıyordu. Sessiz ve
        // yanıltıcı bir durumdu; artık üç hâl ayrı gösteriliyor.
        let studioConnected = false;
        let conflictCount = 0;

        const updateStatusBar = () => {
            if (!connected) {
                statusBar.text = `$(error) Syncix: core is down`;
                statusBar.tooltip = 'The Syncix core is not running. Click to start it and reconnect.';
                statusBar.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
                return;
            }
            if (!studioConnected) {
                statusBar.text = `$(warning) Syncix: Studio not connected`;
                statusBar.tooltip =
                    'The core is running but Roblox Studio is not connected.\n' +
                    'Changes you make right now are NOT reaching Studio.\n' +
                    'Open Studio; the Syncix plugin connects on its own.';
                statusBar.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
                return;
            }
            if (conflictCount > 0) {
                statusBar.text = `$(alert) Syncix: ${conflictCount} conflict(s)`;
                statusBar.tooltip =
                    `${conflictCount} change(s) conflicted and were overwritten.\n` +
                    'See the "Recent changes" tab in the Syncix panel inside Studio.';
                statusBar.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
                return;
            }
            statusBar.text = `$(check) Syncix: connected (${objectCount})`;
            statusBar.tooltip = `Syncix connected — ${objectCount} instances in sync.\nClick for status / reconnect.`;
            statusBar.backgroundColor = undefined;
        };

        // /health'i düzenli yoklayarak Studio durumunu ve çakışma sayısını izle.
        const pollHealth = async () => {
            try {
                const health = await env.probe(
                    parseInt(new URL(env.getBaseUrl()).port || '8080', 10),
                    1200
                );
                if (health) {
                    studioConnected = health.studio_connected === true;
                    conflictCount = health.conflicts ?? 0;
                    if (typeof health.object_count === 'number') {
                        objectCount = health.object_count;
                    }
                } else {
                    studioConnected = false;
                }
            } catch {
                studioConnected = false;
            }
            updateStatusBar();
        };
        const healthTimer = setInterval(pollHealth, 5000);
        context.subscriptions.push({ dispose: () => clearInterval(healthTimer) });
        pollHealth();
        updateStatusBar();
        statusBar.show();
        context.subscriptions.push(statusBar);

        rpcClient.onConnectionChange((c) => { connected = c; updateStatusBar(); });
        rpcClient.onMessage((msg) => {
            switch (msg.event_type) {
                case 'FULL_SYNC':
                case 'TREE_UPDATED':
                    objectCount = msg.data?.nodes?.length ?? objectCount;
                    break;
                case 'INSTANCE_CREATED':
                    objectCount++;
                    break;
                case 'INSTANCE_REMOVED':
                    objectCount = Math.max(0, objectCount - 1);
                    break;
            }
            updateStatusBar();
        });

        // KAYDETME GERİ BİLDİRİMİ
        //
        // Editör dosya kaydetmeyi hiç izlemiyordu: her şey core'un dosya izleyicisine
        // bırakılmıştı. Bir .lua dosyasını kaydettiğinde değişikliğin Studio'ya
        // ulaşıp ulaşmadığını anlamanın hiçbir yolu yoktu — Studio kapalıyken bile
        // kayıt sessizce hiçbir yere gitmiyordu. Artık her kayıtta durum söyleniyor.
        const syncRoot = findSyncWorkspaceRoot() ?? findProjectRoot();
        const saveWatcher = vscode.workspace.onDidSaveTextDocument((savedDoc) => {
            if (!syncRoot) return;
            const filePath = savedDoc.uri.fsPath;
            const syncFolder = path.join(syncRoot, syncFolderName());
            if (!filePath.startsWith(syncFolder)) return;

            const ad = path.basename(filePath);
            if (!studioConnected) {
                vscode.window.showWarningMessage(
                    `Saved ${ad}, but Roblox Studio is not connected — the change did not reach Studio.`
                );
                return;
            }
            vscode.window.setStatusBarMessage(`$(check) ${ad} → Studio`, 2500);
        });
        context.subscriptions.push(saveWatcher);

        const showStatusCmd = vscode.commands.registerCommand('syncix.showStatus', () => {
            if (!connected) {
                vscode.window.showWarningMessage('The Syncix core is not running. Starting it...');
                ensureCoreRunning().then(() => rpcClient.connect());
                return;
            }
            if (!studioConnected) {
                vscode.window.showWarningMessage(
                    'Roblox Studio is not connected — your changes are not reaching Studio. Open Studio.'
                );
                return;
            }
            const conflictNote = conflictCount > 0 ? `  —  ${conflictCount} conflict(s)` : '';
            vscode.window.showInformationMessage(
                `Syncix connected  —  ${objectCount} instances in sync${conflictNote}`
            );
        });
        context.subscriptions.push(showStatusCmd);
    }

    // 2. State Management & Recovery
    const stateManager = new EditorStateManager(rpcClient);
    
    // Restore selection from previous session
    const savedSelection = context.workspaceState.get<string[]>('syncix.selectedNodes', []);
    if (savedSelection.length > 0) {
        stateManager.setSelectedNodes(savedSelection);
    }

    // Save selection changes
    stateManager.onSelectionChanged((ids) => {
        context.workspaceState.update('syncix.selectedNodes', ids);
    });

    // 3. Commands Registration
    const startCmd = vscode.commands.registerCommand('syncix.start', () => {
        rpcClient.connect();
    });

    const stopCmd = vscode.commands.registerCommand('syncix.stop', () => {
        rpcClient.dispose();
        vscode.window.showInformationMessage('Syncix disconnected.');
    });

    // 4. Register Tree Data Provider
    const workspaceExplorer = new WorkspaceExplorer(context, stateManager, rpcClient as any);

    const inspectNodeCmd = vscode.commands.registerCommand('syncix.inspectNode', (node) => {
        PropertyInspector.createOrShow(rpcClient, stateManager, workspaceExplorer);
        if (node) {
            PropertyInspector.currentPanel?.inspect(node);
        }
    });

    const diagnosticsCmd = vscode.commands.registerCommand('syncix.openDiagnostics', () => {
        DiagnosticsPanel.createOrShow(rpcClient);
    });

    // ── Komut Paleti (Ctrl+Shift+P) aksiyonları ──
    const cfg = vscode.workspace.getConfiguration('syncix');
    const projRoot = findProjectRoot() ?? '';
    const syncDir = projRoot ? path.join(projRoot, syncFolderName()) : '';

    const paletteCmds = [
        vscode.commands.registerCommand('syncix.reconnect', async () => {
            await ensureCoreRunning();
            rpcClient.connect();
            vscode.window.setStatusBarMessage('Syncix: reconnecting...', 3000);
        }),
        vscode.commands.registerCommand('syncix.stopCore', () => {
            if (stopCore()) {
                vscode.window.showInformationMessage('Syncix core stopped.');
            } else {
                vscode.window.showWarningMessage(
                    'No Syncix core started by this window was found. ' +
                    'If another window started it, stop it from there.'
                );
            }
        }),
        vscode.commands.registerCommand('syncix.restartCore', async () => {
            stopCore();
            setTimeout(() => ensureCoreRunning().then(() => rpcClient.connect()), 1200);
            vscode.window.showInformationMessage('Restarting the Syncix core...');
        }),
        vscode.commands.registerCommand('syncix.initProject', () => createProject(context)),
        vscode.commands.registerCommand('syncix.installPlugin', () => {
            ensurePluginInstalled(context);
            vscode.window.showInformationMessage('Checked the Syncix plugin. If it changed, restart Roblox Studio.');
        }),
        vscode.commands.registerCommand('syncix.openInspector', () => {
            PropertyInspector.createOrShow(rpcClient, stateManager, workspaceExplorer);
        }),
        vscode.commands.registerCommand('syncix.refresh', () => {
            workspaceExplorer.refresh();
            rpcClient.send('GET_TREE', {});
        }),
        vscode.commands.registerCommand('syncix.openWorkspaceFolder', () => {
            if (syncDir) {
                vscode.commands.executeCommand('revealFileInOS', vscode.Uri.file(syncDir));
            }
        }),
        vscode.commands.registerCommand('syncix.installCliGlobal', async () => {
            // Sistem PATH'i kalıcı bir kullanıcı ayarı; onu eklenti DEĞİŞTİRMİYOR.
            // Klasörü verip nasıl ekleneceğini söylüyoruz, kararı kullanıcı veriyor.
            const shim = ensureCliInstalled(context);
            if (!shim) {
                vscode.window.showErrorMessage(
                    'Could not set up the CLI: core binary not found. Try Syncix: Restart Core first.'
                );
                return;
            }
            const dir = path.dirname(shim);
            const choice = await vscode.window.showInformationMessage(
                `"syncix" already works in this editor's terminals. Syncix does not change your system PATH. To use the command in other terminals too, add this folder to your PATH yourself:

${dir}`,
                { modal: true },
                'Copy Folder Path'
            );
            if (choice === 'Copy Folder Path') {
                await vscode.env.clipboard.writeText(dir);
                vscode.window.showInformationMessage(
                    'Folder path copied. On Windows: Start, search "Edit environment variables for your account", select Path, New, paste.'
                );
            }
        }),
        vscode.commands.registerCommand('syncix.selfTest', async () => {
            const paths = resolveCorePaths();
            if (!paths) {
                vscode.window.showErrorMessage('Core binary not found.');
                return;
            }
            const terminal = vscode.window.createTerminal('Syncix Selftest');
            terminal.show();
            terminal.sendText(`"${paths.exePath}" selftest`);
        }),
    ];

    context.subscriptions.push(startCmd, stopCmd, inspectNodeCmd, diagnosticsCmd, ...paletteCmds);
    context.subscriptions.push({ dispose: () => rpcClient.dispose() });
}

export function deactivate() {
    if (rpcClient) {
        rpcClient.dispose();
    }
}
