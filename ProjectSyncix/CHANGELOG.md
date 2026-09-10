# Changelog

All notable changes to Syncix are recorded here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
