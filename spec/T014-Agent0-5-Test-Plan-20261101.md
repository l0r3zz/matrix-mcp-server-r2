# T014 test plan -- Agent0-5 on g2s

Date: 2026-11-01

Manual gate for feature `002-e2ee-cross-signing`. One agent, Agent0-5. The human joins from Element on `v-site.net`. The other agents stay out of the encrypted room.

This plan installs a binary. Agent0-5 already runs `matrix-mcp-server-r2` as a process inside the `agent0-5` container. Continuwuity and Caddy stay the containers they are. The published GHCR image and the stock `Dockerfile` run `cargo build --release` without `--features e2ee`. Deploying that image leaves `/health` at `"e2ee": false` and the E2EE tools return `E2EE support not enabled; rebuild with --features e2ee`. Build from the current working tree.

Do not print `.env`, the access token, the UIAA password, or the recovery key. Do not invite Agent0-1 through Agent0-4 into the encrypted room. Do not turn encryption on in Fleet HQ.

## What "pass" looks like

The bot staying quiet in the encrypted room is expected. Pass and fail come from `/health`, the MCP tool text, and Element.

| Check | Pass |
| --- | --- |
| 1. Encrypted room | `create-room` text contains `Encrypted: Yes`. `send-message` text contains `Encrypted: Yes` and Element shows the plaintext. A message you send in Element comes back as plaintext from `get-room-messages`. |
| 2. Restart | After `docker restart agent0-5`, the same `get-room-messages` call still returns that plaintext. |
| 3. Unverified device | A second Element session that is not cross-signed cannot read the message the agent sent. |
| 4. Auto fallback | `create-room` with an invitee that has no cross-signed keys returns `Encrypted: No (fallback: ...)` and does not create an encrypted room. |
| 5. Encrypted DM | `send-direct-message` with `encryptionMode: "auto"` to your cross-signed Element user returns `Encrypted: Yes`, and the DM round-trips in Element. |
| 6. Cold bootstrap | On an account with no cross-signing, a fresh store reaches `crossSigned: true` and writes a mode-`0600` `recovery-key`. Record the elapsed seconds. Target is under 60. |

A decrypt failure looks like `[Encrypted message - unable to decrypt (missing_key)]` or a withheld-key marker. `keys query failed` means the room was not created.

## 0. Before you build

On Element, on your `v-site.net` account, finish cross-signing for the session you will use (secure backup / verify this session). Copy that full MXID from Element. The examples below use `@you:v-site.net`. Replace it.

In Element settings, allow sending encrypted messages to unverified sessions for this test. Agent0-5 will publish a new device that you have not emoji-verified, and this server does not answer an interactive SAS prompt. With Element's "never send to unverified sessions" left on, your messages arrive as withheld keys and check 1 fails for that reason. The server still shares keys only with cross-signed devices. Leave that server policy alone.

Confirm Agent0-5 is the container you intend to touch:

```bash
ssh l0r3zz@172.23.100.121
docker ps --format '{{.Names}}' | grep agent0-5
```

Expected names: `agent0-5`, `agent0-5-continuwuity`, `agent0-5-mhs`. Leave the last two running for the whole test.

## 1. Build a bookworm binary with E2EE

On the machine that has this repo (not inside Agent0-5):

```bash
cd /home/l0r3zz/Dropbox/GEN-AI/AGENT-MATRIX/matrix-mcp-server-r2-repo
docker run --rm -v "$PWD":/build -w /build rust:1.99-bookworm \
  bash -c 'cargo build --release --features e2ee'
```

The binary is `target/release/matrix-mcp-server-r2`, linked against bookworm glibc, which is what the agent container already runs. Copy it to g2s and then into the container:

```bash
scp target/release/matrix-mcp-server-r2 l0r3zz@172.23.100.121:/tmp/matrix-mcp-server-r2
ssh l0r3zz@172.23.100.121
docker cp /tmp/matrix-mcp-server-r2 \
  agent0-5:/a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2
docker exec agent0-5 chmod 0755 /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2
docker exec agent0-5 /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2 --version
```

`--version` only proves the file runs. The E2EE proof is `/health` after the restart in section 3.

