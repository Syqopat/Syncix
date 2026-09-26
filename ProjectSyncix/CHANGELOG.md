# Changelog

All notable changes to Syncix are recorded here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/).

## [0.1.6] - 2026-09-27

### Added

- **Renaming a data file renames the object.** `Box.part.json` -> `Kutu.part.json` in
  the editor renames the object in Studio; the file keeps the name you gave it.
  The object is recognised by the `syncix_id` inside the file, so the name is
  yours to change.
- **A data file written by hand becomes an object.** Write
  `Wall.part.json` with as little as `{ "name": "Wall" }` in a synced folder and
  Syncix creates it in Studio: the class comes from the file name (or a
  `class_name` in the file), the parent from the folder it sits in, and a real
  identity is written back into the file. Such a file used to reach Studio
  without an identity or a parent and was dropped.
- **A damaged identity is repaired, not the whole file.** Break the identity in a
  file name (`RampA_1a2b3c4d` -> `RampA_deadbeef`) or inside the file and Syncix
  puts that one value back from the tree; nothing is re-created and the rest of
  the file is left as you wrote it.

### Fixed

- **Commands could land on the wrong object after a copy.** The plugin's cache
  and the objects' own identities could drift apart (a Ctrl+D copy filed under
  its original's identity by an older plugin): a command meant for one object
  moved another, while `pull` reported everything consistent. The cache now
  checks each lookup against the object's identity and repairs itself, and a
  full sync brings the cache in line with the tree it sends.
- **An older Syncix could put its old Studio plugin back.** Every editor with
  Syncix installs its plugin on start whenever the file differs, so an older
  copy (another editor, an old install) silently replaced a newer plugin. The
  installed plugin's version is now recorded and a newer one is left alone.

## [0.1.5] - 2026-09-19

### Fixed

- **Binding an empty folder to a place could rewrite scripts in Studio.** The
  engine never recognised the files it wrote itself: it recorded them as
  `./src/...` while the file watcher reported `C:\...\./src\...`. While the
  files already matched the model this did no harm, but the first write of a
  place into an empty folder was taken for the user creating and renaming
  scripts. A fallback then matched each file to any sibling script whose file
  did not exist yet, and a folder's owner was looked up by name anywhere in the
  place. The result was sent to Studio: in a place with two PlayerModules,
  scripts were renamed, nested copies were created and meta properties such as
  Disabled were applied (26 renames and 54 new scripts in the reported case;
  nothing was deleted). Paths are now compared in one spelling, the fallback is
  gone, a folder's owner is found by its path, and a move is only assumed when
  exactly one deleted script fits.
- **An excluded class kept syncing if it was already tracked.** `ignore_classes` was
  checked only when the plugin first saw an instance, so one tracked before the
  setting arrived kept sending property changes. A debug adornment that recolours
  itself every frame filled Studio's HTTP limit that way and the plugin lost its
  connection. Property changes now check the class too.
- **`syncix import` was very slow.** Every instance and every property was its own
  command, each followed by a pause: 1076 instances took about 6.5 minutes. The
  properties and a script's source now travel with the create, as one command and
  one message to Studio per instance, with no pauses. The same import against the
  core now takes under a second, and the folder is written once instead of
  hundreds of times.
- **The core's log was written beside the project.** A core started in the
  project root wrote `../syncix-core.log`, so every project in a folder shared
  one log. It is now `.syncix/syncix-core.log` in the project itself.
- **A property changing every frame could fill Studio's HTTP limit.** The plugin
  sent a packet on every frame something changed. It now sends at most ten
  packets a second, and a property sent less than half a second ago waits with
  only its last value kept, so a spinning part or a recolouring debug drawing
  costs two updates a second instead of sixty. A single edit still goes out at
  once.
- **Pressing reconnect repeatedly started parallel connection loops.** An attempt
  still discovering, or waiting to retry, kept running next to the new one. Each
  attempt is now numbered and a replaced one stops; a lost connection reported by
  both a send and a poll starts one retry loop, not two.
