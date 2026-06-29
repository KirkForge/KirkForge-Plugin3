//! plugin3-hosts — per-host payload translation layer. Per ADR-0013.
//!
//! ponytail: only the Claude Code shim is implemented today. Cursor
//! and Aider are stub modules that document the intended shape so a
//! future contributor adding the second host has a working outline.
//! Detect_host defaults to Claude Code because that is the only
//! host with a real shim.

pub mod aider;
pub mod canonical;
pub mod claude_code;
pub mod cursor;

pub use canonical::{
    PostToolUsePayload, PostToolUseResponse, PreCompactPayload, PreCompactResponse, Turn,
    UserPromptSubmitPayload, UserPromptSubmitResponse,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Host {
    ClaudeCode,
    Cursor,
    Aider,
}

pub fn detect_host() -> Host {
    // ponytail: production entry point — wraps the pure function
    // below so the host shim layer reads `std::env::var` exactly
    // once per call. The trait-parameterised `detect_host_with`
    // is the seam used by drift tests (ADR-0013 § drift tests).
    detect_host_with(&OsEnv)
}

// ponytail: env source seam mirroring `plugin3_cli::precedence::EnvSource`.
// Production reads `std::env`; tests inject a fixed map so they
// never race with parallel tests that mutate the process env.
pub trait EnvSource {
    fn is_set(&self, key: &str) -> bool;
}

struct OsEnv;
impl EnvSource for OsEnv {
    fn is_set(&self, key: &str) -> bool {
        std::env::var(key).is_ok()
    }
}

pub fn detect_host_with(env: &dyn EnvSource) -> Host {
    // ponytail: only Claude Code has a real shim. The env-var check
    // exists so a future Cursor/Aider detection slot is obvious —
    // `if env.is_set("CURSOR_TRACE_ID") { Host::Cursor }`.
    // The default of Claude Code matches the working shim today.
    // Precedence: CLAUDE_CODE > CURSOR_TRACE_ID > AIDER > ClaudeCode.
    // A contributor who reorders these arms breaks detection for
    // whichever host's env var is set; the drift corpus below
    // catches that swap.
    if env.is_set("CLAUDE_CODE") {
        Host::ClaudeCode
    } else if env.is_set("CURSOR_TRACE_ID") {
        Host::Cursor
    } else if env.is_set("AIDER") {
        Host::Aider
    } else {
        Host::ClaudeCode
    }
}

/// Canonical hook events plugin3 responds to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    PostToolUse,
    UserPromptSubmit,
    PreCompact,
}

