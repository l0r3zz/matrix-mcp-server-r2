# Matrix-MCP-SVR-R2 🦀

> Rust implementation of the Matrix MCP Server — a drop-in replacement for
> [matrix-mcp-server](https://github.com/user/matrix-mcp-server) (TypeScript)

## Status: Phase 0 — Project Setup

| Phase | Description | Status |
|-------|-------------|--------|
| 0 | Project scaffolding | ✅ Complete |
| 1 | Spec writing (Week 1) | ⬜ Not started |
| 2 | v1 Implementation — API parity (Weeks 2-3) | ⬜ Not started |
| 3 | v2 Implementation — E2EE (Weeks 4-5) | ⬜ Not started |
| 4 | Integration testing & deployment | ⬜ Not started |

## What This Is

A Rust rewrite of the Matrix MCP (Model Context Protocol) server that enables
AI agents (specifically Agent Zero) to interact with Matrix rooms, messages,
and users. The server exposes Matrix operations as MCP tools over an SSE
(Server-Sent Events) transport.

## Why Rust?

- **Performance**: Single binary, ~10MB, sub-millisecond tool dispatch
- **Safety**: Memory safety without GC; ideal for long-running server processes
- **E2EE**: Native access to `matrix-sdk-crypto` (Olm/Megolm) — no FFI needed
- **Deployment**: Static binary, no Node.js runtime, no `node_modules/`

## Architecture

```
┌─────────────┐     SSE      ┌──────────────────┐    C-S API    ┌─────────────┐
│  Agent Zero  │◄────────────►│  matrix-mcp-svr  │◄────────────►│   Matrix HS  │
│   (MCP)      │   (tools)    │     (Rust)       │   (HTTP)     │ (Continuwuity)│
└─────────────┘              └──────────────────┘              └─────────────┘
```

## Tool Parity Target (v1)

All 20 tools from the TypeScript implementation:

| Category | Tools |
|----------|-------|
| Rooms | list-joined-rooms, get-room-info, create-room, join-room, leave-room |
| Messages | get-room-messages, get-messages-by-date, send-message, send-direct-message |
| Members | get-room-members, invite-user, identify-active-users |
| Profiles | get-user-profile, get-my-profile, get-all-users |
| Admin | set-room-name, set-room-topic |
| Discovery | search-public-rooms, get-notification-counts, get-direct-messages |

## Tech Stack

| Component | Choice |
|-----------|--------|
| Language | Rust (2021 edition) |
| Async Runtime | tokio (multi-threaded) |
| HTTP/SSE | axum |
| Matrix SDK | matrix-sdk (ruma-based) |
| Serialization | serde + serde_json |
| Error Handling | thiserror + anyhow |
| Logging | tracing + tracing-subscriber |
| E2EE (v2) | matrix-sdk-crypto |

## Project Structure

```
Matrix-MCP-SVR-R2/
├── .a0proj/              # Agent Zero project config
├── reference/            # Reference materials
│   ├── ts-source/        # Original TS implementation
│   └── docs/             # Plans, specs, operational docs
├── specs/                # spec-kit specifications (Phase 1)
├── src/                  # Rust source code (Phase 2+)
├── tests/                # Integration tests
├── Cargo.toml            # (Phase 2)
└── README.md             # This file
```

## Parent Project

This server is a component of the **Agent-Matrix** sovereign agent fleet.
When complete, it replaces the TypeScript MCP server in the agent deployment
pipeline.

---

*Started: 2026-03-11 | Human-AI Collaboration Project*
