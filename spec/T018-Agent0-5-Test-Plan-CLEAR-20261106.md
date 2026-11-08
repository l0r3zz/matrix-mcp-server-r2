---
clear-rules: on
clear-mode: sop
---

# T018 lab -- Agent0-5 with Element

Date: 2026-11-06

This procedure proves the integrated bot on agent0-5 from Element.

The checks come from `.specify/specs/003-integrated-matrix-bot/quickstart.md`. This file adds the install steps. Feature 002 already proved encrypted history on this agent. This lab does not repeat those checks.

## Words in this plan

Element is the Matrix chat app at `matrix.v-site.net`.

A Matrix user id is the full name, such as `@agent0-5:server`.

The binary is the compiled program file `matrix-mcp-server-r2`.

Bookworm is the Debian version inside the agent container.

g2s is the machine that runs the agent containers. Its address is `172.23.100.121`.

supervisord is the process manager inside the container.

`run_mcp` is the supervisord program for this binary.

`run_bot` is the supervisord program for the old bot.

The crypto store is the folder that holds this device's encryption keys.

am-ctl is the fleet control service on g2s.

Maintenance is an agent status. In that status, am-ctl does not start or stop programs.

Pass means the check succeeded.

## What you need before you start

- Element is open on `matrix.v-site.net` as `@admin:v-site.net`.
- That account is on the trusted-sender list.
- Agent0-5 already runs a cross-signed `matrix-mcp-server-r2`.
- Fleet HQ Clean is an unencrypted room. `@agent0-5` is already in it.
- `@agent0-5` is already in one encrypted room.
- You have a shell on g2s.
- `AMCTL_API_KEY` is set in your g2s shell. Do not print it.

Warning: Do not print `.env`, the access token, the API key, or `.a0-api-key`. Do not turn encryption on in Fleet HQ Clean. Do not use another agent as the lab target. Do not delete the old bot device.

## 1. Pause automatic restarts

Warning: If agent0-5 stays `active`, am-ctl starts `run_bot` again. Two programs would then answer one message.

1. On g2s, set agent0-5 to maintenance.

```bash
curl -s -X PUT -H "X-API-Key: $AMCTL_API_KEY" -H "Content-Type: application/json" \
  -d '{"status":"maintenance"}' http://127.0.0.1:8077/api/v1/agents/agent0-5
```

2. Confirm the JSON status is `maintenance`.

## 2. Stop the old bot

3. Stop `run_bot`.

```bash
docker exec agent0-5 supervisorctl stop run_bot
docker exec agent0-5 supervisorctl status run_bot
```

4. Confirm the status line says `STOPPED`.

## 3. Build the binary

5. On the machine that has this repo, build the bookworm binary.
6. Do not build inside the agent container.

```bash
cd /home/l0r3zz/Dropbox/GEN-AI/AGENT-MATRIX/matrix-mcp-server-r2-repo
docker run --rm -v "$PWD":/build -w /build rust:1.99-bookworm \
  bash -c 'cargo build --release --features e2ee'
```

The binary is `target/release/matrix-mcp-server-r2`.

## 4. Copy the binary into agent0-5

7. Copy the binary to g2s.
8. Save the current binary inside the container.
9. Copy the new binary over the current file.

```bash
scp target/release/matrix-mcp-server-r2 l0r3zz@172.23.100.121:/tmp/matrix-mcp-server-r2
ssh l0r3zz@172.23.100.121
docker exec agent0-5 cp -a \
  /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2 \
  /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2.bak-003
docker cp /tmp/matrix-mcp-server-r2 \
  agent0-5:/a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2
docker exec agent0-5 chmod 0755 \
  /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2
docker exec agent0-5 \
  /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2 --version
```

`--version` only shows that the file runs.

## 5. Turn the bot on in the MCP env file

`run_mcp` reads `/a0/usr/workdir/matrix-mcp-server/.env`. It does not read the old bot env file.

