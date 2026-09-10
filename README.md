# Syncix

Keep Roblox Studio and your editor in sync — in both directions.

Move a part in Studio and the file changes on disk. Edit a script in your editor
and it appears in Studio. Your game stops being locked inside a binary place
file: it becomes ordinary files you can put in Git, diff, review and restore.

[![CI](https://github.com/Syqopat/Syncix/actions/workflows/ci.yml/badge.svg)](https://github.com/Syqopat/Syncix/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](ProjectSyncix/LICENSE)

## Where things are

The project lives in [`ProjectSyncix/`](ProjectSyncix/). Start with its
[README](ProjectSyncix/README.md).

| Folder | What it is |
|---|---|
| [`ProjectSyncix/core-engine`](ProjectSyncix/core-engine) | The sync engine and the `syncix` CLI. Rust. |
| [`ProjectSyncix/studio-plugin`](ProjectSyncix/studio-plugin) | The Roblox Studio plugin. Luau. |
| [`ProjectSyncix/vscode-extension`](ProjectSyncix/vscode-extension) | The editor extension. TypeScript. |
| [`ProjectSyncix/example`](ProjectSyncix/example) | A real sync folder — a small complete game, as Syncix writes it to disk. |
| [`ProjectSyncix/tools`](ProjectSyncix/tools) | Code generation and the checks CI runs. |

## Install

Install **Syncix** by *syqopatz* from the VS Code Marketplace. The engine and the
Studio plugin ship inside the extension; nothing else to download.

Windows only in this release — the engine has not been run on macOS or Linux,
and shipping an untested binary would be a claim rather than a fact.

## Build from source

```
cd ProjectSyncix/core-engine && cargo build --release
cd ../vscode-extension && npm install && npm run compile
```

## License

MIT — see [LICENSE](ProjectSyncix/LICENSE). Changes are recorded in
[CHANGELOG.md](ProjectSyncix/CHANGELOG.md).
