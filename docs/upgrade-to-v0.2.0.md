# Upgrade guide: matrix-mcp-server-r2 v0.1.x to v0.2.0 (cross-signed E2EE)

This guide upgrades an existing Agent-Matrix v0.91 deployment to matrix-mcp-server-r2 v0.2.0 with E2EE turned on. v0.2.0 is the merge of branch `002-e2ee-cross-signing`.

Starting point:

- Agent-Matrix v0.91 on the Docker host, with am-ctl running.
- matrix-mcp-server-r2 v0.1.x as the MCP server in each agent container (`run_mcp`).
- matrix-bot-rust v0.1.2 as `run_bot`.
- The bot and the MCP server share one Matrix access token, so they share one Matrix device.

End state:

- r2 v0.2.0 runs with `E2EE_ENABLED=true`. It owns a cross-signed device and a persistent SQLite crypto store.
- matrix-bot runs on its own device, `AgentZeroBot`.
- Element users can exchange encrypted messages with the agent through the MCP tools.

Validated on agent0-5 (g2s) with Continuwuity v26.9.1. See `spec/T014-Agent0-5-Result-20261104.md` and `spec/T016-Agent0-5-Fleet-Drop-In-Result-20261105.md`.

## Read this first

**Agent-Matrix v0.92 is required.** E2EE ties the crypto store to one Matrix device. Do not enable E2EE on a v0.91 deployment for these reasons:

1. In v0.91, the bot shares the MCP token. Both processes call `/sync` on one device. Whichever syncs first consumes the room keys. The other process then sees `missing_key` for every inbound message. This was the T014 root cause.
2. The v0.91 `fleet-token-healthcheck.sh` re-logs in with the fixed device id `matrix-mcp-server-device`. r2 v0.2.0 refuses to start when the token device does not match `$CRYPTO_STORE_PATH/.device-id` (`KeyStoreUnavailable`). One token refresh would take the MCP server down.
3. In v0.91, am-ctl restarts `run_bot` even when the agent is in `maintenance`, through the watchdog report endpoint. That makes a controlled upgrade hard.

Agent-Matrix v0.92 fixes all three. It splits the bot onto its own device, refreshes the MCP token on its stored device, and makes `maintenance` suppress automatic actions.

**Behavior changes that affect the fleet:**

- `create_room` and `send_direct_message` default to `encryptionMode: "auto"`. A room is encrypted when every invitee has cross-signed E2EE keys. Other agents on v0.2.0 qualify.
- matrix-bot v0.1.2 cannot read encrypted rooms. An agent's bot does not answer in an encrypted room. Stage 2 (`003-integrated-matrix-bot`) closes this gap.
  - For rooms where another agent's bot must answer, pass `encryptionMode: "none"`.
  - Existing unencrypted rooms, such as Fleet HQ, stay unencrypted.
- Room keys go only to cross-signed devices. An Element session that the user has not verified gets `m.room_key.withheld` and cannot read the agent's messages. Users must verify each Element session.
- Two new tools: `bootstrap_e2ee` and `get_encryption_status`. The total is 22 tools.
- `GET /health` adds `"e2ee"` and `"crossSigned"`.
- On first start, r2 waits for the initial sync before it binds HTTP. `/health` can take 30 to 90 seconds to answer.

## Before you start

Run these on the Docker host. `N` is the agent number.

1. **Install Agent-Matrix v0.92 on the host.** The host tree at `/opt/agent-zero/multi-instance-deploy` is a copy, not a git checkout. Copy these files from agent-matrix `v0.92` (`release/v0.92` until it is tagged):
   - `templates/scripts/split-bot-device.sh`
   - `templates/scripts/fleet-token-healthcheck.sh`
   - `scripts/finalize-instance.sh`
   - `am-ctl/amctl/services/health_engine.py`

   Then restart am-ctl: `sudo systemctl restart am-ctl`.
2. **Check the account password.** `split-bot-device.sh` logs in with the agent's password from `/opt/agent-zero/agent0-N/.env` (`MATRIX_PASSWORD`). Test it with a throwaway login, and log the probe device out afterward. If it fails, see "Reset a stale password" below. Do not use any reset that signs out devices.
3. **Check the account has no cross-signing identity yet.** This matters if a human ever signed in as the agent in Element and set up cross-signing. In that case, r2 returns `IdentityExistsError` until you give it that identity's recovery key in `MATRIX_RECOVERY_KEY_FILE`.
4. **Plan a backup location for the recovery key.** Keep it off the Docker host.

