//! ADR-0013 (Output shim) drift tests — pin the § Host enum,
//! § Canonical payloads, § Shim entry point, and § Cursor/Aider
//! shim prose against the actual impl in
//! `crates/plugin3-hosts/src/`. Companion to the unit tests
//! inside each shim module (which pin the wire shapes); this
//! file pins the *spec surface* — the documented code blocks,
//! phantom dep names, and stub fallthrough behaviour.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent() // crates/
        .and_then(Path::parent) // workspace root
        .expect("workspace root resolvable")
        .to_path_buf()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn adr_0013() -> String {
    read(&repo_root().join("docs/adr/0013-output-shim.md"))
}

/// Read ADR-0013's § Host enum code block.
fn adr_0013_host_enum_block() -> String {
    let adr = adr_0013();
    let section_start = adr
        .find("### Host enum")
        .expect("ADR-0013 must have a § Host enum subsection");
    let section_end = adr[section_start..]
        .find("### Canonical payloads")
        .expect("ADR-0013 § Host enum must precede § Canonical payloads");
    let section = &adr[section_start..section_start + section_end];
    let fence_start = section
        .find("```rust\n")
        .expect("ADR-0013 § Host enum must contain a rust code block");
    let fence_after = &section[fence_start + "```rust\n".len()..];
    let fence_end_rel = fence_after
        .find("```")
        .expect("ADR-0013 § Host enum rust code block must close");
    fence_after[..fence_end_rel].to_string()
}

/// Read ADR-0013's § Canonical payloads code block.
fn adr_0013_canonical_payloads_block() -> String {
    let adr = adr_0013();
    let section_start = adr
        .find("### Canonical payloads")
        .expect("ADR-0013 must have a § Canonical payloads subsection");
    let section_end = adr[section_start..]
        .find("### Shim entry point")
        .expect("ADR-0013 § Canonical payloads must precede § Shim entry point");
    let section = &adr[section_start..section_start + section_end];
    let fence_start = section
        .find("```rust\n")
        .expect("ADR-0013 § Canonical payloads must contain a rust code block");
    let fence_after = &section[fence_start + "```rust\n".len()..];
    let fence_end_rel = fence_after
        .find("```")
        .expect("ADR-0013 § Canonical payloads rust code block must close");
    fence_after[..fence_end_rel].to_string()
}

/// Read ADR-0013's § Shim entry point code block.
fn adr_0013_shim_entry_point_block() -> String {
    let adr = adr_0013();
    let section_start = adr
        .find("### Shim entry point")
        .expect("ADR-0013 must have a § Shim entry point subsection");
    let section_end = adr[section_start..]
        .find("### Claude Code shim")
        .expect("ADR-0013 § Shim entry point must precede § Claude Code shim");
    let section = &adr[section_start..section_start + section_end];
    let fence_start = section
        .find("```rust\n")
        .expect("ADR-0013 § Shim entry point must contain a rust code block");
    let fence_after = &section[fence_start + "```rust\n".len()..];
    let fence_end_rel = fence_after
        .find("```")
        .expect("ADR-0013 § Shim entry point rust code block must close");
    fence_after[..fence_end_rel].to_string()
}

/// Read ADR-0013's § Claude Code shim code block.
fn adr_0013_claude_code_shim_block() -> String {
    let adr = adr_0013();
    let section_start = adr
        .find("### Claude Code shim")
        .expect("ADR-0013 must have a § Claude Code shim subsection");
    let section_end = adr[section_start..]
        .find("### Cursor shim")
        .expect("ADR-0013 § Claude Code shim must precede § Cursor shim");
    let section = &adr[section_start..section_start + section_end];
    let fence_start = section
        .find("```rust\n")
        .expect("ADR-0013 § Claude Code shim must contain a rust code block");
    let fence_after = &section[fence_start + "```rust\n".len()..];
    let fence_end_rel = fence_after
        .find("```")
        .expect("ADR-0013 § Claude Code shim rust code block must close");
    fence_after[..fence_end_rel].to_string()
}

