# Feature Specification: Invite Reconciliation and Discovery

**Feature Branch**: `004-invite-reconciliation`

**Created**: 2026-11-09

**Status**: Draft

**Depends On**: `.specify/specs/003-integrated-matrix-bot/spec.md` (trusted-invite auto-join, greeting, `BOT_ENABLED`)

**Base**: `main` at `d1bde46` (tag `v0.3.1`). Provenance and gap analysis: `spec/Invite-Reconciliation-Speckit-Approach-20261109.md`.

**Input**: User description: "So that agents can see a room invite and automatically join." Source task: `20261109-222129-matrix-mcp-trusted-invitation-acceptance-60-final.md` (Galadriel, `@agent0-2`). An agent did not join a trusted invite to an encrypted room. The agent had no way to list the invite. A manual join with the recovered room id worked.

## Clarifications

### Session 2026-11-09

- Q: Which inviters may make an agent join a room automatically? → A: The 003 trusted set. That is the am-ctl trusted senders, then `TRUSTED_SENDERS` while the set is empty. No new setting is added. A failed lookup with an empty fallback does not join.
- Q: Is automatic joining on or off by default? → A: Unchanged from 003. It applies when `BOT_ENABLED=true` and the trusted set is not empty.
- Q: Does the agent post the greeting after an automatic join? → A: Yes. It posts it once per room, for rooms joined at startup and at runtime.
- Q: How do failed joins retry? → A: Up to 5 attempts per room. The wait starts at 30 seconds, doubles each time, stops growing at 15 minutes, and has random jitter. After the fifth failure, the agent stops trying and shows the last error in the invite list.
- Q: Which agent is the canary? → A: `@agent0-2`, where the bug was found. This is a stated exception to the `@agent0-5` lab default. Requirements still use `@agent0-N`.
- Q: What is the name of the discovery tool? → A: `list_invitations`.
- Q: When a join fails with an error that will not change on retry, should the agent stop retrying at once? → A: Yes. A permanent error (HTTP 403 or 404, or the matching Matrix error code) stops retries at once and is shown in `list_invitations`. Network errors, timeouts, rate limits, and server errors use the 5-attempt schedule.
- Q: If an inviter becomes trusted after the agent skipped that invite, should the agent join on a later check? → A: Yes. Each pass checks each pending invite against the current trusted set. The agent joins once the inviter is trusted. A decision is logged only when it changes for that room.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Join trusted invites that arrived while the agent was down (Priority: P1)

A trusted sender invites `@agent0-N` to a room while the agent's Matrix service is stopped. When the service starts, the agent joins the room and posts the greeting. Nobody has to restart it again or join it by hand.

**Why this priority**: This is the failure seen on `@agent0-2`. A trusted invite was never handled, so the agent missed a room it was meant to be in.

**Independent Test**: Stop the service. Invite the agent from a trusted sender. Start the service. Check that the agent is a member and that one greeting is in the room.

**Acceptance Scenarios**:

1. **Given** a pending invite from a trusted sender, **When** the service starts, **Then** the agent joins the room within 60 seconds of becoming healthy and posts the greeting once.
2. **Given** a pending invite from a sender that is not trusted, **When** the service starts, **Then** the agent does not join, and it logs the room id and the sender.
3. **Given** a pending invite when the trusted-set lookup fails and the fallback list is empty, **When** the service starts, **Then** the agent does not join, and it logs the room id and the sender.

---

### User Story 2 - List pending invites (Priority: P1)

An agent or an operator asks which rooms the agent is invited to. The answer lists each pending invite with the room id, the room name, and the inviter. It also says whether the inviter is trusted and gives the last join error, if one exists. The agent can then join a listed room with the existing `join_room` tool.

**Why this priority**: Without a list, an untrusted or failed invite cannot be seen at all. The agent cannot act on a room id it does not know.

**Independent Test**: Invite the agent from a sender that is not trusted. Call `list_invitations`. Check that the room appears with the inviter and `trusted: false`. Call `join_room` with that room id, and check that the join works.

**Acceptance Scenarios**:

1. **Given** one pending invite, **When** `list_invitations` is called, **Then** the result contains that room id, the inviter, and the trust result.
2. **Given** no pending invites, **When** `list_invitations` is called, **Then** the result is a success with an empty list.
3. **Given** a trusted invite whose join failed 5 times, **When** `list_invitations` is called, **Then** the room is still listed, with the last error and the attempt count.

---

### User Story 3 - Join trusted invites while running, with bounded retry (Priority: P2)

A trusted sender invites the agent while the service is running. The agent joins the room and posts the greeting. If the join fails, the agent tries again a limited number of times. It never joins the same room twice at once.

**Why this priority**: 003 already joins runtime invites. This story adds retry and the guard against duplicate joins.

**Independent Test**: Make the first join attempt fail. Invite the agent from a trusted sender. Check that a later attempt succeeds, that only one greeting is posted, and that no join request overlaps another for that room.

**Acceptance Scenarios**:

1. **Given** the service is running, **When** a trusted sender invites the agent, **Then** the agent joins and posts the greeting once.
2. **Given** a join fails with a temporary error, **When** the next retry time passes, **Then** the agent tries again, for at most 5 attempts in total.
3. **Given** a join fails with "forbidden" or "room not found", **When** later passes run, **Then** the agent does not try again, and `list_invitations` shows that error.
4. **Given** a join attempt is in progress, **When** another pass sees the same invite, **Then** no second join request starts for that room.

---

### Edge Cases

