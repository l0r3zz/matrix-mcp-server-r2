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

Validated on g2s with Continuwuity v26.9.1:

- agent0-5: `spec/T014-Agent0-5-Result-20261104.md` and `spec/T016-Agent0-5-Fleet-Drop-In-Result-20261105.md`.
- agent0-4: a full run of this guide from a v0.91 state. See `spec/Upgrade-Agent0-4-v0.2.0-rc.1-Result-20261105.md`.

## Read this first

**Agent-Matrix v0.92 is required.** E2EE ties the crypto store to one Matrix device. Do not enable E2EE on a v0.91 deployment for these reasons:

1. In v0.91, the bot shares the MCP token. Both processes call `/sync` on one device. Whichever syncs first consumes the room keys. The other process then sees `missing_key` for every inbound message. This was the T014 root cause.
2. The v0.91 `fleet-token-healthcheck.sh` re-logs in with the fixed device id `matrix-mcp-server-device`. r2 v0.2.0 refuses to start when the token device does not match `$CRYPTO_STORE_PATH/.device-id` (`KeyStoreUnavailable`). One token refresh would take the MCP server down.
3. In v0.91, am-ctl restarts `run_bot` even when the agent is in `maintenance`, through the watchdog report endpoint. That makes a controlled upgrade hard.

Agent-Matrix v0.92 fixes all three. It splits the bot onto its own device, refreshes the MCP token on its stored device, and makes `maintenance` suppress automatic actions.

**Continuwuity v26.9.1 or later is required.** E2EE and the password reset below were validated on v26.9.1 only. Older agents report versions such as `v0.5.8`. Step 2 upgrades them.

**Behavior changes that affect the fleet:**

- `create_room` and `send_direct_message` default to `encryptionMode: "auto"`. A room is encrypted when every invitee has cross-signed E2EE keys. Other agents on v0.2.0 qualify.
- matrix-bot v0.1.2 cannot read encrypted rooms. An agent's bot does not answer in an encrypted room. Stage 2 (`003-integrated-matrix-bot`) closes this gap.
  - For rooms where another agent's bot must answer, pass `encryptionMode: "none"`.
  - Existing unencrypted rooms, such as Fleet HQ, stay unencrypted.
- Room keys go only to cross-signed devices. An Element session that the user has not verified gets `m.room_key.withheld` and cannot read the agent's messages. Users must verify each Element session.
- Two new tools: `bootstrap_e2ee` and `get_encryption_status`. The total is 22 tools.
- `GET /health` adds `"e2ee"` and `"crossSigned"`.
- r2 binds HTTP only after the initial sync. `/health` answered in 2 seconds on agent0-4. Allow up to 90 seconds on a busy account.

## Before you start

Run these on the Docker host. `N` is the agent number.

1. **Install Agent-Matrix v0.92 on the host.** The host tree at `/opt/agent-zero/multi-instance-deploy` is a copy, not a git checkout. Copy these files from agent-matrix `v0.92` (`release/v0.92` until it is tagged):
   - `templates/scripts/split-bot-device.sh`
   - `templates/scripts/fleet-token-healthcheck.sh`
   - `templates/scripts/deploy-mms-r2-to-agent.sh`
   - `scripts/finalize-instance.sh`
   - `am-ctl/amctl/services/health_engine.py`

   Then restart am-ctl: `sudo systemctl restart am-ctl`.
2. **Check the Continuwuity version.**

   ```bash
   docker exec agent0-N-continuwuity conduwuit --version
   # continuwuity 26.9.1 (046074b)
   ```

   If it is older than 26.9.1, upgrade it in step 2.
3. **Check that the account has no cross-signing identity yet.** This matters if a human ever signed in as the agent in Element and set up cross-signing. In that case, r2 returns `IdentityExistsError` until you give it that identity's recovery key in `MATRIX_RECOVERY_KEY_FILE`.

   ```bash
   docker exec -i agent0-N python3 - <<'PY'
   import json, urllib.request as u
   env = dict(l.strip().split("=", 1) for l in open("/a0/usr/workdir/matrix-mcp-server/.env") if "=" in l and not l.startswith("#"))
   hs, tok, uid = env["MATRIX_HOMESERVER_URL"].rstrip("/"), env["MATRIX_ACCESS_TOKEN"], env["MATRIX_USER_ID"]
   r = u.Request(hs + "/_matrix/client/v3/keys/query", data=json.dumps({"device_keys": {uid: []}}).encode())
   r.add_header("Authorization", "Bearer " + tok); r.add_header("Content-Type", "application/json")
   print("cross-signing identity exists:", uid in json.load(u.urlopen(r)).get("master_keys", {}))
   PY
   # expect: cross-signing identity exists: False
   ```

