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
    const saglik = await env.refreshBaseUrl(findProjectRoot());
    if (!saglik) return false;
    uyarSurumUyusmazligi(saglik);
    return true;
}

/** Sürüm uyuşmazlığını bir kez bildirir (sessizce garip davranmasın). */
let surumUyarisiVerildi = false;
function uyarSurumUyusmazligi(saglik: any) {
    if (surumUyarisiVerildi || !saglik?.version) return;
    const kendi = vscode.extensions.getExtension('Syncix.syncix-vscode')?.packageJSON?.version;
    if (!kendi) return;
    const mm = (v: string) => v.split('.').slice(0, 2).join('.');
    if (mm(kendi) !== mm(saglik.version)) {
        surumUyarisiVerildi = true;
        vscode.window.showWarningMessage(
            `Syncix version mismatch — extension ${kendi}, core ${saglik.version}. ` +
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
 * Çalışma dizini her zaman sync klasörünün ÜST dizini olur; core "../src_workspace"
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

/** Sync klasörünün üst dizinini bulur (syncix.toml veya src_workspace'e göre). */
function findSyncWorkspaceRoot(): string | undefined {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders) return undefined;
    for (const f of folders) {
        const p = f.uri.fsPath;
        if (fs.existsSync(path.join(p, 'syncix.toml'))) return p;
        // src_workspace klasörü açıksa üst dizini proje köküdür
        if (path.basename(p).toLowerCase() === 'src_workspace') return path.dirname(p);
    }
    return undefined;
}

/** Core Engine çalışmıyorsa otomatik başlatır (kullanım basitleştirme). */
async function ensureCoreRunning(): Promise<void> {
    const cfg = vscode.workspace.getConfiguration('syncix');
    if (!cfg.get<boolean>('autoStartCore', true)) return;

    const paths = resolveCorePaths();
    if (!paths) return;
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
        // projeyi dogruladigi icin, dogru adres ancak kendi core'umuz acildiktan
        // sonra olusuyor.
        const baslangic = Date.now();
        while (Date.now() - baslangic < 10_000) {
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
 * çalışıyordu. Artık yalnızca kendi başlattığımız süreç, PID ile durduruluyor.
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
 * ("src_workspace", "projectsyncix") — bunlar bu deponun kendi klasör
 * isimleriydi. Sonucu şuydu: kendi makinemizde her şey çalışıyor, projesine
 * "MyGame" adını veren herkeste eklenti sessizce hiçbir şey yapmıyordu.
 * Kurulumu kendi makinende denemenin neden yetmediğinin iyi bir örneği.
 */
async function isSyncixWorkspace(): Promise<boolean> {
    return projeDosyasi() !== undefined;
}

/** Çalışma alanındaki syncix.toml'un yolu, yoksa undefined. */
function projeDosyasi(): string | undefined {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) return undefined;
    for (const folder of folders) {
        const aday = path.join(folder.uri.fsPath, 'syncix.toml');
        if (fs.existsSync(aday)) return aday;
    }
    return undefined;
}

/**
 * Açık klasörde yeni bir Syncix projesi başlatır.
 *
 * Bu komut olmadan yeni kullanıcı kapalı bir döngüde kalıyordu: eklenti
 * yalnızca syncix.toml varsa çalışıyor, syncix.toml'u da yalnızca CLI
 * yaratabiliyor, CLI ise eklenti çalışınca kuruluyordu.
 */
async function projeOlustur(context: vscode.ExtensionContext): Promise<void> {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) {
        vscode.window.showErrorMessage('Open a folder first, then create the Syncix project inside it.');
        return;
    }

    const kok = folders[0].uri.fsPath;
    const tomlYolu = path.join(kok, 'syncix.toml');
    if (fs.existsSync(tomlYolu)) {
        vscode.window.showInformationMessage('This folder already has a syncix.toml.');
        return;
    }

    const syncDir = await vscode.window.showInputBox({
        prompt: 'Folder that will mirror the Roblox tree',
        value: 'src',
        validateInput: (v) => (v && v.trim().length > 0 ? undefined : 'A name is required'),
    });
    if (!syncDir) return;   // kullanıcı vazgeçti

    const ornek = path.join(context.extensionPath, 'resources', 'syncix.example.toml');
    let icerik: string;
    if (fs.existsSync(ornek)) {
        // Örnek dosya her ayarı açıklıyor; yeni kullanıcının neyi
        // değiştirebileceğini görmesi için olduğu gibi veriliyor.
        icerik = fs.readFileSync(ornek, 'utf8').replace(
            /^sync_dir = ".*"$/m,
            `sync_dir = "${syncDir.trim()}"`
        );
    } else {
        icerik = `[files]
sync_dir = "${syncDir.trim()}"
`;
    }

    try {
        fs.writeFileSync(tomlYolu, icerik);
        fs.mkdirSync(path.join(kok, syncDir.trim()), { recursive: true });
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

    const doc = await vscode.workspace.openTextDocument(tomlYolu);
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
 * CLI'ı kullanıcının bin klasörüne kurar ve (ilk seferde) PATH'e ekler.
 * Böylece "syncix" komutu her terminalde/cmd'de çalışır; elle kurulum gerekmez.
 */
function ensureCliInstalled(context: vscode.ExtensionContext): string | undefined {
    try {
        // CLI artık ayrı bir PowerShell betiği değil, core binary'sinin kendisi:
        // `syncix-core <komut>` istemci gibi çalışır. Böylece macOS'ta da çalışır ve
        // değer dönüşümü (Vector3, hex renk) CLI ile core arasında ayrışamaz.
        const paths = resolveCorePaths();
        if (!paths) return undefined;
        const exe = paths.exePath;

        const binDir = env.userBinDir();
        if (!binDir) return undefined;
        fs.mkdirSync(binDir, { recursive: true });

        if (env.isWindows()) {
            const cmdPath = path.join(binDir, 'syncix.cmd');
            const desired = `@echo off\r\n"${exe}" %*\r\n`;
            const current = fs.existsSync(cmdPath) ? fs.readFileSync(cmdPath, 'utf8') : '';
            if (current !== desired) fs.writeFileSync(cmdPath, desired, 'ascii');

            // PATH'e ekleme yalnızca bir kez (kullanıcı PATH'ini her açılışta kurcalamayalım)
            if (context.globalState.get<string>('syncix.cliPathAdded') !== 'v2') {
                const b = binDir.replace(/\\/g, '\\\\');
                const psCmd = `$b='${b}'; $p=[Environment]::GetEnvironmentVariable('PATH','User'); if($p -notlike ('*'+$b+'*')){[Environment]::SetEnvironmentVariable('PATH', ($p+';'+$b), 'User')}`;
                spawn('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', psCmd], {
                    detached: true,
                    stdio: 'ignore',
                }).unref();
                context.globalState.update('syncix.cliPathAdded', 'v2');
            }
            return cmdPath;
        }

        // macOS / Linux: ~/.local/bin genelde zaten PATH'te olur.
        const shPath = path.join(binDir, 'syncix');
        const desired = `#!/bin/sh\nexec "${exe}" "$@"\n`;
        const current = fs.existsSync(shPath) ? fs.readFileSync(shPath, 'utf8') : '';
        if (current !== desired) {
            fs.writeFileSync(shPath, desired, 'utf8');
            fs.chmodSync(shPath, 0o755);
        }
        return shPath;
    } catch (err) {
        console.error('Syncix CLI install error:', err);
        return undefined;
    }
}

export async function activate(context: vscode.ExtensionContext) {
    console.log('Syncix Extension Activated');
    context_extensionPath = context.extensionPath;

    // 0. Yalnızca Syncix çalışma alanında otomatik core başlat + bağlan.
    // Diğer projelerde her şey pasif kalır; istenirse "Syncix: Start" komutu ile elle bağlanılır.
    const autoMode = await isSyncixWorkspace();
    // Kenar çubuğundaki karşılama ekranı bu bağlama bakıyor: proje yoksa
    // kullanıcıya boş bir ağaç değil, "proje oluştur" düğmesi gösterilir.
    await vscode.commands.executeCommand('setContext', 'syncix.hasProject', autoMode);
    if (autoMode) {
        ensurePluginInstalled(context); // Studio plugini'ni otomatik kur/güncelle
        ensureCliInstalled(context);    // "syncix" komutunu otomatik kur (PATH bir kez)
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
        const saglikYokla = async () => {
            try {
                const saglik = await env.probe(
                    parseInt(new URL(env.getBaseUrl()).port || '8080', 10),
                    1200
                );
                if (saglik) {
                    studioConnected = saglik.studio_connected === true;
                    conflictCount = saglik.conflicts ?? 0;
                    if (typeof saglik.object_count === 'number') {
                        objectCount = saglik.object_count;
                    }
                } else {
                    studioConnected = false;
                }
            } catch {
                studioConnected = false;
            }
            updateStatusBar();
        };
        const saglikZamanlayici = setInterval(saglikYokla, 5000);
        context.subscriptions.push({ dispose: () => clearInterval(saglikZamanlayici) });
        saglikYokla();
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
        const senkronKoku = findSyncWorkspaceRoot() ?? findProjectRoot();
        const kaydetIzleyici = vscode.workspace.onDidSaveTextDocument((belge) => {
            if (!senkronKoku) return;
            const yol = belge.uri.fsPath;
            const senkronKlasoru = path.join(senkronKoku, 'src_workspace');
            if (!yol.startsWith(senkronKlasoru)) return;

            const ad = path.basename(yol);
            if (!studioConnected) {
                vscode.window.showWarningMessage(
                    `Saved ${ad}, but Roblox Studio is not connected — the change did not reach Studio.`
                );
                return;
            }
            vscode.window.setStatusBarMessage(`$(check) ${ad} → Studio`, 2500);
        });
        context.subscriptions.push(kaydetIzleyici);

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
            const cakismaNotu = conflictCount > 0 ? `  —  ${conflictCount} conflict(s)` : '';
            vscode.window.showInformationMessage(
                `Syncix connected  —  ${objectCount} instances in sync${cakismaNotu}`
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
    const syncDir = projRoot ? path.join(projRoot, 'src_workspace') : '';

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
        vscode.commands.registerCommand('syncix.initProject', () => projeOlustur(context)),
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
        vscode.commands.registerCommand('syncix.installCliGlobal', () => {
            // Bir kez kurulduktan sonra PATH'i tekrar kurcalamamak için globalState
            // sıfırlanır; kullanıcı bu komutu bilerek çağırdıysa yeniden kurulmalı.
            context.globalState.update('syncix.cliPathAdded', undefined);
            const kuruldu = ensureCliInstalled(context);
            if (kuruldu) {
                vscode.window.showInformationMessage(
                    `Syncix CLI installed (${kuruldu}). Open a new terminal — "syncix" works anywhere.`
                );
            } else {
                vscode.window.showErrorMessage(
                    'Could not install the CLI: core binary not found. Try Syncix: Restart Core first.'
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
