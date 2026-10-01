# Syncix

**Two-way, real-time state synchronisation between Roblox Studio and your editor.**

Whatever is in Studio is in your editor; whatever changes in your editor changes in
Studio. Syncix is not a one-way file pusher — it is a sync engine that keeps both
sides in the same state.

---

## Install

1. Download `syncix.vsix` from the
   [latest release](https://github.com/Syqopat/Syncix/releases/latest).
2. In VS Code open the Extensions panel, use the `...` menu and pick
   **Install from VSIX...**, then select the downloaded file.
3. Open your project folder in VS Code.

That is all. The Studio plugin and the core engine ship inside the package — nothing
else to download. On first launch the extension installs the Studio plugin and makes
the `syncix` command available in the editor's terminals. It never edits your
system PATH.

## Use

Open the project folder in VS Code and open Roblox Studio. The connection is made
automatically; a status bar reading `Syncix: connected (N)` means everything works.

Instances are written under your sync folder as an exact mirror of the Studio
Explorer. Editing a `.lua` file sends the change straight to the script in Studio.

### Command line

The `syncix` command is installed with the extension and works from any terminal.

```
syncix status                 core and Studio connection, metrics
syncix verify                 check model/disk consistency
syncix pull                   ask Studio to resend the tree (source of truth)
syncix selftest               run an end-to-end scenario against real Studio

syncix sourcemap [-o file]    generate sourcemap.json for luau-lsp
syncix build [-o file] [target]
                              export the tree as Roblox XML (.rbxmx)
syncix upload                 show what would be published (does nothing)
syncix upload --confirm       actually publish to Roblox

syncix tree [target]          show the tree
syncix ls [target]            list a node's children
syncix find <word>            search by name or class
syncix props <target>         show every property of an instance

syncix new <class> [name] [parent]
syncix rename <target> <new name>
syncix rm <target>
syncix mv <target> <new parent>
syncix set <target> <property> <value>
syncix attr <target> <name> <value>
syncix attr <target> <name> --delete

syncix up [port]              start the core in the background
syncix serve [port]           run the core in this terminal
syncix down                   stop the core
syncix init                   create syncix.toml and the sync folder
```

**Aliases:** `new`=`create`=`mk`, `rm`=`del`=`delete`, `rename`=`rn`, `mv`=`move`,
`ls`=`list`, `find`=`search`, `props`=`show`=`cat`, `verify`=`check`, `pull`=`resync`,
`up`=`start`, `down`=`stop`.

**Targets:** full UUID, 8-character short UUID, name, or dot path
(`Workspace.Simulator.SellPad`). If a name matches more than one instance, Syncix
lists the candidates instead of picking one at random.

**Values:** `5`, `true`, `text`, `0,0.5,-60` (Vector3), `#ff8800` (colour),
`Enum.Material.Neon`.

```bash
syncix new Part Ground Workspace
```

```bash
syncix set Ground Position 0,0.5,-60
```

```bash
syncix set Ground Color "#5aa832"
```

### Command palette

Press `Ctrl+Shift+P` and type `Syncix:` to reach the same operations — reconnect,
restart the core, install the plugin, open the inspector, run the self test and more.

### The panels

Both sides have the same three views, on their host's own theme.

**In the editor**, the Syncix icon in the activity bar: the workspace explorer and the
**Syncix** panel — *Status* (what state sync is in, the project, the port, objects,
conflicts, versions), *Live* (what is crossing right now) and *Settings* (whether the
core starts with the project, the CLI, and **Send feedback**, which opens the issue
form).

**In Studio**, the Syncix button in the toolbar: the same Status, Live and Settings,
where Settings also holds the port — leave it on *Automatic* unless you pinned one.

## Configuration

`syncix.toml` in the project root. Every key is optional and the file is read when the
core starts:

```toml
[files]
sync_dir = "src"
ignore = ["notes/**"]

[sync]
mode = "two_way"        # or studio_to_disk, disk_to_studio, manual
debounce_ms = 120

[server]
port = 8080

[editor]
sourcemap = true
```

```bash
syncix config
```

prints what is actually in effect, with the settings it did not understand and the
closest spelling for each. For completion and validation in your editor:

```bash
syncix config --schema > syncix.schema.json
```

An older, flat `syncix.toml` (keys at the top level, no sections) is rewritten into
sections the first time the core reads it. Your comments stay where they were and the
file as it was is kept beside it as `syncix.toml.bak`.

Every setting, with its range and default, is in
[`schemas/syncix.schema.json`](schemas/syncix.schema.json); the commented example is in
`packages/vscode-extension/resources/syncix.example.toml`.

When the port is taken Syncix moves to the next one and writes the chosen port to
`.syncix/port`. The editor and the Studio plugin read it from there, so you can have
several projects open at the same time.

### Pinning a specific port

With two projects open the Studio plugin scans ports 8080-8089 and picks the **first**
one that answers — which may be the wrong project. To make it explicit:

```bash
syncix serve 25565
```

Then press the **Syncix** button in the Studio toolbar and type `25565` into the
**Port** field. Only that port is tried; if no Syncix core is there, nothing connects
and the reason is printed to the Output window.

The panel also lists every running project by name and folder, so you can just click
one. Leaving the field empty (the default) lets Syncix find the core itself.

Note: when a port is given explicitly on the command line the core does **not** fall
back to another one. Otherwise the port you typed into the plugin and the port the
core actually used could drift apart.

## File formats

| On disk | In Studio |
|---|---|
| `Name.server.lua` | `Script` |
| `Name.client.lua` | `LocalScript` |
| `Name.lua` / `.luau` | `ModuleScript` |
| `Name.txt` | `StringValue` (file content is `Value`) |
| `Name.csv` | `LocalizationTable` (translations, editable in a spreadsheet) |
| `Name.meta.json` | properties and attributes for the file next to it |
| `Name.json` | everything else |

Files Syncix could not have produced itself — a README, a note, an image — are
**never** deleted. Use `ignore` in `syncix.toml` when you also want a managed
extension (such as a `.lua` file) left out of the sync.

## Publishing

Syncix can build the place file from the live model and publish it through Roblox
Open Cloud.

```toml
[upload]
universe_id = 1234567890
place_id    = 9876543210
```

The API key is **never stored in project files**, only read from the environment:

```bash
export SYNCIX_API_KEY="paste-your-key-here"
```

`syncix upload` publishes **nothing** by default — it prints what it would do and
stops. Publishing requires `--confirm`. The published version is what players see and
it cannot be undone, which is why there are two gates.

If a key ends up inside `syncix.toml`, Syncix refuses to publish and says why — that
file goes into version control and the key would become public.

## The API

The core serves its own description while it runs: `http://127.0.0.1:<port>/docs` to
read and try, `/openapi.json` for a client generator. Every call except `/health`
carries the project token from `.syncix/token`, and the core sends no CORS header, so
no web page can read a reply. [docs/API.md](docs/API.md) has the status codes, the
routes, the environment variables and the files the core writes.

## Architecture

| Component | Language | Role |
|---|---|---|
| `packages/core-engine` | Rust | state model, disk writer, HTTP/WebSocket server, CLI |
| `packages/studio-plugin` | Luau | observer and executor inside Studio |
| `packages/vscode-extension` | TypeScript | explorer, inspector, commands, auto install |

Studio talks to the core over HTTP long-poll; the editor uses a WebSocket. Instance
identity (`__syncix_id`) is created once, at creation time, and **never** changes on
rename, reparent or reconnect.

## Development

```bash
cd packages/core-engine && cargo test          # unit, integration and end-to-end
cd packages/vscode-extension && npm test       # vitest, with a fake vscode module
python tools/luau-test.py                      # the plugin's modules under the luau CLI
```

All three components must carry the same version number
(`packages/core-engine/Cargo.toml`, `packages/vscode-extension/package.json`,
`packages/studio-plugin/src/Network/Protocol.lua`); `tools/check-versions.py` and CI
enforce it. The Studio plugin requires a matching major.minor from the core and
refuses to connect otherwise, stating why.

[CONTRIBUTING.md](CONTRIBUTING.md) has the rest: the layout, the 500-line rule, the
commit format the changelog is generated from, and how to run the core from source
without losing work.

## Licence

MIT — see [LICENSE](LICENSE).
