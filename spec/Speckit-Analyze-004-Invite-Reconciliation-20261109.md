# Spec Kit Analysis: 004 Invite Reconciliation

**Date**: 2026-11-09
**Branch**: `004-invite-reconciliation` at `a1ded0a`
**Inputs**:
- `.specify/specs/004-invite-reconciliation/spec.md`
- `.specify/specs/004-invite-reconciliation/plan.md`
- `.specify/specs/004-invite-reconciliation/tasks.md`
- `.specify/specs/004-invite-reconciliation/research.md`
- `.specify/specs/004-invite-reconciliation/data-model.md`
- `.specify/specs/004-invite-reconciliation/contracts/list-invitations.md`
- `.specify/specs/004-invite-reconciliation/quickstart.md`
- `.specify/memory/constitution.md` v2.0.3

This is a read-only analysis. No spec, plan, or task file was changed.

## Findings

| ID | Category | Severity | Location(s) | Summary | Recommendation |
|----|----------|----------|-------------|---------|----------------|
| D1 | Constitution | CRITICAL | constitution.md:28-30, 35, 41, 51-52, 178; spec/matrix-mcp-v1-v2.yml:9, 120+; README.md:51; .cursor/rules/rust-e2ee.mdc:11 | Principle I says the kebab-case tool names (`create-room`, `send-message`) "MUST remain stable". The running server registers snake_case names (`list_joined_rooms`, `join_room`), because rmcp uses the method name. T001 fixes only line 178. T002 adds the snake_case `list_invitations` to a contract file that lists kebab-case names. Principle X then fails, because the contract and the runtime disagree. | Expand T001 to rename every tool name in the constitution to snake_case. Expand T002 to rename every `name:` in `spec/matrix-mcp-v1-v2.yml`. Add a task for `README.md` and `.cursor/rules/*.mdc`. The runtime names do not change, so Principle I stability holds. |
| D2 | Constitution | CRITICAL | tasks.md T004; data-model.md JoinError | Principle III says "Error types MUST use `thiserror`". T004 defines `JoinError` with a hand-written `Display`. | In T004, derive `thiserror::Error` for `JoinError`, with `#[error(...)]` that produces `"<class> <status> <code>"`. |
| D3 | Constitution | MEDIUM | tasks.md header | Principle III says "Any necessary `.clone()` MUST be documented with a comment". The reconciler clones `Client` and `Arc` handles, and no task says this. | Add the rule to the tasks header beside the no-new-dependency rule. |
| F1 | Inconsistency | HIGH | spec.md SC-003, US2 Independent Test; contracts/list-invitations.md; quickstart.md step 4 | The spec says `trusted: false`, which is a boolean. The contract and the quickstart use the string `"false"`. A client written from the spec would check the wrong type. | Keep the contract (`"true"`, `"false"`, `"unknown"`). Change the spec text to `trusted: "false"`. |
| E1 | Coverage | MEDIUM | spec.md US2 scenario 3, FR-007a, SC-003, SC-004; tasks.md T015, T020 | `list_invitations` is tested only with an inactive registry. No test shows `attempts` and `lastError` after failures, or `trusted: "false"` for an untrusted invite with the bot on. | Add to T020: after five 502 responses and after one 403, call `list_invitations`, and expect `attempts` 5 with `"temporary 502"`, and `attempts` 1 with `"permanent 403 M_FORBIDDEN"`. Add an untrusted case that expects `"trusted": "false"`. |
| E2 | Coverage | MEDIUM | spec.md FR-008; tasks.md T023 | FR-008 ("already a member" is success) has implementation but no test. | Add a T020 case: the join returns 200, and the next `/sync` shows the room as joined. Expect one greeting and no second join. |
| B1 | Ambiguity | MEDIUM | tasks.md T016 | "One more than `v0.3.1` in each feature build" leaves the expected number open. Both E2EE tools are registered in both builds (Principle II). | State the number: 22 tools (21 + 2) in both builds. |
| F2 | Inconsistency | LOW | spec.md FR-007; research.md R6; tasks.md T022 | The spec says "random jitter". The plan uses clock and room-id jitter, which is not random. | Accept it, and add one line to the spec Assumptions that says the jitter does not need to be cryptographically random. |
| F3 | Inconsistency | LOW | contracts/list-invitations.md Output | The contract says other tools "return text the same way". Other tools return plain text lines, not JSON. | Reword to say that the result is one text item that holds JSON, which is new for this server. |
| F4 | Inconsistency | LOW | .specify/specs/003-integrated-matrix-bot/ contracts and research; tasks.md T012 | T012 removes the 003 event-handler join. The 003 docs still describe it. | Add to T025: note in `src/AGENTS.md` that 004 replaces the 003 invite handler. Leave the 003 spec as the record of 003. |
| E3 | Coverage | LOW | spec.md FR-017; tasks.md T013 | There is no test that shutdown ends a retry wait. | Optional: a unit test that aborts the reconciler handle during `sleep_until` and checks that the handle finishes. |
| E4 | Coverage | LOW | spec.md FR-014 | No test checks the log fields or that log lines contain no secrets. | Optional: a `tracing` capture test. The canary log checks in `quickstart.md` cover this today. |
| U1 | Underspecified | LOW | research.md R3; tasks.md T009 | `invite_details()` needs the agent's own stripped `m.room.member` event. It is not yet confirmed that Continuwuity includes it in `invite_state` for an offline invite. | Model the T009 fixture on a real Continuwuity `/sync` invite. Check `inviter` in canary step 2. |
| B2 | Ambiguity | LOW | tasks.md T028; quickstart.md step 7 | The placeholder `T0NN` in the result file name. | Choose the number when the canary runs. |

