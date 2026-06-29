# ADR-0013: Output shim — per-host payload translation

- **Status:** Accepted
- **Date:** 2026-06-24

## Context

Plugin3 speaks the canonical payload schema defined in
ADR-0009 (`PostToolUsePayload`, `UserPromptSubmitPayload`,
`PreCompactPayload`, and their responses). The host agent
speaks its own schema: Claude Code uses one JSON envelope,
Cursor uses another, Aider uses environment variables.

The shim is the boundary. Each host gets one module in
`plugin3-hosts/` that:

1. Parses the host's payload format into the canonical
   payload.
2. Calls the canonical handler.
3. Translates the canonical response back to the host's
   format.

Mirrors Stratum ADR-0009 (`emit_to(host, event, payload)`).
Plugin3 reuses the pattern.

## Decision

### Host enum

```rust
// crates/plugin3-hosts/src/lib.rs

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Host {
    ClaudeCode,
    Cursor,
    Aider,
}

pub trait EnvSource {
    fn is_set(&self, key: &str) -> bool;
}

struct OsEnv;
impl EnvSource for OsEnv {
    fn is_set(&self, key: &str) -> bool { std::env::var(key).is_ok() }
}

pub fn detect_host() -> Host {
    // ponytail: production entry point — wraps the pure
    // function below so the host shim layer reads
    // `std::env::var` exactly once per call. The trait-
    // parameterised `detect_host_with` is the seam used by
    // drift tests (ADR-0013 § drift tests) so they don't
    // race with parallel tests that mutate the process env.
    detect_host_with(&OsEnv)
}

pub fn detect_host_with(env: &dyn EnvSource) -> Host {
    // ponytail: only Claude Code has a real shim. The
    // env-var check exists so a future Cursor/Aider detection
    // slot is obvious. Precedence: CLAUDE_CODE >
    // CURSOR_TRACE_ID > AIDER > ClaudeCode.
    if env.is_set("CLAUDE_CODE") {
        Host::ClaudeCode
    } else if env.is_set("CURSOR_TRACE_ID") {
        Host::Cursor
    } else if env.is_set("AIDER") {
        Host::Aider
    } else {
        Host::ClaudeCode // ponytail: default to Claude Code,
                         // the only host with a real shim.
                         // Add explicit host selection when a
                         // user reports a wrong-default bug.
    }
}
```

### Canonical payloads

```rust
// crates/plugin3-hosts/src/canonical.rs

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PostToolUsePayload {
    pub tool_name: String,
    #[serde(default)]
    pub tool_result_key: String,
    pub content: String,
    // ponytail: session_id is load-bearing for ADR-0010's
    // usage.jsonl grouping. Hosts that don't tag sessions
    // emit default-empty rather than breaking the cost reporter.
    #[serde(default)]
    pub session_id: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PostToolUseResponse {
    /// Modified tool result content. The host replaces its
    /// in-memory tool result with this string.
    pub content: String,
    /// Optional human-readable note for the user.
    pub note: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct UserPromptSubmitPayload {
    pub prompt: String,
    #[serde(default)]
    pub session_id: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UserPromptSubmitResponse {
    Allow,
    Warn { remaining: usize },
    Slice { target_key: String, slice_to: usize },
    Compact { reason: String },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PreCompactPayload {
    pub history_turns: Vec<Turn>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Turn {
    pub index: usize,
    pub role: String,            // "user" | "assistant" | "tool"
    pub content_preview: String, // first 200 chars
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PreCompactResponse {
    // ponytail: hint is `serde_json::Value` rather than the
    // typed `CompactHint` so the host shim can emit any
    // shape the host consumes (turns count, summary text,
    // CompactHint). The CLI side builds a typed
    // CompactHint (ADR-0008); the host shim serialises a
    // thin `{ "turns": N }` envelope that the host can
    // interpret without depending on plugin3-core types.
    pub hint: serde_json::Value,
}
```

### Shim entry point

