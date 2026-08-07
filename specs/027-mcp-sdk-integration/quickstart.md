# Quickstart: MCP SDK Integration Validation

**Feature**: 027-mcp-sdk-integration

This document provides runnable validation scenarios to prove the feature works end-to-end. Implementation details are in `spec.md` and `tasks.md`.

> The MCP runtime lives in the `agent_scope_mcp` crate (client lifecycle,
> tool adapter, `McpExt`); persisted configuration lives in
> `agent_scope_workspace`. All MCP integration tests are in-process over
> `tokio::io::duplex` — no external process or network required (US4).

---

## Prerequisites

- Rust toolchain: `rustc 1.83+`, `cargo`
- Project cloned: `git clone ... && cd agentscope-rust`

---

## Scenario 1: Build Check

```bash
rtk cargo build -p agent_scope_workspace -p agent_scope_mcp
```

**Expected**: Compiles without errors. `rmcp` v3.1.1 resolves with its transitives.

---

## Scenario 2: Unit & Integration Tests (CI-safe, no network)

```bash
rtk cargo test -p agent_scope_mcp -p agent_scope_workspace
```

**Expected**: All tests pass (no regressions). `mcp_integration_tests.rs` passes using the in-process duplex transport.

---

## Scenario 3: In-process MCP client → connect → list tools → call tool

```bash
rtk cargo test -p agent_scope_mcp --test mcp_integration_tests --nocapture
```

**Expected**:
```
test test_connect_and_list_tools ... ok
test test_call_tool_success ... ok
test test_call_tool_error ... ok
test test_call_unknown_tool_returns_typed_error ... ok
test test_disconnect_releases_connection ... ok
test test_not_connected_returns_error ... ok
test test_concurrent_tool_calls ... ok
```

---

## Scenario 4: SSE config backward compatibility

```bash
rtk cargo test -p agent_scope_mcp --test mcp_integration_tests -- test_sse_config_parsed_and_mapped --nocapture
```

**Expected**:
- `.mcp` file with `"type": "sse"` parses to `McpTransportConfig::Sse`
- `connect()` maps SSE → streamable-http and emits `info!`: `MCP SSE config 'legacy-sse' mapped to streamable-http transport` (captured and asserted)
- Unroutable address yields a typed `McpConnectionError`, not a panic

---

## Scenario 5: Sensitive header regression

```bash
rtk cargo test -p agent_scope_workspace --test resource_tests --nocapture
```

**Expected**:
- `test_mcp_headers_bearer_not_persisted` — `.mcp` file does not contain `"secret123"`
- `test_mcp_headers_bearer_not_persisted_streamable_http` — same for streamable-http
- `test_mcp_list_mcps_scrubs_headers` — `list_mcps()` returns `[REDACTED]`

---

## Scenario 6: Lint & Format

```bash
rtk cargo clippy -p agent_scope_mcp -p agent_scope_workspace -- -D warnings
rtk cargo fmt --check -p agent_scope_mcp -p agent_scope_workspace
```

**Expected**: Both pass clean. No new `unsafe`, no new `unwrap`/`expect` in library code.

---

## Scenario 7: Workspace close()/reset() disconnect MCP

```bash
rtk cargo test -p agent_scope_mcp --test mcp_integration_tests -- test_close_disconnects_all_mcps test_reset_clears_mcps --nocapture
rtk cargo test -p agent_scope_workspace --test lifecycle_tests -- test_close_releases_mcp_connections test_reset_releases_mcp_connections --nocapture
```

**Expected**:
- `close()` drains the connections map and later `get_mcp_tools()` reports `McpNotConnected`
- `reset()` clears the connections map and the config list
- Workspace-side tracking handles confirm `disconnect()` is invoked (FR-010)

---

## Scenario 8: Tool name prefix

```bash
rtk cargo test -p agent_scope_mcp --test mcp_integration_tests -- test_mcp_tool_name_prefix --nocapture
```

**Expected**:
- MCP named `"search"` with tool `"query"` → Tool `name()` returns `"search/query"`
- `description()` returns `"[remote MCP: search] Query the search index"`
- `read_only_hint` propagates to `is_read_only()`; `is_concurrency_safe()` is `true`

---

## Scenario 9: Full workspace regression

```bash
rtk cargo test --workspace
```

**Expected**: No regressions across all crates (the only `#[ignore]`d tests are real-transport / external-service tests that require network or credentials).

---

## Full Validation Command

```bash
rtk cargo test -p agent_scope_mcp -p agent_scope_workspace && \
rtk cargo clippy -p agent_scope_mcp -p agent_scope_workspace -- -D warnings && \
rtk cargo fmt --check -p agent_scope_mcp -p agent_scope_workspace
```

All three must pass before the feature is considered done (Constitution Article 17).