// ponytail: pin the § Host enum code block's
// `EnvSource` seam. The impl exposes a trait seam
// (`EnvSource`) with `OsEnv` (production) and a
// test-only `detect_host_with` function so drift tests
// don't race on `std::env::var` mutation. A contributor
// who collapses the seam back to a single
// `detect_host() -> Host` reading `std::env::var` directly
// documents an API the impl no longer has.
#[test]
fn adr_0013_host_enum_block_documents_env_source_seam() {
    let block = adr_0013_host_enum_block();
    assert!(
        block.contains("pub trait EnvSource"),
        "ADR-0013 § Host enum code block must show the `pub trait \
         EnvSource` seam — the impl uses it to inject a fixed env \
         in drift tests without racing on `std::env::var` mutation.",
    );
    assert!(
        block.contains("fn detect_host_with("),
        "ADR-0013 § Host enum code block must show \
         `fn detect_host_with(...)` — the trait-parameterised \
         detection entry point the impl exposes for tests.",
    );
    assert!(
        block.contains("struct OsEnv"),
        "ADR-0013 § Host enum code block must show `struct OsEnv` \
         — the production EnvSource impl wrapping `std::env::var`.",
    );
}

// ponytail: pin the § Canonical payloads code block's
// `session_id` field on both Payload structs. The
// canonical schema absorbs `session_id` (default-empty)
// because a host that doesn't tag sessions still emits
// it as default-empty rather than breaking the cost
// reporter. A contributor who drops `session_id` from
// the documented payload documents a wire shape the
// impl does not emit.
#[test]
fn adr_0013_canonical_payloads_include_session_id() {
    let block = adr_0013_canonical_payloads_block();
    // Scope each check to its own struct: find the struct
    // header, then look for `session_id:` only inside the
    // body (before the next `pub struct` or `pub enum`).
    let post_tool_use_idx = block
        .find("PostToolUsePayload")
        .expect("ADR-0013 § Canonical payloads must show PostToolUsePayload");
    let post_tool_use_body_end = block[post_tool_use_idx..]
        .find("pub struct PostToolUseResponse")
        .expect("PostToolUsePayload must precede PostToolUseResponse");
    let post_tool_use_body = &block[post_tool_use_idx..post_tool_use_idx + post_tool_use_body_end];
    assert!(
        post_tool_use_body.contains("session_id:"),
        "ADR-0013 § Canonical payloads must show `session_id` on \
         `PostToolUsePayload` — the canonical schema absorbs it \
         (default-empty) so the cost reporter (ADR-0010) can group \
         by session without breaking on hosts that don't tag sessions.",
    );

    let user_prompt_submit_idx = block
        .find("UserPromptSubmitPayload")
        .expect("ADR-0013 § Canonical payloads must show UserPromptSubmitPayload");
    let user_prompt_submit_body_end = block[user_prompt_submit_idx..]
        .find("pub enum UserPromptSubmitResponse")
        .expect("UserPromptSubmitPayload must precede UserPromptSubmitResponse");
    let user_prompt_submit_body =
        &block[user_prompt_submit_idx..user_prompt_submit_idx + user_prompt_submit_body_end];
    assert!(
        user_prompt_submit_body.contains("session_id:"),
        "ADR-0013 § Canonical payloads must show `session_id` on \
         `UserPromptSubmitPayload` — same rationale as \
         `PostToolUsePayload`.",
    );
    // Pin negative: the earlier draft listed a typed
    // `CompactHint` for the response field; the impl
    // uses `serde_json::Value` so the shim can emit any
    // host-consumable shape.
    assert!(
        block.contains("pub hint: serde_json::Value"),
        "ADR-0013 § Canonical payloads must show `pub hint: \
         serde_json::Value` on `PreCompactResponse` — the host shim \
         emits a thin `{{ \"turns\": N }}` envelope without depending \
         on plugin3-core types.",
    );
    assert!(
        !block.contains("pub hint: CompactHint"),
        "ADR-0013 § Canonical payloads must not declare \
         `pub hint: CompactHint` — the canonical response uses \
         `serde_json::Value` so the shim is host-agnostic.",
    );
}

// ponytail: pin the § Shim entry point code block's
// stub fallthrough shape. The earlier draft used
// `panic!("unsupported host/event combination: ...")`. The
// impl returns a structured
// `serde_json::json!({ "unsupported": format!(...) })` so
// the host receives a well-formed JSON envelope rather
// than a crash trace. A contributor who re-pastes the
// panic documents an API behaviour the impl no longer
// has.
#[test]
fn adr_0013_shim_entry_point_uses_stub_fallthrough_not_panic() {
    let block = adr_0013_shim_entry_point_block();
    assert!(
        !block.contains("panic!"),
        "ADR-0013 § Shim entry point code block must not call \
         `panic!(...)` on the unsupported stub arm — the impl \
         returns a structured `{{\"unsupported\": \"...\"}}` Value \
         so the host receives a well-formed envelope rather than \
         a crash trace.",
    );
    assert!(
        block.contains("\"unsupported\""),
        "ADR-0013 § Shim entry point code block must emit a \
         `{{\"unsupported\": \"...\"}}` envelope on the stub arm — \
         the impl's `emit_to` returns this shape so callers can \
         log + bail without crashing the host's hook handler.",
    );
}

