---
clear-rules: on
---

# T028 lab result -- Agent0-2

Date: 2026-11-26

This file records the `@agent0-2` canary for feature 004.
The checks come from `.specify/specs/004-invite-reconciliation/quickstart.md`.
The checks passed.
No git tag was created.
No repo was pushed.
`Cargo.toml` is `0.4.0`.

## Setup

The lab target is `@agent0-2:agent0-2-mhs.cybertribe.com`.
The trusted account is `@admin:v-site.net`.
The untrusted account is `@t018-untrusted:v-site.net`.
The Synapse admin API created each room as that account.
The binary was built in `rust:1.99-bookworm` with `--features e2ee`.
`matrix-mcp-server-r2 --version` printed `0.4.0`.
The binary hash is `4e3c5bc89ddacd72b782d9230f6bedf18a352b8774640f74e18e9237ac899dc5`.
The previous binary is `matrix-mcp-server-r2.bak-pre-040`.
The file `matrix-mcp-server-r2.bak-pre-004` was not changed.
`/health` after the restart was `healthy`.
`version` was `0.4.0`.
`e2ee` was true.
`crossSigned` was true.
`botEnabled` was true.
The device id was `XkKM3pArkA` before the restart and after it.
`run_bot` stayed stopped.
The agent was in maintenance during the checks.
It is `active` again.
`run_bot` was still stopped after that change.
`@agent0-5` already ran this same binary before these checks.
The user asked for that upgrade.

## Offline invite

Room A is `!IDRmCfAbOZgfxslL9n9FOMNSy40VXquFZURmJLdNRsc`.
The room name is `T028 offline 20261126`.
`run_mcp` was stopped at 21:46:51 UTC.
`@admin:v-site.net` created the room and invited the agent.
The membership was `invite` at 21:46:59 UTC.
`run_mcp` was started at 21:47:08 UTC.
The log line at 21:47:08.967 UTC was `decision="join"` for `@admin:v-site.net`.
The inviter was not `missing`.
Synapse `make_join` returned 200 at 21:47:10 UTC.
Synapse `send_join` returned 200 at 21:47:11 UTC.
The membership was `join` by 21:47:12 UTC.
The room had one greeting.
The greeting length was 56 characters.
There was no `invite join failed` line.
This binary does not write a `join ok` line.
Pass.

## Runtime invite

Room B is `!td_9V16dgc9_mJ02rJSg-dpncPVYwPzWIhI8icyyuuc`.
The room name is `T028 runtime 20261126`.
`run_mcp` was already running.
`@admin:v-site.net` invited the agent at 21:48:38 UTC.
The log line at 21:48:40.200 UTC was `decision="join"` for `@admin:v-site.net`.
Synapse `make_join` returned 200 at 21:48:41 UTC.
Synapse `send_join` returned 200 at 21:48:42 UTC.
The membership was `join` at 8.7 seconds.
The room had one greeting.
There was no second join request.
There was no `invite join failed` line.
Pass.

## Untrusted invite

Room C is `!QlOcya9F8uRLmg77FojqNYJifpzWVrJUei5iQlJYMHI`.
The room name is `T028 untrusted 20261126`.
The sender was `@t018-untrusted:v-site.net`.
The invite time was 21:50:08 UTC.
The membership stayed `invite` through 21:52:00 UTC.
The log line at 21:50:09.826 UTC was `decision="skip_untrusted"`.
That line appeared once.
Synapse recorded no `make_join` for this room.
`list_invitations` returned this room with `trusted` `false` and `attempts` `0`.
The inviter was `@t018-untrusted:v-site.net`.
`lastError` was null.
Pass.

## Encrypted invite

Room D is `!KoFkjbcggYA_XBfJZfnAeydY58mKcWDkKrdfDlEbmWg`.
The room name is `T028 encrypted 20261126`.
The encryption algorithm is `m.megolm.v1.aes-sha2`.
`@admin:v-site.net` invited the agent at 21:53:36 UTC.
The log line at 21:53:37.410 UTC was `decision="join"` for `@admin:v-site.net`.
Synapse `make_join` returned 403 at 21:53:37.878 UTC.
The error text was `You are not invited to this room.`
The next bot line was `invite join failed` with `attempt=1` and `error_class=temporary`.
The bot did not log the error text.
Synapse `make_join` returned 200 at 21:54:09.902 UTC.
Synapse `send_join` returned 200 at 21:54:11.278 UTC.
The membership was `join` at 37.4 seconds.
`get_room_messages` returned one greeting.
That read had no undecryptable event.
`get_encryption_status` reported `roomEncrypted` true and `crossSigned` true.
The device id was `XkKM3pArkA`.
`/health` still reported `crossSigned` true.
Pass.

## Cleanup

`leave_room` was called for rooms A, B, C, and D.
Each call left the room.
`list_invitations` then returned `{"count":0,"truncated":false,"invitations":[]}`.
The bot-off check in the quickstart was not run.
The agent is `active`.
`run_bot` is still stopped.

## Still true after the run

The canary binary is the running `run_mcp` binary.
The previous binary is the `bak-pre-040` file.
No git tag was created.
No repo was pushed.
