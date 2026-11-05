# Upgrade Result -- agent0-5 to v0.2.0-rc.1 -- 2026-11-05

## Summary

agent0-5 was upgraded with `docs/upgrade-to-v0.2.0.md`, for consistency with agent0-4. Before the upgrade it ran the T016 test build: r2 reporting `0.1.1`, built from the 002 branch with E2EE already on. The upgrade replaced only the binary. All four step 11 tests passed.

## Starting state

- Continuwuity 26.9.1 (046074b)
- r2 binary reporting `0.1.1` (T016 build), `E2EE_ENABLED=true`, cross-signed
- MCP device `XgtvALA4RK`; bot device `AgentZeroBot`

## Steps

| Step | Result |
|------|--------|
| 1. Pause | Pass. am-ctl status `maintenance`. |
| 2. Continuwuity | No-op. Already 26.9.1. |
| 3. Back up | Pass. Backup timestamp `20261105-203726`, plus `matrix-mcp-server-r2.v01x`. |
| 4. Password check | Pass. `password OK`. |
| 5. Split | No-op. Dry run: `OK: already on separate devices`. |
| 6. Install | Pass. `MMS_R2_TAG=0.2.0-rc.1`; binary reports `0.2.0`. |
| 7. Enable E2EE | No-op. `E2EE_ENABLED` and `CRYPTO_STORE_PATH` already set; `.device-id` is `XgtvALA4RK`. |
| 8. Restart | Pass. `/health` answered after about 30 s. No ERROR lines: the existing store, identity, and recovery key were reused. |
| 9. Verify | Pass. `crossSigned:true`, `e2ee:true`, `version:0.2.0`. `recovery-key` and `.device-id` are mode 0600. |
| 10. Recovery key | Unchanged from T014. The user still has to copy it off the host. |
| 11. Resume and test | Pass. Watchdog running; am-ctl status `active`. |

## Tests (room "T014 encrypted 5")

| Test | Result |
|------|--------|
| 1. Encrypted inbound | Pass. `get_room_messages` read "Read you loud and clear" as plain text. |
| 2. Encrypted outbound | Pass. `send_message` returned `Encrypted: Yes`. |
| 3. Bot | Pass. `@agent0-5: ping` in Fleet HQ Clean got a pong. |
| 4. Stability | Pass. `run_bot` pid 483 and `run_mcp` pid 821910 were unchanged from 20:39 to 20:43 UTC, across two watchdog cycles (20:39:54 and 20:41:54).

## Host fix found during pre-flight for agent0-1..3

The host still had pre-v0.92 copies of `templates/scripts/deploy-mms-r2-to-agent.sh` (default tag `0.1.0`) and `scripts/finalize-instance.sh` (no device split). Both were replaced with the `release/v0.92` versions; backups are `*.pre-v092-<timestamp>`. Neither affected agent0-4 or agent0-5: both runs passed the tag explicitly, and `finalize-instance.sh` runs only for new instances. |

## Guide changes

- Step 8 now says that a re-upgrade with an existing `crypto-store/` reuses the identity and logs no first-start errors.
