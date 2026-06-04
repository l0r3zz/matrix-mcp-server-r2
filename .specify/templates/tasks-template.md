# Tasks: [FEATURE NAME]

**Input**: Design documents from `/specs/[###-feature-name]/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Include exact file paths in descriptions

## Phase 1: Setup (Shared Infrastructure)

- [ ] T001 [Description with file path]

## Phase 2: Foundational (Blocking Prerequisites)

- [ ] T002 [Description with file path]

**Checkpoint**: Foundation ready

## Phase 3: User Story 1 - [Title] (Priority: P1)

### Implementation

- [ ] T003 [Description with file path]

**Checkpoint**: User Story 1 functional and testable

## Phase N: Polish & Cross-Cutting Concerns

- [ ] TXXX Documentation updates
- [ ] TXXX Security hardening

## Dependencies & Execution Order

[Document phase dependencies and parallel opportunities]
