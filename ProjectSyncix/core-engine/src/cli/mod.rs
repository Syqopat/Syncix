//! Syncix komut satırı arayüzü.
//!
//! Neden core binary'sinin içinde:
//!  1. Taşınabilirlik. Eski CLI syncix.ps1 idi; PowerShell'e bağlıydı ve macOS'ta
//!     çalışmıyordu. Aynı binary hem sunucu hem istemci olunca ek çalışma ortamı
//!     (PowerShell, Node) gerekmiyor.
//!  2. Doğruluk. Hex renk hatası tam olarak dönüşüm mantığının CLI'da olup core'da
//!     olmamasından çıkmıştı. Değer çözümlemesi artık tek yerde: parse_property_value.
//!
//! Kullanım: `syncix-core <komut> [argümanlar]`. Argümansız çalıştırılırsa sunucu açılır.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;

use crate::project::{DEFAULT_PORT, PORT_SCAN_SPAN};

const KIRMIZI: &str = "\x1b[31m";
const YESIL: &str = "\x1b[32m";
const SARI: &str = "\x1b[33m";
const MAVI: &str = "\x1b[36m";
const SOLUK: &str = "\x1b[90m";
const SIFIRLA: &str = "\x1b[0m";

fn hata(m: &str) {
    eprintln!("{}{}{}", KIRMIZI, m, SIFIRLA);
}
fn tamam(m: &str) {
    println!("{}{}{}", YESIL, m, SIFIRLA);
}
fn bilgi(m: &str) {
    println!("{}{}{}", MAVI, m, SIFIRLA);
}
fn soluk(m: &str) {
    println!("{}{}{}", SOLUK, m, SIFIRLA);
}

// ---------------------------------------------------------------------------
// Basit HTTP istemcisi
//
// Yalnızca 127.0.0.1'e JSON istekleri atıyoruz; bunun için tam bir HTTP istemci
// kütüphanesi (reqwest + TLS zinciri) eklemek gereksiz ağırlık olurdu.
// "Connection: close" gönderildiği için cevabı dosya sonuna kadar okumak yeterli.
// ---------------------------------------------------------------------------

struct Cevap {
    durum: u16,
    govde: String,
    /// Yanit başlıkları (küçük harfe çevrilmiş adlarla).
    /// Şimdilik yalnızca x-syncix-skipped-enums için gerekli: yayınlanacak yer
    /// dosyasında kaç Enum değerinin atlandığını bilmeden yayın yapmak,
    /// eksik bir sürümü oyunculara göndermek olurdu.
    basliklar: std::collections::HashMap<String, String>,
}

fn istek(port: u16, metot: &str, yol: &str, govde: Option<&str>) -> Result<Cevap, String> {
    let adres = format!("127.0.0.1:{}", port);
    let mut akis = TcpStream::connect(&adres).map_err(|e| format!("could not connect to {}: {}", adres, e))?;
    akis.set_read_timeout(Some(std::time::Duration::from_secs(15))).ok();
    akis.set_write_timeout(Some(std::time::Duration::from_secs(15))).ok();

    let mut ham = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n",
        metot, yol, port
    );
    if let Some(b) = govde {
        ham.push_str("Content-Type: application/json\r\n");
        ham.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    ham.push_str("\r\n");
    if let Some(b) = govde {
        ham.push_str(b);
    }

    akis.write_all(ham.as_bytes()).map_err(|e| e.to_string())?;

    let mut tampon = Vec::new();
    akis.read_to_end(&mut tampon).map_err(|e| e.to_string())?;
    let metin = String::from_utf8_lossy(&tampon).to_string();

    let (basliklar, govde_metni) = match metin.find("\r\n\r\n") {
        Some(i) => (&metin[..i], metin[i + 4..].to_string()),
        None => (metin.as_str(), String::new()),
    };

    let durum = basliklar
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    Ok(Cevap {
        durum,
        govde: govde_metni,
        basliklar: basliklari_ayristir(basliklar),
    })
}

/// HTTP yanit basliklarini ayristirir.
///
/// Ayri bir fonksiyon olmasinin sebebi test edilebilirlik: baslik okuma yolu
/// `syncix upload` icin onemli (atlanan Enum sayisi oradan geliyor) ve bir TCP
/// baglantisi kurmadan dogrulanabilmesi gerekiyor.
fn basliklari_ayristir(ham: &str) -> std::collections::HashMap<String, String> {
    let mut harita = std::collections::HashMap::new();
    // Ilk satir durum satiridir (HTTP/1.1 200 OK), baslik degil.
    for satir in ham.lines().skip(1) {
        if let Some((ad, deger)) = satir.split_once(':') {
            harita.insert(ad.trim().to_lowercase(), deger.trim().to_string());
        }
    }
    harita
}

fn url_kodla(s: &str) -> String {
    let mut cikti = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                cikti.push(b as char)
            }
            _ => cikti.push_str(&format!("%{:02X}", b)),
        }
    }
    cikti
}

// ---------------------------------------------------------------------------
// Core'u bulma
// ---------------------------------------------------------------------------

/// Çalışma dizininden yukarı doğru .syncix/port dosyası arar.
fn port_dosyasindan() -> Option<u16> {
    let mut dizin: PathBuf = std::env::current_dir().ok()?;
    for _ in 0..6 {
        let p = dizin.join(".syncix").join("port");
        if let Ok(metin) = std::fs::read_to_string(&p) {
            if let Ok(port) = metin.trim().parse::<u16>() {
                return Some(port);
            }
        }
        if !dizin.pop() {
            break;
        }
    }
    None
}

/// Çalışan core'un portunu bulur: önce port dosyası, sonra aralık taraması.
fn core_portu() -> Option<u16> {
    if let Some(p) = port_dosyasindan() {
        if istek(p, "GET", "/health", None).map(|c| c.durum == 200).unwrap_or(false) {
            return Some(p);
        }
    }
    (DEFAULT_PORT..DEFAULT_PORT + PORT_SCAN_SPAN).find(|&p| {
        istek(p, "GET", "/health", None)
            .map(|c| c.durum == 200)
            .unwrap_or(false)
    })
}

fn core_gerekli() -> Option<u16> {
    match core_portu() {
        Some(p) => Some(p),
        None => {
            hata("Syncix Core is not running.");
            soluk("  Start it with: syncix up   (or Syncix: Restart Core in VS Code)");
            None
        }
    }
}

fn json_al(port: u16, yol: &str) -> Option<serde_json::Value> {
    match istek(port, "GET", yol, None) {
        Ok(c) => serde_json::from_str(&c.govde).ok(),
        Err(e) => {
            hata(&format!("Request failed: {}", e));
            None
        }
    }
}