## 2. Point this instance at a crypto store

The existing `.env` already has `MATRIX_HOMESERVER_URL`, `MATRIX_USER_ID`, and `MATRIX_ACCESS_TOKEN`. Keep those. Read the user id without dumping the token:

```bash
docker exec agent0-5 grep -E '^(MATRIX_USER_ID|MATRIX_HOMESERVER_URL|PORT)=' \
  /a0/usr/workdir/matrix-mcp-server/.env
```

Use that `MATRIX_USER_ID` everywhere below. Docs disagree on `lab.v-site.net` versus `cybertribe.com`. The value in this file is the one Continuwuity knows.

Create the store directory on the bind mount so it survives a container restart:

```bash
docker exec agent0-5 mkdir -p /a0/usr/workdir/matrix-mcp-server/crypto-store
docker exec agent0-5 chmod 0700 /a0/usr/workdir/matrix-mcp-server/crypto-store
```

Append the two E2EE lines once. Skip the append if they are already present.

```bash
docker exec agent0-5 grep -q '^E2EE_ENABLED=' /a0/usr/workdir/matrix-mcp-server/.env \
  || docker exec agent0-5 sh -c 'printf "\nE2EE_ENABLED=true\nCRYPTO_STORE_PATH=/a0/usr/workdir/matrix-mcp-server/crypto-store\n" >> /a0/usr/workdir/matrix-mcp-server/.env'
```

Leave `MATRIX_UIAA_PASSWORD_FILE` unset for the first start. If bootstrap returns `UiaaRequiredError`, section 4 adds the password file and you restart once.

## 3. Restart the MCP process, not the agent

Do not `docker restart agent0-5` yet. That restart is check 2, after a message exists to decrypt. Continuwuity and Caddy stay up.

`supervisord` owns `run_mcp`. Its wrapper sources `.env` and execs the binary. Restart that program so the new file and the new variables are what is running:

```bash
docker exec agent0-5 supervisorctl status run_mcp
docker exec agent0-5 supervisorctl restart run_mcp
```

Do not also run `switch-mcp-server.sh rust-restart` while `run_mcp` is under supervisord. That script kills the process and starts a second one. The second open of the same store exits with `KeyStoreUnavailableError`.

The process syncs, then bootstraps, then binds port 3000. The switch script allows about 90 seconds. Poll from inside the container:

```bash
docker exec agent0-5 curl -sf http://127.0.0.1:3000/health
```

Pass for this step:

```json
{"status":"healthy","server":"matrix-mcp-server-r2","e2ee":true,"crossSigned":true}
```

`e2ee: false` means this binary was built without the feature, or `E2EE_ENABLED` was not read. `crossSigned: false` with `e2ee: true` means bootstrap did not finish a cross-signed device. Read the failure from the log without assuming it is the token:

```bash
docker exec agent0-5 supervisorctl tail -80 run_mcp
```

`UiaaRequiredError` is the branch in section 4. `IdentityExistsError` means this account already has cross-signing and the new store has no recovery secret. Stop there and record it. Do not delete the server-side identity to make the test pass.

Record the seconds from `supervisorctl restart` until `/health` shows `crossSigned: true`. That interval is check 6. It includes one initial sync (30 second timeout) plus bootstrap. The recovery key file should appear in the same window:

```bash
docker exec agent0-5 stat -c '%a %n' \
  /a0/usr/workdir/matrix-mcp-server/crypto-store/recovery-key
```

Pass is mode `600`. Do not `cat` the file.

## 4. Only if bootstrap asks for UIAA

Continuwuity may require the account password for `/keys/device_signing/upload`. The server reads it from a file. It does not read a password out of the environment.

On g2s, write the password into a mode-`0600` file inside the container. Do not echo it, and do not put it in `.env`:

```bash
docker exec -i agent0-5 sh -c 'umask 077; cat > /a0/usr/workdir/matrix-mcp-server/uiaa-password'
# paste the account password, then Ctrl-D
docker exec agent0-5 chmod 0600 /a0/usr/workdir/matrix-mcp-server/uiaa-password
docker exec agent0-5 sh -c 'grep -q "^MATRIX_UIAA_PASSWORD_FILE=" /a0/usr/workdir/matrix-mcp-server/.env || printf "MATRIX_UIAA_PASSWORD_FILE=/a0/usr/workdir/matrix-mcp-server/uiaa-password\n" >> /a0/usr/workdir/matrix-mcp-server/.env'
docker exec agent0-5 supervisorctl restart run_mcp
```

Wait for `/health` again. A wrong mode (`0644`) fails config before listen, with `MATRIX_UIAA_PASSWORD_FILE must be mode 0600`. No identity is uploaded and no recovery key is written on a rejected UIAA.

## 5. How the curl calls work

Port 3000 is not published on the host. Run curl inside `agent0-5`. `/health` is plain HTTP. Tool calls are MCP on `POST /mcp`.

One `initialize` opens a session. Later calls send the `Mcp-Session-Id` from that response. The `Accept` header must list both JSON and the event stream, or the server returns 406. The body may be a single JSON object or one SSE `data:` line. Tool errors come back as MCP text that starts with `Error:`. They are not HTTP 500s.

Initialize:

```bash
docker exec agent0-5 sh -c 'curl -sS -D - \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "MCP-Protocol-Version: 2025-06-18" \
  -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"t014\",\"version\":\"0\"}}}" \
  http://127.0.0.1:3000/mcp'
```

Copy the `mcp-session-id` header value into `SID` for the rest of the session. Then tell the server the client is ready:

```bash
docker exec agent0-5 sh -c "curl -sS -D - \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -H 'MCP-Protocol-Version: 2025-06-18' \
  -H 'Mcp-Session-Id: $SID' \
  -d '{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}' \
  http://127.0.0.1:3000/mcp"
```

Helper used by every tool call below. Set `SID` in the shell you pass to `docker exec`, or expand it before the call.

```bash
mcp() {
  docker exec agent0-5 curl -sS \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json, text/event-stream' \
    -H 'MCP-Protocol-Version: 2025-06-18' \
    -H "Mcp-Session-Id: ${SID}" \
    -d "$1" \
    http://127.0.0.1:3000/mcp
  echo
}
```

`SID` has to be visible inside `mcp`. Easiest path: `export SID=...` on g2s and change the function to pass it in the `docker exec` environment:

```bash
export SID='paste-session-id'
mcp() {
  docker exec -e SID="$SID" agent0-5 curl -sS \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json, text/event-stream' \
    -H 'MCP-Protocol-Version: 2025-06-18' \
    -H "Mcp-Session-Id: ${SID}" \
    -d "$1" \
    http://127.0.0.1:3000/mcp
  echo
}
```

If a call returns `session not found`, run `initialize` again and replace `SID`. Sessions live in the process. A restart clears them.

## 6. Check 6 and check 4, before any encrypted room

Status, no room id. Pass includes `"crossSigned": true` and a `cryptoStoreState` of `ready`. The recovery key string must not appear.

```bash
mcp '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get-encryption-status","arguments":{}}}'
```

Fallback room. Invite one other agent, using the MXID pattern from Agent0-5's own user id. That agent has no cross-signed device, so auto mode must refuse encryption. Pass text contains `Encrypted: No (fallback:` and names that user. Save the room id only to confirm you do not use it for the encrypted test.

```bash
mcp '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create-room","arguments":{"roomName":"T014 fallback","isPrivate":true,"encryptionMode":"auto","inviteUsers":["@agent0-1:agent0-1-mhs.example"]}}}'
```

Replace the invitee with the real sibling MXID. If this returns `Encrypted: Yes`, stop. The preflight did not see a missing identity.

## 7. Check 1 -- encrypted room with you

Create the room with only your Element MXID. `isPrivate: true` keeps it out of the public directory. Omit `encryptionMode` or set `auto`. Both encrypt when every invitee has cross-signed keys.

```bash
mcp '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"create-room","arguments":{"roomName":"T014 encrypted","isPrivate":true,"encryptionMode":"auto","inviteUsers":["@you:v-site.net"]}}}'
```