## Upgrade one agent

Do one agent at a time. Finish all checks before the next agent.

### 1. Pause automation

```bash
curl -s -X PUT -H "X-API-Key: $AMCTL_API_KEY" -H "Content-Type: application/json" \
  -d '{"status":"maintenance"}' http://127.0.0.1:8077/api/v1/agents/agent0-N
```

With am-ctl v0.92, the agent's health checks are still recorded, but restart and stop actions are skipped. The audit log shows `auto_action_suppressed`.

### 2. Back up

```bash
D=/opt/agent-zero/agent0-N/usr/workdir
TS=$(date +%Y%m%d-%H%M%S)
cp -p $D/matrix-mcp-server/.env           $D/matrix-mcp-server/.env.bak-$TS-pre-v020
cp -p $D/matrix-bot/.env                  $D/matrix-bot/.env.bak-$TS-pre-v020
cp -p $D/matrix-mcp-server/matrix-mcp-server-r2 $D/matrix-mcp-server/matrix-mcp-server-r2.v01x
```

### 3. Give the bot its own device

```bash
cd /opt/agent-zero/multi-instance-deploy/templates/scripts
./split-bot-device.sh --dry-run N
./split-bot-device.sh N
```

Expected: `OK: bot on AgentZeroBot, MCP on <device>`. The MCP server keeps its current device, and that device becomes the E2EE device. The script restarts `run_bot`.

### 4. Install the v0.2.0 binary

```bash
MMS_R2_TAG=0.2.0 ./deploy-mms-r2-to-agent.sh N
docker exec agent0-N /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2 --version
# matrix-mcp-server-r2 0.2.0
```

The GHCR image for v0.2.0 is built with the `e2ee` feature. Builds of v0.1.x are not, and they ignore `E2EE_ENABLED`.

### 5. Turn on E2EE

Add these lines to `/a0/usr/workdir/matrix-mcp-server/.env` in the container. The host path is `/opt/agent-zero/agent0-N/usr/workdir/matrix-mcp-server/.env`.

```bash
E2EE_ENABLED=true
CRYPTO_STORE_PATH=/a0/usr/workdir/matrix-mcp-server/crypto-store
```

`CRYPTO_STORE_PATH` must be on the host bind mount (`/a0/usr/workdir`). Then the store survives a container recreate. r2 creates the directory with mode 0700.

Optional settings:

- `MATRIX_RECOVERY_KEY_FILE` -- defaults to `$CRYPTO_STORE_PATH/recovery-key`. Set it only to restore an existing identity.
- `MATRIX_UIAA_PASSWORD_FILE` -- a mode 0600 file with the account password. r2 uses it only if the homeserver asks for UIAA when it uploads the cross-signing keys. Continuwuity v26.9.1 did not ask on agent0-5. Add it only if the log shows `UiaaRequiredError`.

### 6. Restart the MCP server

```bash
docker exec agent0-N supervisorctl restart run_mcp
```

If the agent runs r2 through `switch-mcp-server.sh` instead of supervisord, run `switch-mcp-server.sh rust-restart`.

On first start r2 does these things:

1. Writes `crypto-store/.device-id` from `whoami`.
2. Opens the SQLite store and takes `.store.lock`.
3. Creates the cross-signing identity.
4. Writes `crypto-store/recovery-key` with mode 0600.

### 7. Verify

```bash
docker exec agent0-N curl -s http://127.0.0.1:3000/health
# {"crossSigned":true,"e2ee":true,...,"status":"healthy",...,"version":"0.2.0"}

docker exec agent0-N ls -la /a0/usr/workdir/matrix-mcp-server/crypto-store
# .device-id, recovery-key (-rw-------), matrix-sdk-*.sqlite3, .store.lock
```

Check that the bot and the MCP server are on different devices. `split-bot-device.sh --dry-run N` prints `OK: already on separate devices` when they are.

### 8. Back up the recovery key

Copy `crypto-store/recovery-key` to a secret store off the host. This key restores the cross-signing identity if the crypto store is lost. Without it, a lost store means a new identity, and every user must verify the agent again.

### 9. Resume automation and test

```bash
curl -s -X PUT -H "X-API-Key: $AMCTL_API_KEY" -H "Content-Type: application/json" \
  -d '{"status":"active"}' http://127.0.0.1:8077/api/v1/agents/agent0-N
```

