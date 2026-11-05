# Upgrade result -- agent0-4 to matrix-mcp-server-r2 v0.2.0-rc.1 (g2s)

On 2026-11-05, agent0-4 was upgraded from a v0.91 state to r2 `0.2.0-rc.1` with E2EE on. The run followed `docs/upgrade-to-v0.2.0.md` as committed in `a606ef6`. All four acceptance tests passed. Six guide problems were found and fixed in the same branch.

## Starting state

- r2 `0.1.1` with E2EE off. No `crypto-store/`.
- matrix-bot v0.1.2, sharing the MCP token and device `4B6p3cc6QU`. The account had one device.
- No cross-signing identity on the account.
- Continuwuity `v0.5.8` (image `2688ae09`, the same 2026-04-24 build agent0-5 ran before T014).
- `MATRIX_PASSWORD` in `/opt/agent-zero/agent0-4/.env` was stale.
- `watchdog.sh` was not running. Its last log line was on 2026-10-09.

## Run (UTC)

| Time | Step | Result |
|------|------|--------|
| 19:39 | Tag `v0.2.0-rc.1` on `1ad5b32` | CI, Publish Docker image, and Release Binary started. Publish succeeded in 11m45s. `:latest` still equals `0.1.1` (`sha256:0785d9d3...`). |
| 19:39 | Pause automation | agent0-4 `maintenance`. |
| 19:39 | Continuwuity upgrade | Data backup `mhs/continuwuity-data.bak-20261105-193953`. Now `continuwuity 26.9.1 (046074b)`. Token, device `4B6p3cc6QU`, and 2 joined rooms intact. |
| 19:40 | Password reset (guide block, `N=4`) | One `Successfully reset the password` line. Probe login OK. Device list unchanged. |
| 19:41 | Back up | `.env` (MCP, bot) `.bak-20261105-194100-pre-v020`, binary `.v01x`. |
| 19:41 | Split bot device | `OK: bot on AgentZeroBot, MCP on 4B6p3cc6QU`. |
| 19:52 | Install `MMS_R2_TAG=0.2.0-rc.1` | Image digest `sha256:bb6e635d...`. Binary reports `matrix-mcp-server-r2 0.2.0`. |
| 19:52 | Enable E2EE, restart `run_mcp` | `/health` after 2 s: `"crossSigned":true,"e2ee":true,"version":"0.2.0"`. `recovery-key` mode 0600. No UIAA. |
| 19:53 | Verify | `.device-id` = `4B6p3cc6QU`. Master and self-signing keys present. The device has 2 signatures. Split dry-run: `already on separate devices`. |
| 19:53 | Resume | agent0-4 `active`. Watchdog started by hand (pid 330408). |
| 19:54 | Test room | `create_room` (`auto`, invite `@admin:v-site.net`): `Encrypted: Yes`. 22 tools. `get_encryption_status`: `crossSigned: true`, `canDecryptNow: true`, `trustPolicy: cross-signed`. |

## Acceptance tests

| Test | Result | Evidence |
|------|--------|----------|
| 1. Encrypted inbound | Pass | Element sent `hello agent0-4` in "v0.2.0 upgrade test agent0-4" (`!J3MAQo7jtC2_2ukAFD4sZtu0deEQaImVJTJkHiXCrO8`). `get_room_messages` returned it in plain text at 20:05 UTC. |
| 2. Encrypted outbound | Pass | `send_message` returned `Encrypted: Yes` (`$5QYOiyfCYWD9ba_ybuWopT7IYAtfUqIaHv424sCuh3o`). The user confirmed Element shows the text. |
| 3. Bot | Pass | `Agent0-4: ping` in Fleet HQ Clean got `PONG`. |
| 4. Stability | Pass | `run_bot` pid 287950 and `run_mcp` pid 324658 unchanged from 19:53 to 20:06 UTC, with the watchdog running and the agent `active`. |

## Guide problems found and fixed

1. The password check and reset were listed before "pause automation", but the reset recreates Continuwuity twice. They are now step 4, after the pause.
2. The guide had no Continuwuity version check or upgrade. It now requires v26.9.1 or later and has an upgrade step with a data backup.
3. "Check the account has no cross-signing identity" had no command. A `/keys/query` snippet is added.
4. `deploy-mms-r2-to-agent.sh` ends with "switch-mcp-server.sh rust". The guide now says to ignore it, and shows release-candidate tags.
5. On first bootstrap, the SDK logs 404 `ERROR` lines for secret storage, cross-signing account data, and key backup. The guide now says these are expected. It also corrects the `/health` delay (2 s here).
6. The stability test assumed a running watchdog. The guide now checks for it and shows how to start it.

The new snippets (identity check, `whoami`, password check, and a wrong-password 403) were run against agent0-4 after the fix. No probe devices remained.

## Follow-ups

- Resolved: `watchdog.sh` was missing on agent0-1, agent0-2, and agent0-3 as well. Root cause and fix are in `spec/Watchdog-Startup-Script-Investigation-20261105.md`. All five agents now run v3.20 `startup-services.sh` and one watchdog each.
- Back up the agent0-4 and agent0-5 recovery keys off the host (guide step 10). Not confirmed at the time of writing.
