---
clear-rules: on
clear-mode: standard
source: spec/T014-Agent0-5-Test-Plan-20261101.md
---

# T014 test plan -- Agent0-5 on g2s

Date: 2026-11-02

This file is the CLEAR rewrite of `spec/T014-Agent0-5-Test-Plan-20261101.md`. The checks and the commands are the same.

This plan is the manual gate for feature `002-e2ee-cross-signing`. A manual gate is a test a person runs before the feature is accepted. You test one agent, Agent0-5. You join from Element on `v-site.net`. The other agents stay out of the encrypted room.

This plan installs a binary. Agent0-5 already runs `matrix-mcp-server-r2` inside the `agent0-5` container. Continuwuity stays in its own container. Caddy stays in its own container. GHCR is the GitHub Container Registry. The published GHCR image runs `cargo build --release` without `--features e2ee`. The Dockerfile in this repo does the same. If you deploy that image, `/health` stays `"e2ee": false`. The E2EE tools then return `E2EE support not enabled; rebuild with --features e2ee`. Build from the current files in this repo.

Warning: Do not print `.env`, the access token, the UIAA password, or the recovery key. Do not invite Agent0-1 through Agent0-4 into the encrypted room. Do not turn encryption on in Fleet HQ.

## Words in this plan

E2EE means end-to-end encryption. Only the sender and the invited readers can read the message.

Cross-signed means this account has signed the device. Other devices can then trust it.

An MXID is the full Matrix user id. An example is `@you:v-site.net`.

The crypto store is the local folder that holds this device's encryption keys.

UIAA is the homeserver password check during key upload.

The recovery key is the secret that can restore the crypto store. Do not print it.

SAS is the emoji check two devices show each other.

A withheld key means the sender did not share the message key with that device.

Sync means the client downloads the current account state.

Bootstrap means the client creates or loads its cross-signing keys.

supervisord is the process manager inside the container. It runs the program `run_mcp`.

Preflight is the key check that runs before a room is created.

## What "pass" looks like

The bot stays quiet in the encrypted room. That quiet is expected. Pass and fail come from `/health`, the MCP tool text, and Element.

| Check | Pass |
| --- | --- |
| 1. Encrypted room | `create_room` text contains `Encrypted: Yes`. `send_message` text contains `Encrypted: Yes`. Element shows the plaintext. A message you send in Element comes back as plaintext from `get_room_messages`. |
| 2. Restart | After `docker restart agent0-5`, the same `get_room_messages` call still returns that plaintext. |
| 3. Unverified device | A second Element session that is not cross-signed cannot read the message the agent sent. |
| 4. Auto fallback | `create_room` invites a user who has no cross-signed keys. The text returns `Encrypted: No (fallback: ...)`. That call does not create an encrypted room. |
| 5. Encrypted DM | `send_direct_message` uses `encryptionMode: "auto"`. The target is your cross-signed Element user. The text returns `Encrypted: Yes`. Element shows the message. Your reply comes back as plaintext. |
| 6. Cold bootstrap | The account has no cross-signing. A fresh crypto store reaches `crossSigned: true`. The crypto store writes a mode-`0600` `recovery-key`. Record the elapsed seconds. The target is under 60. |

A decrypt failure looks like `[Encrypted message - unable to decrypt (missing_key)]`. It can also look like a withheld-key marker. `keys query failed` means the room was not created.

## 0. Before you build

1. Open Element on your `v-site.net` account.
2. Finish cross-signing for the session you will use.
3. Use secure backup, or verify this session.
4. Copy that full MXID from Element.
5. Use `@you:v-site.net` in the examples below. Replace it with your MXID.

Warning: In Element settings, allow encrypted messages to unverified sessions for this test. Agent0-5 will publish a new device. You have not compared the SAS emoji with that device. This server does not answer an interactive SAS prompt. If "never send to unverified sessions" stays on, your messages arrive as withheld keys. Check 1 then fails for that reason. The server still shares keys only with cross-signed devices. Leave that server policy alone.

6. Confirm `agent0-5` is the container you will change.

```bash
ssh l0r3zz@172.23.100.121
docker ps --format '{{.Names}}' | grep agent0-5
```

Expected names: `agent0-5`, `agent0-5-continuwuity`, `agent0-5-mhs`. Leave the last two running for the whole test.

## 1. Build a bookworm binary with E2EE

Bookworm is the Debian version this container uses.

1. Run this build on the machine that has this repo.
2. Do not run the build inside Agent0-5.