Pass: the text contains `Encrypted: Yes` and a `Room ID: !...` line. Copy that room id into `ROOM`.

In Element, accept the invite. The room shows as encrypted. Send a short plaintext note, for example `hello from element`.

Read it back:

```bash
mcp '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"get-room-messages","arguments":{"roomId":"'"$ROOM"'","limit":20}}}'
```

Pass: the tool text contains `hello from element`. Fail: `unable to decrypt` or a withheld marker. The usual cause is Element still refusing unverified devices, or the agent device not being cross-signed yet (`/health` `crossSigned` is false).

Send the other direction:

```bash
mcp '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"send-message","arguments":{"roomId":"'"$ROOM"'","message":"hello from agent0-5"}}}'
```

Pass: the tool text contains `Encrypted: Yes`, and Element shows `hello from agent0-5` as plaintext.

The matrix-bot will not reply in this room. That is not a failure.

## 8. Check 2 -- restart the agent container

Now restart the agent container. This restarts Agent Zero, the bot, and the MCP server. Continuwuity (`agent0-5-continuwuity`) and Caddy (`agent0-5-mhs`) keep running. The store is on `./usr`, so the device id and the recovery key stay.

```bash
docker restart agent0-5
```

Wait until supervisord has brought `run_mcp` back. Sync runs again before port 3000 binds.

```bash
docker exec agent0-5 curl -sf http://127.0.0.1:3000/health
```

`crossSigned` stays true. The session id from section 5 is dead. `initialize` again, then:

```bash
mcp '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"get-room-messages","arguments":{"roomId":"'"$ROOM"'","limit":20}}}'
```

Pass: both `hello from element` and `hello from agent0-5` are still plaintext.

## 9. Check 3 -- a device the agent must not key

On a second Element session for a different user, or a new login that you do not cross-sign, join only if you can do it without adding that user as a cross-signed invitee at create time. Practical version: log the same account in on a second client and do not verify that session.

From the verified Element session you already used, that is enough for Element. From the agent side, send one more line:

```bash
mcp '{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"send-message","arguments":{"roomId":"'"$ROOM"'","message":"withheld check"}}}'
```

Pass: the verified Element session shows `withheld check`. The unverified session does not. It shows an unable-to-decrypt placeholder. Record which session, and that the server trust policy was left on `cross-signed`.

## 10. Check 5 -- encrypted DM

```bash
mcp '{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"send-direct-message","arguments":{"targetUserId":"@you:v-site.net","message":"dm from agent0-5","encryptionMode":"auto"}}}'
```

Pass: `Encrypted: Yes` and `New DM room created` (or `Used existing DM room` if one already exists, in which case confirm that existing room is encrypted before treating this as the DM gate). Element shows the plaintext. Reply in Element, take the `Room:` id from the tool text, and `get-room-messages` on it. Pass: your reply is plaintext.

If auto mode returns `Encrypted: No (fallback: ...)`, your Element user had no cross-signed keys at preflight time. Fix cross-signing and create a new DM. Do not flip the server to trust unverified devices.

## 11. Write the result

Record pass or fail for the six checks in a dated file next to this plan, `spec/T014-Agent0-5-Result-YYYYMMDD.md`. Include the `/health` body, the elapsed seconds for check 6, the recovery-key mode, and the tool lines that contain `Encrypted:` or the decrypt marker. Omit tokens, the recovery key, and the UIAA password.

Leave T014 unchecked until that file matches a run you actually did.

## Rollback

On Agent0-5 only:

```bash
docker exec agent0-5 sh -c 'sed -i "/^E2EE_ENABLED=/d;/^CRYPTO_STORE_PATH=/d;/^MATRIX_UIAA_PASSWORD_FILE=/d" /a0/usr/workdir/matrix-mcp-server/.env'
docker exec agent0-5 supervisorctl restart run_mcp
```

That stops crypto on the next start. It does not delete the store or the server-side cross-signing identity. To return to the previous binary, copy the previous `matrix-mcp-server-r2` back into `/a0/usr/workdir/matrix-mcp-server/` and restart `run_mcp` the same way. Fleet HQ and the other agents are untouched if they were never invited.
