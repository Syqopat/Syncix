# Syncix

Keep Roblox Studio and your editor in sync — in both directions.

Move a part in Studio and the file changes on disk. Edit a script in your
editor and it appears in Studio. Your game stops being locked inside a binary
place file: it becomes ordinary files you can put in Git, diff, review and
restore.

> **Windows only in this release.** See [Platform support](#platform-support).

## What gets synced

- **Instances and hierarchy** — creation, deletion, renaming and reparenting
- **Properties** — 1325 properties across 627 classes, generated from Roblox's
  own API dump, including `CFrame` with its full rotation matrix, `Color3`,
  `UDim2`, `ColorSequence`, `NumberSequence`, `Font`, `Rect`,
  `PhysicalProperties` and asset ids
- **Scripts** — source code as plain `.lua` files
- **Attributes** and **CollectionService tags**
- **References between instances** — `ObjectValue.Value`, `Motor6D.Part0`,
  `Model.PrimaryPart`
- **Service settings** — `Lighting.ClockTime`, `Workspace.Gravity` and the rest

## Getting started

1. Install this extension.
2. Open a folder for your project.
3. Open the Syncix view in the sidebar and choose **Create Syncix Project**.
   That writes `syncix.toml`, installs the Studio plugin, and starts the sync
   engine.
4. Open your place in Roblox Studio. If HTTP requests are off, the plugin says
   so — turn them on under *File → Game Settings → Security*.

The status bar tells you which of three states you are in: the engine is down,
the engine runs but Studio is not connected, or everything is in sync.

## Settings

Everything lives in `syncix.toml` next to your project, and every setting is
documented in the file the project wizard writes for you. The one people reach
for first:

```toml
[sync]
# two_way         Studio <-> disk (default)
# studio_to_disk  Studio is the source; disk changes are not sent back
# disk_to_studio  the file system is the source
# manual          nothing syncs automatically
mode = "two_way"
```

There are seventeen settings in total, covering what gets synced, how deletions
are handled, and how the engine behaves while a game is running.

## Safety

A sync tool's first duty is not to lose your work.

- **Nothing is deleted without a copy.** Files the reconciler removes go to a
  trash folder; `syncix trash` lists them and `syncix restore` puts them back.
- **A folder belongs to one place.** If a different place connects to the same
  folder, sync stops and asks instead of merging the two trees.
- **Undo works.** Changes Syncix makes go on Studio's undo stack, so Ctrl+Z
  takes them back.
- **Moving a file moves the instance** rather than creating a duplicate.

## Command line

Terminals opened inside the editor get a `syncix` command automatically. It
talks to the same engine:

```
syncix status          # engine and Studio connection
syncix tree            # show the tree
syncix props <target>  # every property of an instance
syncix set <target> <property> <value>
syncix trash           # what the reconciler removed
syncix config          # the settings actually in effect
```

`syncix --help` lists all 30 commands.

Syncix never edits your system PATH. To use the command in other terminals
too, run **Syncix: Use CLI Outside the Editor** — it shows the folder to add.

## What ships inside this package

For transparency, since this extension bundles a compiled program:

- `resources/bin/win32-x64/syncix-core.exe` — the sync engine. It is written in
  Rust; the source is in `core-engine/` in the repository below and it is built
  by the repository's own GitHub Actions workflow. It listens only on
  `127.0.0.1` and talks to nothing outside your machine.
- `resources/SyncixPlugin.rbxm` — the Roblox Studio plugin, built from
  `studio-plugin/`. The extension copies it into your local Roblox plugins
  folder so you do not have to install it separately.

Syncix does not collect telemetry and makes no network requests beyond
localhost. Publishing your game to Roblox is a separate, explicit command that
never runs on its own.

## Platform support

This release ships a Windows engine binary only. The code is written to be
portable, but it has not been run on macOS or Linux, and shipping an untested
binary would be a claim rather than a fact. Roblox Studio does not exist on
Linux at all.

On a non-Windows machine the extension says so rather than failing silently.
You can build the engine from source and point `syncix.coreExePath` at it.

## Source

[github.com/Syqopat/Syncix](https://github.com/Syqopat/Syncix) — MIT licensed.
Changes are recorded in `CHANGELOG.md`.
