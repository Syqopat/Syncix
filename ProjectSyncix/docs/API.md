# The core's HTTP API

The core binds `127.0.0.1` only, on the port in `.syncix/port`. While it runs it
serves its own description:

- `http://127.0.0.1:<port>/docs` — the API, to read and to try
- `http://127.0.0.1:<port>/openapi.json` — the same thing as OpenAPI 3

Both are generated from the handlers, so they cannot drift away from the code. What
follows is the part that is worth saying in prose.

## The token

Every call except `/health` carries the project's token:

```
X-Syncix-Token: <the token>
```

The token is 48 hex characters, written by the core to `<project>/.syncix/token`
(mode `0600` where the platform has modes) and generated fresh on every start. Clients
find it like this:

- the CLI and the editor extension read the file;
- the Studio plugin cannot read files, so it takes the token from `/health`.

A WebSocket cannot carry a custom header from every client, so `/rpc` also accepts
`?token=<the token>`.

**Why a token at all, on a local port.** Any web page you had open could reach
`127.0.0.1` and, before this, call `/shutdown` or push changes into your game. The
token stops that, and the core sends **no CORS header at all**, so a browser will not
hand a reply to page scripts even when the request goes through. `/health` is
therefore the only call that answers without a token, and it reports the project's
name, never its path — the path carried the user's account name.

## What a status code means

| Code | Meaning |
|---|---|
| 200 | Here is the answer. |
| 202 | Taken. The core has it; Studio has not applied it yet. |
| 400 | The request itself is wrong (unknown `event_type`, `data` that is not an object). |
| 401 | Missing or wrong token. |
| 404 | No such target. |
| 409 | The target matches more than one object, so Syncix will not guess. |
| 500 | The core could not pass the work on. |

Every failure answers with the same body, so a client never has to parse prose:

```json
{ "error": "not_found", "message": "No instance called \"Ground\"." }
```

## The routes

| Route | What it is for |
|---|---|
| `GET /health` | identity, state, the token, the job pool. No token needed. |
| `GET /sync/poll?batch=N` | the Studio plugin's long poll; waits up to 10 s, up to 64 messages. |
| `POST /sync/push` | everything the plugin sends: tree changes, full syncs, selection, metrics. |
| `GET /rpc` | the editor's WebSocket. |
| `POST /commands` | one change, asked for by the CLI or the editor. |
| `GET /model/tree` | every object, flat: id, name, class, parent. |
| `GET /model/object?target=` | one object with its properties. |
| `GET /model/verify` | orphaned references and the class distribution. |
| `GET /model/sourcemap` | `sourcemap.json`, as luau-lsp reads it. |
| `GET /model/export?target=` | the tree, or one subtree, as Roblox XML. |
| `POST /core/stop` | end the process. |

Paths say what they act on. The older names (`/object`, `/build`, `/shutdown`) did not
say which part of the system they belonged to, and `/shutdown` took a request from
anyone at all.

## Targets

`target=` takes a full UUID, an 8-character short UUID, a name, or a dot path
(`Workspace.Simulator.SellPad`). A name that matches more than one object is a `409`,
with the candidates in the message.

## Example

```bash
TOKEN=$(cat .syncix/token)
PORT=$(cat .syncix/port)

curl -s "http://127.0.0.1:$PORT/health" | jq '{project, version, port}'

curl -s -H "X-Syncix-Token: $TOKEN" \
     "http://127.0.0.1:$PORT/model/object?target=Workspace.Ground" | jq .

curl -s -X POST -H "X-Syncix-Token: $TOKEN" -H "Content-Type: application/json" \
     -d '{"event_type":"PROPERTY_UPDATE","data":{"target":"Ground","property":"Transparency","value":0.5}}' \
     "http://127.0.0.1:$PORT/commands"
```

## Environment variables

| Variable | Used by | What it does |
|---|---|---|
| `SYNCIX_API_KEY` | `syncix upload` | Roblox Open Cloud key. Read from the environment only — a key in `syncix.toml` is refused, because that file goes into version control. |
| `RUST_LOG` | the core | log filter, e.g. `RUST_LOG=debug` or `RUST_LOG=syncix_core=trace`. Default `info,syncix_core=debug`. |

Nothing else is read from the environment. Every other setting lives in
`syncix.toml`; `syncix config` prints what is actually in effect and
`syncix config --schema` prints the JSON schema for your editor.

## Files the core writes

| Path | What it is |
|---|---|
| `.syncix/port` | the port actually bound, so nobody has to guess 8080. Removed on exit. |
| `.syncix/token` | the token above. |
| `.syncix/place` | which Roblox place this folder is bound to. |
| `.syncix/syncix-core.log` | the core's log. |
| `.syncix/trash/<run>/` | files the reconciler removed or replaced, newest runs kept. |
| `sourcemap.json` | for luau-lsp, when `editor.sourcemap` is on. |
