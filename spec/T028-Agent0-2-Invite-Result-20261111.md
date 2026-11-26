---
clear-rules: on
---

# T028 lab result -- Agent0-2

Date: 2026-11-11

This file records the `@agent0-2` canary for feature 004.
The checks come from `.specify/specs/004-invite-reconciliation/quickstart.md`.
The canary did not pass every check.
No git tag was created.
No repo was pushed.
`Cargo.toml` stays at `0.3.1`.

## Setup

The lab target is `@agent0-2:agent0-2-mhs.cybertribe.com`.
The trusted account is `@admin:v-site.net`.
The untrusted account is `@t018-untrusted:v-site.net`.
Rooms were created with the Synapse admin login path.
The binary was built in `rust:1.99-bookworm` with `--features e2ee`.
The old binary is `matrix-mcp-server-r2.bak-pre-004` in the MCP directory.
`/health` after the restart was `healthy`.
`version` was `0.3.1`.
`e2ee` was true.
`crossSigned` was true.
`botEnabled` was true.
The device id was `XkKM3pArkA` before the restart and after it.
`run_bot` stayed stopped.
The agent was in maintenance during the checks.
It is `active` again.
`run_bot` was still stopped eight seconds after that change.

## Offline invite

Room A is `!SgyTHzlIzBIELYASA4Z7SFn6q_m7TnJ_c3G350goXEY`.
`run_mcp` was stopped.
`@admin:v-site.net` created the room and invited the agent.
`run_mcp` was started.
The log line was `decision="join"` for `@admin:v-site.net`.
Synapse `make_join` returned 200.
Synapse `send_join` returned 200.
The agent membership was `join`.
The room had one greeting.
Pass.

## Runtime invite

Room B is `!1umw1DLGQAcFG_iRcd09-IfuQ8iQmJ9rMzrOo6Nz06s`.
`run_mcp` was already running.
The log line was `decision="join"` for `@admin:v-site.net`.
The next line was `invite join failed` with `attempt=1` and `error_class=permanent`.
Synapse had returned 200 for the invite.
About one second later, `make_join` returned 403.
The error text was `You are not invited to this room.`
The membership stayed `invite` for the wait.
The greeting count was 0.
There was no second join attempt.
Fail.

## Untrusted invite

Room C is `!sWCjB0dnVPdShPGV_jMmA4tKOPuMZtXtr6ucHoAbG5A`.
The sender was `@t018-untrusted:v-site.net`.
The log line was `decision="skip_untrusted"`.
That line appeared once.
There was no `invite join failed` line for this room.
The admin membership read returned `M_FORBIDDEN`.
The admin account is not in that room.
Pass for the decision.
The `list_invitations` body was not captured during this step.

## Encrypted invite

Room D is `!0w54MFqagnp_kTEMi_Rbwob1IjI7gE7h22hTiUuW-fY`.
The log line was `decision="join"` for `@admin:v-site.net`.
The next line was `invite join failed` with `attempt=1` and `error_class=permanent`.
Synapse had returned 200 for the invite.
`make_join` then returned 403.
The error text was `You are not invited to this room.`
The membership stayed `invite`.
No greeting was read.
Fail.

## Cleanup

`leave_room` was called for rooms A, B, C, and D.
`list_invitations` then returned `{"count":0,"invitations":[],"truncated":false}`.
The bot-off check in the quickstart was not run.

## Still true after the run

The canary binary is the running `run_mcp` binary.
The previous binary is the `bak-pre-004` file.
No other agent was changed.
