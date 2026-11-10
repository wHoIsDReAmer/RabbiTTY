# Rabbitty plugin interface (WIT)

`world.wit` is the contract between Rabbitty and a plugin. Plugins are
WebAssembly **components** run under wasmtime; the host generates its bindings
with `wit-bindgen`, so you can write one in any language with component tooling
(Rust, C, JS via ComponentizeJS, …).

Everything crosses the boundary as plain data — no host resources, no shared
memory.

## What a plugin can do

Plugins are **sans-IO**. The only host import is `read-config`. Every other
side effect is an `action` the plugin *returns* from `init`, `on-shutdown`,
`on-event` or `run-command`; the host executes the list after the call returns,
in order. Anything that needs an answer (`query`, `connect`, `schedule`) comes
back later as an `event`.

| | |
|---|---|
| **Run commands** | Declare them in `contributions()`, handle `run-command(id)` → `list<action>` |
| **React to events** | `on-event(ev)` — session start/close, pattern match, cwd, title, focus, tab, selection, bell, setting change, menu activation, plus the answers below. Events carry ids, not screen content: `selection-changed(pane)` tells you *that* the selection moved; `query(selection(pane))` gets you the text |
| **Act** | `write-pty`, `notify`, `show-text`, `set-clipboard`, `open-url`, `set-status`, `schedule`/`cancel-timer`, `open-tab`, `focus-pane`, `close-pane`, `query`, `connect`/`send`/`close` |
| **Contribute UI** | `contributions()` returns commands, output patterns, settings fields, status items, menu items; `set-contributions` replaces them at any time, and a status item the plugin declares again keeps the text it last set. A `secret` setting is kept in the OS keychain instead of `config.toml`, shown masked, and reaches the plugin through `read-config` like any other |
| **Supply profiles** | `set-profiles(list)` at any time, from `init` or after fetching an inventory; each call replaces the plugin's list, and a restarted plugin starts with none |
| **Run a tab** | `connect(tab(title))` opens a tab whose process side is the plugin — see below |

Answers arrive as events, keyed by the id the plugin chose:

| Action | Reply event |
|---|---|
| `schedule(timer)` | `timer(id)` — once, or every `after-ms` when `repeat` |
| `query(panes)` | `panes(list<pane-info>)` |
| `query(scrollback(range))` | `scrollback(chunk)` — `from` counts lines upward from the bottom of the grid (0 = newest line, viewport included), `count` lines ending there, capped at 10000; `lines` are top-to-bottom and `from` is echoed |
| `query(selection(pane))` | `selection(selection-event)` |
| `query(prompt-input(pane))` | `prompt-input(prompt-input-event)` — what is typed at the shell prompt, from the end of the prompt to the cursor, so a right prompt or a suggestion drawn past the cursor is left out; `none` when the shell does not mark its prompt, while a command runs, or on the alternate screen |
| `connect(request)` | `connected(id)`, then `data(io-frame)` per read, `closed(io-closed)` at EOF or on error — a failed connect sends `closed` with a reason and no `connected` |
| `open-tab(plugin(key))`, or the user picking a `plugin(key)` profile | `profile-opened(key)` |

`send`/`close` only reach connections this plugin opened; an unknown id is
dropped silently. `connect(local(name))` resolves `$TMPDIR/{name}-{0..9}` on
unix and `\\.\pipe\{name}-{0..9}` on Windows, so Discord IPC is
`local("discord-ipc")`. `connect(tcp)` is a raw stream: no TLS.

`connect(tab(title))` opens a tab titled after the plugin and `title`, marked
with a plugin icon, and treats it as one more stream: `connected(id)`, then
`resized(io-size)` with the grid size (again on every resize), `data(io-frame)`
for each keystroke and terminal reply typed into it, and `send` writes bytes to
its screen as if a program printed them. `close` ends the tab like a process
exiting; the user closing it, or the plugin stopping, sends `closed`. Splitting
such a tab opens a shell, and it is not kept in session history. A profile with
a `plugin(key)` target is how a plugin offers these tabs in the new-tab picker:
picking it sends `profile-opened(key)`, and the plugin answers with the
`connect`.

