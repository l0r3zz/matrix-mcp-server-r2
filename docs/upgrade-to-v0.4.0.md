# Upgrade guide: matrix-mcp-server-r2 to v0.4.0

This guide moves an agent that already runs matrix-mcp-server-r2 v0.3.x to v0.4.0. The previous release is v0.3.1. v0.4.0 keeps the same Matrix device and the same crypto store. It adds `list_invitations`, startup and retry handling for trusted invites, and one image on a triggered prompt.

## Which guide

| You are on | Do this first |
|------------|----------------|
| v0.1.x on Agent-Matrix v0.91 | [`docs/upgrade-to-v0.2.0.md`](upgrade-to-v0.2.0.md), then the Agent-Matrix v0.93 integrated-bot upgrade, then this guide |
| v0.2.x with `run_bot` still answering Matrix | The Agent-Matrix v0.93 upgrade (`docs/upgrade-to-v0.93.md` in the agent-matrix repo), then this guide |
| v0.3.0 or v0.3.1 with `BOT_ENABLED=true` and `run_bot` stopped | This guide |

## What changes

- The tool surface is 21 operational tools plus 2 E2EE tools, 23 in total. `list_invitations` is the tool added since v0.3.1. `bootstrap_e2ee` and `get_encryption_status` are the two E2EE tools.
- With `BOT_ENABLED=true`, a trusted invite that arrived while `run_mcp` was stopped is joined after startup. A join that returns HTTP 403 because the user is not invited yet is retried while the invite remains, for at most 5 attempts. A ban, a missing room, an unsupported room version, and a 403 after the invite is gone stop after one attempt.
- One image file on a triggered prompt is sent to the Agent Zero model only when `MODEL_ACCEPTS_IMAGES=true`. Any other value, including an unset variable, posts one room error and does not evaluate the prompt. Leave the variable unset unless that model accepts images.

`GET /health` still reports `version`, `e2ee`, `crossSigned`, and `botEnabled`. After this upgrade, `version` is `0.4.0`.

## Do not

- Do not run `switch-mcp-server.sh rust-restart`. A second opener of the crypto store fails with `KeyStoreUnavailableError`.
- Do not restart the agent container. Restart only `run_mcp`.
- Do not start `run_bot`. v0.93 leaves that program stopped while the integrated bot is on.
- Do not change `CRYPTO_STORE_PATH`, the recovery key, or the Matrix access token as part of this upgrade.

## Upgrade one agent

Do one agent at a time. `N` is the instance number. Run the commands on the Docker host.

### 1. Pause automation

```bash
curl -s -X PUT -H "X-API-Key: $AMCTL_API_KEY" -H "Content-Type: application/json" \
  -d '{"status":"maintenance"}' http://127.0.0.1:8077/api/v1/agents/agent0-N
```

Record the device id before the swap:

```bash
docker exec agent0-N cat /a0/usr/workdir/matrix-mcp-server/crypto-store/.device-id
```

### 2. Install the v0.4.0 binary

The published image is `ghcr.io/l0r3zz/matrix-mcp-server-r2:0.4.0`. The binary in that image is `/usr/local/bin/matrix-mcp-server-r2`. Its sha256 is `4e3c5bc89ddacd72b782d9230f6bedf18a352b8774640f74e18e9237ac899dc5`.

```bash
docker pull ghcr.io/l0r3zz/matrix-mcp-server-r2:0.4.0
cid=$(docker create ghcr.io/l0r3zz/matrix-mcp-server-r2:0.4.0)
docker cp "$cid":/usr/local/bin/matrix-mcp-server-r2 /tmp/matrix-mcp-server-r2-040
docker rm "$cid"
chmod 755 /tmp/matrix-mcp-server-r2-040
sha256sum /tmp/matrix-mcp-server-r2-040
/tmp/matrix-mcp-server-r2-040 --version
```

`--version` must print `matrix-mcp-server-r2 0.4.0`. The checksum must match the hash above.

Keep the previous binary. Do not overwrite an older backup such as `bak-pre-004`.

```bash
docker exec agent0-N cp -a \
  /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2 \
  /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2.bak-pre-040
docker exec agent0-N supervisorctl stop run_mcp
docker cp /tmp/matrix-mcp-server-r2-040 \
  agent0-N:/a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2
docker exec agent0-N chmod 755 /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2
docker exec agent0-N /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2 --version
docker exec agent0-N supervisorctl start run_mcp
```

`deploy-mms-r2-to-agent.sh` still defaults to tag `0.3.1`. If you use it, set `MMS_R2_TAG=0.4.0`, then stop and start `run_mcp` yourself. Do not follow that script's hint to run `switch-mcp-server.sh rust`.

### 3. Check health

`/health` can take about 30 seconds.

```bash
docker exec agent0-N curl -sf http://127.0.0.1:3000/health
docker exec agent0-N cat /a0/usr/workdir/matrix-mcp-server/crypto-store/.device-id
docker exec agent0-N supervisorctl status run_mcp run_bot
```

Pass when all of these are true:

- `version` is `0.4.0`
- `status` is `healthy`
- `botEnabled`, `e2ee`, and `crossSigned` are true
- the device id is the same string you recorded in step 1
- `run_mcp` is `RUNNING`
- `run_bot` is still `EXITED`

`supervisorctl status` returns exit code 3 when `run_bot` is `EXITED`. That is the expected state.

### 4. Resume automation

```bash
curl -s -X PUT -H "X-API-Key: $AMCTL_API_KEY" -H "Content-Type: application/json" \
  -d '{"status":"active"}' http://127.0.0.1:8077/api/v1/agents/agent0-N
```

Confirm `run_bot` is still stopped after the status change. The v0.93 health engine does not start `run_bot` while `botEnabled` is true.

## Rollback

Stop `run_mcp`, copy `matrix-mcp-server-r2.bak-pre-040` back to `matrix-mcp-server-r2`, and start `run_mcp`. The device id must still match. `/health` then reports `0.3.1` for a backup taken from that release.
