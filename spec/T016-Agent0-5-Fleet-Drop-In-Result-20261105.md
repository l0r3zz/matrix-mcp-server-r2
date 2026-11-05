# T016 result -- Fleet drop-in gate on agent0-5 (g2s)

T016 passed on 2026-11-05 at about 18:45 UTC. agent0-5 ran in its normal fleet configuration: `run_bot` (matrix-bot-rust v0.1.2) and `run_mcp` (this server) both running, `watchdog.sh` running, and am-ctl status `active`. The fix ships in agent-matrix `release/v0.92` (commits `5ef2a44` and `3cdf15d`). T014 and T016 have both passed, so 002 is complete.

## Conditions

| # | Condition | Result | Evidence |
|---|-----------|--------|----------|
| 1 | Bot on its own device and token; MCP keeps its cross-signed device; no path writes one token into both `.env` files | Pass | `split-bot-device.sh 5` at 06:05:45 UTC: `OK: bot on AgentZeroBot, MCP on XgtvALA4RK`. A repo audit found no remaining writer of the bot `.env` token (see below). |
| 2 | am-ctl `maintenance` suppresses restart and stop actions from the fleet cycle and `POST /health/{agent}/report` | Pass | Guard in `health_engine._record_check_and_act` (`agent.status != "active"` -> audit `auto_action_suppressed`, no action). Test `test_report_on_non_active_agent_records_but_suppresses_actions`; am-ctl suite 14 passed. Deployed to g2s am-ctl. |
| 3 | With `run_bot`, the watchdog, and `active` status, an Element message decrypts through `get-room-messages` after at least two watchdog cycles | Pass | Element sent `PING!!` in T014 encrypted 5 (`!0vizODELXR9WY5Nw51vGfGYe7nMbT8aKbET-Ng80Js0`) at about 18:42 UTC. `get_room_messages` returned `@admin:v-site.net: PING!!` in plaintext. The stack had run for about 12.5 hours (about 375 watchdog cycles) since the 06:06 UTC restart. |
| 4 | The bot still answers in an unencrypted room | Pass | `ping!` to agent0-5 in Fleet HQ Clean got `Agent0-5 here -- pong! Ready and awaiting your instructions.` |

State at 18:43 UTC: `run_bot` pid 483 (uptime 12:36:51), `run_mcp` pid 27 (uptime 12:37:16), am-ctl `active`, no open alerts, bot `.env` has `MATRIX_DEVICE_ID=AgentZeroBot`. Neither process restarted during the run.

## Condition 1 audit

Writers of the bot `.env` token on `release/v0.92`:

- `scripts/finalize-instance.sh` -- logs the bot in as `AgentZeroBot` and writes separate tokens. It fails if the two devices match.
- `templates/scripts/split-bot-device.sh` -- one-time migration for existing agents.
- `templates/scripts/fleet-token-healthcheck.sh` -- refreshes the MCP token on the device in `crypto-store/.device-id`. It does not touch the bot `.env`.

Readers only (no change needed): `watchdog.sh`, `fleet-membership-healthcheck.sh`, `destroy-and-rebuild.sh` (room leave), am-ctl `matrix.py`, `deployer.py`, `reconciler.py`, `agent_zero.py`, and `backup.py`. The deployer fingerprints the bot `.env` to prove a deploy leaves it unchanged.

am-ctl `check_sync_cursor` and the deployer pre-sync call `/sync?timeout=0` with the bot token. With separate devices, those calls can only touch `AgentZeroBot` to-device traffic, never the MCP device.

## Password reset on agent0-5

`split-bot-device.sh` needs the account password. The `MATRIX_PASSWORD` in `/opt/agent-zero/agent0-5/.env` was stale. The password was reset without signing out any device:

1. Source check at the running build (Continuwuity v26.9.1, commit `046074b`):
   - `users issue-token` calls `check_password` first, so it cannot help.
   - `users reset-password` removes devices only with `--logout`.
   - `set_password` writes only the password hash.
2. Wrote a new random 32-character password to `/opt/agent-zero/agent0-5/.env`. The old file is backed up as `.env.bak-<timestamp>-prereset`.
3. Recreated `agent0-5-continuwuity` once with a temporary compose override: `CONTINUWUITY_ADMIN_EXECUTE='["users reset-password agent0-5 <new>"]'`.
4. Recreated it again without the override, and deleted the override file. The second recreate discarded the logs that held the password.
5. Verified: the r2 token still resolves to `XgtvALA4RK`, the device list was `[XgtvALA4RK]`, and a probe login with the new password worked and was logged out.

## Known limit

matrix-bot-rust v0.1.2 has no E2EE. It does not answer in encrypted rooms. `003-integrated-matrix-bot` closes this.

## Fleet rollout (outside T016)

| Agent | Shared token today | `.env` password | Next action |
|-------|--------------------|-----------------|-------------|
| agent0-1 | Not checked | Works | `split-bot-device.sh 1` |
| agent0-2 | Not checked | Works | `split-bot-device.sh 2` |
| agent0-3 | Not checked | Works | `split-bot-device.sh 3` |
| agent0-4 | Yes (`4B6p3cc6QU`) | Stale | Same reset as agent0-5, then `split-bot-device.sh 4` |
| agent0-5 | No (split) | Reset | Done |