```bash
cd /home/l0r3zz/Dropbox/GEN-AI/AGENT-MATRIX/matrix-mcp-server-r2-repo
docker run --rm -v "$PWD":/build -w /build rust:1.99-bookworm \
  bash -c 'cargo build --release --features e2ee'
```

The binary is `target/release/matrix-mcp-server-r2`. It links against bookworm glibc. glibc is the C library the agent container already runs.

3. Copy the binary to g2s.
4. Copy the binary into the container.

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

The existing `.env` already has `MATRIX_HOMESERVER_URL`, `MATRIX_USER_ID`, and `MATRIX_ACCESS_TOKEN`. Keep those values.

1. Read the user id.
2. Do not print the token.

```bash
docker exec agent0-5 grep -E '^(MATRIX_USER_ID|MATRIX_HOMESERVER_URL|PORT)=' \
  /a0/usr/workdir/matrix-mcp-server/.env
```

Use that `MATRIX_USER_ID` everywhere below. Docs disagree on `lab.v-site.net` versus `cybertribe.com`. The value in this file is the one Continuwuity knows.

A bind mount is a host folder the container can see. Create the crypto store on that mount. The directory then survives a container restart.

```bash
docker exec agent0-5 mkdir -p /a0/usr/workdir/matrix-mcp-server/crypto-store
docker exec agent0-5 chmod 0700 /a0/usr/workdir/matrix-mcp-server/crypto-store
```

3. Append the two E2EE lines once.
4. Skip the append if those lines are already present.

```bash
docker exec agent0-5 grep -q '^E2EE_ENABLED=' /a0/usr/workdir/matrix-mcp-server/.env \
  || docker exec agent0-5 sh -c 'printf "\nE2EE_ENABLED=true\nCRYPTO_STORE_PATH=/a0/usr/workdir/matrix-mcp-server/crypto-store\n" >> /a0/usr/workdir/matrix-mcp-server/.env'
```

Leave `MATRIX_UIAA_PASSWORD_FILE` unset for the first start. If bootstrap returns `UiaaRequiredError`, section 4 adds the password file. You then restart once.

## 3. Restart the MCP process, not the agent

Warning: Do not run `docker restart agent0-5` yet. That restart is check 2. Run it only after a message exists to decrypt. Continuwuity stays up. Caddy stays up.

The `run_mcp` wrapper reads `.env`. The wrapper then starts the binary. Restart that program. The new file and the new variables are then what is running.

```bash
docker exec agent0-5 supervisorctl status run_mcp
docker exec agent0-5 supervisorctl restart run_mcp
```

Warning: Do not also run `switch-mcp-server.sh rust-restart` while supervisord owns `run_mcp`. That script stops the MCP process. That script then starts a second MCP process. The second open of the same crypto store exits with `KeyStoreUnavailableError`.

The MCP process syncs. It then bootstraps. It then binds port 3000. The switch script allows about 90 seconds.

1. Poll `/health` from inside the container.

```bash
docker exec agent0-5 curl -sf http://127.0.0.1:3000/health
```

Pass for this step:

```json
{"status":"healthy","server":"matrix-mcp-server-r2","e2ee":true,"crossSigned":true}
```

`e2ee: false` means this binary was built without the feature. It can also mean the process did not read `E2EE_ENABLED`. `crossSigned: false` with `e2ee: true` means bootstrap did not finish. The device is not cross-signed yet.

2. Read the failure from the log.
3. Do not assume the failure is the token.

```bash
docker exec agent0-5 supervisorctl tail -80 run_mcp
```

Section 4 handles `UiaaRequiredError`. `IdentityExistsError` means this account already has cross-signing. The new crypto store has no recovery key. Stop there and record that error. Do not delete the server-side identity to make the test pass.

4. Record the seconds from `supervisorctl restart` until `/health` shows `crossSigned: true`.

That interval is check 6. The interval includes one initial sync. The sync timeout is 30 seconds. The interval also includes bootstrap.

The recovery key file should appear in that same period.

```bash
docker exec agent0-5 stat -c '%a %n' \
  /a0/usr/workdir/matrix-mcp-server/crypto-store/recovery-key
```

Pass is mode `600`. Do not `cat` the file.

## 4. Only if bootstrap asks for UIAA

Continuwuity may require the account password for `/keys/device_signing/upload`. The MCP process reads that password from a file. It does not read a password from the environment.

1. On g2s, write the password into a mode-`0600` file inside the container.
2. Do not echo the password.
3. Do not put the password in `.env`.

