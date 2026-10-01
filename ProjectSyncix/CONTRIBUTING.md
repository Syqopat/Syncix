# Contributing

## Layout

```
packages/core-engine        Rust: state model, disk writer, HTTP server, CLI
packages/studio-plugin      Luau: observer and executor inside Studio
packages/vscode-extension   TypeScript: explorer, inspector, panel, commands
apps/                       programs built on Syncix
examples/                   sample projects
schemas/                    syncix.schema.json, generated from the settings table
tools/                      the checks and generators CI runs
docs/                       API.md and the rest
```

## Rules the checks enforce

**No product file over 500 lines.** `python tools/check-file-size.py`. Generated data
is exempt and the exemption lists its reason; tests are not counted. A long file is
split by subject into a folder, not by line count into `part1`, `part2`.

**All three components carry the same version.** `python tools/check-versions.py`
compares `packages/core-engine/Cargo.toml`,
`packages/vscode-extension/package.json` and
`packages/studio-plugin/src/Network/Protocol.lua`. The plugin refuses a core whose
major.minor differs, so a release that forgets one of them installs and then will not
connect.

**Comments are in English, and say why.** A comment that repeats the code is noise;
the ones worth writing record the incident behind a decision, so nobody undoes it by
accident. Public items carry doc comments (`///`, `---`, `/** */`); the rest are
ordinary comments.

**Nothing personal in the repository.** No account names, no paths that contain one,
no tokens. `/health` reports the project's name and not its path for this reason.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). The changelog
and the release notes are generated from them, so a message that does not follow the
format is left out of both:

```
feat(plugin): rebuild the Studio panel on the native Studio theme
fix(core): keep an edit made while the core was down
refactor(core)!: split the 3000-line main
```

Types in use: `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `ci`, `chore`. A `!`
or a `BREAKING CHANGE:` footer marks a change that breaks the API or the file format.
The body says what was wrong before, not what the diff shows.

## Tests

```bash
cd packages/core-engine && cargo test
```

Unit tests sit beside the code; `tests/` holds the integration and end-to-end tests,
which start the real binary in a temporary project and play the Studio plugin over
HTTP. They bind ports, so run them one at a time (`-- --test-threads=1`) if your
machine is busy.

```bash
cd packages/vscode-extension && npm test
```

vitest, with a fake `vscode` module in `tests/vscode.mock.ts` - the real one only
exists inside the editor.

```bash
python tools/luau-test.py
```

The plugin's modules are written for Roblox, so the runner builds one chunk: a fake
Roblox API, then each module with its requires turned into lookups the test fills in,
then the test. The modules' own code is untouched.

```bash
sh tools/luau-check.sh packages/studio-plugin/src
python tools/require-check.py
```

Syntax, and the "used but never defined" mistakes that only show up when Studio runs
that line.

## Running from source

```bash
cd packages/core-engine && cargo build --release
cd packages/vscode-extension && npm run compile
```

Point the extension at your build with `syncix.coreExePath` and `syncix.coreCwd`, or
run the core yourself:

```bash
syncix serve 8080
```

A core edited while Studio is connected is the one way to lose work: Studio's next
full sync rewrites the disk from its own tree. Syncix now copies what was on disk into
`.syncix/trash` first and says so, but back up anything you care about before you
restart the core.

## Settings

Every setting is described once, in
`packages/core-engine/src/project/settings.rs`. The warning check, the JSON schema and
the migration all read that table, so adding a setting means adding one entry there -
and regenerating the schema:

```bash
syncix config --schema > schemas/syncix.schema.json
```

A test fails when the committed schema and the table disagree.
