# Quickstart: Invite Reconciliation Canary on `@agent0-2`

**Feature**: `004-invite-reconciliation` | **Canary**: `@agent0-2` (user choice for 004)

Run these checks on `@agent0-2` before you upgrade any other agent (SC-007). Run them from g2s. The commands use `${N}=2`.

> Warning: Do not run `switch-mcp-server.sh rust-restart`. A second store opener fails with `KeyStoreUnavailableError`. Restart only `run_mcp` through supervisord.

> Warning: Do not paste tokens, recovery keys, or API keys into logs or chat.

## 0. Local checks

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features e2ee -- -D warnings
cargo test
cargo test --features e2ee
```

## 1. Deploy the candidate to `@agent0-2`

1. Set `agent0-2` to `maintenance` in am-ctl.
2. Deploy the candidate image tag with `templates/scripts/deploy-mms-r2-to-agent.sh`.
3. Restart `run_mcp` only.
4. Confirm `/health` reports the new `version`, `crossSigned: true`, and `botEnabled: true`.
5. Confirm no `run_bot` process runs.
6. Set `agent0-2` back to `active`.

## 2. Offline invite (US1, SC-001)

1. Stop `run_mcp` on `agent0-2`.
2. From a trusted sender, create room A and invite `@agent0-2`.
3. Start `run_mcp`.
4. Wait for `/health` to report healthy.
5. Expect a join of room A within 60 seconds.
6. Expect exactly one greeting in room A.
7. Expect one `decision=join` log line and one `join ok` line for room A.

## 3. Runtime invite (US3, SC-002)

1. With `run_mcp` running, create room B from a trusted sender and invite `@agent0-2`.
2. Expect a join within 60 seconds and exactly one greeting.
3. Expect no second join request for room B in the logs (SC-005).

## 4. Untrusted invite (SC-003)

1. From the `t018-untrusted` account, create room C and invite `@agent0-2`.
2. Wait for 3 background syncs, which is about 90 seconds.
3. Expect no join.
4. Expect exactly one `decision=skip_untrusted` log line for room C.
5. Call `list_invitations` and expect room C with `"trusted": "false"` and `"attempts": 0`.

## 5. Discovery with the bot off (FR-011)

This step is optional, for a lab agent only.

1. Set `BOT_ENABLED=false` and restart `run_mcp`.
2. Call `list_invitations`.
3. Expect room C with `"trusted": "unknown"` and `"attempts": 0`.
4. Restore `BOT_ENABLED=true` and restart `run_mcp`.

## 6. Encrypted room (SC-006)

1. Invite `@agent0-2` to an encrypted room from a trusted sender.
2. Expect a join and a greeting the agent can decrypt.
3. Confirm `/health` still reports `crossSigned: true`.
4. Call `get_encryption_status`, and expect the same device id as before.

## 7. Clean up

1. Leave or decline rooms A, B, and C.
2. Confirm `list_invitations` returns `{"count":0,"truncated":false,"invitations":[]}`.
3. Record the results in `spec/T0NN-Agent0-2-Invite-Result-YYYYMMDD.md`.