```rust
// crates/plugin3-hosts/src/lib.rs

pub fn emit_to(host: Host, event: Event, payload: serde_json::Value) -> serde_json::Value {
    match (host, event) {
        (Host::ClaudeCode, Event::PostToolUse) =>
            claude_code::handle_post_tool_use(payload),
        (Host::ClaudeCode, Event::UserPromptSubmit) =>
            claude_code::handle_user_prompt_submit(payload),
        (Host::ClaudeCode, Event::PreCompact) =>
            claude_code::handle_pre_compact(payload),
        // Future variants land here when Cursor/Aider graduate
        // from stub to real shim. The stub branch returns a
        // structured `{"unsupported": "..."}` envelope so callers
        // can log + bail without crashing the host's hook handler.
        _ => serde_json::json!({
            "unsupported": format!("{host:?}/{event:?}"),
        }),
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    PostToolUse,
    UserPromptSubmit,
    PreCompact,
}
```

The shim is the *only* place that knows about per-host
payload differences. Everything else in Plugin3 is host-
agnostic.

ponytail: the earlier draft's stub fall-through was a
`panic!("unsupported host/event ...")`. The MVP returns a
structured `{"unsupported": "..."}` Value so the host
receives a well-formed JSON envelope rather than a crash
trace — a host that picks up a stubbed (Cursor, Aider)
combination today logs + bails cleanly. Promoting a stub
to a real shim is then a one-line match-arm addition.

### Claude Code shim

```rust
// crates/plugin3-hosts/src/claude_code.rs

pub fn handle_post_tool_use(payload: Value) -> Value {
    let canonical: PostToolUsePayload = serde_json::from_value(payload)
        .expect("claude_code PostToolUse payload");
    let response = PostToolUseResponse {
        content: canonical.content,
        note: None,
    };
    // ponytail: passthrough today. The canonical slicing
    // happens inside plugin3-core's SlicingOrchestrator (called
    // from the CLI's `post_tool_use` handler); the shim here
    // is a thin envelope adapter — it parses the host envelope
    // into the canonical payload and serialises a passthrough
    // response. A future contributor who wires the
    // SlicingOrchestrator through the shim fills in
    // `response.content` from the orchestrator's Sliced
    // head/tail. The host wire shape is fixed: `{content, note}`.
    translate_post_tool_use_response(response)
}

pub fn handle_user_prompt_submit(payload: Value) -> Value {
    let _canonical: UserPromptSubmitPayload = serde_json::from_value(payload)
        .expect("claude_code UserPromptSubmit payload");
    // ponytail: same passthrough rationale. The budget guard
    // (ADR-0005) is what produces the Allow/Warn/Slice/Compact
    // variants; the shim just serialises the verdict. Today
    // the verdict is `Allow` because the host-side budget
    // guard runs upstream of the shim.
    let response = UserPromptSubmitResponse::Allow;
    serde_json::to_value(response).expect("UserPromptSubmit response")
}

pub fn handle_pre_compact(payload: Value) -> Value {
    let canonical: PreCompactPayload = serde_json::from_value(payload)
        .expect("claude_code PreCompact payload");
    let response = PreCompactResponse {
        hint: serde_json::json!({ "turns": canonical.history_turns.len() }),
    };
    serde_json::to_value(response).expect("PreCompact response")
}

fn translate_post_tool_use_response(r: PostToolUseResponse) -> Value {
    json!({
        "content": r.content,
        "note": r.note,
    })
}
```

### Cursor shim

ponytail: the Cursor shim is a stub today — the file
`crates/plugin3-hosts/src/cursor.rs` exists with a
`stub_present` test but no real handler. The MVP routes
through `Host::default()` for Cursor (via `emit_to`'s
`{"unsupported": "..."}` stub envelope), so a Cursor
detection today is a logged no-op rather than a panic.

When a user reports a need, the stub graduates to a real
shim using the translation sketched below:

