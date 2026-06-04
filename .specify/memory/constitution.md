# Constitution: matrix-mcp-server-r2

**Created**: 2026-05-11
**Last Updated**: 2026-05-11

## Project Identity

matrix-mcp-server-r2 is a Rust MCP (Model Context Protocol) server that exposes
Matrix homeserver functionality as MCP tools to AI agents. It is a component of the
Agent-Matrix sovereign agent fleet. The server is a drop-in replacement for the
TypeScript matrix-mcp-server and adds E2EE support as its primary differentiator.

## Non-Negotiable Principles

### 1. v1 API Compatibility is Sacred

The 20 existing v1 MCP tools MUST remain backwards-compatible with their current
behavior. No changes to tool names, parameter names, response shapes, or error
semantics. Any agent currently using the v1 tools must continue to work without
modification after E2EE features are added.

### 2. Feature-Gated E2EE

All E2EE code paths MUST be gated behind the `e2ee` Cargo feature flag. The server
MUST build and run correctly without E2EE support when the feature is not enabled.
When E2EE is not enabled, v2 tools must return a clear error message indicating how
to enable the feature.

### 3. Rust Idioms and Safety

- Prefer borrowing over cloning; document any necessary `.clone()` with a comment
- Use `thiserror` for error types; no `unwrap()` or `expect()` in production paths
- Use `tokio::sync::Mutex` (not `std::sync::Mutex`) when holding locks across `.await`
- All public functions must have doc comments
- No `unsafe` code unless absolutely required and thoroughly documented

### 4. Matrix SDK as the Crypto Authority

Do NOT implement custom cryptography. All encryption, decryption, key management,
and device verification MUST go through `matrix-sdk` and `matrix-sdk-crypto`. The
underlying `vodozemac` library handles the Olm/Megolm primitives. We are consumers
of the SDK, not crypto implementors.

### 5. Security First

- Never log encryption keys, session tokens, or decrypted message content at INFO
  level or above
- Crypto store paths must be configurable via environment variables
- Recovery keys must never be stored in plaintext in container images
- All tool responses involving encrypted content must clearly indicate encryption
  status

### 6. Transparent Encryption

When E2EE is enabled, the SDK handles encryption and decryption transparently.
Tool implementations should not need to branch on "is this room encrypted?" for
basic operations. The v2 tools should handle both encrypted and unencrypted rooms
uniformly, with encryption status reported in responses.

### 7. Test Everything

- Unit tests for all new modules
- Integration tests using `wiremock` for homeserver mocking
- E2EE tests gated behind `#[cfg(feature = "e2ee")]`
- Both feature-enabled and feature-disabled builds must pass CI

### 8. Container-Aware Design

The server runs in Docker containers within the Agent-Matrix fleet. Design choices
must account for:
- Persistent crypto store via volume mounts
- Stateless restarts with session restoration
- Environment-variable-driven configuration
- Health check endpoint must reflect E2EE status when enabled

### 9. Incremental Delivery

Features are delivered incrementally per the roadmap phases. Each increment must be
independently deployable and testable. Never introduce partially-implemented tools
that could confuse agents or break existing workflows.

### 10. Spec-Driven Development

New features follow the spec-kit SDD workflow: specify -> plan -> tasks -> implement.
Specs are the source of truth. The existing `spec/matrix-mcp-v1-v2.yml` serves as
the API contract artifact. Spec-kit specs describe the broader feature intent and
acceptance criteria. Both artifacts must remain consistent.

## Tech Stack (Locked)

| Component | Choice | Version |
|-----------|--------|---------|
| Language | Rust | 2021 edition, MSRV 1.88 |
| MCP SDK | rmcp | 1.2 |
| HTTP | axum + tower-http | 0.8 / 0.6 |
| Matrix SDK | matrix-sdk | 0.10 |
| Matrix types | ruma | 0.12 |
| Serialization | serde + schemars | 1.0 / 1 |
| Error handling | thiserror | 2.0 |
| Logging | tracing + tracing-subscriber | 0.1 / 0.3 |
| Async runtime | tokio | 1.43 |
| E2EE (v2) | matrix-sdk with `e2e-encryption` feature | via `e2ee` Cargo feature |

## Code Organization

```
src/
  main.rs              -- Entry point, axum/rmcp wiring, graceful shutdown
  lib.rs               -- Library re-exports
  config.rs            -- Configuration from environment variables
  error.rs             -- Error types (thiserror)
  auth.rs              -- Per-request auth context
  matrix/
    client.rs          -- matrix-sdk client creation and background sync
    cache.rs           -- TTL-based client cache
  mcp/
    server.rs          -- MatrixMcpServer: all MCP tools + ServerHandler
spec/
  matrix-mcp-v1-v2.yml -- API contract (v1 and v2 tool definitions)
.specify/
  memory/              -- constitution and project knowledge
  specs/               -- feature specifications
  templates/           -- spec-kit templates
```

## Naming Conventions

- v1 tools: `kebab-case` (e.g., `list-joined-rooms`, `send-message`)
- v2 tools: `kebab-case-r2` suffix (e.g., `bootstrap-e2ee-r2`, `create-room-r2`)
- Rust modules: `snake_case`
- Environment variables: `SCREAMING_SNAKE_CASE`
- Feature flags: `lowercase` (e.g., `e2ee`)

## References

- Parent project: Agent-Matrix sovereign agent fleet
- API contract: `spec/matrix-mcp-v1-v2.yml`
- Cursor rules: `.cursor/rules/rust-e2ee.mdc`
- Research: Cursor-Writing-Assistant-repo `outputs/research/research_e2ee_mcp_landscape_20260402_0206Z.md`