- **Team Create: one new object could get two identities.** An object a teammate
  creates reaches your Studio before the identity their plugin gives it, so your
  plugin named it too and the two cores knew it by different identities. Children
  added later pointed at a parent one core did not know, and files were rewritten
  on the next connect. The plugins now settle on one identity without talking to
  each other (the smaller one wins) and the core follows (a new REKEY message
  that keeps the object's files, children and references).
- **A copy took the original's identity.** Ctrl+D, copy and paste or a clone
  carries the attributes, identity included, and the copy overwrote the original
  in the cache and on disk. A copy now gets its own identity; an object brought
  back by undo keeps its old one.
- **Scripts are written the way Roblox asks plugins to.** Source changes use
  `ScriptEditorService:UpdateSourceAsync`, which also works for scripts open in
  the editor and under Team Create's Collaborative Editing; the direct write is
  the fallback.

## [0.1.4] - 2026-09-12

### Added

- **"Did you mean ...?" everywhere a name is typed.** A slip is answered with the
  closest real spelling instead of a bare "not found" or "invalid":
  - commands and options: `syncix staus`, `rm Box --yse`. A misspelt option is no
    longer taken as a plain argument, so `attr Box Hp --delet` no longer sets Hp
    to "--delet" and `tag Box --nnoe` no longer adds a tag called "--nnoe";
  - targets and dotted paths in every command (`Workspace.Tycons` ->
    `Workspace.Tycoons`), and `syncix find`;
  - property names (`set Box Szie 4,1,2` -> Size), checked against Roblox's class
    table; a name close to a real one is refused before anything is sent;
  - class names for `syncix new` (`Prat` -> Part; `part` is created as Part);
  - values: colours (`oragne`), true/false (`ture`) and enum items (`Neno` ->
    Neon, `Enum.Materail.Neon`). A value that cannot be what the property holds
    (a word for a number, two numbers for a Vector3) is refused instead of
    reaching Studio as text while the terminal printed success;
  - `syncix restore` names, runs and `--class`, and `syncix import` file names;
  - syncix.toml: unknown sections and settings, sync modes, service and class
    names, listed by `syncix config` and in the engine log;
  - Studio's Output, when a file names a property or enum item that does not exist.
- More colour names for `syncix set` (magenta, gold, turquoise, skyblue, ...), also
  written with spaces: `sky blue`.
- An enum item typed in the wrong case (`neon`) is written the way Roblox spells it.

### Fixed

- **Large imports no longer lose or duplicate instances.** When Studio resent its
  tree (FULL_SYNC) while an import was running, the engine rebuilt its model from
  that tree and dropped everything Studio had not applied yet; the files went to
  the trash and the instances came back minutes later as duplicates. Every message
  to Studio is now numbered, the plugin reports how far it got and what it is still
  holding, and the engine keeps what is on its way: new instances, property changes,
  renames, moves, deletions, attributes and tags. Anything Studio was handed but did
  not apply is sent again.
- **Big places connect again.** Studio refuses to send more than 1024 KB per
  request. A place whose tree passed that could never finish connecting: the
  refusal looked like a lost connection, and every reconnect resent all failed
  messages at once until Studio's request limit was hit too. The tree now goes in
  parts the engine puts back together, big batches are split, a refused or
  rate-limited message no longer triggers a reconnect, and stale messages are not
  resent after one (the tree that follows already carries them).
- Changes made in the editor while sync is paused are applied when it resumes,
  instead of being moved to the trash.
- The plugin fetches up to 64 messages per request instead of one, so a large
  import no longer runs into Studio's HTTP limit and a reconnect storm. A reconnect
  can no longer leave two polling loops running, and one failing message no longer
  stops the loop or leaves Studio's change reporting switched off.
- `syncix ls` and `syncix tree` accept dotted paths (`Workspace.Tycoons`).
- Import: properties Studio writes under serialized names (`size`, `shape`,
  `Color3uint8`) land on the right member; enum values the engine has no name for
  are kept instead of skipped; `FontFace` is imported.
- `syncix set` with a reference to a missing target no longer stops the engine.

### Changed

- `syncix set` understands what was meant: property names in any case (`size` is
  `Size`), and colours as `#FF8800`, `#F80`, `255,136,0`, `rgb(255, 136, 0)` or a
  name such as `orange`. A value that is not a colour is refused before anything is
  sent, with the accepted forms.
- Parts Syncix creates come out with smooth top and bottom surfaces.
- The plugin corrects a wrong-case property name in a file and accepts colour text
  for Color3 properties.

### Removed

- The `play_mode` setting. It never took effect: changes always reach the edit
  session, even during a playtest, which picks them up after a restart. A
  `syncix.toml` that still sets it gets a warning.

## [0.1.3] - 2026-09-11

### Added

- **Restore single instances.** `syncix restore <name>` brings back the newest
  copy of one instance (a folder brings its contents) instead of a whole trash
  run, which also held things removed on purpose. Narrow it down with
  `--in <path>`, `--class <class>` and `--since <30m|2h|1d>`; `--dry-run` shows
  what would come back. `syncix trash --files` lists single files. A script's
  `.meta.json` comes back with it, and an existing file is never overwritten.
  When one name belongs to several instances, restore lists them and asks for
  `--in <path>` (or `--all`) instead of bringing every copy back.

### Fixed

- **Importing a whole place.** `syncix import` of a file exported without a
  target tried to create Lighting, ReplicatedStorage and the other services as
  new children, failed, and left the parent name ambiguous (409). Services and
  singleton containers now merge into the ones the place already has; their own
  settings are left unchanged. The parent is resolved once, before anything is
  created. The engine refuses to create a service outright.
- **Nothing lands in Workspace by accident.** The plugin put an instance whose
  parent it could not find into Workspace, where the engine never knew about it.
  Such an instance now waits for its parent, and an instance already in Studio is
  no longer moved when its parent is unknown.
- **No phantoms for what Studio cannot create.** When Studio refuses to create
  an instance, the plugin now tells the engine, which drops it (with its
  subtree) and moves its files to the trash; before, the engine kept a copy
  Studio never had. If Studio already has the one instance of such a class
  without an identity, the plugin adopts it instead. TextChatService's four
  configurations (BubbleChatConfiguration and the like) are treated as
  singletons on import.
- A folder restored from the trash could list children that were never
  restored, and `syncix verify` reported them as missing. The engine now keeps
  only children that exist, and a moved instance leaves its old parent's list.
- `syncix new Lighting` (or any class that exists once per place) fails with a
  message instead of printing "Created" for nothing.
- `syncix set <target> CFrame x,y,z` on a part whose CFrame the engine has not
  seen yet sends a CFrame rather than a Vector3 Studio refuses. (With a known
  CFrame the rotation was, and is, kept.)
- The last Turkish log lines, comments and test messages are now in English.

### Changed

- Non-script instances serialized to JSON now reflect their Roblox class name in the file extension (`<name>.<class>.json` and `init.<class>.json`, such as `Baseplate.part.json` or `init.folder.json`) instead of generic `.json`.
- Backward-compatible path and uuid resolution ensures existing `.json` files are still matched and handled during migration.
- The example project and VS Code explorer now use the class-specific extension convention.

## [0.1.2] - 2026-09-11

### Changed

- **English throughout.** Every identifier, comment, log line, test fixture,
  CI job and tool message is in English now. Three of these changes are
  visible to users:
  - Settings the plugin saves in Studio use English keys. A folder approval or
    a manual port override saved by an earlier build is not carried over;
    Studio asks once more.
  - The Turkish CLI aliases `--onayla`, `--sil` and `yayinla` are gone. Use
    `--confirm`, `--delete` and `upload`, which have always been the documented
    forms.
  - The place-conflict object in `/health` uses `folder_place`,
    `incoming_place`, `incoming_name` and `incoming_place_id`.
- The example game's localization table ships Spanish instead of Turkish as its
  second language.

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
