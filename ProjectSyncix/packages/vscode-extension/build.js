// Bundles the extension into a SINGLE file.
//
// Why: dependencies (only `ws` here) are embedded in the output file, so
// node_modules never has to ship with the package, and `ws` can no longer go
// missing from it — it did once, and the extension never opened.
//
// `vscode` is left EXTERNAL: VS Code provides it at runtime; bundling it would
// be both unnecessary and wrong.

const esbuild = require("esbuild");

const watch = process.argv.includes("--watch");

const options = {
  entryPoints: ["src/extension.ts"],
  bundle: true,
  outfile: "out/extension.js",
  external: ["vscode"],
  format: "cjs",
  platform: "node",
  // The Node version VS Code runs; targeting it avoids needless down-level
  // transforms.
  target: "node18",
  sourcemap: false,
  minify: false,
  logLevel: "info",
};

async function run() {
  if (watch) {
    const context = await esbuild.context(options);
    await context.watch();
    console.log("watching...");
    return;
  }
  await esbuild.build(options);
}

run().catch((error) => {
  console.error(error);
  process.exit(1);
});
