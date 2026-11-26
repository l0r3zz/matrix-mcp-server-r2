# Feature 004 -- Spec and Implementation Status

Date: 2026-11-25

Feature: `.specify/specs/004-invite-reconciliation/`
Branch name in the spec: `004-invite-reconciliation`
Base: `main` at `d1bde46` (tag `v0.3.1`)
Crate version in `Cargo.toml`: `0.4.0` (applied on 2026-11-26; not tagged)

## Spec

Status in `spec.md`: Draft. The requirements checklist is fully checked. No `[NEEDS CLARIFICATION]` markers remain.

The feature exists because `@agent0-2` missed a trusted invite to an encrypted room and had no tool that listed pending invites. A later manual `join_room` with the recovered room id worked.

Three stories:

| Story | Priority | Behavior |
|-------|----------|----------|
| US1 | P1 | When `BOT_ENABLED=true`, join trusted invites that arrived while the service was stopped, then post the 003 greeting once. |
| US2 | P1 | `list_invitations` lists pending invites with room fields, trust, attempts, and last error. It works with the bot off (`trusted: "unknown"`). |
| US3 | P2 | Join trusted invites while running. Temporary failures retry at most 5 times. Permanent failures stop at once. One join per room at a time. |

Trust stays the 003 set: am-ctl trusted senders, then `TRUSTED_SENDERS` only while that set is empty. A failed lookup with an empty fallback does not join. Match is the exact full Matrix user id.

Tool surface grows from `20 + 2` to `21 + 2`. `join_room` is unchanged. Canary is `@agent0-2` (stated exception to the `@agent0-5` lab default).

Supporting artifacts, all present:

- `plan.md`, `research.md`, `data-model.md`
- `contracts/list-invitations.md`
- `quickstart.md`
- `spec/Invite-Reconciliation-Speckit-Approach-20261109.md`
- `spec/Speckit-Analyze-004-Invite-Reconciliation-20261109.md`
- Constitution v2.1.0 (T001)

## Implementation

Tasks T001 through T024 are marked done in `tasks.md`. The code matches that claim:

- `src/bot/invites.rs` -- registry, `decide`, `reconcile_pass`, `classify`, `RetryPolicy`, `spawn_reconciler`, `render_invitations`
- `src/bot/mod.rs` -- `pub mod invites`
- `src/mcp/server.rs` -- `list_invitations` (`#[tool]`, no input)
- `src/main.rs` -- one `InviteRegistry`, one `Notify`, reconciler started after `bot::attach` returns `Some`
- Tests in `src/bot/invites.rs`, `tests/bot_invites.rs`, and `tests/server_tool_list_tests.rs`

Phase 6 is open:

| Task | State |
|------|--------|
| T025 docs | Done on 2026-11-25. Tool names in `src/AGENTS.md`, the root `AGENTS.md` index, and the three Cursor rules are snake_case. The tool count is `21 + 2`. JSON input fields stay camelCase. |
| T026 README | Done on 2026-11-25. `README.md` lists snake_case tools, `list_invitations`, and automatic join. |
| T026b version | Done on 2026-11-26. `Cargo.toml` `version` is `0.4.0`. Not tagged. |
| T027 fmt, clippy, tests | Passed on 2026-11-26. Both clippy feature sets and both test feature sets. |
| T028 `@agent0-2` canary | Passed on 2026-11-26. Recorded in `spec/T028-Agent0-2-Invite-Result-20261126.md`. The 2026-11-11 run remains the failed record. No tag or push. |

## Canary (2026-11-11)

Target: `@agent0-2:agent0-2-mhs.cybertribe.com`. Binary built with `--features e2ee`. `/health` was healthy, `version` `0.3.1`, `crossSigned` true, `botEnabled` true. Device id stayed `XkKM3pArkA`. No tag. No push. No other agent was upgraded.

| Check | Result |
|-------|--------|
| Offline trusted invite | Pass. `decision="join"`, membership `join`, one greeting. |
| Runtime trusted invite | Fail. Invite returned 200, then `make_join` returned 403 `You are not invited to this room.` Classified permanent. No second attempt. No greeting. |
| Untrusted invite | Pass for the decision (`skip_untrusted` once). `list_invitations` body was not captured on this step. |
| Encrypted trusted invite | Fail. Same 403 on `make_join` after a 200 invite. Membership stayed `invite`. |
| Cleanup | `list_invitations` returned `count: 0` after leaves. Bot-off quickstart check was not run. |

During that run, the `run_mcp` binary was the 004 canary build. The previous binary was `matrix-mcp-server-r2.bak-pre-004`. The 2026-11-26 deploy replaced the running binary.

## Left before the `v0.4.0` tag

1. T029 fixed the runtime `make_join` 403 (`You are not invited to this room`) on 2026-11-26. That 403 retries while the invite remains. A ban, a missing room, an unsupported room version, and a 403 after the invite is gone stay one attempt.
2. T026b and T027 are done. `Cargo.toml` and `Cargo.lock` are `0.4.0`. The tag is not created.
3. T028 passed on 2026-11-26. The record is `spec/T028-Agent0-2-Invite-Result-20261126.md`. Tag and push only when you ask.

Agent-matrix kebab-case docs (`operations-manual.md` and the other files listed at the bottom of `tasks.md`) stay out of 004. They are a follow-up after `v0.4.0`.
