# Rabbitty plugin interface (WIT)

`world.wit` defines the contract between the Rabbitty host and **Tier 2** plugins —
untrusted, sandboxed WASM components. It is the versioned, self-owned compatibility
surface for the plugin ecosystem.

> This is Tier 2. First-party features (SFTP, SSH/serial transports, rendering)
> are **Tier 1** — native Rust behind traits like `Transport`, never routed through
> this boundary. See epic #11 for the two-tier rationale.

## Runtime

- **wasmtime + Component Model + WIT.** Plugins are WebAssembly *components*; the
  host generates bindings with `wit-bindgen`. Plugin authors can target any
  language with component tooling (Rust, C, JS via ComponentizeJS, …).
- Everything crosses the boundary as **serializable data** — no host resources,
  no shared memory. Event delivery is line/event granularity, never per-frame, so
  the rendering hot path stays in native Rust.

## Extension axes

| Axis | WIT surface |
|------|-------------|
| Commands | `run-command(id)` + `command` records in `contributions` |
| Events | `on-event(event)` — session lifecycle, output match, cwd change |
| Host functions | `import host` — `write-pty`, `notify`, `read-config` |
| Declarative contributions | `contributions()` — commands, output patterns |

Output is **not** streamed to plugins. A plugin declares `output-pattern` records;
the host matches every line natively and only calls `on-event` on a hit. Sending
each line across the boundary would put serialization, a boundary crossing, and a
synchronous guest call on the render path — a smaller fuel budget does not fix
that, because a plugin that spends its budget on every line still stalls the
terminal.

Profile sources need a surface these axes do not have — the host pulling data from
the plugin. That is tracked separately.

## Capability model

`manifest()` returns the capabilities the plugin requests. The host reviews them
before the plugin runs anything, then enforces the grant at two layers:

| Capability | Enforced by |
|------------|-------------|
| `write-pty`, `read-config`, `notify` | The `host` imports — an ungranted one becomes a deny/no-op |
| `network` | `wasi:sockets`, off unless granted (`WasiCtxBuilder` denies every address by default) |
| `filesystem` | `wasi:filesystem`, preopened to the plugin's own data directory and nothing else |

WASI supplies the filesystem and socket *mechanism*; it deliberately leaves the
*policy* to the embedder, and this enum is that policy. Nothing is granted while
`manifest()` runs, so a plugin cannot influence its own review.

## Lifecycle

```
manifest()  → host reviews/grants capabilities
init()      → one-time setup
contributions() → commands + output patterns registered
on-event(…) / run-command(…)  → steady state
shutdown()  → last call before teardown (disable, reload, app exit)
```

A trap — fuel exhaustion, or any panic in the guest — permanently poisons the
instance: the Component Model refuses re-entry because the guest may have stopped
mid-allocation. The host records the original failure, blocks further calls, and
skips `shutdown`. Recovery is a fresh instantiation, nothing less.

## Versioning

`package rabbitty:plugin@0.2.0`.

**Pre-1.0** — breaking changes bump the minor, and the host supports exactly one
version. Plugins are recompiled against the new world. Additive-only is not a goal
yet: the ABI is still being shaped from real plugins, and a surface that nothing
uses is worse than one break.

**From 1.0** — breaking changes bump the major, and the host supports the previous
major for a transition period.

Note that "additive" is misleading here: adding a `variant` case or a world export
is a **breaking** change. The Component Model type-checks the world, so a guest
compiled against the old shape is rejected at instantiation:

```
type-checking export func `on-event`
```

A wildcard arm in the guest's `match` does not help — the mismatch is in the type,
not the code.
