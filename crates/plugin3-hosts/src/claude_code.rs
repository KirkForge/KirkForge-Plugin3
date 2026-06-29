//! Claude Code shim — JSON-in / JSON-out, content+note envelope.
//! Per ADR-0013.

use serde_json::{json, Value};

use crate::canonical::{
    PostToolUsePayload, PostToolUseResponse, PreCompactPayload, PreCompactResponse,
    UserPromptSubmitPayload, UserPromptSubmitResponse,
};

pub fn handle_post_tool_use(payload: Value) -> Value {
    let canonical: PostToolUsePayload =
        serde_json::from_value(payload).expect("claude_code PostToolUse payload");
    let response = PostToolUseResponse {
        content: canonical.content,
        note: None,
    };
    // ponytail: passthrough today. ADR-0013 says the shim is the
    // *only* layer that knows about host envelopes — but the
    // canonical slicing happens inside plugin3-core's
    // SlicingOrchestrator; the shim here is a thin envelope
    // adapter. A future contributor who wires the canonical
    // PostToolUsePayload through the orchestrator fills in
    // `response.content` from the orchestrator's Sliced head/tail.
    translate_post_tool_use_response(response)
}

pub fn handle_user_prompt_submit(payload: Value) -> Value {
    let _canonical: UserPromptSubmitPayload =
        serde_json::from_value(payload).expect("claude_code UserPromptSubmit payload");
    // ponytail: same passthrough rationale. The budget guard
    // (ADR-0005) is what produces the Allow/Warn/Slice/Compact
    // variants; the shim just serialises the verdict.
    let response = UserPromptSubmitResponse::Allow;
    serde_json::to_value(response).expect("UserPromptSubmit response")
}

pub fn handle_pre_compact(payload: Value) -> Value {
    let canonical: PreCompactPayload =
        serde_json::from_value(payload).expect("claude_code PreCompact payload");
    let response = PreCompactResponse {
        hint: serde_json::json!({ "turns": canonical.history_turns.len() }),
    };
    serde_json::to_value(response).expect("PreCompact response")
}

fn translate_post_tool_use_response(r: PostToolUseResponse) -> Value {
    // ponytail: pin the wire shape. Drift tests assert these field
    // names; a contributor who renames `content` breaks the host.
    json!({
        "content": r.content,
        "note": r.note,
    })
}

#[cfg(test)]
mod drift_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn post_tool_use_round_trips_envelope() {
        let input = json!({
            "tool_name": "cargo test",
            "tool_result_key": "abc",
            "content": "running 5 tests\ntest foo ... ok\n",
            "session_id": "s1",
        });
        let output = handle_post_tool_use(input);
        assert!(
            output["content"].is_string(),
            "content must be a string: {output}"
        );
        assert!(
            output["note"].is_string() || output["note"].is_null(),
            "note must be string or null: {output}"
        );
    }

    #[test]
    fn user_prompt_submit_round_trips_allow() {
        let input = json!({ "prompt": "hello", "session_id": "s1" });
        let output = handle_user_prompt_submit(input);
        assert_eq!(output["kind"], "allow");
    }

    #[test]
    fn pre_compact_round_trips_with_turn_count() {
        let input = json!({
            "history_turns": [
                { "index": 0, "role": "user", "content_preview": "hi" },
                { "index": 1, "role": "assistant", "content_preview": "yo" },
            ],
        });
        let output = handle_pre_compact(input);
        assert_eq!(output["hint"]["turns"], 2);
    }
}