```bash
docker exec -i agent0-5 sh -c 'umask 077; cat > /a0/usr/workdir/matrix-mcp-server/uiaa-password'
# paste the account password, then Ctrl-D
docker exec agent0-5 chmod 0600 /a0/usr/workdir/matrix-mcp-server/uiaa-password
docker exec agent0-5 sh -c 'grep -q "^MATRIX_UIAA_PASSWORD_FILE=" /a0/usr/workdir/matrix-mcp-server/.env || printf "MATRIX_UIAA_PASSWORD_FILE=/a0/usr/workdir/matrix-mcp-server/uiaa-password\n" >> /a0/usr/workdir/matrix-mcp-server/.env'
docker exec agent0-5 supervisorctl restart run_mcp
```

4. Wait for `/health` again.

Warning: Mode `0644` fails config before the process listens. The log says `MATRIX_UIAA_PASSWORD_FILE must be mode 0600`. No identity is uploaded. No recovery key is written on a rejected UIAA.

## 5. How the curl calls work

Port 3000 is not published on the host. Run curl inside `agent0-5`. `/health` is plain HTTP. Tool calls are MCP on `POST /mcp`. MCP is the tool protocol this server speaks.

One `initialize` opens a session. Later calls send the `Mcp-Session-Id` from that response. The `Accept` header must list both JSON and the event stream. If it does not, the server returns 406. The body may be a single JSON object. The body may also be one SSE `data:` line. SSE is a stream of text events. A tool error comes back as MCP text. That text starts with `Error:`. A tool error is not an HTTP 500.

1. Initialize the session.

```bash
docker exec agent0-5 sh -c 'curl -sS -D - \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "MCP-Protocol-Version: 2025-06-18" \
  -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"t014\",\"version\":\"0\"}}}" \
  http://127.0.0.1:3000/mcp'
```

2. Copy the `mcp-session-id` header value into `SID`.
3. Use that `SID` for the rest of the session.
4. Tell the server the client is ready.

```bash
docker exec agent0-5 sh -c "curl -sS -D - \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -H 'MCP-Protocol-Version: 2025-06-18' \
  -H 'Mcp-Session-Id: $SID' \
  -d '{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}' \
  http://127.0.0.1:3000/mcp"
```

Every tool call below uses this helper. Set `SID` in the shell you pass to `docker exec`. Or expand `SID` before the call.

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

`SID` must be visible inside `mcp`.

5. Export `SID` on g2s.
6. Pass `SID` into `docker exec`.

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

If a call returns `session not found`, run `initialize` again. Then replace `SID`. Sessions live in the MCP process. A restart clears them.

## 6. Check 6 and check 4, before any encrypted room

The server registers each tool by its Rust function name. Use underscores in the tool name. A hyphenated name returns `tool not found`.

Run these two checks before you create an encrypted room.

The status call returns no room id. Pass includes `"crossSigned": true`. Pass also includes a `cryptoStoreState` of `ready`. The recovery key string must not appear.

```bash
mcp '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_encryption_status","arguments":{}}}'
```

1. Create a fallback room.
2. Invite one other agent.
3. Use the MXID pattern from Agent0-5's own user id.

That agent has no cross-signed device. Auto mode must refuse encryption. Pass text contains `Encrypted: No (fallback: ...)`. Pass text names that user. Save the room id only so you do not use it for the encrypted test.

```bash
mcp '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_room","arguments":{"roomName":"T014 fallback","isPrivate":true,"encryptionMode":"auto","inviteUsers":["@agent0-1:agent0-1-mhs.cybertribe.com"]}}}'
```

4. The invitee is `@agent0-1:agent0-1-mhs.cybertribe.com`. Do not invite `@agent0-5:agent0-5-mhs.cybertribe.com`.

Warning: If this call returns `Encrypted: Yes`, stop. Preflight did not see a missing identity.

## 7. Check 1 -- encrypted room with you

1. Create the room with only your Element MXID.
2. Set `isPrivate: true`. That setting keeps the room out of the public directory.
3. Omit `encryptionMode`, or set `auto`.
4. Both choices encrypt when every invitee has cross-signed keys.

```bash
mcp '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"create_room","arguments":{"roomName":"T014 encrypted","isPrivate":true,"encryptionMode":"auto","inviteUsers":["@you:v-site.net"]}}}'
```

Pass: the text contains `Encrypted: Yes`. Pass: the text contains a `Room ID: !...` line.

5. Copy that room id into `ROOM`.
6. In Element, accept the invite.
7. Confirm the room shows as encrypted.
8. Send a short plaintext note, for example `hello from element`.