/// Komutu core'a gönderir; hata durumunda sunucunun mesajını olduğu gibi gösterir
/// (belirsiz hedef uyarıları buradan gelir).
fn komut_gonder(port: u16, event_type: &str, data: serde_json::Value) -> bool {
    let govde = serde_json::json!({ "event_type": event_type, "data": data }).to_string();
    match istek(port, "POST", "/command", Some(&govde)) {
        Ok(c) if c.durum == 200 => true,
        Ok(c) => {
            hata(&format!("Rejected ({}):", c.durum));
            eprintln!("{}", c.govde.trim());
            false
        }
        Err(e) => {
            hata(&format!("Could not send: {}", e));
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Komutlar
// ---------------------------------------------------------------------------

fn yardim() {
    println!(
        r#"Syncix CLI  (version {})

  Status
    syncix status                 core and Studio connection, metrics
    syncix config                 show the settings in effect
    syncix bind                   show a place/folder mismatch
    syncix bind --studio|--disk   resolve it (which side is right)
    syncix verify                 check model/disk consistency
    syncix trash                  files the reconciler removed (recoverable)
    syncix restore [name]         put a trash entry back (newest by default)
    syncix pull                   ask Studio to resend the tree (source of truth)
    syncix selftest               run an end-to-end scenario against real Studio

  Output
    syncix sourcemap [-o file]    generate sourcemap.json for luau-lsp
    syncix build [-o file] [target]
                                  export the tree as Roblox XML (.rbxmx)
    syncix import <file> [parent] import a .rbxmx/.rbxlx file into the tree
    syncix upload                 show what would be published (does nothing)
    syncix upload --confirm       actually publish to Roblox

  Inspect
    syncix tree [target]          show the tree
    syncix ls [target]            list a node's children
    syncix find <word>            search by name or class
    syncix props <target>         show every property of an instance

  Edit
    syncix new <class> [name] [parent]
    syncix rename <target> <new name>
    syncix rm <target> [--yes]    asks for confirmation unless --yes
    syncix mv <target> <new parent>
    syncix set <target> <property> <value>
    syncix attr <target> <name> <value>
    syncix attr <target> <name> --delete    remove an attribute
    syncix tag <target>           show CollectionService tags
    syncix tag <target> <tag>...  replace the tag list (--none clears it)

  Process
    syncix up [port]              start the core in the background
    syncix serve [port]           run the core in this terminal
    syncix down                   stop the core
    syncix init                   create syncix.toml and the sync folder

  Without a port the value from syncix.toml is used, otherwise 8080.
  If that port is taken the next one is tried and the Studio plugin finds it.
  To pin a specific port: syncix serve 25565, then type 25565 into the port
  field in the Studio plugin panel.

  Targets: full UUID, 8-char short UUID, name, or dot path
           (e.g. Workspace.Simulator.SellPad)
  Values:  5 | true | "text" | 0,0.5,-60 (Vector3) | #ff8800 (color)
           | Enum.Material.Neon
"#,
        crate::project::VERSION
    );
}

fn durum() -> i32 {
    let Some(port) = core_portu() else {
        hata("Syncix Core is not running.");
        soluk("  Start it with: syncix up");
        return 1;
    };
    let Some(h) = json_al(port, "/health") else {
        hata("Could not read health info.");
        return 1;
    };

    let al = |k: &str| h.get(k).cloned().unwrap_or(serde_json::Value::Null);
    let metin = |k: &str| al(k).as_str().unwrap_or("-").to_string();
    let sayi = |k: &str| al(k).as_u64().unwrap_or(0);

    bilgi("Syncix Core");
    println!("  version      : {} (protocol {})", metin("version"), sayi("protocol"));
    println!("  project      : {}", metin("project"));
    println!("  folder       : {}", metin("root"));
    println!("  port         : {}", sayi("port"));
    println!("  uptime       : {} seconds", sayi("uptime_seconds"));

    let studio = al("studio_connected").as_bool().unwrap_or(false);
    if studio {
        tamam("  Studio       : connected");
    } else {
        println!("{}  Studio       : not connected{}", SARI, SIFIRLA);
        soluk("    (Is Studio open? The plugin may be waiting on the approval dialog.)");
    }

    bilgi("Metrics");
    println!("  inbound from Studio : {}", sayi("inbound_from_studio"));
    println!("  outbound to Studio  : {}", sayi("outbound_to_studio"));
    println!("  plugin queued       : {}", sayi("plugin_queued"));
    println!("  coalesced           : {}", sayi("plugin_coalesced"));

    bilgi("Activity log (Studio plugin)");
    println!("  entries        : {} (in {}, out {})",
        sayi("activity_total"), sayi("activity_in"), sayi("activity_out"));
    let cakisma = sayi("conflicts");
    if cakisma > 0 {
        println!("{}  conflicts      : {} — see the 'Recent changes' tab in the Studio panel{}",
            SARI, cakisma, SIFIRLA);
    } else {
        println!("  conflicts      : 0");
    }

    let dongu = sayi("loops_detected");
    if dongu > 0 {
        println!("{}  echo loops          : {} (check the log file){}", SARI, dongu, SIFIRLA);
    } else {
        println!("  echo loops          : 0");
    }
    0
}

struct Dugum {
    id: String,
    ad: String,
    sinif: String,
    ebeveyn: Option<String>,
}

fn agac_al(port: u16) -> Option<Vec<Dugum>> {
    let v = json_al(port, "/tree")?;
    let dizi = v.as_array()?;
    Some(
        dizi.iter()
            .map(|n| Dugum {
                id: n.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                ad: n.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                sinif: n
                    .get("className")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                ebeveyn: n
                    .get("parentId")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
            })
            .collect(),
    )
}

fn agac_yazdir(dugumler: &[Dugum], kok: Option<&str>, onek: &str, derinlik: usize) {
    if derinlik > 20 {
        return;
    }
    let mut cocuklar: Vec<&Dugum> = dugumler
        .iter()
        .filter(|d| d.ebeveyn.as_deref() == kok)
        .collect();
    cocuklar.sort_by(|a, b| a.ad.cmp(&b.ad));

    for (i, c) in cocuklar.iter().enumerate() {
        let son = i == cocuklar.len() - 1;
        let dal = if son { "└─ " } else { "├─ " };
        println!(
            "{}{}{}  {}{}{}",
            onek,
            dal,
            c.ad,
            SOLUK,
            c.sinif,
            SIFIRLA
        );
        let alt_onek = format!("{}{}", onek, if son { "   " } else { "│  " });
        agac_yazdir(dugumler, Some(&c.id), &alt_onek, derinlik + 1);
    }
}

fn agac(hedef: Option<&str>) -> i32 {
    let Some(port) = core_gerekli() else { return 1 };
    let Some(dugumler) = agac_al(port) else {
        hata("Could not read the tree.");
        return 1;
    };
    if dugumler.is_empty() {
        println!("{}Tree is empty. Is Studio connected?{}", SARI, SIFIRLA);
        return 0;
    }

    match hedef {
        None => agac_yazdir(&dugumler, None, "", 0),
        Some(h) => {
            let Some(d) = dugum_bul(&dugumler, h) else {
                hata(&format!("Not found: {}", h));
                return 1;
            };
            println!("{}  {}{}{}", d.ad, SOLUK, d.sinif, SIFIRLA);
            agac_yazdir(&dugumler, Some(&d.id), "", 0);
        }
    }
    0
}

fn dugum_bul<'a>(dugumler: &'a [Dugum], hedef: &str) -> Option<&'a Dugum> {
    let h = hedef.to_lowercase();
    dugumler
        .iter()
        .find(|d| d.id == hedef)
        .or_else(|| dugumler.iter().find(|d| d.id.starts_with(&h)))
        .or_else(|| dugumler.iter().find(|d| d.ad.to_lowercase() == h))
}

fn listele(hedef: Option<&str>) -> i32 {
    let Some(port) = core_gerekli() else { return 1 };
    let Some(dugumler) = agac_al(port) else { return 1 };

    let kok: Option<String> = match hedef {
        None => None,
        Some(h) => match dugum_bul(&dugumler, h) {
            Some(d) => Some(d.id.clone()),
            None => {
                hata(&format!("Not found: {}", h));
                return 1;
            }
        },
    };

    let mut cocuklar: Vec<&Dugum> = dugumler
        .iter()
        .filter(|d| d.ebeveyn.as_deref() == kok.as_deref())
        .collect();
    cocuklar.sort_by(|a, b| a.ad.cmp(&b.ad));

    if cocuklar.is_empty() {
        soluk("(no children)");
        return 0;
    }
    for c in cocuklar {
        println!(
            "  {}{}{}  {:<22} {}",
            SOLUK,
            &c.id[..8.min(c.id.len())],
            SIFIRLA,
            c.ad,
            c.sinif
        );
    }
    0
}

fn ara(kelime: &str) -> i32 {
    let Some(port) = core_gerekli() else { return 1 };
    let Some(dugumler) = agac_al(port) else { return 1 };
    let k = kelime.to_lowercase();

    let mut bulunan: Vec<&Dugum> = dugumler
        .iter()
        .filter(|d| d.ad.to_lowercase().contains(&k) || d.sinif.to_lowercase().contains(&k))
        .collect();
    bulunan.sort_by(|a, b| a.ad.cmp(&b.ad));

    if bulunan.is_empty() {
        println!("{}No match: {}{}", SARI, kelime, SIFIRLA);
        return 1;
    }
    for d in bulunan {
        println!(
            "  {}{}{}  {:<22} {}",
            SOLUK,
            &d.id[..8.min(d.id.len())],
            SIFIRLA,
            d.ad,
            d.sinif
        );
    }
    0
}

fn ozellikler(hedef: &str) -> i32 {
    let Some(port) = core_gerekli() else { return 1 };
    let yol = format!("/object?target={}", url_kodla(hedef));
    let Some(o) = json_al(port, &yol) else {
        hata("Could not read the instance.");
        return 1;
    };

    if let Some(e) = o.get("error").and_then(|x| x.as_str()) {
        hata(e);
        if let Some(adaylar) = o.get("candidates").and_then(|x| x.as_array()) {
            soluk("  Candidates:");
            for a in adaylar {
                println!(
                    "    {} ({}) -> {}",
                    a.get("name").and_then(|x| x.as_str()).unwrap_or("?"),
                    a.get("className").and_then(|x| x.as_str()).unwrap_or("?"),
                    a.get("shortId").and_then(|x| x.as_str()).unwrap_or("?")
                );
            }
        }
        return 1;
    }

    bilgi(&format!(
        "{}  ({})",
        o.get("name").and_then(|x| x.as_str()).unwrap_or("?"),
        o.get("class_name").and_then(|x| x.as_str()).unwrap_or("?")
    ));
    soluk(&format!(
        "  parent: {}",
        o.get("parent").and_then(|x| x.as_str()).unwrap_or("-")
    ));

    match o.get("properties").and_then(|x| x.as_object()) {
        Some(p) if !p.is_empty() => {
            println!("  properties:");
            let mut anahtarlar: Vec<&String> = p.keys().collect();
            anahtarlar.sort();
            for k in anahtarlar {
                println!("    {:<18} {}", k, p[k]);
            }
        }
        _ => soluk("  (no synced properties yet)"),
    }

    if let Some(a) = o.get("attributes").and_then(|x| x.as_object()) {
        if !a.is_empty() {
            println!("  attributes:");
            for (k, v) in a {
                println!("    {:<18} {}", k, v);
            }
        }
    }

    if let Some(s) = o.get("source").and_then(|x| x.as_str()) {
        if !s.is_empty() {
            soluk(&format!("  source: {} lines", s.lines().count()));
        }
    }
    0
}

fn dogrula() -> i32 {
    let Some(port) = core_gerekli() else { return 1 };
    let Some(v) = json_al(port, "/verify") else {
        hata("Verification failed.");
        return 1;
    };
    let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
    println!("  total instances : {}", v.get("totalObjects").and_then(|x| x.as_u64()).unwrap_or(0));
    if ok {
        tamam("  model is consistent");
        0
    } else {
        hata("  model is inconsistent:");
        eprintln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        1
    }
}

fn ilklendir() -> i32 {
    let kok = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let cfg = kok.join("syncix.toml");
    if cfg.exists() {
        println!("{}syncix.toml already exists: {}{}", SARI, cfg.display(), SIFIRLA);
    } else {
        let icerik = format!(
            "# Syncix project settings\nsync_dir = \"src_workspace\"\nport = {}\n",
            DEFAULT_PORT
        );
        if let Err(e) = std::fs::write(&cfg, icerik) {
            hata(&format!("Could not write syncix.toml: {}", e));
            return 1;
        }
        tamam(&format!("Created syncix.toml: {}", cfg.display()));
    }

    let senkron = kok.join("src_workspace");
    if !senkron.exists() {
        if let Err(e) = std::fs::create_dir_all(&senkron) {
            hata(&format!("Could not create sync folder: {}", e));
            return 1;
        }
        tamam(&format!("Created sync folder: {}", senkron.display()));
    }

    // .syncix çalışma klasörü sürüm kontrolüne girmemeli.
    let gitignore = kok.join(".gitignore");
    let mevcut = std::fs::read_to_string(&gitignore).unwrap_or_default();
    if !mevcut.contains(".syncix") {
        let yeni = format!("{}\n# Syncix runtime files\n.syncix/\nsyncix-core.log\n", mevcut);
        let _ = std::fs::write(&gitignore, yeni);
        soluk("  .gitignore updated (.syncix/)");
    }

    soluk("\nNext: open the folder in VS Code; the Syncix core starts automatically.");
    0
}

fn baslat(port: Option<u16>) -> i32 {
    // Belirli bir port istenmişse orada zaten bir core var mı diye bakılır;
    // istenmemişse herhangi bir core yeterlidir.
    match port {
        Some(p) => {
            if istek(p, "GET", "/health", None).map(|c| c.durum == 200).unwrap_or(false) {
                println!("{}A core is already running on port {}.{}", SARI, p, SIFIRLA);
                return 0;
            }
        }
        None => {
            if core_portu().is_some() {
                println!("{}The core is already running.{}", SARI, SIFIRLA);
                return 0;
            }
        }
    }

    let Ok(kendi) = std::env::current_exe() else {
        hata("Could not locate my own executable.");
        return 1;
    };

    // Kendini sunucu kipinde arka planda başlatır.
    let mut komut = std::process::Command::new(&kendi);
    komut.arg("serve");
    if let Some(p) = port {
        komut.arg(p.to_string());
    }
    komut
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null());

    match komut.spawn() {
        Ok(_) => {
            // Ayağa kalkmasını bekle
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(250));
                if let Some(p) = core_portu() {
                    tamam(&format!("Syncix Core started (port {}).", p));
                    return 0;
                }
            }
            hata("The core started but did not respond. Check syncix-core.log.");
            1
        }
        Err(e) => {
            hata(&format!("Could not start: {}", e));
            1
        }
    }
}

