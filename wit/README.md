# Rabbitty plugin interface (WIT)

`world.wit` is the contract between Rabbitty and a plugin. Plugins are
WebAssembly **components** run under wasmtime; the host generates its bindings
with `wit-bindgen`, so you can write one in any language with component tooling
(Rust, C, JS via ComponentizeJS, …).

Everything crosses the boundary as plain data — no host resources, no shared
memory.

## What a plugin can do

Plugins are **sans-IO**. The only host import is `read-config`. Every other
side effect is an `action` the plugin *returns* from `init`, `shutdown`,
`on-event` or `run-command`; the host executes the list after the call returns,
in order. Anything that needs an answer (`query`, `connect`, `schedule`) comes
back later as an `event`.

| | |
|---|---|
| **Run commands** | Declare them in `contributions()`, handle `run-command(id)` → `list<action>` |
| **React to events** | `on-event(ev)` — session start/close, pattern match, cwd, title, focus, tab, selection, bell, setting change, menu activation, plus the answers below. Events carry ids, not screen content: `selection-changed(pane)` tells you *that* the selection moved; `query(selection(pane))` gets you the text |
| **Act** | `write-pty`, `notify`, `open-url`, `set-status`, `schedule`/`cancel-timer`, `open-tab`, `focus-pane`, `close-pane`, `query`, `connect`/`send`/`close` |
| **Contribute UI** | `contributions()` returns commands, output patterns, settings fields, status items, menu items |
| **Supply profiles** | `list-profiles()` |

Answers arrive as events, keyed by the id the plugin chose:

| Action | Reply event |
|---|---|
| `schedule(timer)` | `timer(id)` — once, or every `after-ms` when `repeat` |
| `query(panes)` | `panes(list<pane-info>)` |
| `query(scrollback(range))` | `scrollback(chunk)` — `from` counts lines upward from the bottom of the grid (0 = newest line, viewport included), `count` lines ending there, capped at 10000; `lines` are top-to-bottom and `from` is echoed |
| `query(selection(pane))` | `selection(selection-event)` |
| `connect(request)` | `connected(id)`, then `data(io-frame)` per read, `closed(io-closed)` at EOF or on error — a failed connect sends `closed` with a reason and no `connected` |

`send`/`close` only reach connections this plugin opened; an unknown id is
dropped silently. `connect(local(name))` resolves `$TMPDIR/{name}-{0..9}` on
unix and `\\.\pipe\{name}-{0..9}` on Windows, so Discord IPC is
`local("discord-ipc")`. `connect(tcp)` is a raw stream: no TLS.

Terminal output is **not** streamed to you. Declare `output-pattern` records
instead; the host matches every line natively and calls `on-event` only on a hit.

Rust plugins should build on `plugins/sdk` (`rabbitty-plugin-sdk`): a `Plugin`
trait returning `Vec<Action>`, `export_plugin!`, and a sans-IO `http` helper
that turns a request into actions and `data` frames into a response.

## Capabilities

`manifest()` declares what the plugin requests. The host reviews it before
running anything else, then enforces the grant when executing actions:

| Capability | Gates |
|---|---|
| `read-config` | The `read-config` import — ungranted returns `none` |
| `read-screen` | `query(scrollback)` and `query(selection)` — ungranted is dropped, so no answer ever arrives |
| `notify`, `open-url`, `write-pty` | The action of the same name — ungranted is dropped |
| `control` | `open-tab`, `focus-pane`, `close-pane` |
| `network` | `connect(tcp)` |
| `local-ipc` | `connect(local)` |
| `filesystem` | `wasi:filesystem`, preopened to the plugin's own data directory and nothing else |

`set-status`, `schedule`, `cancel-timer`, `query(panes)`, `send` and `close` need
no capability. The two exceptions to "events carry no screen content" are
deliberate: `output-matched` carries the one line that hit a pattern the plugin
declared, and `menu-activated` carries the selection the user invoked the
plugin's own menu item on. Nothing is granted while `manifest()` runs, so a
plugin cannot influence its own review.

## Lifecycle

```
manifest()       → host reviews and grants capabilities
init()           → one-time setup; returns the first actions
contributions()  → commands, patterns, settings, status and menu items
on-event(…) / run-command(…) / list-profiles()   → steady state, each returning actions
shutdown()       → last call before teardown (disable, reload, app exit); its actions still run, but at app exit a `send` is best-effort
```

Calls are strictly sequential: the host never re-enters the plugin while a
call is in progress, and actions returned by one call are executed before the
next call is made.

A trap — fuel exhaustion, memory limit or a guest panic — **permanently**
poisons the instance. The Component Model refuses re-entry, so the host records
the failure, blocks further calls, and skips `shutdown`. Recovery means a fresh
instantiation.

## Versioning

`package rabbitty:plugin@0.5.0`.

Pre-1.0, breaking changes bump the minor and the host supports exactly one
version — recompile against the new world. From 1.0, breaking changes bump the
major and the previous major is supported for a transition period.
