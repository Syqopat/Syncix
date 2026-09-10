/**
 * Ortam çözümlemesi: core'un adresi ve platforma göre dosya yolları.
 *
 * Neden ayrı bir modül:
 *  - Adres eskiden dört ayrı dosyada 'http://127.0.0.1:8080' olarak sabit yazılıydı.
 *    Port artık değişebiliyor (core dolu portu atlıyor), dolayısıyla tek bir
 *    kaynaktan okunmak zorunda.
 *  - Windows varsayımları (.exe, powershell, taskkill) de aynı şekilde dağınıktı.
 *    Roblox Studio macOS'ta da çalışıyor; bu varsayımlar burada tek yerde toplandı.
 */

import * as fs from 'fs';
import * as http from 'http';
import * as path from 'path';

const DEFAULT_PORT = 8080;
const PORT_RANGE = 10;

let baseUrl = `http://127.0.0.1:${DEFAULT_PORT}`;

/** Core'un HTTP adresi (ör. http://127.0.0.1:8081). */
export function getBaseUrl(): string {
    return baseUrl;
}

/** Core'un WebSocket RPC adresi. */
export function getWsUrl(): string {
    return baseUrl.replace(/^http/, 'ws') + '/rpc';
}

/** Belirli bir portta Syncix core var mı? Varsa /health cevabını döndürür. */
export function probe(port: number, timeoutMs = 1200): Promise<any | undefined> {
    return new Promise((resolve) => {
        const req = http.get(`http://127.0.0.1:${port}/health`, (res) => {
            let bodyText = '';
            res.on('data', (c) => (bodyText += c));
            res.on('end', () => {
                try {
                    const j = JSON.parse(bodyText);
                    // Portta başka bir program olabilir; Syncix imzası aranır.
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

/** Core'un çalışırken yazdığı port dosyası: <proje>/.syncix/port */
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
 * İki filePath aynı projeyi mi gösteriyor?
 * Windows'ta büyük/küçük harf ayrımı yok; sondaki ayraç da fark etmemeli.
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
 * Çalışan core'u bulur ve adresi günceller.
 * Önce port dosyasına bakar (kesin bilgi), yoksa aralığı tarar.
 * Bulamazsa adres değişmez ve undefined döner.
 *
 * KRİTİK: bulunan core'un BU projeye ait olduğu doğrulanır.
 *
 * Eskiden yalnızca "portta sağlıklı bir Syncix var mı" diye bakılıyordu. İki
 * proje aynı anda açıkken ikincisi, birincisinin core'una bağlanıyordu: editör
 * "bağlı" diyor, ağaç görünüyor, ama yapılan her değişiklik BAŞKA bir oyuna
 * gidiyordu. Sessiz ve tehlikeli bir durumdu; /health zaten `root` alanını
 * taşıdığı için doğrulama bedava.
 */
export async function refreshBaseUrl(projectRoot?: string): Promise<any | undefined> {
    // Proje kökü bilinmiyorsa doğrulanacak bir şey yok; eski davranış korunur.
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

/** Çalıştırılabilir dosya adı (Windows'ta .exe uzantılı). */
export function coreBinaryName(): string {
    return isWindows() ? 'syncix-core.exe' : 'syncix-core';
}

/**
 * Gömülü binary'nin extension içindeki yolu.
 * Binary'ler platforma göre ayrı klasörlerde tutulur:
 *   resources/bin/win32-x64/syncix-core.exe
 *   resources/bin/darwin-arm64/syncix-core
 * Eski tek-dosya yerleşimi (resources/bin/syncix-core.exe) da destekleniyor ki
 * eski paketler bozulmasın.
 */
export function bundledCorePaths(extensionPath: string): string[] {
    const ad = coreBinaryName();
    return [
        path.join(extensionPath, 'resources', 'bin', `${process.platform}-${process.arch}`, ad),
        path.join(extensionPath, 'resources', 'bin', process.platform, ad),
        path.join(extensionPath, 'resources', 'bin', ad),
    ];
}

/** Roblox'un plugin klasörü (platforma göre). */
export function robloxPluginsDir(): string | undefined {
    if (isWindows()) {
        const local = process.env.LOCALAPPDATA;
        return local ? path.join(local, 'Roblox', 'Plugins') : undefined;
    }
    if (process.platform === 'darwin') {
        const home = process.env.HOME;
        // macOS'ta Studio plugin'leri kullanıcı Documents altında tutulur.
        return home ? path.join(home, 'Documents', 'Roblox', 'Plugins') : undefined;
    }
    // Linux'ta resmi Roblox Studio yok; plugin kurulumu atlanır.
    return undefined;
}