9. Read the note back.

```bash
mcp '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"get_room_messages","arguments":{"roomId":"'"$ROOM"'","limit":20}}}'
```

Pass: the tool text contains `hello from element`. Fail: `unable to decrypt` or a withheld marker. One cause is Element still refusing unverified devices. Another cause is that the agent device is not cross-signed yet. Then `/health` shows `crossSigned` as false.

10. Send a message the other way.

```bash
mcp '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"send_message","arguments":{"roomId":"'"$ROOM"'","message":"hello from agent0-5"}}}'
```

Pass: the tool text contains `Encrypted: Yes`. Element shows `hello from agent0-5` as plaintext.

The matrix-bot will not reply in this room. That silence is not a failure.

## 8. Check 2 -- restart the agent container

This restart restarts Agent Zero, the bot, and the MCP server. Leave Continuwuity (`agent0-5-continuwuity`) running. Leave Caddy (`agent0-5-mhs`) running.

1. Restart the agent container now.

The crypto store is on `./usr`. The device id stays. The recovery key stays.

```bash
docker restart agent0-5
```

2. Wait until supervisord has started `run_mcp` again.

Sync runs again before port 3000 binds.

```bash
docker exec agent0-5 curl -sf http://127.0.0.1:3000/health
```

`crossSigned` stays true. The session id from section 5 is no longer valid.

3. Run `initialize` again.
4. Read the room again.

```bash
mcp '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"get_room_messages","arguments":{"roomId":"'"$ROOM"'","limit":20}}}'
```

Pass: both `hello from element` and `hello from agent0-5` are still plaintext.

## 9. Check 3 -- a device the agent must not key

1. Log the same account in on a second Element client.
2. Do not cross-sign that session.
3. Do not verify that session.

A different user is allowed only when you can join without adding that user at room create time. A user added at create time is an invitee. Do not make that user a cross-signed invitee.

The verified Element session is enough to judge Element's side of this check.

4. From the agent, send one more line.

```bash
mcp '{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"send_message","arguments":{"roomId":"'"$ROOM"'","message":"withheld check"}}}'
```

Pass: the verified Element session shows `withheld check`. The unverified session does not show it. The unverified session shows an unable-to-decrypt placeholder.

5. Record which session you used.
6. Record that the server trust policy stayed `cross-signed`.

## 10. Check 5 -- encrypted DM

A DM is a direct message room for two users.

```bash
mcp '{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"send_direct_message","arguments":{"targetUserId":"@you:v-site.net","message":"dm from agent0-5","encryptionMode":"auto"}}}'
```

Pass: the text contains `Encrypted: Yes`. Pass: the text contains `New DM room created`. If a DM already exists, the text says `Used existing DM room`. Confirm that existing room is encrypted before you treat this call as the DM gate. Element shows the plaintext.

1. Reply in Element.
2. Take the `Room:` id from the tool text.
3. Call `get_room_messages` on that room.

Pass: your reply is plaintext.

Warning: If auto mode returns `Encrypted: No (fallback: ...)`, your Element user had no cross-signed keys at preflight. Fix cross-signing. Create a new DM. Do not change the server so it trusts unverified devices.

## 11. Write the result

1. Record pass or fail for the six checks.
2. Put that record in a dated file next to this plan.
3. Name the file `spec/T014-Agent0-5-Result-YYYYMMDD.md`.
4. Include the `/health` body.
5. Include the elapsed seconds for check 6.
6. Include the recovery-key mode.
7. Include the tool lines that contain `Encrypted:` or the decrypt marker.
8. Omit tokens, the recovery key, and the UIAA password.

Leave T014 unchecked until that file matches a run you actually did.

## Rollback

Run this rollback on Agent0-5 only.

```bash
docker exec agent0-5 sh -c 'sed -i "/^E2EE_ENABLED=/d;/^CRYPTO_STORE_PATH=/d;/^MATRIX_UIAA_PASSWORD_FILE=/d" /a0/usr/workdir/matrix-mcp-server/.env'
docker exec agent0-5 supervisorctl restart run_mcp
```

That change stops crypto on the next start. It does not delete the crypto store. It does not delete the server-side cross-signing identity.

To return to the previous binary, copy the previous `matrix-mcp-server-r2` back. Put it in `/a0/usr/workdir/matrix-mcp-server/`. Restart `run_mcp` the same way. Fleet HQ stays unchanged if you never invited it. The other agents stay unchanged if you never invited them.