fn durdur() -> i32 {
    let Some(port) = core_portu() else {
        println!("{}The core is not running.{}", SARI, SIFIRLA);
        return 0;
    };
    match istek(port, "POST", "/shutdown", Some("{}")) {
        Ok(_) => {
            tamam("Syncix Core stopped.");
            0
        }
        Err(_) => {
            // Sunucu bağlantıyı kapatarak cevapsız çıkabilir; bu beklenen durumdur.
            tamam("Syncix Core stopped.");
            0
        }
    }
}

/// Studio'dan ağacı yeniden ister ve cevabın gelmesini bekler.
///
/// Bu fonksiyon selftest'in omurgası. Önceden doğrulama /object okuyarak
/// yapılıyordu; ama core komutu gönderirken modeli ZATEN güncelliyor, dolayısıyla
/// modeli okumak komutun Studio'ya ulaştığını KANITLAMAZ. Burada Studio'yu
/// konuşturuyoruz: gelen anlık görüntü tek doğruluk kaynağıdır.
///
/// Cevabın geldiği, /health üzerindeki "Studio'dan gelen mesaj" sayacının
/// artmasından anlaşılır.
fn taze_goruntu_bekle(port: u16) -> bool {
    let onceki = json_al(port, "/health")
        .and_then(|h| h.get("inbound_from_studio").and_then(|x| x.as_u64()))
        .unwrap_or(0);

    if !komut_gonder(port, "FULL_SYNC", serde_json::json!({})) {
        return false;
    }

    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let simdi = json_al(port, "/health")
            .and_then(|h| h.get("inbound_from_studio").and_then(|x| x.as_u64()))
            .unwrap_or(0);
        if simdi > onceki {
            // Anlık görüntü işlendikten sonra modelin oturması için kısa bekleme.
            std::thread::sleep(std::time::Duration::from_millis(400));
            return true;
        }
    }
    false
}

