# This folder mirrors Roblox Studio

The file tree here is an exact copy of the Studio Explorer and is written
automatically by Syncix. Both sides are live: a change here reaches Studio, and a
change in Studio reaches this folder.

## Safe to edit by hand

- `.lua` / `.luau` — script source
- `.txt` — StringValue content, plain text
- `.csv` — LocalizationTable entries (Key, Source, Context, Example, then one
  column per locale)
- `.meta.json` — properties and attributes of the file next to it

## Better left alone

- `.json` — instance definitions. Syncix generates these from Studio; use
  `syncix set` or Studio itself instead of editing them by hand.

## Your own files

Files you put in this folder — READMEs, notes, images — are never deleted. Syncix
only touches files it could have produced itself. To keep a managed extension
(a `.lua` file, for example) out of the sync, add it to the `ignore` list in
`syncix.toml` at the project root.