- The invite has no inviter, or the inviter id is not a valid Matrix user id. The agent does not join, and the invite stays listed as not trusted.
- A display name or a user id part looks like a trusted id. Only an exact match of the full Matrix user id is trusted.
- The join answer says the agent is already a member. This counts as success: no retry and no second greeting.
- The inviter is added to the trusted set after the agent skipped the invite. The next pass joins the room and posts the greeting.
- The invite is withdrawn before the join. The agent removes it from the retry state, and the room leaves the list.
- The room is encrypted. Joining does not reset the crypto store or change device trust. Messages sent before the join may not decrypt. That is a Matrix limit, not a join failure.
- `BOT_ENABLED` is false. Nothing joins automatically. `list_invitations` and `join_room` still work.
- The service stops during a retry wait. The pending retry ends with the service, and no task keeps running.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When `BOT_ENABLED=true`, the system MUST check all pending invites once after the inbound handlers are active at startup.
- **FR-002**: When `BOT_ENABLED=true`, the system MUST check all pending invites again after each successful background sync. Each pass MUST decide each pending invite against the current trusted set, so a skipped invite is joined once its inviter becomes trusted.
- **FR-003**: The system MUST use the 003 trusted set to decide each invite. That is the am-ctl trusted senders, then `TRUSTED_SENDERS` only while the set is empty. A lookup failure with an empty fallback MUST NOT join. A pass MUST read the trusted set at most once, and only when at least one invite is pending.
- **FR-004**: The system MUST compare the inviter by exact, full Matrix user id. Display names, user id parts, and domain patterns MUST NOT match.
- **FR-005**: The system MUST NOT join an invite with a missing or malformed inviter.
- **FR-006**: The system MUST NOT start a second join for a room while one join for that room is in progress.
- **FR-007**: The system MUST retry a join that fails with a temporary error, for at most 5 attempts in total. Temporary errors are network errors, timeouts, rate limits, and server errors. The wait MUST start at 30 seconds, double each time, stop growing at 15 minutes, and include random jitter.
- **FR-007a**: A join that fails with a permanent error MUST NOT be retried. Permanent errors are HTTP 403 and 404, or the matching Matrix error codes. The room MUST stay in `list_invitations` with that error.
- **FR-008**: The system MUST treat an "already a member" result as success.
- **FR-009**: The system MUST post the 003 greeting once per room after an automatic join, whether the join happened at startup or at runtime.
- **FR-010**: The system MUST provide an MCP tool `list_invitations`. For each pending invite it returns the room id, and where known the room name, canonical alias, topic, inviter, encryption state, trust result, attempt count, and last join error. With no invites it MUST return a success with an empty list.
- **FR-011**: `list_invitations` MUST work when `BOT_ENABLED` is false. It then reports trust as unknown and shows no join attempts.
- **FR-012**: The output of `list_invitations` MUST NOT contain room messages, access tokens, recovery keys, or API keys. The list MUST hold at most 100 invites.
- **FR-013**: The existing `join_room` tool MUST keep its current behavior. An invite that was not joined automatically MUST stay listed until someone joins or declines it.
- **FR-014**: A decision MUST be logged with the room id, the inviter, the decision, and the outcome when it is first made for a room and each time it changes. A pass that repeats the same decision MUST NOT log it again. Each join attempt and its outcome MUST be logged. Logs MUST NOT contain message text or credentials.
- **FR-015**: A join MUST NOT reset the crypto store, create a new device, change the cross-signed trust policy, or change the room's history or encryption settings.
- **FR-016**: Retry and in-progress state MUST be removed when the agent becomes a member or when the invite disappears.
- **FR-017**: Stopping the service MUST end any pending retry wait.

### Key Entities

- **Pending invite**: one room where the agent's membership is `invite`. It holds the room id, room details, the inviter, and the trust result.
- **Join attempt state**: per room. It holds the attempt count, the next retry time, whether a join is in progress, the last error text, and whether that error was permanent.
- **Trusted set**: the 003 set of Matrix user ids that may invite the agent.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An invite from a trusted sender, sent while the service is stopped, is joined within 60 seconds after the service reports healthy. Exactly one greeting is posted.
- **SC-002**: An invite from a trusted sender, sent while the service runs, is joined within 60 seconds. Exactly one greeting is posted.
- **SC-003**: An invite from a sender that is not trusted is never joined automatically. It appears in `list_invitations` with `trusted: false`.
- **SC-004**: A join that keeps failing with a temporary error makes at most 5 attempts. A join that fails with a permanent error makes 1 attempt. Both then appear in `list_invitations` with the last error.
- **SC-005**: Over 20 background syncs with one pending trusted invite, the room gets at most one join request in progress at any time.
- **SC-006**: All existing MCP tools still answer, and only `list_invitations` is added. Each agent's `/health` still reports `crossSigned` true after an encrypted-room join.
- **SC-007**: On the `@agent0-2` canary, the offline invite, runtime invite, and untrusted invite tests pass before any other agent is upgraded.

## Assumptions

- The base is `main` at `v0.3.1`. The stale `matrix-sdk` 0.10 checkout at `/opt/agent-zero/matrix-mcp-server-r2` on g2s is not used.
- The 003 trust contract stays as is. The input task's separate `MATRIX_AUTO_JOIN_INVITERS` allowlist is not adopted.
- Retry state is kept in memory. After a restart, the startup check begins the attempts again from zero.
- The tool surface grows from 20 + 2 to 21 + 2. The constitution and `spec/matrix-mcp-v1-v2.yml` are amended in this feature.
- The release is `v0.4.0`, because it adds a tool and changes startup behavior.
- Continuwuity stale-membership behavior and the decryption of messages sent before a join are out of scope.
