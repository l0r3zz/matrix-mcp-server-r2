# Implementation Plan: [FEATURE]

**Branch**: `[###-feature-name]` | **Date**: [DATE] | **Spec**: [link]
**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

## Summary

[Extract from feature spec: primary requirement + technical approach from research]

## Technical Context

**Language/Version**: [e.g., Rust 1.88, 2021 edition]
**Primary Dependencies**: [e.g., matrix-sdk 0.10, rmcp 1.2]
**Storage**: [if applicable]
**Testing**: [e.g., cargo test]
**Target Platform**: [e.g., Linux server / Docker container]
**Project Type**: [e.g., library/cli/web-service]
**Performance Goals**: [domain-specific]
**Constraints**: [domain-specific]
**Scale/Scope**: [domain-specific]

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

[Gates determined based on constitution file]

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
└── tasks.md
```

### Source Code (repository root)

```text
src/
├── main.rs
├── config.rs
├── error.rs
├── auth.rs
├── matrix/
│   ├── client.rs
│   └── cache.rs
└── mcp/
    └── server.rs
tests/
```

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
