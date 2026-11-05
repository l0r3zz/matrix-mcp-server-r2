# Watchdog / startup-services.sh Investigation -- 2026-11-05

## Summary

watchdog.sh v3 was not running on agent0-1, agent0-2, or agent0-3 (or agent0-4 before its v0.2.0 upgrade). There are two causes.

1. The October boots ran an old in-container startup-services.sh (v3.17 to v3.19) with no watchdog phase.
2. On 2026-10-31 at 00:29 UTC, sync-fleet.sh overwrote startup-services.sh on all five agents with a March v3.16 stub. The script takes its source from a stale tree.

The host template is current: v3.20, md5 `25b600a8`. It is not the problem.

## Evidence

| Item | Finding |
|------|---------|
| In-container `/a0/usr/workdir/startup-services.sh` (all 5) | md5 `cb30af0b`, 35-line "Golden Template (v3.16)" stub, mtime 2026-10-31 00:29 UTC |
| `agent0-1/usr/projects/agent-matrix/.../templates/scripts/startup-services.sh` | md5 `cb30af0b`, the same stub |
| `templates/scripts/sync-fleet.sh` (g2s, line 19) | `TEMPLATE_DIR=${BASE_DIR}/agent0-1/usr/projects/agent-matrix/multi-instance-deploy/templates`; lines 128-137 copy `startup-services.sh` from that tree |
| Same 00:29 window | matrix_bot.py, requirements.txt, and both `.env` files rewritten on all agents. The `.env` rewrite is sync-fleet's backup-and-`mv` restore. |
| `/tmp/rust-build.log` 00:29:13, `/tmp/fixfleet.sh` 00:31, `/tmp/normalize.sh` 03:00 | Oct 31 bot-tracing session that rebuilt and redeployed matrix-bot-rust |
| am-ctl `audit_log` 00:31 UTC | `auto_restart_run_bot` on agents 1, 2, 4, 5 (bots killed during the sync) |
| am-ctl release bundles v1.0.0-v1.2.0 | Contain a 5.5-5.9 KB startup-services.sh, not the stub, so not the writer |
| startup-services.log on agent0-1/3 (Oct 14) | Ends at "Phase 5: ... under supervisord management". No Phase 5.5 line, no "complete". |
| startup-services.log on agent0-2 (Oct 17) | Completes, but has no Phase 5.5 line |

v3.20 logs "Phase 5.5: Watchdog v3.0 launched" before the "under supervisord management" line. Its absence shows the October boots ran a pre-v3.20 script without a watchdog phase.

sync-fleet.sh was retired on 2026-10-17: it was moved to `scripts/legacy/` with `chmod -x`, superseded by `am-ctl reconcile` (see `docs/permanent-repair-progress-gates.md`). The Oct 31 session ran a copy from agent0-1's tree. `/tmp/normalize.sh` then copied it back into the live `templates/scripts/`.

## State before the fix

| Agent | watchdog |
|-------|----------|
| agent0-1 | not running |
| agent0-2 | not running |
| agent0-3 | not running |
| agent0-4 | running (started by hand during the v0.2.0 upgrade) |
| agent0-5 | running (the stub launches it) |

The stub launches the watchdog but skips these v3.20 phases:

- Phase 0.1: SSH key restore
- Phase 0.5: Time Travel patches
- Phase 5: supervisord preflight
- Phase 6: wiki server

Phase 6 serves only `galadriel-workspace/wiki-site`, which exists only on agent0-2, and that wiki is up.

## Resolution (applied 2026-11-05 ~20:22 UTC)

1. Re-retired sync-fleet.sh on g2s: moved `templates/scripts/sync-fleet.sh` to `scripts/legacy/sync-fleet.sh` and ran `chmod -x`. It was not repointed for two reasons:
   - Its MCP step rsyncs the template directory, which holds an April 2026 `matrix-mcp-server-r2`, and would downgrade every agent.
   - Its `--restart` step launches the retired Node and Python services.
2. Installed v3.20 (md5 `25b600a8`) as `/a0/usr/workdir/startup-services.sh` on all five agents. The previous copy is kept as `startup-services.sh.bak.stub-20261105`.
3. Started watchdog v3.0 on agent0-1, agent0-2, and agent0-3 with no container restart. All five agents now run one watchdog, and agents 1-3 logged "Reported health cycle".
4. Left agent0-1's `usr/projects/agent-matrix` in place. It is an active Agent Zero project (`.a0proj`, 548 MB). With sync-fleet.sh retired, nothing uses it as a fleet source.

## Open items

- g2s SSH access is granted per agent. Only Gandalf (agent0-1) and Galadriel (agent0-2) have infrastructure access: each has an ed25519 key in `/root/.ssh`, a backup in the ssh2g2s skill, and an entry in g2s `authorized_keys`. agent0-3, agent0-4, and agent0-5 have no key by design. Phase 0.1 logs "skipping" on those agents.
- Galadriel's wiki on agent0-2 port 8080 serves OpenBrain to human readers. It was not touched; preserve its state. Phase 6 starts it only if no server is running.
- Fleet-wide template pushes should go through `am-ctl reconcile apply` / `am-ctl deploy` or `deploy-mms-r2-to-agent.sh`, never through scripts in agent project trees.
