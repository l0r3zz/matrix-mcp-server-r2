# Upgrade Result -- v0.2.0-rc.2 -- 2026-11-05

## Summary

v0.2.0-rc.2 changes encrypted `create_room` history visibility from `joined` to `invited`. That lets an invitee join after the agent has already posted. The image was installed on all five agents. The agent-posts-first inbound test then passed on agent0-1, agent0-2, and agent0-3.

## Code

Commit `a8d806e` on `002-e2ee-cross-signing`. Tag `v0.2.0-rc.2`. CI and the Docker image publish both succeeded. Each agent kept its crypto store and device:

| Agent | Device after rc.2 |
|-------|-------------------|
| agent0-1 | `matrix-mcp-server-device` |
| agent0-2 | `XkKM3pArkA` |
| agent0-3 | `nBfSBxuXUe` |
| agent0-4 | `4B6p3cc6QU` |
| agent0-5 | `XgtvALA4RK` |

## Proof of the join fix

Rooms created with `encryptionMode: "auto"`, invite `@admin:v-site.net`, then an encrypted hello **before** the user joined. `m.room.history_visibility` is `invited`.

The user accepted the three invites and sent messages. Element was able to send, which is the failure that rc.1 hit with `joined`.

| Room | Latest admin message as r2 sees it |
|------|-------------------------------------|
| agent0-3 `!L50fC3X1teAAHSQm9ov6Qwpq2D6lk-e90bYC8pwhywU` | `Hello` |
| agent0-2 / Galadriel `!79IVfo5gdQdPpKnXXQFQwe0xkr1LbZKAWOhXTkFyKqQ` | `verified` |
| agent0-1 / Gandalf `!oNGgjVOEptTx7-gw3ZtPeMg_mNouGMqSWZJKKwY1O24` | `and verified` |

Gandalf and Galadriel needed a manual identity verify in Element (master key compare). The agent cannot complete emoji or QR verification. Older messages in those rooms stay `missing_key` / `unknown_index`; that is expected for sessions from before the verify.

## Fleet

All five agents run r2 0.2.0 (image tag `0.2.0-rc.2`), Continuwuity 26.9.1, E2EE on, watchdog running, am-ctl `active`.
