# Rabbitty plugin interface (WIT)

`world.wit` is the contract between Rabbitty and a plugin. Plugins are
WebAssembly **components** run under wasmtime; the host generates its bindings
with `wit-bindgen`, so you can write one in any language with component tooling
(Rust, C, JS via ComponentizeJS, …).

Everything crosses the boundary as plain data — no host resources, no shared
memory.

## What a plugin can do

| | |
|---|---|
| **Run commands** | Declare them in `contributions()`, handle `run-command(id)` |
| **React to events** | `on-event(ev)` — session start/close, pattern match, cwd, title, focus, selection, bell, setting change, menu activation |
| **Call the host** | `write-pty`, `notify`, `read-config`, `open-url`, `set-status` |
| **Contribute UI** | `contributions()` returns commands, output patterns, settings fields, status items, menu items |
| **Supply profiles** | `list-profiles()` |

Terminal output is **not** streamed to you. Declare `output-pattern` records
instead; the host matches every line natively and calls `on-event` only on a hit.

## Capabilities

`manifest()` declares what the plugin requests. The host reviews it before
running anything else, then enforces the grant:

| Capability | Enforced by |
|---|---|
| `write-pty`, `read-config`, `notify`, `open-url` | The `host` imports — an ungranted one is a deny/no-op |
| `network` | `wasi:sockets`, off unless granted |
| `filesystem` | `wasi:filesystem`, preopened to the plugin's own data directory and nothing else |

Nothing is granted while `manifest()` runs, so a plugin cannot influence its own
review.

## Lifecycle

```
manifest()       → host reviews and grants capabilities
init()           → one-time setup
contributions()  → commands, patterns, settings, status and menu items
on-event(…) / run-command(…) / list-profiles()   → steady state
shutdown()       → last call before teardown (disable, reload, app exit)
```

A trap — fuel exhaustion or a guest panic — **permanently** poisons the instance.
The Component Model refuses re-entry, so the host records the failure, blocks
further calls, and skips `shutdown`. Recovery means a fresh instantiation.

## Versioning

`package rabbitty:plugin@0.4.0`.

Pre-1.0, breaking changes bump the minor and the host supports exactly one
version — recompile against the new world. From 1.0, breaking changes bump the
major and the previous major is supported for a transition period.

"Additive" is misleading here: adding a `variant` case or a world export is
**breaking**. The Component Model type-checks the world, so a guest built against
the old shape is rejected at instantiation with `type-checking export func …`. A
wildcard `match` arm in the guest does not help — the mismatch is in the type.