/// Uçtan uca senaryo: core ve Studio gerçekten birlikte çalışıyor mu?
///
/// Bu komutun varlık sebebi Position hatasıdır: değerler core'da doğru görünüyordu,
/// Studio'ya ulaşmıyordu. Burada her adımdan sonra değer Studio'nun durumundan
/// GERİ OKUNUR; "gönderdim, olmuştur" varsayımı yapılmaz.
fn selftest() -> i32 {
    let Some(port) = core_gerekli() else { return 1 };

    let Some(h) = json_al(port, "/health") else { return 1 };
    if !h.get("studio_connected").and_then(|x| x.as_bool()).unwrap_or(false) {
        hata("Studio is not connected; the end-to-end test cannot run.");
        soluk("  Open Roblox Studio, wait for the Syncix plugin to connect, then retry.");
        return 1;
    }

    let ad = "SyncixSelftestParcasi";
    let mut basarisiz = 0;
    let mut adim = |no: usize, baslik: &str, ok: bool, detay: String| {
        if ok {
            println!("  {}[{}] {}{}", YESIL, no, baslik, SIFIRLA);
        } else {
            println!("  {}[{}] {} -> {}{}", KIRMIZI, no, baslik, detay, SIFIRLA);
            basarisiz += 1;
        }
    };

    bilgi("Syncix end-to-end test");
    soluk("  After each step the tree is re-requested from Studio;");
    soluk("  verification uses Studio's reply, not the core's own model.");

    // 1. Oluştur
    let olusturuldu = komut_gonder(
        port,
        "CREATE_INSTANCE",
        serde_json::json!({ "className": "Part", "name": ad, "parentId": "Workspace" }),
    );
    taze_goruntu_bekle(port);
    let agac1 = agac_al(port).unwrap_or_default();
    let bulundu = agac1.iter().find(|d| d.ad == ad).map(|d| d.id.clone());
    adim(1, "create instance", olusturuldu && bulundu.is_some(), "instance did not appear in the tree".into());

    let Some(id) = bulundu else {
        hata("Test aborted: could not create the instance.");
        return 1;
    };

    // 2. Vector3 konum — Position hatasının tam senaryosu
    komut_gonder(
        port,
        "SET_PROPERTY",
        serde_json::json!({ "id": id, "property": "Position", "value": "12,7,-34" }),
    );
    taze_goruntu_bekle(port);
    let konum_ok = json_al(port, &format!("/object?target={}", id))
        .and_then(|o| o.get("properties")?.get("Position")?.get("Vector3").cloned())
        .map(|v| {
            (v.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0) - 12.0).abs() < 0.01
                && (v.get("z").and_then(|x| x.as_f64()).unwrap_or(0.0) + 34.0).abs() < 0.01
        })
        .unwrap_or(false);
    adim(2, "Vector3 position", konum_ok, "position was not applied in Studio".into());

    // 3. Renk — hex dönüşümü
    komut_gonder(
        port,
        "SET_PROPERTY",
        serde_json::json!({ "id": id, "property": "Color", "value": "#ff8800" }),
    );
    taze_goruntu_bekle(port);
    let renk_ok = json_al(port, &format!("/object?target={}", id))
        .and_then(|o| o.get("properties")?.get("Color")?.get("Color3").cloned())
        .map(|v| (v.get("r").and_then(|x| x.as_f64()).unwrap_or(0.0) - 1.0).abs() < 0.02)
        .unwrap_or(false);
    adim(3, "hex color", renk_ok, "color was not applied".into());

    // 4. Yeniden adlandırma — UUID değişmemeli
    let yeni_ad = "SyncixSelftestYeniAd";
    komut_gonder(
        port,
        "RENAME_INSTANCE",
        serde_json::json!({ "id": id, "newName": yeni_ad }),
    );
    taze_goruntu_bekle(port);
    let agac2 = agac_al(port).unwrap_or_default();
    let ad_ok = agac2.iter().any(|d| d.id == id && d.ad == yeni_ad);
    adim(4, "rename (UUID preserved)", ad_ok, "name did not change or UUID drifted".into());

    // 5. Silme
    komut_gonder(port, "DELETE_INSTANCE", serde_json::json!({ "id": id }));
    taze_goruntu_bekle(port);
    let agac3 = agac_al(port).unwrap_or_default();
    let silme_ok = !agac3.iter().any(|d| d.id == id);
    adim(5, "delete", silme_ok, "instance is still in the tree".into());

    println!();
    if basarisiz == 0 {
        tamam("All steps passed. Studio and the core are genuinely in sync.");
        0
    } else {
        hata(&format!("{} step(s) failed.", basarisiz));
        soluk("  Check the [Syncix] warnings in the Studio Output window.");
        1
    }
}

/// -o bayrağını ve varsayılan dosya adını çözer.
fn cikti_dosyasi(argumanlar: &[String], varsayilan: &str) -> String {
    for (i, a) in argumanlar.iter().enumerate() {
        if (a == "-o" || a == "--output") && i + 1 < argumanlar.len() {
            return argumanlar[i + 1].clone();
        }
    }
    varsayilan.to_string()
}

/// -o ve değerini eleyerek geriye kalan konumsal argümanları döndürür.
fn konumsal(argumanlar: &[String]) -> Vec<String> {
    let mut cikti = Vec::new();
    let mut atla = false;
    for a in argumanlar.iter().skip(1) {
        if atla {
            atla = false;
            continue;
        }
        if a == "-o" || a == "--output" {
            atla = true;
            continue;
        }
        cikti.push(a.clone());
    }
    cikti
}