// ponytail: pin the § Claude Code shim code block's
// passthrough behaviour. The earlier draft routed
// payloads through `crate::handle_canonical_post_tool_use`.
// The impl is a thin passthrough that constructs
// `PostToolUseResponse { content: canonical.content, note: None }`
// and serialises via `translate_post_tool_use_response`. The
// canonical slicing happens upstream in plugin3-core's
// SlicingOrchestrator (called from the CLI), not in the
// shim. A contributor who re-pastes the
// `handle_canonical_*` calls documents an API the shim
// does not depend on.
#[test]
fn adr_0013_claude_code_shim_is_passthrough_not_canonical_handler() {
    let block = adr_0013_claude_code_shim_block();
    assert!(
        !block.contains("handle_canonical_post_tool_use"),
        "ADR-0013 § Claude Code shim must not call \
         `crate::handle_canonical_post_tool_use` — the shim is a \
         thin passthrough that constructs `PostToolUseResponse` \
         directly. The canonical slicing happens in plugin3-core's \
         SlicingOrchestrator (ADR-0007), not in the shim.",
    );
    assert!(
        !block.contains("handle_canonical_user_prompt_submit"),
        "ADR-0013 § Claude Code shim must not call \
         `crate::handle_canonical_user_prompt_submit` — same \
         rationale as `handle_canonical_post_tool_use`.",
    );
    assert!(
        !block.contains("handle_canonical_pre_compact"),
        "ADR-0013 § Claude Code shim must not call \
         `crate::handle_canonical_pre_compact` — same rationale.",
    );
    // Positive: the impl builds a `PostToolUseResponse`
    // with `note: None` and the `history_turns.len()` hint
    // for PreCompact.
    assert!(
        block.contains("note: None"),
        "ADR-0013 § Claude Code shim must show `note: None` on \
         the constructed `PostToolUseResponse` — the impl does \
         not carry notes from the canonical layer to the shim's \
         output (notes are CLI-side).",
    );
    assert!(
        block.contains("history_turns.len()"),
        "ADR-0013 § Claude Code shim must show \
         `history_turns.len()` in the PreCompact hint — the \
         shim emits `{{ \"turns\": N }}` rather than the typed \
         `CompactHint`.",
    );
}

// ponytail: pin the absence of the earlier draft's full
// Cursor/Aider shim code blocks (those showed `handle_post_tool_use`
// etc. as if they were real handlers). The impl ships only
// stub modules with a `stub_present` test. A contributor
// who re-pastes the full Cursor/Aider code blocks into
// ADR-0013 documents shims the impl does not have.
#[test]
fn adr_0013_cursor_and_aider_shims_documented_as_stubs() {
    let adr = adr_0013();
    let cursor_section_start = adr
        .find("### Cursor shim")
        .expect("ADR-0013 must have a § Cursor shim subsection");
    let cursor_section_end = adr[cursor_section_start..]
        .find("### Aider shim")
        .expect("ADR-0013 § Cursor shim must precede § Aider shim");
    let cursor_section = &adr[cursor_section_start..cursor_section_start + cursor_section_end];

    let aider_section_start = adr
        .find("### Aider shim")
        .expect("ADR-0013 must have a § Aider shim subsection");
    let aider_section_end = adr[aider_section_start..]
        .find("### Drift tests")
        .expect("ADR-0013 § Aider shim must precede § Drift tests");
    let aider_section = &adr[aider_section_start..aider_section_start + aider_section_end];

    for (label, section) in [("Cursor", cursor_section), ("Aider", aider_section)] {
        // ponytail: the stub rationale must be present.
        assert!(
            section.contains("stub") || section.contains("STUB"),
            "ADR-0013 § {label} shim must document that the shim \
             is a stub today — the impl file has a `stub_present` \
             test but no real handler. Re-pasting the full \
             `handle_post_tool_use` body documents a shim the \
             impl does not have.",
        );
        // ponytail: the section's `handle_post_tool_use`
        // reference must be inside the "(future)" fenced
        // block, not the leading prose. We pin the
        // positive presence of `future` so a contributor
        // who re-labels the stub as live surfaces here.
        assert!(
            section.contains("future"),
            "ADR-0013 § {label} shim must mark the sketched code \
             block as `future` — the MVP ships only a stub module, \
             the full handler is a future contributor's job.",
        );
    }
}