Warning: Do not copy `MATRIX_ACCESS_TOKEN` from the old bot env file. This process keeps the token it already has.

10. Copy the bot settings that are still missing.
11. Set `BOT_ENABLED` to the exact word `true`.

```bash
docker exec agent0-5 sh -c '
mcp=/a0/usr/workdir/matrix-mcp-server/.env
bot=/a0/usr/workdir/matrix-bot/.env
for name in AMCTL_URL AMCTL_API_KEY BOT_DISPLAY_NAME AGENT_IDENTITY A0_API_URL; do
  grep -q "^${name}=" "$mcp" && continue
  line=$(grep "^${name}=" "$bot" | head -n 1 || true)
  if [ -n "$line" ]; then
    printf "%s\n" "$line" >> "$mcp"
  fi
done
grep -q "^BOT_ENABLED=" "$mcp" || printf "BOT_ENABLED=true\n" >> "$mcp"
grep -q "^BOT_DISPLAY_NAME=" "$mcp" || printf "BOT_DISPLAY_NAME=Agent0-5\n" >> "$mcp"
'
```

12. Check the setting names without printing the key.

```bash
docker exec agent0-5 sh -c '
mcp=/a0/usr/workdir/matrix-mcp-server/.env
grep -E "^(BOT_ENABLED|BOT_DISPLAY_NAME|AGENT_IDENTITY|A0_API_URL|AMCTL_URL)=" "$mcp"
grep -q "^AMCTL_API_KEY=." "$mcp" && echo AMCTL_API_KEY=set || echo AMCTL_API_KEY=missing
test -s /a0/usr/workdir/matrix-bot/.a0-api-key && echo a0-api-key=present || echo a0-api-key=missing
'
```

13. Stop if `AMCTL_API_KEY` is missing.
14. Stop if `a0-api-key` is missing.
15. Do not type either secret into the shell.

`BOT_ENABLED` must be the exact word `true`. Any other value leaves the bot off.

## 6. Restart the MCP program

Warning: Do not run `switch-mcp-server.sh rust-restart`. That script starts a second process. The second open of the crypto store fails.

Warning: Do not run `docker restart agent0-5` for this lab.

16. Restart `run_mcp`.

```bash
docker exec agent0-5 supervisorctl restart run_mcp
docker exec agent0-5 curl -sf http://127.0.0.1:3000/health
```

17. Repeat the health command until it returns JSON.

Pass for this step: `"e2ee":true`, `"crossSigned":true`, and `"botEnabled":true`.

`botEnabled` false means `BOT_ENABLED` was not the word `true`. It can also mean the bot code did not start.

18. Read the log if health does not reach that state.

```bash
docker exec agent0-5 supervisorctl tail -80 run_mcp
```

`KeyStoreUnavailableError` means two processes opened the crypto store. Stop. Do not start `run_bot`.

## 7. Send the fast commands

Warning: Send `agent0-5: ping`. Do not start the line with `@`. The code treats `@agent0-5` as the whole prefix. The letters `: ping` then remain. That text is not the command `ping`.

A mention pill is the linked name Element inserts when you type `@`.

Warning: Do not use a mention pill for these lines. A mention pill becomes the full Matrix user id. The remainder is then not `ping`.

Warning: Send each line after health is good. The bot ignores a message older than 5 minutes.

The name in the hello line is `AGENT_IDENTITY` when that value is set. Otherwise it is `BOT_DISPLAY_NAME`. Otherwise it is the words `Agent Zero`.

19. In Fleet HQ Clean, send `agent0-5: ping`.
20. Confirm one reply: `PONG`.
21. In Fleet HQ Clean, send `agent0-5: hello`.
22. Confirm one reply: `Hello! I'm {name}, online and ready.`
23. In the encrypted room, send `agent0-5: ping`.
24. Confirm one reply: `PONG`.
25. Confirm Element shows that reply as readable text.
26. In the encrypted room, send `agent0-5: hello`.
27. Confirm one reply.
28. Confirm Element can read that reply.

