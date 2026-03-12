# Matrix-MCP-SVR-R2 — Project Instructions

## Project Identity
- **Name**: Matrix-MCP-SVR-R2
- **Type**: Application software development (Rust)
- **Goal**: Drop-in Rust replacement for the TypeScript `matrix-mcp-server`
- **Parent Project**: Agent-Matrix (infrastructure — separate A0 project)

## Domain Focus
This project is a **systems programming** effort. The agent should optimize for:
- **Rust** — idiomatic async Rust with `tokio`, strong type system, zero-copy where possible
- **MCP Protocol** — Model Context Protocol server implementation (SSE transport)
- **Matrix Client-Server API** — rooms, messages, members, profiles, notifications
- **Cryptography (Phase 2)** — E2EE via `matrix-sdk-crypto` (Olm/Megolm)

## Architecture Constraints
- Must be a **drop-in replacement** for the existing TS MCP server
- Same SSE transport, same tool names, same argument schemas
- Agent Zero connects via MCP config — no agent-side changes required
- Single binary deployment (static linking preferred)
- Configuration via environment variables (same `.env` format as TS version)

## Development Workflow
1. **Spec-driven**: Write specs first, then implement
2. **Test-driven**: Every tool gets integration tests against a live Matrix server
3. **Incremental**: Ship v1 (API parity) before starting v2 (E2EE)
4. **AI-assisted**: AI handles code generation and scaffolding; human reviews architecture

## Key Reference Materials
| File | Path | Purpose |
|------|------|---------|
| Rust MCP Plan | `reference/docs/rust-mcp-server-plan.md` | 5-week roadmap |
| TS Source | `reference/ts-source/src/` | Current implementation to match |
| Tools Spec | `reference/docs/tools-spec.md` | Tool API contract (parity target) |
| TS README | `reference/docs/ts-mcp-readme.md` | Current server documentation |
| Ops Manual | `reference/docs/agent-matrix-ops-manual.md` | Deployment context |

## Coding Standards
- Use `rustfmt` defaults
- Use `clippy` with `warn` level
- Error handling via `thiserror` + `anyhow`
- Async runtime: `tokio` (multi-threaded)
- HTTP framework: `axum` (for SSE endpoints)
- Matrix SDK: `matrix-sdk` (ruma-based)
- Serialization: `serde` + `serde_json`
- Logging: `tracing` + `tracing-subscriber`

## Testing Strategy
- Unit tests in `src/` modules
- Integration tests in `tests/` against a test Dendrite/Continuwuity instance
- Agent0-2 homeserver (172.23.89.2) available as test target

## Deployment Target
- Replaces `node dist/index.js` in Agent-Matrix `startup-services.sh`
- Binary placed in agent workdir, launched by watchdog
- Same env vars: `MATRIX_HOMESERVER_URL`, `MATRIX_ACCESS_TOKEN`, `MCP_API_KEY`, `MCP_PORT`

## Important: Do NOT
- Modify agent-matrix infrastructure from this project
- Deploy to production agents without explicit approval
- Break API compatibility with the TS version
- Add features not in the spec without discussion