4. **Plan a backup location for the recovery key.** Keep it off the Docker host, for example in a password manager.

## Upgrade one agent

Do one agent at a time. Finish all checks before the next agent. The password check and the Continuwuity upgrade restart the homeserver, so they come after step 1.

### 1. Pause automation

```bash
curl -s -X PUT -H "X-API-Key: $AMCTL_API_KEY" -H "Content-Type: application/json" \
  -d '{"status":"maintenance"}' http://127.0.0.1:8077/api/v1/agents/agent0-N
```

With am-ctl v0.92, the agent's health checks are still recorded, but restart and stop actions are skipped. The audit log shows `auto_action_suppressed`.

### 2. Upgrade Continuwuity (only if older than 26.9.1)

```bash
cd /opt/agent-zero/agent0-N
docker pull ghcr.io/continuwuity/continuwuity:latest
docker stop agent0-N-continuwuity
cp -a mhs/continuwuity-data mhs/continuwuity-data.bak-$(date +%Y%m%d-%H%M%S)
docker compose up -d continuwuity
docker exec agent0-N-continuwuity conduwuit --version
```

The data copy is the rollback. It can be larger than the live directory after the upgrade, because the new version compacts the database. Then confirm the MCP token still works and keeps its device:

```bash
docker exec -i agent0-N python3 - <<'PY'
import json, urllib.request as u
env = dict(l.strip().split("=", 1) for l in open("/a0/usr/workdir/matrix-mcp-server/.env") if "=" in l and not l.startswith("#"))
r = u.Request(env["MATRIX_HOMESERVER_URL"].rstrip("/") + "/_matrix/client/v3/account/whoami")
r.add_header("Authorization", "Bearer " + env["MATRIX_ACCESS_TOKEN"])
print(json.load(u.urlopen(r)))
PY
```

### 3. Back up

```bash
D=/opt/agent-zero/agent0-N/usr/workdir
TS=$(date +%Y%m%d-%H%M%S)
cp -p $D/matrix-mcp-server/.env           $D/matrix-mcp-server/.env.bak-$TS-pre-v020
cp -p $D/matrix-bot/.env                  $D/matrix-bot/.env.bak-$TS-pre-v020
cp -p $D/matrix-mcp-server/matrix-mcp-server-r2 $D/matrix-mcp-server/matrix-mcp-server-r2.v01x
```

### 4. Check the account password

`split-bot-device.sh` logs in with `MATRIX_PASSWORD` from `/opt/agent-zero/agent0-N/.env`. Test it with a throwaway login, and log the probe device out:

```bash
grep '^MATRIX_PASSWORD=' /opt/agent-zero/agent0-N/.env | cut -d= -f2- | docker exec -i agent0-N python3 -c '
import sys, json, urllib.request as u
pw = sys.stdin.read().strip()
env = dict(l.strip().split("=", 1) for l in open("/a0/usr/workdir/matrix-mcp-server/.env") if "=" in l and not l.startswith("#"))
hs = env["MATRIX_HOMESERVER_URL"].rstrip("/") + "/_matrix/client/v3"
def post(path, data, tok=None):
    r = u.Request(hs + path, data=json.dumps(data).encode()); r.add_header("Content-Type", "application/json")
    if tok: r.add_header("Authorization", "Bearer " + tok)
    return json.load(u.urlopen(r))
user = env["MATRIX_USER_ID"].split(":")[0][1:]
lg = post("/login", {"type": "m.login.password", "identifier": {"type": "m.id.user", "user": user}, "password": pw, "device_id": "PasswordProbe"})
post("/logout", {}, lg["access_token"]); print("password OK")
'
```

An HTTP 403 error means the password is stale. Use "Reset a stale password" below. Do not use any reset that signs out devices.

### 5. Give the bot its own device