29. Confirm `run_bot` is still stopped.

```bash
docker exec agent0-5 supervisorctl status run_bot
```

Two replies to one `ping` means `run_bot` is running. It can also mean Agent Zero sent its own Matrix message.

No reply in the encrypted room means this device could not decrypt. Read the log for that event id. Do not expect a reply in the room for that failure.

## 8. Send one real question

30. In the encrypted room, send `agent0-5:` and a short question.
31. Confirm Element shows one reply as readable text.
32. Send a second short question in the same room.
33. Confirm Element shows one more reply.

34. List the room ids in the context file without printing the stored values.

```bash
docker exec agent0-5 python3 -c 'import json; print(list(json.load(open("/a0/usr/workdir/matrix-bot/room_contexts.json"))))'
```

35. Confirm the encrypted room id is in that list.
36. Confirm that room id appears once.

No reply means the Agent Zero call failed. The bot sends no answer when that call fails. Read the MCP log for the status code. The log must not include the API key.

## 9. Invite from the trusted account

37. From `@admin:v-site.net`, invite `@agent0-5` to a new room.
38. Confirm the agent joins.
39. Confirm one greeting: `Hello! I'm {BOT_DISPLAY_NAME}, an AI agent powered by Agent Zero.`
40. Use the display name from the check in step 12.

## 10. Invite from an account that is not trusted

41. Invite `@agent0-5` from a Matrix user id that is not on the trusted-sender list.
42. Use a different new room.
43. If you have no second account, record this check as not run.

44. Confirm the agent does not join.
45. Read the log.

```bash
docker exec agent0-5 supervisorctl tail -80 run_mcp
```

46. Confirm the log contains `reject_untrusted`.
47. Confirm the log contains that sender id.

Warning: Stop reading if a line contains an access token or an API key. Do not copy that line into the result file.

## 11. Write the result

48. Write pass or fail for each check in `spec/T018-Agent0-5-Result-YYYYMMDD.md`.
49. Include the `/health` JSON and the Element lines you saw.
50. Omit tokens, the API key, and `.a0-api-key`.

Leave T018 unchecked until that file records a run you did.

## How you know this task is done

Every check below passed:

- `/health` shows `e2ee` true, `crossSigned` true, and `botEnabled` true.
- Fleet HQ Clean returned one `PONG` and one hello line.
- The encrypted room returned one `PONG` and one hello line.
- Element could read those encrypted replies.
- One question got one reply.
- The second question got one reply.
- `room_contexts.json` lists that encrypted room id once.
- The trusted invite joined and greeted once.
- The untrusted invite did not join.
- The log shows `reject_untrusted`.
- `run_bot` stayed `STOPPED`.

Warning: Leave agent0-5 in maintenance after a pass. If you set it back to `active`, am-ctl can start `run_bot` again.

## Rollback

Use this only if you want the old bot to answer again.

Warning: Start `run_bot` only after `BOT_ENABLED` is not `true`. Otherwise both programs answer.

51. Set `BOT_ENABLED=false` in `/a0/usr/workdir/matrix-mcp-server/.env`.
52. Restore the saved binary if you also want the old program file.

```bash
docker exec agent0-5 cp -a \
  /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2.bak-003 \
  /a0/usr/workdir/matrix-mcp-server/matrix-mcp-server-r2
```

53. Restart `run_mcp`.
54. Start `run_bot`.
55. Set agent0-5 back to `active`.

```bash
docker exec agent0-5 supervisorctl restart run_mcp
docker exec agent0-5 supervisorctl start run_bot
curl -s -X PUT -H "X-API-Key: $AMCTL_API_KEY" -H "Content-Type: application/json" \
  -d '{"status":"active"}' http://127.0.0.1:8077/api/v1/agents/agent0-5
```

56. Confirm `/health` shows `botEnabled` false.
57. Confirm `run_bot` is `RUNNING`.
