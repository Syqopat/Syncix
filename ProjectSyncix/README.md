# Syncix

**Two-way, real-time state synchronisation between Roblox Studio and your editor.**

Whatever is in Studio is in your editor; whatever changes in your editor changes in
Studio. Syncix is not a one-way file pusher — it is a sync engine that keeps both
sides in the same state.

---

## Install

1. Download `syncix.vsix` from the [Releases](../../releases) page.
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

## Configuration

`syncix.toml` in the project root:

```toml
sync_dir = "src"
port = 8080
sourcemap = true
ignore = ["notes/**"]
```

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

## Architecture

| Component | Language | Role |
|---|---|---|
| `core-engine` | Rust | state model, disk writer, HTTP/WebSocket server, CLI |
| `studio-plugin` | Luau | observer and executor inside Studio |
| `vscode-extension` | TypeScript | explorer, inspector, commands, auto install |

Studio talks to the core over HTTP long-poll; the editor uses a WebSocket. Instance
identity (`__syncix_id`) is created once, at creation time, and **never** changes on
rename, reparent or reconnect.

## Development

```bash
cd core-engine && cargo test
```

```bash
sh tools/luau-check.sh $(find studio-plugin/src -name "*.lua")
```

```bash
cd vscode-extension && npm run compile
```

All three components must carry the same version number (`Cargo.toml`,
`package.json`, `init.server.lua`); CI enforces it. The Studio plugin requires a
matching major.minor from the core and refuses to connect otherwise, stating why.

## Licence

MIT — see [LICENSE](LICENSE).
