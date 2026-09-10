# Example project

This folder is a real Syncix sync folder, not a mock-up. Every file in it was
written by Syncix from a Roblox Studio place, and it is the sync folder this
repository's own `syncix.toml` points at.

It contains **Coin Simulator** — a small but complete game: collect coins in a
zone, sell them at the green pad, buy upgrades, unlock a second zone worth five
times as much.

## What it demonstrates

| File | Feature |
|---|---|
| `ServerScriptService/SimulatorMain.server.lua` | Scripts sync as plain `.lua` — write them in the editor, run them in Studio |
| `ServerScriptService/SimulatorMain.meta.json` | Script properties (`Disabled`, `Enabled`, `LinkedSource`) live beside the source |
| `Workspace/Simulator/Zones/Zone1Floor.json` | Full part properties — `CFrame` with its rotation matrix, `BrickColor`, `Material`, `Color3`, surfaces |
| `ReplicatedStorage/WelcomeMessage.txt` | A `StringValue` is a text file. Change the sentence, save, and the banner in Studio changes |
| `ReplicatedStorage/GameStrings.csv` | A `LocalizationTable` is a spreadsheet. Add a column, add a language |
| `TextChatService/…` | Service configuration, including nested UI instances, is synced like anything else |
| `Lighting/Atmosphere.json` | Service settings — the whole lighting rig is version-controlled |

## How the files are laid out

The tree mirrors the Studio Explorer exactly. An instance with children becomes
a folder holding an `init.json`; an instance without children is a single file.
Identity is carried by the `syncix_id` field, not by the path, so renaming or
moving a file renames or reparents the instance instead of creating a copy.

## Safe to edit by hand

- `.lua` / `.luau` — script source
- `.txt` — `StringValue` content, plain text
- `.csv` — `LocalizationTable` entries (Key, Source, Context, Example, then one
  column per locale)
- `.meta.json` — properties and attributes of the file next to it

## Better left alone

- `.json` — instance definitions. Syncix generates these from Studio; prefer
  `syncix set` or Studio itself over editing them by hand.

## Your own files

Files you add here — READMEs, notes, images — are never deleted. Syncix only
touches files it could have produced itself. To keep a managed extension (a
`.lua` file, say) out of the sync, add it to the `ignore` list in `syncix.toml`
at the project root.