`command-finished` arrives when the shell emits OSC 133 `D`, and
`prompt-input` needs the OSC 133 `B` mark at the end of the prompt. Rabbitty
adds these marks to zsh and bash itself (Settings → Terminal, on by default,
for new tabs): it loads the user's own startup files first and hooks in after
them. bash needs 4.4 or later for `command-finished`; older bash, and other
shells, only get what they emit on their own. `command-finished` carries the
exit code and two `scrollback-range`s — the typed command (`input`, starting at
`input-col`) and its output — but no text: fetch either with
`query(scrollback)` in the same dispatch, since ranges are measured from the
bottom of the grid at delivery time and shift as new output arrives.

Terminal output is **not** streamed to you. Declare `output-pattern` records
instead; the host matches every line natively and calls `on-event` only on a hit.

Rust plugins should build on `plugins/sdk` (`rabbitty-plugin-sdk`): a `Plugin`
trait returning `Vec<Action>`, `export_plugin!`, and a sans-IO `http::Client`
that keeps one connection alive per origin and turns requests into actions and
events into responses. With the SDK's `tls` feature and a rustls crypto
provider (`Client::with_tls`), it speaks https, verifying certificates against
the Mozilla roots inside the plugin.

## Capabilities

`manifest()` declares what the plugin requests. The host reviews it before
running anything else, then enforces the grant when executing actions:

| Capability | Gates |
|---|---|
| `read-config` | The `read-config` import — ungranted returns `none` |
| `read-screen` | `query(scrollback)`, `query(selection)` and `query(prompt-input)` — ungranted is dropped, so no answer ever arrives |
| `notify` | `notify` and `show-text` — ungranted is dropped |
| `open-url`, `write-pty` | The action of the same name — ungranted is dropped |
| `control` | `open-tab`, `focus-pane`, `close-pane`, `connect(tab)` |
| `network` | `connect(tcp)` |
| `local-ipc` | `connect(local)` |
| `clipboard` | `set-clipboard` — write only; a plugin cannot read the clipboard |
| `filesystem` | `wasi:filesystem`, preopened to the plugin's own data directory and nothing else |

`set-status`, `set-profiles`, `set-contributions`, `schedule`, `cancel-timer`,
`query(panes)`, `send` and `close` need no capability. The two exceptions to
"events carry no screen content" are
deliberate: `output-matched` carries the one line that hit a pattern the plugin
declared, and `menu-activated` carries the selection the user invoked the
plugin's own menu item on. Nothing is granted while `manifest()` runs, so a
plugin cannot influence its own review.

## Lifecycle

```
manifest()       → host reviews and grants capabilities
init()           → one-time setup; returns the first actions
contributions()  → commands, patterns, settings, status and menu items
on-event(…) / run-command(…)   → steady state, each returning actions
on-shutdown()    → last call before teardown (disable, reload, app exit); its actions still run, but at app exit a `send` is best-effort
```

Calls are strictly sequential: the host never re-enters the plugin while a
call is in progress, and actions returned by one call are executed before the
next call is made.

Every call runs under two budgets, whichever runs out first: 500 million
instructions (about 35 ms on an Apple M-series CPU; slower machines take
longer for the same count), and a wall-clock deadline of 2 s for `manifest`,
`init`, `contributions` and `run-command`, 250 ms for `on-event` and 500 ms for
`on-shutdown`. That is enough for a TLS handshake with
full certificate verification inside the plugin. Longer work should be split
across calls with `schedule`.

A trap — fuel exhaustion, memory limit or a guest panic — **permanently**
poisons the instance. The Component Model refuses re-entry, so the host records
the failure, blocks further calls, and skips `on-shutdown`. Recovery means a fresh
instantiation.

## Versioning

`package rabbitty:plugin@0.1.0`.

While nothing outside this repository builds against it, the interface changes
in place without a version bump: rebuild plugins against the current world
after pulling. A plugin built against an older 0.1.0 world then fails to load
with a type error rather than a version error. Once plugins are published,
pre-1.0 breaking changes bump the minor and the host supports exactly one
version; from 1.0, breaking changes bump the major and the previous major is
supported for a transition period.
