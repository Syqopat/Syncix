// Eklentiyi TEK bir dosyaya paketler.
//
// Neden: Marketplace, icinde ham `node_modules` klasoru olan paketleri
// "suspicious content" diye reddediyor. Yukleme tam olarak bu yuzden iki kez
// geri geldi. Bagimliliklari (bizde yalnizca `ws`) cikti dosyasinin icine
// gomunce node_modules'u hic gondermemiz gerekmiyor.
//
// Ek fayda: paket kuculuyor ve `ws`in eksik kalmasi imkansiz hale geliyor —
// bir kez paketten dusmustu ve eklenti hic acilmamisti.
//
// `vscode` DISARIDA birakiliyor: onu calisma aninda VS Code'un kendisi
// sagliyor, pakete girmesi hem gereksiz hem hatali olur.

const esbuild = require("esbuild");

const izleme = process.argv.includes("--watch");

const ayarlar = {
  entryPoints: ["src/extension.ts"],
  bundle: true,
  outfile: "out/extension.js",
  external: ["vscode"],
  format: "cjs",
  platform: "node",
  // VS Code'un calistirdigi Node surumu; daha yenisini hedeflemek gereksiz
  // donusum yapmamizi onluyor.
  target: "node18",
  sourcemap: false,
  minify: false,
  logLevel: "info",
};

async function calistir() {
  if (izleme) {
    const baglam = await esbuild.context(ayarlar);
    await baglam.watch();
    console.log("izleniyor...");
    return;
  }
  await esbuild.build(ayarlar);
}

calistir().catch((hata) => {
  console.error(hata);
  process.exit(1);
});