## Coverage Summary

| Requirement | Has Task? | Task IDs | Notes |
|-------------|-----------|----------|-------|
| FR-001 | Yes | T013 | |
| FR-002 | Yes | T005, T013, T024 | |
| FR-003 | Yes | T009, T011 | |
| FR-004 | Yes | T008, T010 | |
| FR-005 | Yes | T008, T011 | |
| FR-006 | Yes | T020, T023 | |
| FR-007 | Yes | T019, T020, T022, T023 | |
| FR-007a | Yes | T019, T020, T021, T023 | The listing part is not tested (E1) |
| FR-008 | Yes | T023 | No test (E2) |
| FR-009 | Yes | T009, T011, T020 | |
| FR-010 | Yes | T014, T015, T017, T018 | |
| FR-011 | Yes | T015 | |
| FR-012 | Yes | T014, T015 | |
| FR-013 | Yes | T018 | Existing `join_room` tests cover it |
| FR-014 | Yes | T008, T011, T023 | No log test (E4) |
| FR-015 | Yes | T028 | By design (shared client) and the canary |
| FR-016 | Yes | T011, T020 | |
| FR-017 | Yes | T013 | No test (E3) |
| SC-001 | Yes | T009, T013, T028 | |
| SC-002 | Yes | T013, T028 | |
| SC-003 | Yes | T009, T028 | Bot-on listing not tested (E1) |
| SC-004 | Yes | T020 | |
| SC-005 | Yes | T020 | |
| SC-006 | Yes | T016, T028 | |
| SC-007 | Yes | T028 | |

## Constitution Alignment Issues

- **D1**: Tool naming in Principle I and in `spec/matrix-mcp-v1-v2.yml` does not match the running server. This drift was already there before 004. 004 makes it visible.
- **D2**: `JoinError` must use `thiserror`.

## Unmapped Tasks

None. T001 to T007 map to the plan and to Principles I, IX, and X. T025 to T027 map to DOX and CI.

## Metrics

- Total requirements: 25 (18 functional, 7 success criteria)
- Total tasks: 28
- Coverage: 100 percent (25 of 25 with at least one task)
- Requirements without a test: 3 (FR-008, FR-014, FR-017)
- Ambiguity count: 2 (B1, B2)
- Duplication count: 0
- Critical issues: 2 (D1, D2)

## Remediation Applied (2026-11-09)

The user chose to apply all findings. No task IDs changed.

| ID | Fix |
|----|-----|
| D1 | T001 renames every tool name in Principles I and II. T002 renames every name in `spec/matrix-mcp-v1-v2.yml`. T025 covers `.cursor/rules/*.mdc`, and T026 covers `README.md`. The plan's naming bullet now says the same. |
| D2 | T004 and `data-model.md` derive `thiserror::Error` for `JoinError`. |
| D3 | The tasks header lists the Principle III rules, including the `.clone()` comments. |
| F1 | `spec.md` SC-003 and the US2 Independent Test use `trusted: "false"`. |
| E1, E2, E4 | T020 adds the `list_invitations` checks after failures, the bot-on untrusted check, the already-a-member case, and a `tracing` capture check. |
| E3 | T019 adds the abort test for FR-017. |
| B1 | T016 expects exactly 22 tools in both builds. |
| B2 | The result file is `spec/T028-Agent0-2-Invite-Result-YYYYMMDD.md`. |
| F2 | `spec.md` Assumptions say the jitter does not need to be cryptographically random. |
| F3 | The contract says `list_invitations` is the first tool that returns JSON. |
| F4 | T025 records that 004 replaces the 003 invite handler. |
| U1 | T009 models the fixture on a Continuwuity invite. Quickstart step 2 checks the inviter in the log. |

## Next Actions

1. Run `/speckit-implement`, starting with Phases 1 to 3 (the MVP).