```rust
// crates/plugin3-hosts/src/cursor.rs (future)

pub fn handle_post_tool_use(payload: Value) -> Value {
    // Cursor's PostToolUse payload has the tool result under
    // a different field name. Translate.
    let tool_name = payload["tool_name"].as_str().unwrap_or("unknown").to_string();
    let content = payload["result"]["content"].as_str().unwrap_or("").to_string();
    let tool_result_key = payload["result"]["id"].as_str().unwrap_or("").to_string();
    let canonical = PostToolUsePayload {
        tool_name,
        tool_result_key,
        content,
        session_id: String::new(),
    };
    let response = crate::canonical::PostToolUseResponse {
        content: canonical.content,
        note: None,
    };
    // Cursor expects the response in a `patch` field.
    serde_json::json!({
        "patch": {
            "content": response.content,
        },
        "note": response.note,
    })
}
```

### Aider shim

ponytail: the Aider shim is a stub today for the same
reason as Cursor — the file
`crates/plugin3-hosts/src/aider.rs` exists with a
`stub_present` test but no real handler. Aider uses
environment variables, not JSON envelopes, so the shim
will be different from Claude Code's. The MVP routes
through the `{"unsupported": "..."}` stub envelope.

The sketched future shape:

```rust
// crates/plugin3-hosts/src/aider.rs (future)

pub fn handle_post_tool_use(payload: Value) -> Value {
    // Aider pipes tool results via stdin; the shim reads
    // from stdin directly. The `payload` is the parsed
    // JSON; the response is written to stdout as a JSON
    // patch.
    let canonical: PostToolUsePayload = serde_json::from_value(payload)
        .expect("aider PostToolUse payload");
    let response = crate::canonical::PostToolUseResponse {
        content: canonical.content,
        note: None,
    };
    serde_json::json!({
        "content": response.content,
        "note": response.note,
    })
}
```

### Drift tests

Each shim has a drift test that pins the translation. The
Claude Code tests live in
`crates/plugin3-hosts/src/claude_code.rs::drift_tests`
and use the canonical payload shape:

```rust
#[test]
fn post_tool_use_round_trips_envelope() {
    let input = serde_json::json!({
        "tool_name": "cargo test",
        "tool_result_key": "abc",
        "content": "running 5 tests\ntest foo ... ok\n",
        "session_id": "s1",
    });
    let output = handle_post_tool_use(input);
    assert!(output["content"].is_string(), "content must be a string: {output}");
    assert!(output["note"].is_string() || output["note"].is_null(),
        "note must be string or null: {output}");
}
```

The Cursor and Aider shims today have only a `stub_present`
test — when a future contributor graduates a stub to a
real shim, the corresponding drift test moves into
`drift_tests` alongside the Claude Code tests.

A contributor who adds a new host writes one new module
under `crates/plugin3-hosts/src/<host>.rs`, adds the host
to the `Host` enum, extends `detect_host_with`'s env-var
arms, and adds the host's `(host, event)` arms to
`emit_to`. Drift tests pin the new host's behaviour.

A contributor who adds a new host writes one new module
under `crates/plugin3-hosts/src/<host>.rs` and adds the
host to the `Host` enum, the `detect_host` function, and
the `emit_to` match. Drift tests pin the new host's
behaviour.

## Consequences

Negative first:

- Three shim modules is more than one. The trade is per-host
  payload differences are isolated to the shim layer; the
  rest of Plugin3 is host-agnostic.
- A new host is a non-trivial addition: enum variant,
  detector function, shim module, drift tests. The README
  documents the steps.

Positive:

- The canonical payload schema is documented in code. A
  contributor adding a new shim has a clear contract.
- Drift tests catch shim regressions: a contributor who
  changes a host's payload format fails CI.
- The shim layer is the only place that parses host JSON.
  Plugin3's core never sees a host-specific envelope.

## Implementation notes

The shim layer lives at `crates/plugin3-hosts/src/`. The
canonical payload definitions live at
`crates/plugin3-hosts/src/canonical.rs` and are re-exported
from the crate root.

The host detection is a one-time cost at startup. The
detected host is cached in the plugin's state file
(ADR-0014) so subsequent hook invocations skip detection.

The shim is the *only* layer that depends on the host. The
canonical handlers (`handle_canonical_post_tool_use`, etc.)
live in `plugin3-core` and are host-agnostic.