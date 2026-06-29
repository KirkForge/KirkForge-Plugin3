# KirkForge-Plugin3

Output-side sibling of the KirkForge plugin ecosystem.
Slices oversized tool results and enforces a session-level
token budget. See `docs/adr/README.md` for the design record.

## Layout

```
crates/
├── plugin3-core/      # pure logic: slicing, budget, offload, cost
├── plugin3-hosts/     # per-host shim registry (Claude Code; Cursor/Aider stub)
└── plugin3-cli/       # host hooks + budget + report subcommands
docs/adr/              # architecture decision records
```

## Building

```bash
cargo build --release                # default build (ADR-0017)
cargo build --release --no-default-features
cargo test --workspace
cargo run --release --bin plugin3 -- self-check
```

The release binary target is `<8 MB` (ADR-0017 § Size budget).

## CLI surface (ADR-0015)

```bash
plugin3 hook post-tool-use       # host hook; reads JSON on stdin
plugin3 hook user-prompt-submit
plugin3 hook pre-compact
plugin3 budget status
plugin3 budget set <ceiling> [--default]
plugin3 budget compact
plugin3 report [--last N] [--session S] [--kind K]
plugin3 config [--show-sources] [--validate]
plugin3 init [--host H] [--dry-run] [--force]
plugin3 store prune
plugin3 store get <marker>
plugin3 self-check
plugin3 --version
```

Every subcommand accepts `--json` for machine-readable output.

## Environment

The CLI honours `PLUGIN3_CONFIG_DIR`, `PLUGIN3_DATA_DIR`,
`PLUGIN3_RUNTIME_DIR` (ADR-0014). Defaults follow the XDG
base-directory spec.

## Testing

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings   # ADR-0016 CI gate
```

## State

| Stage     | Status                                                |
|-----------|-------------------------------------------------------|
| ADRs      | 15 Accepted, 2 Deferred (0011, 0012)                  |
| Crates    | `plugin3-core`, `plugin3-hosts`, `plugin3-cli`        |
| Tests     | 481 passing across the workspace                      |
| Hooks     | PostToolUse, UserPromptSubmit, PreCompact (ADR-0009) |
| Hosts     | Claude Code only; Cursor/Aider/KirkForge stubs (ADR-0013) |

## License

Dual-licensed under MIT or Apache-2.0, at your option.