Wait at least two watchdog cycles (about 5 minutes). Then do these tests:

1. **Encrypted inbound.** Use `create_room` with `encryptionMode: "auto"` and invite a user with a verified Element session. Send a message from Element. Read it with `get_room_messages`. It must show plain text, not `[Encrypted message - unable to decrypt (missing_key)]`.
2. **Encrypted outbound.** Use `send_message` in that room. The result shows `Encrypted: Yes`, and Element shows the text.
3. **Bot.** In an unencrypted fleet room, send `@agent0-N: ping`. The bot answers.
4. **Stability.** `supervisorctl status` shows the same `run_bot` and `run_mcp` pids as before the wait.

If Element still shows `missing_key` for new messages, the Element session may be reusing an old Megolm session. Run `/discardsession` in the room and send again.

## Operating rules after the upgrade

- **Never re-login the MCP token on a new device.** A manual token refresh must pass the stored device id: `"device_id": "<contents of crypto-store/.device-id>"`. The v0.92 `fleet-token-healthcheck.sh` does this.
- **Never share one token between `.env` files.** The bot uses `AgentZeroBot`. The MCP server uses the device in `.device-id`.
- **Never run two processes on one crypto store.** r2 holds an exclusive lock and returns `KeyStoreUnavailable` to a second process.
- **Do not delete `crypto-store/`** to fix a problem. Deleting it creates a new device, and every user must verify the agent again. If you must recreate it, keep `recovery-key` and set `MATRIX_RECOVERY_KEY_FILE` to it.
- **Do not reset the password with "log out all devices".** That deletes the MCP device and its keys. Continuwuity's `users reset-password` without `--logout` is safe. See below.

## Reset a stale password

Use this when `split-bot-device.sh` fails with `M_FORBIDDEN` and nobody knows the password. Confirmed against Continuwuity v26.9.1 source: `users reset-password` removes devices only with `--logout`. `users issue-token` needs the current password, so it does not help.

The agent is not in its server's admin room, so run the command at startup with `admin_execute`:

```bash
cd /opt/agent-zero/agent0-N
NEWPW=$(python3 -c 'import secrets,string;a=string.ascii_letters+string.digits;print("".join(secrets.choice(a) for _ in range(32)))')
cp -p .env .env.bak-$(date +%Y%m%d-%H%M%S)-prereset
sed -i "s/^MATRIX_PASSWORD=.*/MATRIX_PASSWORD=${NEWPW}/" .env
umask 077
cat > docker-compose.reset.yml <<YML
services:
  continuwuity:
    environment:
      CONTINUWUITY_ADMIN_EXECUTE: '["users reset-password agent0-N ${NEWPW}"]'
YML
docker compose -f docker-compose.yml -f docker-compose.reset.yml up -d continuwuity
sleep 8
docker logs agent0-N-continuwuity 2>&1 | grep -c "Successfully reset the password"   # expect 1
docker compose up -d continuwuity     # recreate without the override; discards logs that held the password
rm -f docker-compose.reset.yml
unset NEWPW
```

Never pass `--logout` or `-l`. Then confirm the MCP token still works, and that `/devices` still lists the MCP device.

## Roll back

Rollback returns the agent to unencrypted r2 v0.1.x. Encrypted rooms that the agent created stay encrypted, and the agent cannot read them after rollback.

1. Set the agent to `maintenance`.
2. Restore the binary: `cp -p matrix-mcp-server-r2.v01x matrix-mcp-server-r2`.
3. Set `E2EE_ENABLED=false` in the MCP `.env`, or restore `.env.bak-*-pre-v020`.
4. Leave `crypto-store/` in place, so a later re-upgrade keeps the same identity.
5. `supervisorctl restart run_mcp`, then set the agent to `active`.

Keep the bot on its own device. That split is safe with v0.1.x and needs no rollback.

## Fleet rollout checklist

| Step | Done |
|------|------|
| Agent-Matrix v0.92 files on the host, am-ctl restarted | |
| Per agent: password checked or reset safely | |
| Per agent: maintenance, backup, split, install, enable, restart, verify | |
| Per agent: recovery key copied off the host | |
| Per agent: active, two watchdog cycles, four tests passed | |
| Fleet rooms that need bot replies use `encryptionMode: "none"` | |
