# Changelog

All notable changes to Syncix are recorded here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/).

## [0.1.1] - 2026-09-10

### Fixed

- **Opening the editor without Studio could empty the sync folder.** Until
  Studio had synced, the model was empty, and the reconciler treated every
  file on disk as stale: one request from the editor was enough to move the
  whole sync folder to the trash. The reconciler now removes files only after
  Studio has completed a full sync in the current session. Files went to the
  trash rather than being deleted, so affected projects can recover them with
  `syncix trash` and `syncix restore`.

## [0.1.0] - 2026-09-10

### Added

- **Selection sync.** Clicking an object in Studio selects it in the editor
  tree, and clicking in the editor selects it in Studio. Selection is transient
  state: it is never written to the model or to disk, because a click should not
  produce a file change.
- **Place identity.** A sync folder now belongs to one place. If a different
  place connects to the same folder, sync stops and asks instead of merging.
  Resolve with `syncix bind --studio` (the place is right, rewrite the folder)
  or `syncix bind --disk` (the folder is right, load it into the place).
- **`syncix bind`** — show or resolve a place/folder mismatch.
- **`tools/require-check.py`** — catches modules that are used but never
  required, and constants that are used but never defined. Neither is caught by
  a syntax check: the file compiles and only fails when Studio executes that
  line.

### Fixed

- **The version-mismatch warning never fired.** The extension looked itself up
  under a hardcoded id that never existed, got nothing back and skipped the
  check. It now reads its own version from the extension context.
- **Two places could silently merge into one folder.** Service UUIDs are the
  same in every place, so two trees landed on the same skeleton and singletons
  such as `StarterPlayerScripts` ended up duplicated. Files were then written
  with disambiguating name suffixes. Sync now stops on a mismatch.
- **Stale files were pushed into a newly opened place.** A file the model did
  not know about was treated as a new object and created in Studio. While sync
  is on hold this no longer happens.
- **Disk changes reached Studio in the wrong wire format.** Property values from
  disk were serialised with serde's externally tagged form (`{"Number":0.5}`)
  while the plugin expects the plain form, so Studio rejected them with
  "unsupported table value for property". Found in normal use on
  `Part.Transparency`.
- **Changes sent while the game is running are held until Play ends** — with
  a caveat found while testing it. The queue never actually triggers on current
  Studio versions: Studio restarts plugins in the game's own server and client
  sessions, those copies stop at the edit-session guard, and the one instance
  still talking to the core lives in the edit tree, which never reports itself
  as running. The intended behaviour happens regardless, because Studio plays
  from a separate copy and the edit tree keeps the change. The code stays as a
  fallback if that isolation ever changes, and says so.
- **The plugin ran inside Play sessions.** Studio also starts plugins in the
  game's server and client sessions; those copies had nothing to sync and filled
  the Output with connection attempts. The plugin now runs only in the edit
  session.
- **The plugin could connect to another project's core.** Port discovery took
  the first healthy core it found. It now skips cores bound to a different
  place.
- **Rect, PhysicalProperties, ColorSequence, NumberSequence and Font could not
  be set from the CLI.** The values were carried by the model and the plugin but
  the command-line text conversion was missing, so they arrived as plain strings
  and were rejected.

### Changed

- **The extension no longer edits your system PATH.** It used to write a
  `syncix.cmd` into your home folder and, on every project open, start a hidden
  PowerShell process with the execution policy bypassed to append that folder
  to the PATH in the registry — without asking. That is a persistent system
  change nobody consented to.
  The shortcut now lives in the extension's own storage folder and is added to
  PATH only for terminals opened inside the editor, through the VS Code API.
  **Syncix: Use CLI Outside the Editor** shows the folder for anyone who wants
  the command in other terminals too; adding it is their decision.
- The `syncix.coreExePath` and `syncix.coreCwd` settings no longer default to a
  path on the developer's machine. With them set, the installed extension ran
  a local dev build instead of the bundled engine.
- The bundled engine binary no longer embeds absolute build paths.

- Place identity is derived from `game.PlaceId`. It was previously stored as a
  Workspace attribute, which is saved with the place — closing Studio without
  saving lost the identity and the next session looked like a different place.
  The attribute remains as a fallback for places that have never been saved.

### Branding

- Added a logo and a sidebar icon. The mark is two arrows — one to the editor,
  one back to Studio — so it states what the product does. The sidebar entry
  previously pointed at an icon file that did not exist.
- The Studio toolbar button no longer shows Roblox's built-in **Robux** icon,
  which had nothing to do with Syncix. It shows the name only; a real icon needs
  an image uploaded to Roblox, which is the account owner's decision.
  `vscode-extension/resources/logo.png` is ready to upload.

### Known limitations

- **Windows only.** The macOS and Linux build targets were removed from the
  release workflow on purpose: the code is written to be portable but has never
  been run on those platforms, and shipping an untested binary would be a claim
  rather than a fact. Roblox Studio does not exist on Linux at all. On a
  non-Windows machine the extension now says so instead of failing silently.
- Terrain is out of scope and this is not surfaced to the user.
- `.rbxmx` export skips Rect, Font, ColorSequence, NumberSequence,
  PhysicalProperties, BrickColor and instance references; their XML forms are
  composite and writing a wrong one is worse than omitting it. Direct sync with
  Studio carries all of them.
- The VS Code property inspector shows the newer value types read-only; they can
  be edited from the CLI.
- Duplicating a place with "Save As" copies its identity, so both places can
  bind to the same folder without a conflict being raised.