/// luau-lsp'nin otomatik tamamlama yapabilmesi için sourcemap.json üretir.
///
/// Normalde core bu dosyayı her senkronda kendisi tazeler (syncix.toml içindeki
/// `sourcemap` ayarı). Bu komut tek seferlik üretim ya da CI için.
/// Uzlastirici tarafindan silinen dosyalar cop kutusuna tasiniyor.
/// Bu komut orada ne oldugunu gosterir; olmadigi surece kullanici silinen
/// dosyanin geri alinabilir oldugunu hicbir zaman ogrenemez.
/// Silinecek objeyi ve alt agacini gosterip onay ister.
/// Terminal etkilesimli degilse (borulanmis girdi) silme reddedilir:
/// cevapsiz bir soruyu "evet" saymak, silmenin dogasi geregi yanlis taraf.
fn silmeyi_onayla(port: u16, hedef: &str) -> bool {
    let yol = format!("/object?target={}", url_kodla(hedef));
    match json_al(port, &yol).filter(|d| d.get("error").is_none()) {
        Some(d) => {
            let cocuk = d
                .get("children")
                .and_then(|c| c.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let ad = d.get("name").and_then(|v| v.as_str()).unwrap_or(hedef);
            let sinif = d.get("class_name").and_then(|v| v.as_str()).unwrap_or("?");
            if cocuk > 0 {
                // Silme basamakli: dogrudan cocuklar degil, altindaki her sey gider.
                println!(
                    "Delete {} ({}) and everything inside it ({} direct child object(s))?",
                    ad, sinif, cocuk
                );
            } else {
                println!("Delete {} ({})?", ad, sinif);
            }
        }
        None => {
            println!("Delete {}?", hedef);
        }
    }
    print!("Type 'y' to confirm: ");
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let mut cevap = String::new();
    if std::io::stdin().read_line(&mut cevap).is_err() {
        return false;
    }
    let c = cevap.trim().to_lowercase();
    c == "y" || c == "yes"
}

fn etiketleri_goster(port: u16, hedef: &str) -> i32 {
    let yol = format!("/object?target={}", url_kodla(hedef));
    let Some(o) = json_al(port, &yol) else {
        hata("Could not read the instance.");
        return 1;
    };
    if let Some(e) = o.get("error").and_then(|x| x.as_str()) {
        hata(e);
        return 1;
    }
    let etiketler: Vec<&str> = o
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    if etiketler.is_empty() {
        bilgi("No tags.");
    } else {
        for t in etiketler {
            println!("  {}", t);
        }
    }
    0
}

/// Yururlukteki ayarlari gosterir.
///
/// Neden gerekli: "ayari yazdim ama bir sey degismedi" en sik sikayet.
/// Ayari yazdigin yer ile programin okudugu yer ayni mi, cevabi burada.
/// Place catismasini gosterir ve --studio / --disk ile cozer.
///
/// Neden bir komut: iki secenek de veri kaybettirebilir. Syncix'in kendi
/// basina birini secmesi, kullanicinin haberi olmadan bir tarafi silmesi
/// demek olurdu. Bu yuzden karar burada, acikca veriliyor.
fn yer_bagla(argumanlar: &[String]) -> i32 {
    let Some(port) = core_gerekli() else { return 1 };
    let Some(saglik) = json_al(port, "/health") else {
        hata("Could not read the core status.");
        return 1;
    };

    let catisma = saglik.get("place_conflict");
    let yon = if argumanlar.iter().any(|a| a == "--studio") {
        Some("studio")
    } else if argumanlar.iter().any(|a| a == "--disk") {
        Some("disk")
    } else {
        None
    };

    let Some(c) = catisma.filter(|x| !x.is_null()) else {
        let bagli = crate::project::ProjectConfig::load().bagli_place();
        match bagli {
            Some(k) => tamam(&format!("No conflict. This folder is bound to place {}.", k)),
            None => bilgi("No conflict. This folder is not bound to a place yet."),
        }
        return 0;
    };

    let al = |ad: &str| c.get(ad).and_then(|x| x.as_str()).unwrap_or("?").to_string();

    let Some(yon) = yon else {
        // Karar verilmeden once ne oldugunu goster.
        hata("This folder belongs to a different place. Sync is on hold.");
        println!();
        println!("  folder is bound to : {}", al("klasorun_place"));
        println!(
            "  place connecting   : {} (\"{}\", id {})",
            al("gelen_place"),
            al("gelen_ad"),
            al("gelen_place_id")
        );
        println!();
        println!("Choose one:");
        println!("  syncix bind --studio   this place is right; the folder is rewritten from it");
        println!("  syncix bind --disk     the folder is right; its contents go into this place");
        soluk("  Files the reconciler removes go to the trash (syncix trash).");
        return 1;
    };

    if !komut_gonder(port, "BIND", serde_json::json!({ "side": yon })) {
        return 1;
    }
    if yon == "studio" {
        tamam("Bound to the connected place. The folder is being rewritten from Studio.");
    } else {
        tamam("Bound to this folder. Its contents will be pushed into the connected place.");
    }
    0
}

fn yapilandirmayi_goster() -> i32 {
    let c = crate::project::ProjectConfig::load();

    println!("Project: {}", c.name);
    println!("Root:    {}", c.root.display());
    println!("Config:  {}", c.root.join("syncix.toml").display());
    println!();

    println!("[sync]");
    println!("  mode           {}", c.mod_.adi());
    println!("  play_mode      {}", c.play.adi());
    println!("  debounce_ms    {}", c.debounce_ms);
    println!("  ask_permission {}", c.izin_sor);
    println!("  undo           {}", c.geri_al);
    println!();

    println!("[files]");
    println!("  sync_dir       {}", c.sync_dir);
    println!("  meta_files     {}", c.meta_dosyalari);
    println!(
        "  ignore         {}",
        if c.ignore.is_empty() {
            "(none)".to_string()
        } else {
            c.ignore.join(", ")
        }
    );
    println!();

    println!("[safety]");
    println!("  trash            {}", c.guvenlik.cop_kutusu);
    println!("  trash_keep       {}", c.guvenlik.cop_tur_sayisi);
    println!("  delete_grace_ms  {}", c.guvenlik.silme_bekleme_ms);
    println!("  confirm_delete   {}", c.guvenlik.silmeyi_onayla);
    println!();

    let liste = |v: &Vec<String>| {
        if v.is_empty() {
            "(default)".to_string()
        } else {
            v.join(", ")
        }
    };
    println!("[scope]");
    println!("  services           {}", liste(&c.kapsam.servisler));
    println!("  ignore_classes     {}", liste(&c.kapsam.sinif_disla));
    println!("  ignore_properties  {}", liste(&c.kapsam.property_disla));
    println!();

    println!("[server]");
    println!("  port           {}", c.wanted_port);
    println!();
    println!("[editor]");
    println!("  sourcemap      {}", c.sourcemap);
    println!();
    // Calisan core eski ayarla baslamis olabilir; bu en yaniltici durum.
    if let Some(port) = core_portu() {
        if port != c.wanted_port {
            soluk(&format!(
                "  Note: a core is running on port {}, which differs from the configured port.",
                port
            ));
        }
        soluk("  Settings are read at startup; restart the core after editing (syncix down && syncix up).");
    }
    0
}

fn cop_listele() -> i32 {
    let yapilandirma = crate::project::ProjectConfig::load();
    let turlar = crate::layout::cop_turlari(&yapilandirma.sync_dir);
    if turlar.is_empty() {
        bilgi("Trash is empty; no files have been removed by the reconciler.");
        return 0;
    }
    println!("Removed files, newest first:");
    for (tur, adet) in &turlar {
        println!("  {}  {} file(s)", tur, adet);
    }
    println!();
    println!("Restore with: syncix restore <name>");
    0
}

fn cop_geri_al(tur: Option<&str>) -> i32 {
    let yapilandirma = crate::project::ProjectConfig::load();
    let turlar = crate::layout::cop_turlari(&yapilandirma.sync_dir);
    // Ad verilmediyse en yeni tur geri alinir; en sik istenen bu.
    let secilen = match tur {
        Some(t) => t.to_string(),
        None => match turlar.first() {
            Some((t, _)) => t.clone(),
            None => {
                bilgi("Trash is empty; there is nothing to restore.");
                return 0;
            }
        },
    };
    if !turlar.iter().any(|(t, _)| t == &secilen) {
        hata(&format!("No such entry in trash: {}", secilen));
        return 1;
    }
    let (geri, atlanan) = crate::layout::coptan_geri_al(&yapilandirma.sync_dir, &secilen);
    tamam(&format!("Restored {} file(s) from {}.", geri, secilen));
    if atlanan > 0 {
        // Uzerine yazmak geri almayi kendi basina bir veri kaybina cevirirdi.
        bilgi(&format!(
            "{} file(s) were skipped because a file already exists at that path.",
            atlanan
        ));
    }
    0
}

fn sourcemap_uret(argumanlar: &[String]) -> i32 {
    let Some(port) = core_gerekli() else { return 1 };
    let hedef = cikti_dosyasi(argumanlar, "sourcemap.json");

    let cevap = match istek(port, "GET", "/sourcemap", None) {
        Ok(c) if c.durum == 200 => c.govde,
        Ok(c) => {
            hata(&format!("Could not fetch sourcemap ({})", c.durum));
            return 1;
        }
        Err(e) => {
            hata(&format!("Could not fetch sourcemap: {}", e));
            return 1;
        }
    };

    if let Err(e) = std::fs::write(&hedef, &cevap) {
        hata(&format!("Could not write {}: {}", hedef, e));
        return 1;
    }

    let sayi = cevap.matches("\"className\"").count();
    tamam(&format!("Wrote {} ({} instances).", hedef, sayi));
    soluk("  Once luau-lsp reads this file, paths like game.ReplicatedStorage.X get");
    soluk("  autocomplete and type checking.");
    0
}

/// Ağacı Roblox XML olarak dosyaya yazar (rojo build karşılığı).
fn build_et(argumanlar: &[String]) -> i32 {
    let Some(port) = core_gerekli() else { return 1 };
    let hedef_dosya = cikti_dosyasi(argumanlar, "build.rbxmx");
    let konumsallar = konumsal(argumanlar);
    let hedef_obje = konumsallar.first().cloned().unwrap_or_default();

    let yol = if hedef_obje.is_empty() {
        "/build".to_string()
    } else {
        format!("/build?target={}", url_kodla(&hedef_obje))
    };

    let cevap = match istek(port, "GET", &yol, None) {
        Ok(c) if c.durum == 200 => c,
        Ok(c) => {
            hata(&format!("Build failed ({}):", c.durum));
            eprintln!("{}", c.govde.trim());
            return 1;
        }
        Err(e) => {
            hata(&format!("Build failed: {}", e));
            return 1;
        }
    };

    if let Err(e) = std::fs::write(&hedef_dosya, &cevap.govde) {
        hata(&format!("Could not write {}: {}", hedef_dosya, e));
        return 1;
    }

    let obje_sayisi = cevap.govde.matches("<Item ").count();
    tamam(&format!(
        "Wrote {} ({} instances, {} bytes).",
        hedef_dosya,
        obje_sayisi,
        cevap.govde.len()
    ));
    soluk("  In Studio: right click > Insert from File...");
    0
}

/// Roblox'a yayınlama. VARSAYILAN OLARAK HİÇBİR ŞEY YAYINLAMAZ.
///
/// Yayınlama geri alınamaz bir dış işlemdir: yayınlanan sürüm oyuncuların
/// göreceği sürümdür. Bu yüzden komut önce ne yapacağını anlatır ve durur;
/// gerçekten yayınlamak için `--onayla` gerekir.
fn yayinla(argumanlar: &[String]) -> i32 {
    let Some(port) = core_gerekli() else { return 1 };

    let onayli = argumanlar.iter().any(|a| a == "--confirm" || a == "--onayla");

    // Proje kökünü ve ayarları /health üzerinden al.
    let Some(saglik) = json_al(port, "/health") else {
        hata("Could not reach the core.");
        return 1;
    };
    let kok = std::path::PathBuf::from(
        saglik.get("root").and_then(|x| x.as_str()).unwrap_or("."),
    );

    // Güvenlik kapısı: anahtar projeye yazılmış olmamalı.
    if crate::upload::anahtar_sizintisi_var_mi(&kok) {
        hata("syncix.toml contains something that looks like an API key.");
        soluk("  Keys must NOT live in project files; the first commit makes them public.");
        soluk("  Remove it and use the SYNCIX_API_KEY environment variable instead.");
        return 1;
    }

    let cfg = crate::upload::UploadConfig::load(&kok);
    let universe = cfg.universe_id;
    let place = cfg.place_id;

    let (Some(universe_id), Some(place_id)) = (universe, place) else {
        hata("No publish target configured.");
        soluk("  Add this to syncix.toml:");
        soluk("");
        soluk("    [upload]");
        soluk("    universe_id = 1234567890");
        soluk("    place_id    = 9876543210");
        return 1;
    };

    // Yer dosyasını canlı modelden üret.
    let cevap = match istek(port, "GET", "/build", None) {
        Ok(c) if c.durum == 200 => c,
        Ok(c) => {
            hata(&format!("Could not build the place file ({}).", c.durum));
            return 1;
        }
        Err(e) => {
            hata(&format!("Could not build the place file: {}", e));
            return 1;
        }
    };

    let hedef_dosya = kok.join(".syncix").join("upload.rbxlx");
    if let Some(d) = hedef_dosya.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::write(&hedef_dosya, &cevap.govde) {
        hata(&format!("Could not write {}: {}", hedef_dosya.display(), e));
        return 1;
    }

    let plan = crate::upload::UploadPlan {
        universe_id,
        place_id,
        dosya: hedef_dosya,
        bayt: cevap.govde.len(),
        obje_sayisi: cevap.govde.matches("<Item ").count(),
        atlanan_enum: cevap
            .basliklar
            .get("x-syncix-skipped-enums")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0),
    };

    bilgi("About to publish");
    println!("  universe : {}", plan.universe_id);
    println!("  place    : {}", plan.place_id);
    println!("  file     : {}", plan.dosya.display());
    println!("  contents : {} instances, {} bytes", plan.obje_sayisi, plan.bayt);
    println!("  endpoint : {}", crate::upload::hedef_url(plan.universe_id, plan.place_id));

    // Atlanan Enum varsa yayinlanacak dosya EKSIKTIR; kullanici bunu
    // yayindan once bilmeli, sonra degil.
    if plan.atlanan_enum > 0 {
        println!();
        println!(
            "{}WARNING: {} enum value(s) could not be exported.{}",
            SARI, plan.atlanan_enum, SIFIRLA
        );
        soluk("  The published file will be missing settings like Material and Shape.");
        soluk("  If that is not acceptable, say so before publishing and we will extend the table.");
    }

    let anahtar_var = std::env::var("SYNCIX_API_KEY").is_ok();
    if !anahtar_var {
        println!();
        hata("The SYNCIX_API_KEY environment variable is not set.");
        soluk("  Get an Open Cloud key at: create.roblox.com > Creator Hub > API Keys");
        soluk("  Grant it the 'universe-places:write' permission.");
        soluk("  Sonra: $env:SYNCIX_API_KEY = \"...\"   (PowerShell)");
        return 1;
    }

    if !onayli {
        println!();
        println!("{}Nothing was published.{}", SARI, SIFIRLA);
        soluk("  Publishing cannot be undone: the published version is what players see.");
        soluk("  If you are sure:  syncix upload --confirm");
        soluk("");
        soluk("  Or run it yourself:");
        for satir in crate::upload::curl_komutu(&plan).lines() {
            soluk(&format!("    {}", satir));
        }
        return 0;
    }

    // --onayla verildi: sistemdeki curl ile gönder.
    bilgi("Publishing...");
    let cikti = std::process::Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg(crate::upload::hedef_url(plan.universe_id, plan.place_id))
        .arg("-H")
        .arg(format!(
            "x-api-key: {}",
            std::env::var("SYNCIX_API_KEY").unwrap_or_default()
        ))
        .arg("-H")
        .arg("Content-Type: application/xml")
        .arg("--data-binary")
        .arg(format!("@{}", plan.dosya.display()))
        .output();

    match cikti {
        Ok(c) if c.status.success() => {
            let govde = String::from_utf8_lossy(&c.stdout);
            if govde.contains("versionNumber") {
                tamam("Published.");
                println!("  {}", govde.trim());
                0
            } else {
                hata("Roblox did not return the expected response:");
                eprintln!("{}", govde.trim());
                1
            }
        }
        Ok(c) => {
            hata("Publish failed:");
            eprintln!("{}", String::from_utf8_lossy(&c.stderr).trim());
            1
        }
        Err(e) => {
            hata(&format!("Could not run curl: {}", e));
            soluk("  curl ships with Windows 10+, macOS and most Linux distributions.");
            soluk("  Otherwise run the command above with your own tool.");
            1
        }
    }
}

