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

## The four extension axes

| Axis | WIT surface |
|------|-------------|
| 1. Commands | `run-command(id)` + `command` records in `contributions` |
| 2. Events | `on-event(event)` (session lifecycle, line output, cwd change) |
| 3. Host functions | `import host` — `write-pty`, `notify`, `read-config` |
| 4. Declarative contributions | `contributions()` — commands, menu items |

Additional plugin types (output matchers, profile sources, themes, notifications)
layer on top of these axes as data, without new host machinery.

## Capability model

`manifest()` returns the capabilities the plugin requests (`write-pty`,
`read-config`, `notify`, and the reserved `network` / `filesystem`). The host gates
each host import against the granted set — an ungranted capability makes its import
deny/no-op. `network` and `filesystem` are declared but not yet host-backed.

## Lifecycle

```
manifest()  → host reviews/grants capabilities
init()      → one-time setup
contributions() → commands + menu items registered
on-event(…) / run-command(…)  → steady state
```

## Versioning

`package rabbitty:plugin@0.1.0`. Semver: **additive-only within a major**. Breaking
changes bump the major and are gated behind a new world.