```bash
cd /opt/agent-zero/multi-instance-deploy/templates/scripts
./split-bot-device.sh --dry-run N
./split-bot-device.sh N
```

Expected: `OK: bot on AgentZeroBot, MCP on <device>`. The MCP server keeps its current device, and that device becomes the E2EE device. The script restarts `run_bot`.

### 6. Install the v0.2.0 binary

```bash
MMS_R2_TAG=0.2.0 ./deploy-mms-r2-to-agent.sh N
docker exec agent0-N /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2 --version
# matrix-mcp-server-r2 0.2.0
```

To test a release candidate, use its image tag, for example `MMS_R2_TAG=0.2.0-rc.1`. The binary still reports `0.2.0`.

The script ends with "Switch with: ... switch-mcp-server.sh rust". Ignore that message. The old binary keeps running until step 8 restarts `run_mcp` through supervisord.

The GHCR image for v0.2.0 is built with the `e2ee` feature. Builds of v0.1.x are not, and they ignore `E2EE_ENABLED`.

### 7. Turn on E2EE

Add these lines to `/opt/agent-zero/agent0-N/usr/workdir/matrix-mcp-server/.env` on the host. In the container, the path is `/a0/usr/workdir/matrix-mcp-server/.env`.

```bash
E2EE_ENABLED=true
CRYPTO_STORE_PATH=/a0/usr/workdir/matrix-mcp-server/crypto-store
```

`CRYPTO_STORE_PATH` must be on the host bind mount (`/a0/usr/workdir`). Then the store survives a container recreate. r2 creates the directory with mode 0700.

Optional settings:

- `MATRIX_RECOVERY_KEY_FILE` -- defaults to `$CRYPTO_STORE_PATH/recovery-key`. Set it only to restore an existing identity.
- `MATRIX_UIAA_PASSWORD_FILE` -- a mode 0600 file with the account password. r2 uses it only if the homeserver asks for UIAA when it uploads the cross-signing keys. Continuwuity v26.9.1 did not ask on agent0-4 or agent0-5. Add it only if the log shows `UiaaRequiredError`.

### 8. Restart the MCP server

```bash
docker exec agent0-N supervisorctl restart run_mcp
```

If the agent runs r2 through `switch-mcp-server.sh` instead of supervisord, run `switch-mcp-server.sh rust-restart`.

On first start r2 does these things:

1. Writes `crypto-store/.device-id` from `whoami`.
2. Opens the SQLite store and takes `.store.lock`.
3. Creates the cross-signing identity.
4. Writes `crypto-store/recovery-key` with mode 0600.

On the first start, `docker logs agent0-N` shows several `ERROR` lines with HTTP 404 status. They are for `m.secret_storage.default_key`, `m.cross_signing.*`, `m.megolm_backup.v1`, and `room_keys/version`. They are expected, because r2 checks for these items before it creates them. Success ends with `Recovery state changed from Disabled to Enabled`.

### 9. Verify

```bash
docker exec agent0-N curl -s http://127.0.0.1:3000/health
# {"crossSigned":true,"e2ee":true,...,"status":"healthy",...,"version":"0.2.0"}

docker exec agent0-N ls -la /a0/usr/workdir/matrix-mcp-server/crypto-store
# .device-id, recovery-key (-rw-------), matrix-sdk-*.sqlite3, .store.lock
```

Check that the bot and the MCP server are on different devices. `split-bot-device.sh --dry-run N` prints `OK: already on separate devices` when they are.

### 10. Back up the recovery key

This key restores the cross-signing identity if the crypto store is lost. Without it, a lost store means a new identity, and every user must verify the agent again.

Copy it to your workstation without printing it. Run this on the workstation:

```bash
ssh <user>@<docker-host> 'docker exec agent0-N cat /a0/usr/workdir/matrix-mcp-server/crypto-store/recovery-key' > ~/agent0-N-recovery-key
chmod 600 ~/agent0-N-recovery-key
wc -c ~/agent0-N-recovery-key    # expect about 60 bytes
```

Put the contents in your secret store, then delete the local copy: `shred -u ~/agent0-N-recovery-key`. Never paste the key into chat or a Matrix room.

### 11. Resume automation and test

Make sure the watchdog runs. It is started at container boot, but it can die without notice:

```bash
docker exec agent0-N ps -eo pid,etime,args | grep '[w]atchdog.sh'
# If there is no output, start it the same way startup-services.sh does:
docker exec -d agent0-N bash -c "nohup /a0/usr/workdir/watchdog.sh >> /a0/usr/workdir/startup-services.log 2>&1 &"
```

```bash
curl -s -X PUT -H "X-API-Key: $AMCTL_API_KEY" -H "Content-Type: application/json" \
  -d '{"status":"active"}' http://127.0.0.1:8077/api/v1/agents/agent0-N
```

Record the `run_bot` and `run_mcp` pids. Wait at least two watchdog cycles (about 5 minutes). Then do these tests:

1. **Encrypted inbound.** Use `create_room` with `encryptionMode: "auto"` and invite a user with a verified Element session. The result shows `Encrypted: Yes`. In Element, accept the invite and send a message. Read it with `get_room_messages`. It must show plain text, not `[Encrypted message - unable to decrypt (missing_key)]`.
2. **Encrypted outbound.** Use `send_message` in that room. The result shows `Encrypted: Yes`, and Element shows the text.
3. **Bot.** In an unencrypted fleet room, send `@agent0-N: ping`. The bot answers.
4. **Stability.** `supervisorctl status` shows the same `run_bot` and `run_mcp` pids as before the wait.

If `get_room_messages` says "No messages found", the message has not reached the agent's homeserver. Check that Element shows it as sent. If Element shows `missing_key` for new messages, the Element session may be reusing an old Megolm session. Run `/discardsession` in the room and send again.

## Operating rules after the upgrade

- **Never re-login the MCP token on a new device.** A manual token refresh must pass the stored device id: `"device_id": "<contents of crypto-store/.device-id>"`. The v0.92 `fleet-token-healthcheck.sh` does this.
- **Never share one token between `.env` files.** The bot uses `AgentZeroBot`. The MCP server uses the device in `.device-id`.
- **Never run two processes on one crypto store.** r2 holds an exclusive lock and returns `KeyStoreUnavailable` to a second process.
- **Do not delete `crypto-store/`** to fix a problem. Deleting it creates a new device, and every user must verify the agent again. If you must recreate it, keep `recovery-key` and set `MATRIX_RECOVERY_KEY_FILE` to it.
- **Do not reset the password with "log out all devices".** That deletes the MCP device and its keys. Continuwuity's `users reset-password` without `--logout` is safe. See below.

## Reset a stale password

Use this in step 4 when the password check fails and nobody knows the password. The agent must be in `maintenance` (step 1), because the reset recreates Continuwuity twice. Requires Continuwuity v26.9.1 (step 2).

Confirmed against the Continuwuity v26.9.1 source: `users reset-password` removes devices only with `--logout`. `users issue-token` needs the current password, so it does not help.

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

Never pass `--logout` or `-l`. Then run the step 2 `whoami` check, and run the step 4 password check again.

## Roll back

Rollback returns the agent to unencrypted r2 v0.1.x. Encrypted rooms that the agent created stay encrypted, and the agent cannot read them after rollback.

1. Set the agent to `maintenance`.
2. Restore the binary: `cp -p matrix-mcp-server-r2.v01x matrix-mcp-server-r2`.
3. Set `E2EE_ENABLED=false` in the MCP `.env`, or restore `.env.bak-*-pre-v020`.
4. Leave `crypto-store/` in place, so a later re-upgrade keeps the same identity.
5. `supervisorctl restart run_mcp`, then set the agent to `active`.

Keep the bot on its own device. That split is safe with v0.1.x and needs no rollback. A Continuwuity upgrade can be rolled back by restoring the `continuwuity-data.bak-*` copy and the previous image.

## Fleet rollout checklist

| Step | Done |
|------|------|
| Agent-Matrix v0.92 files on the host, am-ctl restarted | |
| Per agent: maintenance, Continuwuity 26.9.1 or later | |
| Per agent: backup, password checked or reset safely | |
| Per agent: split, install, enable, restart, verify | |
| Per agent: recovery key copied off the host | |
| Per agent: watchdog running, active, two watchdog cycles, four tests passed | |
| Fleet rooms that need bot replies use `encryptionMode: "none"` | |