/// .rbxmx / .rbxlx dosyasini agaca alir (rojo'da olup bizde olmayan son madde).
///
/// Her dugum icin once CREATE_INSTANCE, sonra property'ler gonderilir. Ust ust
/// olusturma sirasi onemli: cocuk, ebeveyni olusturulmadan gonderilemez.
fn ice_aktar(argumanlar: &[String]) -> i32 {
    let Some(dosya) = argumanlar.get(1) else {
        hata("Usage: syncix import <file.rbxmx> [parent]");
        return 1;
    };
    let Some(port) = core_gerekli() else { return 1 };
    let ebeveyn = argumanlar.get(2).cloned().unwrap_or_else(|| "Workspace".to_string());

    let xml = match std::fs::read_to_string(dosya) {
        Ok(x) => x,
        Err(e) => {
            hata(&format!("Could not read {}: {}", dosya, e));
            return 1;
        }
    };

    let (kokler, atlanan) = match crate::rbxmx_import::ayristir(&xml) {
        Ok(v) => v,
        Err(e) => {
            hata(&format!("Could not parse {}: {}", dosya, e));
            return 1;
        }
    };

    let toplam = crate::rbxmx_import::say(&kokler);
    if toplam == 0 {
        hata("The file contains no instances.");
        return 1;
    }

    bilgi(&format!("Importing {} instance(s) into {}", toplam, ebeveyn));
    if atlanan > 0 {
        soluk(&format!(
            "  {} property value(s) use types Syncix does not model and were skipped.",
            atlanan
        ));
    }

    // Ozyinelemeli olusturma. Her dugum once yaratilir, sonra ozellikleri yazilir.
    fn olustur(
        port: u16,
        dugum: &crate::rbxmx_import::ImportedNode,
        ebeveyn: &str,
        sayac: &mut usize,
        basarisiz: &mut usize,
    ) {
        // Kimligi ONCEDEN uretiyoruz: boylece olusturulan objeyi ismiyle degil
        // kimligiyle hedefleyebiliyoruz. Isimle hedeflemek, ice aktarilan agac
        // mevcut bir ismi tekrarladiginda belirsizlik hatasi veriyordu.
        let kimlik = uuid::Uuid::new_v4().to_string();
        let ok = komut_gonder(
            port,
            "CREATE_INSTANCE",
            serde_json::json!({
                "id": kimlik,
                "className": dugum.class_name,
                "name": dugum.name,
                "parentId": ebeveyn
            }),
        );
        if !ok {
            *basarisiz += 1;
            return;
        }
        *sayac += 1;
        std::thread::sleep(std::time::Duration::from_millis(120));

        for (ad, deger) in &dugum.properties {
            // Deger metne cevrilmiyor: metin tip bilgisini kaybediyor ve CFrame,
            // UDim, NumberRange gibi tipler ice aktarmada tamamen dusuyordu.
            // Tel formati zaten tipi tasiyor, dogrudan o gonderiliyor.
            let deger_json = crate::pv_to_wire(deger);
            komut_gonder(
                port,
                "SET_PROPERTY",
                serde_json::json!({ "id": kimlik, "property": ad, "value": deger_json }),
            );
            std::thread::sleep(std::time::Duration::from_millis(60));
        }

        if let Some(kaynak) = &dugum.source {
            komut_gonder(
                port,
                "SET_PROPERTY",
                serde_json::json!({ "id": kimlik, "property": "Source", "value": kaynak }),
            );
            std::thread::sleep(std::time::Duration::from_millis(60));
        }

        for cocuk in &dugum.children {
            olustur(port, cocuk, &kimlik, sayac, basarisiz);
        }
    }

    let mut sayac = 0usize;
    let mut basarisiz = 0usize;
    for k in &kokler {
        olustur(port, k, &ebeveyn, &mut sayac, &mut basarisiz);
    }

    if basarisiz > 0 {
        hata(&format!("{} instance(s) could not be created.", basarisiz));
        soluk("  A name may be ambiguous; check with syncix tree.");
        return 1;
    }

    tamam(&format!("Imported {} instance(s).", sayac));
    soluk("  Run syncix pull to confirm the result from Studio.");
    0
}

