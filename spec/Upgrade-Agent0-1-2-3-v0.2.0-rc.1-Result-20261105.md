# Upgrade Result -- agent0-1, agent0-2, agent0-3 to v0.2.0-rc.1 -- 2026-11-05

## Summary

All three agents were upgraded with `docs/upgrade-to-v0.2.0.md` between 21:03 and 21:06 UTC. Steps 1-9 passed on each, with no password resets and no restarts beyond the guide's. Tests 2 (outbound), 3 (bot), and 4 (stability) passed. Test 1 (inbound) is blocked by an r2 bug in `create_room` (see below). The bug is not caused by the upgrade.

## Per agent

| | agent0-3 | agent0-2 (Galadriel) | agent0-1 (Gandalf) |
|---|---|---|---|
| Continuwuity | 0.5.8 to 26.9.1 | 0.5.8 to 26.9.1 | 0.5.8 to 26.9.1 |
| Data backup | `continuwuity-data.bak-20261105-210339` | `continuwuity-data.bak-20261105-210435` | `continuwuity-data.bak-20261105-210537` |
| Password check | OK | OK | OK |
| Split | bot moved to `AgentZeroBot` | bot moved to `AgentZeroBot` | no-op; bot stays on `1SIEbW45yd` (user decision) |
| E2EE device | `nBfSBxuXUe` | `XkKM3pArkA` | `matrix-mcp-server-device` |
| `/health` | 0.2.0, e2ee, crossSigned | 0.2.0, e2ee, crossSigned | 0.2.0, e2ee, crossSigned |
| Extra checks | none | wiki pid 20 and HTTP 200 before and after; SSH key unchanged | SSH key and ssh2g2s backup unchanged |

## Tests

| Test | agent0-3 | agent0-2 | agent0-1 |
|---|---|---|---|
| 1. Encrypted inbound | Blocked (bug below) | Blocked | Blocked |
| 2. Encrypted outbound | Pass | Pass | Pass |
| 3. Bot ping in Fleet HQ Clean | Pass | Pass | Pass (bot not restarted; reconnected after the homeserver restart) |
| 4. Stability, 21:04-21:30 UTC | Pass (`run_bot` 60585, `run_mcp` 60849) | Pass (2803061, 2803729) | Pass (3443612, 3775500) |

## Bug: invitee cannot send when the agent posts before the invitee joins

`create_room` (`src/mcp/server.rs`) sets `m.room.history_visibility` to `joined` in encrypted rooms.

Sequence in the three test rooms:

1. The agent created the room and invited `@admin:v-site.net` (21:04).
2. The agent sent an encrypted hello (21:04).
3. The user joined from Element (21:28).

v-site.net Synapse joined with partial state ("faster joins"). To resolve state, it requested the hello event with `GET /_matrix/federation/v1/event/<id>`. Continuwuity 26.9.1 returned `403 Forbidden`, because no v-site.net user was joined at that event. The Synapse log shows `Failed to get state for <room>`. The room stays in partial state, and Element does not send the user's messages.

agent0-4 passed test 1 because the user joined (19:55) before the agent posted (20:04).

Proposed fix: use `HistoryVisibility::Invited`. Invitees can then fetch events sent after their invite, and users who were never invited still cannot read history.

The stuck test rooms are `!9mizzRKbKAa7SWzS944D_8EYxk_HcCg52lN4WzAuNkA` (agent0-3), `!TJ43X83CjT2UAJV5RGPc1ipwFpGLOtH1BbrPfFRtLeA` (agent0-2), and `!sG79IdE39FwJChCHeRzlyO8wtWGd6zw_wLHQyvXVRps` (agent0-1).