// ponytail: ADR-0013 § Implementation notes prescribes a single
// dispatch entry so the canonical handlers in plugin3-core are the
// only place that knows about payloads. Today only ClaudeCode has
// a real shim; Cursor and Aider return a stub Value so a host
// detector that picks them up gets an obvious `{"unsupported":
// "..."}` shape rather than a silent no-op. Promoting a stub
// to a real shim is then a one-line match-arm addition.
pub fn emit_to(host: Host, event: Event, payload: serde_json::Value) -> serde_json::Value {
    match (host, event) {
        (Host::ClaudeCode, Event::PostToolUse) => claude_code::handle_post_tool_use(payload),
        (Host::ClaudeCode, Event::UserPromptSubmit) => {
            claude_code::handle_user_prompt_submit(payload)
        }
        (Host::ClaudeCode, Event::PreCompact) => claude_code::handle_pre_compact(payload),
        // Future variants land here when Cursor/Aider graduate
        // from stub to real shim. The stub branch returns a
        // structured Value so callers can log + bail without
        // crashing the host's hook handler.
        _ => serde_json::json!({
            "unsupported": format!("{host:?}/{event:?}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn emit_to_routes_post_tool_use_to_claude_code_shim() {
        // ponytail: drift guard for the dispatcher. The shim is
        // the only layer that knows about per-host envelopes; if
        // a contributor re-routes (Host::ClaudeCode, PostToolUse)
        // to the wrong module the Claude Code host breaks at
        // runtime. One round-trip covers the dispatch.
        let input = json!({
            "tool_name": "cargo test",
            "tool_result_key": "abc",
            "content": "running 1 test\n",
            "session_id": "s1",
        });
        let out = emit_to(Host::ClaudeCode, Event::PostToolUse, input);
        assert!(
            out["content"].is_string(),
            "expected content field, got: {out}"
        );
        assert!(
            out["note"].is_null() || out["note"].is_string(),
            "expected nullable note, got: {out}"
        );
    }

    #[test]
    fn emit_to_routes_user_prompt_submit_to_claude_code_shim() {
        let input = json!({ "prompt": "hi", "session_id": "s1" });
        let out = emit_to(Host::ClaudeCode, Event::UserPromptSubmit, input);
        assert_eq!(out["kind"], "allow");
    }

    #[test]
    fn emit_to_routes_pre_compact_to_claude_code_shim() {
        let input = json!({ "history_turns": [
            { "index": 0, "role": "user", "content_preview": "hi" },
        ]});
        let out = emit_to(Host::ClaudeCode, Event::PreCompact, input);
        assert_eq!(out["hint"]["turns"], 1);
    }

    // ponytail: pin the Host enum's three variants and their
    // kebab-case wire spelling. ADR-0013 § Host enum defines
    // three variants; a contributor who adds a fourth (e.g.
    // `Host::Codex`) without updating the detect_host arms and
    // the dispatcher's stub fall-through surfaces here. The
    // kebab-case spelling is load-bearing: a future shim
    // auto-config script reads `Host::ClaudeCode` as
    // `"claude-code"` from a JSON manifest; renaming the
    // variant or the rename rule breaks the manifest.
    #[test]
    fn host_enum_three_variants_kebab_case() {
        for (h, expected) in [
            (Host::ClaudeCode, "\"claude-code\""),
            (Host::Cursor, "\"cursor\""),
            (Host::Aider, "\"aider\""),
        ] {
            assert_eq!(
                serde_json::to_string(&h).unwrap(),
                expected,
                "Host {h:?} must serialise as {expected}"
            );
        }
    }

    // ponytail: pin the Event enum's three variants. ADR-0013
    // § Shim entry point names three: PostToolUse, UserPromptSubmit,
    // PreCompact. A contributor who adds `Event::SessionStart`
    // (mirroring Plugin1) without updating the dispatcher's stub
    // fall-through surfaces here.
    #[test]
    fn event_enum_three_variants_pinned() {
        // Event is intentionally NOT serde-serialised (the wire
        // format is hook-name string from the host shim), but we
        // pin the variant set so a contributor who renames
        // `PreCompact` to `Pre_Compact` for style breaks the
        // pattern match in claude_code.rs at compile time.
        let all = [
            Event::PostToolUse,
            Event::UserPromptSubmit,
            Event::PreCompact,
        ];
        assert_eq!(all.len(), 3, "Event must have exactly three variants");
    }

    #[test]
    fn emit_to_unsupported_branch_pins_stub_wire_shape() {
        // ponytail: pin the wire shape for the unsupported dispatch
        // arm. The stub at the dispatcher's `_ =>` branch returns
        // `{"unsupported": "<HostDebug>/<EventDebug>"}` — the
        // Host and Event both use `{:?}` (Debug), so the rendered
        // strings are the Rust variant names ("Cursor",
        // "PostToolUse"), NOT kebab-case. The kebab-case rename
        // rule on `Host` is a serde attribute only; Display would
        // be the same ("ClaudeCode") but Debug is the chosen
        // format because it's free with no Display impl. A
        // contributor who:
        //   - renames `unsupported` → `error` or `not_supported`
        //     breaks downstream log parsers that grep for the
        //     literal `unsupported` key as the "host not wired"
        //     signal.
        //   - switches `{host:?}/{event:?}` → `{host}/{event}`
        //     (Display) happens to look the same today (no Display
        //     impl, derived Debug falls back) but is a different
        //     code path that future Display impls would change.
        //   - changes the separator from `/` to `::` or ` `
        //     breaks split-on-`/` parsers that decompose the
        //     value back into (host, event) for retry/diagnostics.
        //
        // Exact shape (one representative):
        let out = emit_to(Host::Cursor, Event::PostToolUse, json!({}));
        assert_eq!(
            out,
            json!({"unsupported": "Cursor/PostToolUse"}),
            "Cursor/PostToolUse stub must serialise to the exact shape \
             `{{\"unsupported\": \"Cursor/PostToolUse\"}}` — a contributor \
             who renames the key, switches to Display format, or changes \
             the separator surfaces here."
        );

        // Sweep the 6 unsupported combos: today that's (Cursor, *)
        // and (Aider, *) — ClaudeCode is fully supported, so its
        // three arms never reach the stub. Each combo must produce
        // a single `unsupported` string of the form `<HostDebug>/
        // <EventDebug>` with exactly one `/` separator.
        for (host, event) in [
            (Host::Cursor, Event::PostToolUse),
            (Host::Cursor, Event::UserPromptSubmit),
            (Host::Cursor, Event::PreCompact),
            (Host::Aider, Event::PostToolUse),
            (Host::Aider, Event::UserPromptSubmit),
            (Host::Aider, Event::PreCompact),
        ] {
            let out = emit_to(host, event, json!({}));
            let v = out
                .get("unsupported")
                .and_then(|x| x.as_str())
                .unwrap_or_else(|| {
                    panic!(
                        "{host:?}/{event:?} stub must carry a string `unsupported` \
                     field, got: {out}"
                    )
                });
            // ponytail: exactly one '/' separator (the one between
            // host and event). A future contributor who adds a
            // version suffix ("Cursor/PostToolUse/v1") surfaces
            // here because the slash count goes to 2.
            let parts: Vec<&str> = v.split('/').collect();
            assert_eq!(
                parts.len(),
                2,
                "{host:?}/{event:?} stub value must split on '/' into exactly 2 parts, got: {v:?}"
            );
            // First half must mention the host variant, second half the event variant.
            assert!(
                parts[0].contains("Cursor") || parts[0].contains("Aider"),
                "{host:?}/{event:?} stub first half must name the host variant, got: {parts:?}"
            );
            assert!(
                parts[1].contains("PostToolUse")
                    || parts[1].contains("UserPromptSubmit")
                    || parts[1].contains("PreCompact"),
                "{host:?}/{event:?} stub second half must name the event variant, got: {parts:?}",
            );
        }
    }

    #[test]
    fn emit_to_supported_branch_does_not_return_stub_shape() {
        // ponytail: the supported arm must produce the real shim's
        // shape, not the `{"unsupported": "..."}` stub. A
        // contributor who accidentally fat-fingers a match arm
        // into the stub branch surfaces here — the supported
        // event's real envelope is missing. The shim drift tests
        // (above) cover the exact field set; this test guards the
        // *dispatch*.
        let out = emit_to(
            Host::ClaudeCode,
            Event::PostToolUse,
            json!({
                "tool_name": "x", "tool_result_key": "k",
                "content": "hi", "session_id": "s",
            }),
        );
        assert!(
            out.get("unsupported").is_none(),
            "supported combination must not return stub shape, got: {out}"
        );
    }

    // ponytail: pin the canonical `UserPromptSubmitResponse` wire
    // shape on this side of the bridge. The mirror pin on
    // `Intervention` lives in `plugin3-core::budget::tests` — both
    // enums are `#[serde(tag = "kind", rename_all = "snake_case")]`
    // over the same four-variant shape, and the CLI's `hooks::mod`
    // round-trips via serde rather than a hand-written 4-arm match.
    // The two pins together enforce: drop the serde tag on EITHER
    // enum and one of the two tests fails. Without this pin, a
    // contributor who flips the canonical enum off tagged-enum
    // form desyncs the Claude Code host shim while the core
    // pin still passes — runtime breakage with no CI signal. The
    // dispatch test (`emit_to_routes_user_prompt_submit_…`) covers
    // Allow's shape via `handle_user_prompt_submit`'s serialisation
    // but exercises only one variant; the other three (Warn, Slice,
    // Compact) carry payloads whose field names (`remaining`,
    // `target_key`, `slice_to`, `reason`) are load-bearing — Claude
    // Code's envelope parser reads them by name. A rename here
    // would desync the host shim from the CLI's `Intervention`
    // serialiser.
    #[test]
    fn user_prompt_submit_response_wire_shape_pins_all_four_variants() {
        use crate::canonical::UserPromptSubmitResponse;

        // Allow — unit variant, just the tag.
        let allow = serde_json::to_value(&UserPromptSubmitResponse::Allow).unwrap();
        assert_eq!(
            allow,
            json!({"kind": "allow"}),
            "Allow must serialise as a tagged-enum {{kind: allow}} object — \
             the Claude Code shim emits Allow on parse-failure (ADR-0009 § \
             Error contract) and the host envelope parser reads the literal \
             \"kind\": \"allow\" key"
        );

        // Warn { remaining } — payload field name is load-bearing.
        let warn = serde_json::to_value(&UserPromptSubmitResponse::Warn { remaining: 42 }).unwrap();
        assert_eq!(
            warn,
            json!({"kind": "warn", "remaining": 42}),
            "Warn must serialise with kind=warn and inline `remaining` field; \
             a contributor who renames `remaining` → `tokens_left` desyncs the \
             host's read of the budget warning envelope"
        );

        // Slice { target_key, slice_to } — both payload field names
        // are load-bearing; the host uses `target_key` to look up the
        // tool output to slice and `slice_to` as the byte budget.
        let slice = serde_json::to_value(&UserPromptSubmitResponse::Slice {
            target_key: "abc".into(),
            slice_to: 100,
        })
        .unwrap();
        assert_eq!(
            slice,
            json!({
                "kind": "slice", "target_key": "abc", "slice_to": 100,
            }),
            "Slice must serialise with kind=slice and inline `target_key` \
             and `slice_to` fields — the host's auto-slicer reads both by \
             name; a rename breaks the auto-slice round-trip"
        );

        // Compact { reason } — payload is the only structured field
        // (see plugin3-core::budget::tests::compact_reason_string_format_is_pinned).
        let compact = serde_json::to_value(&UserPromptSubmitResponse::Compact {
            reason: "session at 100/100 tokens".into(),
        })
        .unwrap();
        assert_eq!(
            compact,
            json!({
                "kind": "compact", "reason": "session at 100/100 tokens",
            }),
            "Compact must serialise with kind=compact and inline `reason` field"
        );
    }

    // ponytail: ADR-0013 § Implementation notes — the env-var
    // precedence chain (CLAUDE_CODE > CURSOR_TRACE_ID > AIDER >
    // default-to-ClaudeCode) is load-bearing: a contributor who
    // reorders the arms, renames an env var, or changes the
    // default silently breaks host detection. The dispatcher
    // tests above cover `emit_to`; this module covers
    // `detect_host` with an `EnvSource` trait seam so tests
    // don't race on `std::env::var` mutation.
    mod detect_host_drift {
        use super::{detect_host_with, EnvSource, Host};
        use std::collections::HashSet;

        struct TestEnv {
            set: HashSet<&'static str>,
        }
        impl EnvSource for TestEnv {
            fn is_set(&self, key: &str) -> bool {
                self.set.contains(key)
            }
        }
        fn env(vars: &[&'static str]) -> TestEnv {
            TestEnv {
                set: vars.iter().copied().collect(),
            }
        }

        // ponytail: pin the precedence chain end-to-end. Each row
        // is a fixture in code form — the columns are (env-vars
        // set, expected Host). Adding a new env var = new row.
        // Reordering or renaming surfaces in the assertion message.
        #[test]
        fn precedence_chain_is_pinned() {
            let rows: &[(&[&'static str], Host, &str)] = &[
                (&[], Host::ClaudeCode, "no env vars → default ClaudeCode"),
                (&["CLAUDE_CODE"], Host::ClaudeCode, "CLAUDE_CODE only"),
                (&["CURSOR_TRACE_ID"], Host::Cursor, "CURSOR_TRACE_ID only"),
                (&["AIDER"], Host::Aider, "AIDER only"),
                // Precedence: Claude Code beats Cursor when both set.
                (
                    &["CLAUDE_CODE", "CURSOR_TRACE_ID"],
                    Host::ClaudeCode,
                    "CLAUDE_CODE beats CURSOR_TRACE_ID",
                ),
                // Precedence: Cursor beats Aider when both set.
                (
                    &["CURSOR_TRACE_ID", "AIDER"],
                    Host::Cursor,
                    "CURSOR_TRACE_ID beats AIDER",
                ),
                // Precedence: Claude Code beats Aider.
                (
                    &["CLAUDE_CODE", "AIDER"],
                    Host::ClaudeCode,
                    "CLAUDE_CODE beats AIDER",
                ),
                // All three set → Claude Code wins.
                (
                    &["CLAUDE_CODE", "CURSOR_TRACE_ID", "AIDER"],
                    Host::ClaudeCode,
                    "CLAUDE_CODE beats all",
                ),
                // ponytail: case-sensitivity. Env vars are case-sensitive
                // on Linux/macOS; a contributor who downcased the
                // check to "claude_code" would break detection on
                // the canonical uppercase. Drift catches.
                (
                    &["claude_code"],
                    Host::ClaudeCode,
                    "lowercase doesn't trigger",
                ),
                (
                    &["Claude_Code"],
                    Host::ClaudeCode,
                    "titlecase doesn't trigger",
                ),
            ];
            for (vars, expected, label) in rows {
                let got = detect_host_with(&env(vars));
                assert_eq!(
                    got, *expected,
                    "row `{label}`: vars={vars:?} expected {expected:?} got {got:?}"
                );
            }
        }

        // ponytail: pin the canonical env-var names. A contributor
        // who renames CLAUDE_CODE → CLAUDE_PROJECT, CURSOR_TRACE_ID
        // → CURSOR_SESSION, or AIDER → AIDER_ACTIVE surfaces here
        // before the dispatcher's host lookup starts returning
        // stub envelopes for users running with the canonical
        // vars. We pin the canonical hits + the near-miss defaults
        // (which fall through to ClaudeCode, the spec default).
        #[test]
        fn canonical_env_var_names_are_pinned() {
            // Canonical names: a contributor who renames any of
            // these three (e.g. CLAUDE_CODE → CLAUDE_PROJECT)
            // breaks host detection for users running with the
            // original vars. Drift catches here.
            assert_eq!(detect_host_with(&env(&["CLAUDE_CODE"])), Host::ClaudeCode);
            assert_eq!(detect_host_with(&env(&["CURSOR_TRACE_ID"])), Host::Cursor);
            assert_eq!(detect_host_with(&env(&["AIDER"])), Host::Aider);
            // Near-miss names: these do not match the canonical
            // spellings, so detection falls through to the
            // default (ClaudeCode per ADR-0013). A contributor who
            // widens the check to a prefix match (e.g.
            // key.starts_with("CLAUDE")) would route these to
            // ClaudeCode *as a hit* rather than via the default;
            // since both end up at ClaudeCode, distinguish via the
            // mixed-Cursor case below.
            assert_eq!(
                detect_host_with(&env(&["CLAUDE_PROJECT"])),
                Host::ClaudeCode
            );
            assert_eq!(detect_host_with(&env(&["CURSOR"])), Host::ClaudeCode);
            // Stronger signal: a near-miss CLAUDE_PROJECT must
            // NOT shadow a real Cursor signal. If the check
            // became a starts_with, CLAUDE_PROJECT alone would
            // still default — but a starts_with on CURSOR_ would
            // flip the Cursor lookup. The Cursor pair is the load-
            // bearing near-miss test.
            assert_eq!(
                detect_host_with(&env(&["CURSOR_PROJECT"])),
                Host::ClaudeCode,
                "near-miss CURSOR_PROJECT must not be treated as Cursor",
            );
            assert_eq!(
                detect_host_with(&env(&["CURSOR_SESSION"])),
                Host::ClaudeCode,
                "near-miss CURSOR_SESSION must not be treated as Cursor",
            );
        }
    }
}