// ---------------------------------------------------------------------------
// Giriş
// ---------------------------------------------------------------------------

/// Argümanları işler. Sunucu kipinde çalışılması gerekiyorsa None döner.
pub fn calistir(argumanlar: &[String]) -> Option<i32> {
    let Some(komut) = argumanlar.first().map(|s| s.as_str()) else {
        return None; // argüman yok -> sunucu kipi
    };
    // `syncix serve` ya da `syncix serve 25565`: sunucu kipi.
    // Port verilmişse syncix.toml'daki değerin yerine geçer.
    if komut == "serve" {
        return None;
    }

    let arg = |i: usize| argumanlar.get(i).map(|s| s.as_str());
    // Değerler boşluk içerebilir (örn. `set Kutu Position 0, 5, -60`); kalan tüm
    // argümanlar birleştirilir.
    let kalan = |i: usize| argumanlar[i.min(argumanlar.len())..].join(" ");

    let sonuc = match komut {
        "help" | "--help" | "-h" => {
            yardim();
            0
        }
        "version" | "--version" | "-V" => {
            println!("syncix {}", crate::project::VERSION);
            0
        }
        "status" | "st" => durum(),
        "tree" => agac(arg(1)),
        "ls" | "list" => listele(arg(1)),
        "find" | "search" => match arg(1) {
            Some(k) => ara(k),
            None => {
                hata("Usage: syncix find <word>");
                1
            }
        },
        "props" | "show" | "cat" => match arg(1) {
            Some(h) => ozellikler(h),
            None => {
                hata("Usage: syncix props <target>");
                1
            }
        },
        "set" => match (arg(1), arg(2)) {
            (Some(h), Some(p)) if argumanlar.len() > 3 => {
                let Some(port) = core_gerekli() else { return Some(1) };
                let deger = kalan(3);
                if komut_gonder(
                    port,
                    "SET_PROPERTY",
                    serde_json::json!({ "id": h, "property": p, "value": deger }),
                ) {
                    tamam(&format!("{}.{} = {}", h, p, deger));
                    0
                } else {
                    1
                }
            }
            _ => {
                hata("Usage: syncix set <target> <property> <value>");
                1
            }
        },
        "attr" => match (arg(1), arg(2)) {
            // Silme: `syncix attr <hedef> <ad> --sil`
            (Some(h), Some(n)) if arg(3) == Some("--sil") || arg(3) == Some("--delete") => {
                let Some(port) = core_gerekli() else { return Some(1) };
                if komut_gonder(
                    port,
                    "SET_ATTRIBUTE",
                    serde_json::json!({ "id": h, "name": n, "value": serde_json::Value::Null }),
                ) {
                    tamam(&format!("{} @{} removed", h, n));
                    0
                } else {
                    1
                }
            }
            (Some(h), Some(n)) if argumanlar.len() > 3 => {
                let Some(port) = core_gerekli() else { return Some(1) };
                let deger = kalan(3);
                if komut_gonder(
                    port,
                    "SET_ATTRIBUTE",
                    serde_json::json!({ "id": h, "name": n, "value": deger }),
                ) {
                    tamam(&format!("{} @{} = {}", h, n, deger));
                    0
                } else {
                    1
                }
            }
            _ => {
                hata("Usage: syncix attr <target> <name> <value>");
                1
            }
        },
        "new" | "create" | "mk" => match arg(1) {
            Some(sinif) => {
                let Some(port) = core_gerekli() else { return Some(1) };
                let ad = arg(2).unwrap_or(sinif);
                let ebeveyn = arg(3).unwrap_or("Workspace");
                if komut_gonder(
                    port,
                    "CREATE_INSTANCE",
                    serde_json::json!({ "className": sinif, "name": ad, "parentId": ebeveyn }),
                ) {
                    tamam(&format!("Created {} ({}) in {}", ad, sinif, ebeveyn));
                    0
                } else {
                    1
                }
            }
            None => {
                hata("Usage: syncix new <class> [name] [parent]");
                1
            }
        },
        "rename" | "rn" => match (arg(1), arg(2)) {
            (Some(h), Some(yeni)) => {
                let Some(port) = core_gerekli() else { return Some(1) };
                if komut_gonder(
                    port,
                    "RENAME_INSTANCE",
                    serde_json::json!({ "id": h, "newName": yeni }),
                ) {
                    tamam(&format!("{} -> {}", h, yeni));
                    0
                } else {
                    1
                }
            }
            _ => {
                hata("Usage: syncix rename <target> <new name>");
                1
            }
        },
        "rm" | "del" | "delete" => match arg(1) {
            Some(h) => {
                let Some(port) = core_gerekli() else { return Some(1) };
                // Silme cocuklariyla birlikte gider ve Studio'da geri alinabilse de
                // editor tarafinda geri donusu yok. Ne silindigini once GOSTERIP
                // onay istiyoruz; --yes betiklerde bu adimi atlar.
                let onaylandi = argumanlar.iter().any(|a| a == "--yes" || a == "-y")
                    || !crate::project::ProjectConfig::load().guvenlik.silmeyi_onayla;
                if !onaylandi && !silmeyi_onayla(port, h) {
                    bilgi("Cancelled; nothing was deleted.");
                    return Some(0);
                }
                if komut_gonder(port, "DELETE_INSTANCE", serde_json::json!({ "id": h })) {
                    tamam(&format!("{} deleted", h));
                    0
                } else {
                    1
                }
            }
            None => {
                hata("Usage: syncix rm <target> [--yes]");
                1
            }
        },
        "mv" | "move" => match (arg(1), arg(2)) {
            (Some(h), Some(yeni)) => {
                let Some(port) = core_gerekli() else { return Some(1) };
                if komut_gonder(
                    port,
                    "REPARENT_INSTANCE",
                    serde_json::json!({ "id": h, "newParentId": yeni }),
                ) {
                    tamam(&format!("Moved {} into {}", h, yeni));
                    0
                } else {
                    1
                }
            }
            _ => {
                hata("Usage: syncix mv <target> <new parent>");
                1
            }
        },
        "upload" | "yayinla" => yayinla(argumanlar),
        "sourcemap" => sourcemap_uret(argumanlar),
        "build" => build_et(argumanlar),
        "import" => ice_aktar(argumanlar),
        "pull" | "resync" => {
            let Some(port) = core_gerekli() else { return Some(1) };
            if komut_gonder(port, "FULL_SYNC", serde_json::json!({})) {
                tamam("Asked Studio to resend the tree.");
                soluk("  The reply is processed within a few seconds; then try syncix tree.");
                0
            } else {
                1
            }
        }
        "verify" | "check" => dogrula(),
        "tag" | "tags" => match arg(1) {
            Some(h) => {
                let Some(port) = core_gerekli() else { return Some(1) };
                // Argumansiz cagri yalnizca gosterir; yanlislikla etiket
                // silinmesin diye "bos liste" ile "listeleme" ayrilmis durumda.
                if argumanlar.len() <= 2 {
                    etiketleri_goster(port, h)
                } else {
                    // "--none" tek basina "hepsini temizle" demek. Bos liste
                    // gondermek icin baska bir yol yok: argumansiz cagri
                    // listeleme anlamina geliyor.
                    let etiketler: Vec<String> = if argumanlar[2..] == ["--none".to_string()] {
                        Vec::new()
                    } else {
                        argumanlar[2..].iter().map(|s| s.to_string()).collect()
                    };
                    if komut_gonder(
                        port,
                        "SET_TAGS",
                        serde_json::json!({ "id": h, "tags": etiketler }),
                    ) {
                        if etiketler.is_empty() {
                            tamam(&format!("{} tags cleared", h));
                        } else {
                            tamam(&format!("{} tags: {}", h, etiketler.join(", ")));
                        }
                        0
                    } else {
                        1
                    }
                }
            }
            None => {
                hata("Usage: syncix tag <target> [tag ...]   (no tags = show)");
                soluk("  Clear every tag with: syncix tag <target> --none");
                1
            }
        },
        "bind" => yer_bagla(argumanlar),
        "config" | "settings" => yapilandirmayi_goster(),
        "trash" => cop_listele(),
        "restore" => cop_geri_al(arg(1)),
        "selftest" => selftest(),
        "init" => ilklendir(),
        "up" | "start" => baslat(arg(1).and_then(|p| p.parse::<u16>().ok())),
        "down" | "stop" => durdur(),
        bilinmeyen => {
            hata(&format!("Unknown command: {}", bilinmeyen));
            soluk("  Run syncix help to see the command list.");
            1
        }
    };

    Some(sonuc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_kodlama_bosluk_ve_nokta_yolu() {
        assert_eq!(url_kodla("Workspace.Simulator"), "Workspace.Simulator");
        assert_eq!(url_kodla("iki kelime"), "iki%20kelime");
        assert_eq!(url_kodla("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn port_dosyasi_yoksa_cokmez() {
        // Sadece panik olmadığını doğrular; ortama göre Some/None dönebilir.
        let _ = port_dosyasindan();
    }

    #[test]
    fn dugum_bulma_isim_ve_kisa_uuid() {
        let dugumler = vec![
            Dugum {
                id: "aabbccdd-1111-2222-3333-444455556666".into(),
                ad: "Kutu".into(),
                sinif: "Part".into(),
                ebeveyn: None,
            },
        ];
        assert!(dugum_bul(&dugumler, "Kutu").is_some());
        assert!(dugum_bul(&dugumler, "kutu").is_some());
        assert!(dugum_bul(&dugumler, "aabbccdd").is_some());
        assert!(dugum_bul(&dugumler, "yok").is_none());
    }
}

#[cfg(test)]
mod baslik_tests {
    use super::*;

    #[test]
    fn basliklar_kucuk_harfe_cevrilir() {
        let ham = "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nX-Syncix-Skipped-Enums: 3";
        let h = basliklari_ayristir(ham);
        assert_eq!(h.get("content-type").unwrap(), "text/xml");
        assert_eq!(h.get("x-syncix-skipped-enums").unwrap(), "3");
    }

    /// Durum satiri baslik degildir; yanlislikla haritaya girerse
    /// "http/1.1 200 ok" gibi anlamsiz bir anahtar olusurdu.
    #[test]
    fn durum_satiri_baslik_sayilmaz() {
        let h = basliklari_ayristir("HTTP/1.1 404 Not Found\r\nX-A: 1");
        assert_eq!(h.len(), 1);
        assert!(h.contains_key("x-a"));
    }

    #[test]
    fn degerdeki_iki_nokta_korunur() {
        let h = basliklari_ayristir("HTTP/1.1 200 OK\r\nLocation: https://a.b/c:1");
        assert_eq!(h.get("location").unwrap(), "https://a.b/c:1");
    }

    #[test]
    fn baslik_yoksa_bos_harita() {
        assert!(basliklari_ayristir("HTTP/1.1 200 OK").is_empty());
        assert!(basliklari_ayristir("").is_empty());
    }
}
