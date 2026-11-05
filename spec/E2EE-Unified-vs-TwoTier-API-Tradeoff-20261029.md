# Tradeoff Analysis: Two-Tier (`v1` + `-r2`) vs. Unified E2EE-By-Default API

**Date**: 2026-10-29
**Repository**: `matrix-mcp-server-r2` (branch `002-e2ee-cross-signing`)
**Context**: Re-evaluating the two-tier MCP tool design now that strict plug-compatibility with the legacy TypeScript `mjknowles/matrix-mcp-server` is no longer a hard requirement, and the primary goal is **transparent E2EE-by-default (`cross-signed`) with seamless fallback to unencrypted rooms**.

---

## 1. Direct Answers to Your Core Questions

### Q1: Does the existing two-tier (`v1` + `-r2`) strategy meet your goal that *"external clients can have complete E2EE without having to be aware of any change in operation"*?
**No -- it actively prevents that goal.**
Look at how the current two-tier specification (`spec/matrix-mcp-v1-v2.yml:16-17, 40-48, 67-72, 174-191`) is wired:
1. **`non_goals` (`spec/matrix-mcp-v1-v2.yml:17`) explicitly says:** *"Auto-upgrading existing clients to E2EE without an explicit switch to -r2 tools."*
2. **Existing tools fail on encrypted rooms:** If an external client (Agent Zero, Cursor, or an operator prompt) calls the standard `send-message` or `get-room-messages` tool on an encrypted room, the two-tier spec forces the server to abort with `EncryptedRoomError ("room is encrypted; use *-r2 tools instead")`.
3. **Standard room creation is unencrypted:** Calling `create-room` (`src/mcp/server.rs:562-594`) or `send-direct_message` (`src/mcp/server.rs:524-560`) creates an **unencrypted** room. Even calling `create-room-r2` without arguments creates an **unencrypted** room because `EncryptionMode` defaults to `"none"` (`spec/matrix-mcp-v1-v2.yml:47`)!
4. **LLM / Agent Tool-Selection Confusion:** When an MCP server exposes both `create-room` AND `create-room-r2`, `send-message` AND `send-message-r2`, `get-room-messages` AND `get-room-messages-r2`, LLM agents frequently pick the base tool name out of habit, hit `EncryptedRoomError`, and burn a retry turn (or fail outright if prompts/code hardcode the tool names).

### Q2: Is `TOFU` required to support non-encrypted rooms or to fall back to non-encrypted rooms?
**No. `TOFU` has zero relation to non-encrypted rooms.**
This is one of the cleanest separations in the Matrix protocol (`matrix-sdk` 0.19):
- **Room Encryption (`m.room.encryption` state event)** is a **per-room** property. A single `matrix-sdk::Client` with E2EE and Cross-Signing enabled can simultaneously sit in **both** encrypted rooms (e.g., private task rooms, agent scratchpads, confidential DMs) and unencrypted rooms (e.g., `Fleet HQ Clean !gahcrWFWAUJQPPkuQy:v-site.net`).
  - When `room.send(...)` is called on a room **with** `m.room.encryption`, `matrix-sdk` automatically encrypts the payload into `m.room.encrypted` (Megolm) and distributes room keys according to the trust policy.
  - When `room.send(...)` is called on a room **without** `m.room.encryption`, `matrix-sdk` automatically sends a standard plaintext `m.room.message` -- **no Olm/Megolm keys, device keys, or trust policies (`cross-signed` vs `TOFU`) are consulted at all**.
- **What `TOFU` vs. `cross-signed` actually controls:** It **only** controls which devices receive the Megolm decryption key *inside an already-encrypted room*.
  - Under `cross-signed` trust, if user `@alice` is in an encrypted room on a cross-signed Element session AND also logged into an unverified incognito browser, her cross-signed Element session gets the room key and reads the message, while her unverified incognito browser receives `m.room_key.withheld` (`m.unverified`).
  - Meanwhile, in any **unencrypted** room, `@alice` (and any unverified bot or client) can read and send plaintext messages with zero involvement from E2EE or TOFU.

### Q3: Can a fallback to unencrypted rooms be introduced into the design *without* re-introducing `TOFU`? What is the penalty?
**Yes -- you can have automatic or explicit fallback to unencrypted rooms during room creation while keeping `cross-signed` 100% strict for all encrypted rooms.**
To understand the tradeoff, we must distinguish two places where "client can't support encrypted rooms" can happen:

1. **At Room Creation Time (`create-room` / `send-direct-message`) -- Pre-Flight Capability Check (Safe Fallback):**
   - When an agent calls `create-room` (or `send-direct-message` to a new peer) with default encryption (`encrypt: true` / `encryptionMode: "auto"`), the server can query `/_matrix/client/v3/keys/query` for the invited users *before* creating the room:
     - **Case A (All invited users have cross-signed E2EE devices, or no invitees are specified):** Create the room **with** `m.room.encryption` (`RoomEncryptionEventContent::with_recommended_defaults()`).
     - **Case B (An invited user has no E2EE device keys or no cross-signed identity on their homeserver -- e.g., a legacy plaintext webhook/bot or an unbootstrapped account):**
       - If policy is `auto` (fallback enabled): Create the room **without** `m.room.encryption` (v1 plaintext room) and include a clear notice in the response (`"Encrypted: No (fallback: invitee @legacy:domain has no cross-signed E2EE device keys)"`).
       - If policy is `required` (`encrypt: true` explicitly passed): Refuse room creation with an actionable error instead of silently downgrading.
   - **What is the penalty of `auto` fallback at room creation?**
     - **Security / Downgrade Consideration:** If a human or agent *assumed* a newly created room with `@someone` would be E2EE, and `@someone` had not bootstrapped cross-signing yet, `auto` fallback silently creates a plaintext room (and in Matrix, while you can upgrade a plaintext room to E2EE later, any initial messages sent before upgrade are plaintext).
     - **Mitigation:** Make `encrypt` default to `true` (or configurable via `DEFAULT_ROOM_ENCRYPTION=e2ee|auto|none`), and always report `Encrypted: Yes/No` prominently in the tool output.

2. **Inside an Existing Encrypted Room (`send-message`) -- Why You Should NEVER Fall Back to Plaintext or TOFU Here:**
   - Once a room has `m.room.encryption` state enabled in Matrix, the room is permanently encrypted (Matrix specification forbids downgrading an encrypted room back to plaintext because doing so would allow homeserver admins to inject a fake state event to strip encryption).
   - If you sent a plaintext `m.room.message` inside an encrypted room, compliant Matrix clients (Element) either flag it with a red warning shield or ignore it, and the homeserver sees the cleartext.
   - If you fell back to `TOFU` (`TrustRequirement::Untrusted`) inside an encrypted room when a peer device isn't cross-signed, you would defeat the entire purpose of `002-e2ee-cross-signing`: a compromised homeserver or rogue unverified session could silently intercept room keys (the exact vulnerability class in `CVE-2026-45056` and the October 2026 Landscape Review).

---

## 2. How `matrix-sdk` 0.19 Actually Works Under the Hood

Ironically, the existing codebase's `.cursor/rules/rust-e2ee.mdc:35-40, 157` already noted how `matrix-sdk` is designed to work:

```35:40:matrix-mcp-server-r2-repo/.cursor/rules/rust-e2ee.mdc
When the `e2e-encryption` feature is enabled, `matrix-sdk` handles encryption **transparently**:

- `room.send(content)` automatically encrypts if the room has `m.room.encryption` state
- Incoming encrypted events are automatically decrypted during sync
- The `OlmMachine` state machine (from `matrix-sdk-crypto`) manages all Olm/Megolm sessions
- `vodozemac` (pure Rust, replaces deprecated `libolm`) provides the underlying Olm/Megolm crypto primitives
```

Look at what `src/mcp/server.rs` currently does in the 20 v1 tools:
- **`send_message` (`src/mcp/server.rs:505-521`):** Calls `room.send(content).await`. In `matrix-sdk` 0.19 with `e2e-encryption` enabled, `room.send(content)` **already** inspects `room.is_encrypted()`. If the room is encrypted, `matrix-sdk` automatically encrypts the event via Megolm and shares keys only with cross-signed devices. If the room is unencrypted, `matrix-sdk` sends it as plaintext `m.room.message`!
- **`get_room_messages` (`src/mcp/server.rs:302-317`), `get_messages_by_date` (`320-335`), `identify_active_users` (`338-358`):** Call `room.messages(opts).await`. In `matrix-sdk` 0.19 with `e2e-encryption` enabled, `room.messages()` **automatically decrypts** `m.room.encrypted` events using the SQLite crypto store before returning `TimelineEvent`s! If a key is missing, `TimelineEvent.kind` is `UnableToDecrypt`, which `parse_message_events` (`src/mcp/server.rs:227-249`) can format as `[Encrypted message - unable to decrypt (missing_key)]` alongside decrypted messages.
- **`create_room` (`src/mcp/server.rs:562-594`) & `send_direct_message` (`524-560`):** Currently construct `CreateRoomRequest::new()` without `RoomEncryptionEventContent`. Adding `RoomEncryptionEventContent::with_recommended_defaults()` to `req.initial_state` when `encrypt: true` (the new default when E2EE is enabled) makes standard `create-room` and `send-direct-message` create E2EE rooms out of the box!

In other words: **The two-tier (`v1` + `-r2`) split actually requires writing extra code (`EncryptedRoomError` guards and duplicate `*-r2` handlers) to artificially block `matrix-sdk` from doing what it already does transparently!**

---

## 3. Side-by-Side Architecture Comparison

We compare three architectural options for `matrix-mcp-server-r2`:

- **Option A: Current Two-Tier Spec (`20 v1 tools` + `5 v2 -r2 tools`)**
  - v1 tools (`create-room`, `send-message`, `get-room-messages`, etc.) remain unencrypted-only and throw `EncryptedRoomError` on encrypted rooms.
  - v2 tools (`create-room-r2`, `send-message-r2`, `get-room-messages-r2`, `bootstrap-e2ee-r2`, `get-encryption-status-r2`) must be explicitly called by clients wanting E2EE.
- **Option B: Unified E2EE-By-Default API (`20 transparent tools` + `2 E2EE admin/status tools`) -- RECOMMENDED**
  - Keep the 20 existing tool names (`create-room`, `send-message`, `send-direct-message`, `get-room-messages`, `get-messages-by-date`, etc.).
  - Auto-bootstrap E2EE on server startup when `E2EE_ENABLED=true` (while also exposing `bootstrap-e2ee` and `get-encryption-status` for explicit diagnostics/repair).
  - `create-room` and `send-direct-message` default to **E2EE enabled** (`encrypt: Option<bool>`, defaulting to `true` when server E2EE is active, or `auto` pre-flight check with fallback to unencrypted if an invitee lacks cross-signed E2EE keys; callers can also pass `encrypt: false` explicitly to create an unencrypted room).
  - `send-message`, `get-room-messages`, `get-messages-by-date`, and `identify-active-users` work **transparently on both encrypted and unencrypted rooms** (sensing `room.is_encrypted()` automatically via `matrix-sdk` 0.19).
- **Option C: Unified E2EE-By-Default + Thin `-r2` Aliases**
  - Same as Option B, except `create-room-r2`, `send-message-r2`, `get-room-messages-r2`, `bootstrap-e2ee-r2`, and `get-encryption-status-r2` are also registered as thin aliases/wrappers for any script or spec test that explicitly calls `-r2` names.

### Comparison Matrix

| Dimension | Option A: Current Two-Tier (`v1` + `-r2`) | Option B: Unified E2EE-By-Default (`20 + 2 tools`) | Option C: Unified + `-r2` Aliases (`20 + 5 tools`) |
|---|---|---|---|
| **External Client Zero-Config E2EE** | **Fails goal.** Clients calling `create-room`, `send-message`, or `get-room-messages` get unencrypted rooms or `EncryptedRoomError`. Every client prompt/workflow must be rewritten to call `*-r2`. | **100% Seamless.** Existing Agent Zero prompts, Cursor sessions, and scripts call `create-room`, `send-message`, `get-room-messages` unchanged and get cross-signed E2EE automatically. | **100% Seamless.** Works whether caller uses standard names or `-r2` names. |
| **Default Room Creation Behavior** | `create-room` -> unencrypted. `create-room-r2` -> also defaults to `encryption_mode: "none"` unless `"e2ee"` is passed! | `create-room` & `send-direct-message` -> **E2EE by default** (with optional `encrypt: false` or `auto` fallback if invitee lacks E2EE keys). | Same as Option B. |
| **Entering & Using Non-Encrypted Rooms (e.g., Fleet HQ)** | Works via v1 tools; `-r2` tools require `encryption_mode: "none"`. | **Automatic.** `join-room`, `send-message`, and `get-room-messages` inspect room state (`room.is_encrypted()`). Unencrypted rooms work natively with zero config. | Same as Option B. |
| **Coverage of All 20 Tools on Encrypted Rooms** | **Incomplete.** Only `send-message` and `get-room-messages` have `-r2` counterparts. `get-messages-by-date`, `identify-active-users`, and `send-direct-message` have **no `-r2` version at all**! | **Complete.** Because decryption happens in shared `parse_message_events()` and encryption happens in `room.send()`, **all** tools (`get-messages-by-date`, `identify-active-users`, `send-direct-message`) gain E2EE support automatically. | Same as Option B. |
| **LLM Tool-Selection Overhead** | High friction: 25 tools with overlapping descriptions (`send-message` vs `send-message-r2`). LLMs frequently guess wrong and hit `EncryptedRoomError`. | Low friction: 22 distinct tools (20 operational + `bootstrap-e2ee` + `get-encryption-status`). Zero duplicate messaging/room tools. | Medium friction: 25 tools in MCP tool list if aliases are advertised. |
| **Startup vs. Manual Bootstrap** | Requires a client to call `bootstrap-e2ee-r2` before `send-message-r2` works (otherwise returns `CryptoNotBootstrappedError`). | Server runs E2EE/cross-signing bootstrap **automatically on startup** when `E2EE_ENABLED=true`. External clients don't even need to know `bootstrap-e2ee` exists unless diagnosing an issue. | Same as Option B. |
| **Implementation Complexity** | Higher: must maintain artificial `EncryptedRoomError` guards on v1 tools + duplicate input/output structs for `-r2` tools. | **Simpler & cleaner:** upgrades the core client + `parse_message_events()` + `create_room` in place, plus 2 new diagnostic/admin tools. | Slightly more boilerplate than B (thin alias methods). |

---

## 4. Deep Dive: How Seamless Fallback Works in Option B (Unified E2EE-By-Default)

If we adopt **Option B (Unified E2EE-By-Default)**, here is how every lifecycle stage works so external clients don't have to change a single tool call:

### 4.1 Startup Auto-Bootstrap (No Manual Tool Call Needed)
Instead of waiting for an external MCP client to call `bootstrap-e2ee-r2` (which a v1-style client doesn't know how to call!), `matrix-mcp-server-r2` performs the bootstrap automatically during `create_matrix_client()` in `src/matrix/client.rs` when `E2EE_ENABLED=true`:
1. Open SQLite store at `CRYPTO_STORE_PATH` and read/initialize `.device-id` (matching `/_matrix/client/v3/account/whoami`).
2. Upload device keys and one-time prekeys.
3. Resolve the recovery secret (`MATRIX_RECOVERY_KEY_FILE` / env var) to unlock Secret Storage and cross-sign this device (or create initial cross-signing identity if none exists on Continuwuity).
4. Set `matrix-sdk` 0.19 recipient strategy to **cross-signed only** (no TOFU).
5. Expose `bootstrap-e2ee` and `get-encryption-status` as MCP tools for operators/re-bootstrap, and expose `"e2ee": true, "crossSigned": true` on `GET /health`.

### 4.2 Creating Rooms (`create-room` and `send-direct-message`)
We add one optional field to `CreateRoomInput` (and `SendDirectMessageInput`):
```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateRoomInput {
    #[serde(rename = "roomName")]
    pub room_name: String,
    #[serde(rename = "isPrivate")]
    pub is_private: Option<bool>,
    pub topic: Option<String>,
    #[serde(rename = "inviteUsers")]
    pub invite_users: Option<Vec<String>>,
    #[serde(rename = "roomAlias")]
    pub room_alias: Option<String>,
    /// Optional encryption mode: "e2ee" (default when E2EE is enabled),
    /// "auto" (E2EE if all invitees have cross-signed keys, otherwise fallback to unencrypted),
    /// or "none" (unencrypted room).
    #[serde(rename = "encryptionMode", alias = "encryption_mode")]
    pub encryption_mode: Option<String>,
}
```
- **When an existing client calls `create-room` without `encryptionMode`:**
  - If the room is **public** (`isPrivate: false` and `roomAlias` set): public directory rooms in Matrix are typically unencrypted so anyone can join and read history, or encrypted if private.
  - If `encryptionMode` defaults to `"auto"` (or `"e2ee"`):
    - The server queries `/keys/query` for `inviteUsers`.
    - If all invitees have cross-signed E2EE identity keys (or `inviteUsers` is empty), the server attaches `RoomEncryptionEventContent::with_recommended_defaults()` and `HistoryVisibility::Joined` to `req.initial_state`, creating an **E2EE room by default**.
    - If an invitee has no E2EE/cross-signing keys and fallback is enabled (`"auto"`), the server logs a warning, omits `RoomEncryptionEventContent`, creates a v1 **unencrypted room**, and reports in the tool output:
      `Encrypted: No (fell back to unencrypted because @user:domain has no cross-signed E2EE keys)`
- **When an existing client calls ` create-room` and explicitly wants an unencrypted room:**
  - It passes `"encryptionMode": "none"` (or joins an existing unencrypted room via `join-room`).

### 4.3 Sending Messages (`send-message` and `send-direct-message`)
- The client simply calls `send-message({ "roomId": "!abc:domain", "message": "Hello" })`.
- `matrix-mcp-server-r2` checks `room.is_encrypted().await`:
  - **If the room is encrypted:** `room.send(content).await` automatically encrypts with Megolm via `matrix-sdk` 0.19 and shares the room key **only** with cross-signed devices of room members. The tool response includes `Encrypted: Yes`.
  - **If the room is NOT encrypted (e.g., Fleet HQ Clean or a fallback room):** `room.send(content).await` sends standard plaintext `m.room.message`. The tool response includes `Encrypted: No`.
- **Zero `EncryptedRoomError` friction!** The caller never gets rejected for using `send-message` on an encrypted room.

### 4.4 Reading Messages (`get-room-messages`, `get-messages-by-date`, `identify-active-users`)
- The client simply calls `get-room-messages({ "roomId": "!abc:domain", "limit": 20 })`.
- `room.messages(opts).await` returns timeline events:
  - Plaintext events in unencrypted rooms are parsed as before.
  - Decrypted `m.room.encrypted` events in E2EE rooms are seamlessly parsed and returned in the exact same `{sender}: {body}` format!
  - Undecryptable events (`UnableToDecrypt` / missing Megolm key) are never dropped silently or panicked on; they return `{sender}: [Encrypted message - unable to decrypt (missing_key)]` so the agent/human knows an encrypted message exists whose key wasn't shared with this device.

---

## 5. Summary of Penalties / Risks to Weigh

| Design Choice | Benefit | Penalty / Risk | Mitigation |
|---|---|---|---|
| **1. Dropping the Two-Tier Refusal (`EncryptedRoomError`) in favor of Unified Transparent Tools** | External clients get E2EE automatically with zero code/prompt changes; all 20 tools (`get-messages-by-date`, `send-direct-message`, etc.) work in E2EE rooms. | A naive caller sending sensitive data into an **existing room** might not check whether that room is encrypted before calling `send-message`. | Include `Encrypted: Yes/No` in `get-room-info`, `create-room`, and `send-message` outputs; optionally support `requireEncrypted: true` on `send-message` for high-security callers. |
| **2. Automatic Fallback (`auto`) at Room Creation when an Invitee Lacks Cross-Signed Keys** | Seamless interoperability with un-bootstrapped accounts or plaintext bots without failing `create-room`. | **Silent Downgrade Risk:** If an invitee hasn't bootstrapped E2EE yet (or if federation `/keys/query` transiently times out), the room is created unencrypted permanently. | Distinguish `" federation timeout / error "` (fail closed, do not downgrade on network error!) from `" invitee has 0 cross-signing keys on homeserver "` (safe to fallback if `auto` is enabled, and clearly state `Encrypted: No (fallback)` in the response). |
| **3. Keeping Strict `cross-signed` Trust (Never Re-Introducing `TOFU`)** | Eliminates impersonation and unverified key-leakage attacks inside E2EE rooms; satisfies October 2026 security posture. | Inside an encrypted room, if a human opens a quick unverified Element session without entering their Recovery Key, that unverified tab will see *"Unable to decrypt (The sender has disabled encrypting to unverified devices)"*. | None needed -- this is the exact intended security property of `SC-003` (`spec.md:110`), and the human simply verifies their Element session with their Recovery Key. |

---

## 6. Impact on Existing Spec Kit Artifacts (`002-e2ee-cross-signing`) if You Switch to Unified E2EE-By-Default

If you decide to replace the two-tier (`v1` refusal + `-r2` explicit tools) model with the **Unified E2EE-By-Default** model, the code changes haven't started yet (`0/15` tasks in `tasks.md`), so **now is the cheapest possible moment to pivot!**

Specifically, we would update:
1. **`.specify/memory/constitution.md` (Principle I & Naming Conventions):** Change *"v1 tools refuse encrypted rooms; v2 tools use `-r2` suffix"* to *"Existing MCP tool names transparently support E2EE by default while remaining backwards-compatible with unencrypted rooms."*
2. **`.specify/specs/002-e2ee-cross-signing/{spec.md, plan.md, data-model.md, tasks.md}`:**
   - Remove `T011` (`EncryptedRoomError` refusal on v1 tools).
   - Replace `create-room-r2`, `send-message-r2`, `get-room-messages-r2` with transparent E2EE upgrades to `create-room`, `send-direct-message`, `send-message`, `get-room-messages`, `get-messages-by-date`, and `identify-active-users`.
   - Keep `bootstrap-e2ee` and `get-encryption-status` (with or without `-r2` suffix) plus automatic startup bootstrap when `E2EE_ENABLED=true`.
3. **`spec/matrix-mcp-v1-v2.yml`:** Update the contract to reflect unified E2EE-by-default tool schemas and fallback rules.

---

## 7. Does `matrix-bot` (Rust and Python) Have to Be Refactored?

### 7.1 Crucial Fact: `matrix-bot` Does NOT Call `matrix-mcp-server-r2` Today
Neither the Rust bot (`agent-matrix-repo/multi-instance-deploy/templates/matrix-bot/rust/src/main.rs`) nor the legacy Python bot (`templates/matrix-bot/matrix_bot.py`, which is retired in favor of Rust per `AGENTS.md`) calls `matrix-mcp-server-r2` (port `3000`).
- **How `matrix-bot` works:** It connects directly to the Continuwuity homeserver (`http://agent0-N-mhs:8008/_matrix/client/v3/sync`), watches for `"type": "m.room.message"`, forwards the message text to Agent Zero (`http://localhost:80/api/api_message`), and posts Agent Zero's plain-text response directly back to Continuwuity via HTTP `PUT /_matrix/client/v3/rooms/{room}/send/m.room.message/{txnId}` (`main.rs:343-375, 896-995`).
- **Who calls `matrix-mcp-server-r2`:** Agent Zero (`run_ui.py` MCP client configured in `/a0/usr/settings.json`), Cursor IDE, and operator automation scripts.

Therefore, **changing `matrix-mcp-server-r2`'s MCP tool surface (whether Option A Two-Tier or Option B Unified E2EE-By-Default) will NEVER break `matrix-bot`!**

### 7.2 When Would `matrix-bot` Need Refactoring?
Whether `matrix-bot` needs any changes depends strictly on **whether you want `matrix-bot` to auto-reply via push when a human types `@agent0-N: ...` inside an encrypted room**:

| Scenario | Does `matrix-bot` Need Refactoring? | Why |
|---|---|---|
| **1. `matrix-bot` continues to handle unencrypted rooms (e.g., `Fleet HQ Clean`), while encrypted rooms are created/read/written via MCP tools (`matrix-mcp-server-r2`)** | **NO refactoring needed.** | Unencrypted rooms still deliver `"type": "m.room.message"` over `/sync`. `matrix-bot-rust` works 100% unchanged. *(Only caveat: `finalize-instance.sh` should give `matrix-bot` and `matrix-mcp-server-r2` separate login tokens/device IDs so they don't share one `device_id`.)* |
| **2. You also want `matrix-bot-rust` to wake up and auto-reply when a human sends an encrypted message (`m.room.encrypted`) in an E2EE room** | **YES (for `matrix-bot-rust` only; Python bot is legacy/unused).** | Because `matrix-bot-rust` uses raw `reqwest` HTTP (no `matrix-sdk`) and skips `event_type != "m.room.message"` (`main.rs:896`), it cannot decrypt `m.room.encrypted` or encrypt replies on its own. |

### 7.3 The Smart Way to Handle Encrypted Rooms in `matrix-bot-rust` (If/When Wanted)
Because `.specify/specs/002-e2ee-cross-signing/spec.md:85` forbids two processes from opening the same SQLite `CRYPTO_STORE_PATH` or sharing one E2EE `device_id`, you should **NOT** add a second `matrix-sdk` + SQLite crypto store inside `matrix-bot-rust`. Instead, you have two lightweight options whenever you want reactive bot replies in encrypted rooms:
1. **Option 1 (Delegate to `matrix-mcp-server-r2`):** When `matrix-bot-rust` sees `event_type == "m.room.encrypted"` in `/sync`, it asks `matrix-mcp-server-r2` (`localhost:3000`, which already owns the single unlocked SQLite crypto store and `matrix-sdk` 0.19 client) to decrypt the event and send the encrypted reply.
## 8. Owner Architectural Decisions (Locked 2026-10-29)

1. **Tool Surface Architecture (`Option B -- Unified E2EE-By-Default`):**
   - Drop the artificial Two-Tier (`v1` refusal + duplicate `-r2` room/message tools) split.
   - The existing 20 MCP tools (`create-room`, `send-direct-message`, `send-message`, `get-room-messages`, `get-messages-by-date`, `identify-active-users`, etc.) will transparently support both E2EE (`cross-signed`) and unencrypted rooms.
   - Add two dedicated E2EE operational/diagnostic tools: `bootstrap-e2ee` and `get-encryption-status`.
2. **Room Creation Default & Fallback Policy (`Auto Pre-Flight Check`):**
   - When `create-room` or `send-direct-message` is called without an explicit `encryptionMode` override, default to `"auto"`:
     - Query `/_matrix/client/v3/keys/query` for all `inviteUsers` (`targetUserId`).
     - If all invitees possess cross-signed E2EE keys (or no invitees are specified), create an **encrypted (`m.room.encryption`) room by default**.
     - If `/keys/query` succeeds (HTTP 200) and shows an invitee lacks E2EE / cross-signing keys, **automatically fall back to creating a v1 unencrypted room** and include a clear notice in the tool result (`Encrypted: No (fallback: @user:domain has no cross-signed E2EE keys)`).
     - If `/keys/query` fails due to network/federation error, **fail closed** (do not silently downgrade on transient errors).
     - Callers may also pass `encryptionMode: "none"` (force unencrypted) or `encryptionMode: "e2ee"` (strict E2EE, error if invitee lacks keys).
3. **Startup E2EE Auto-Bootstrap:**
   - When `E2EE_ENABLED=true`, the server automatically runs device-key and cross-signing bootstrap during startup (`create_matrix_client`), so external MCP clients get E2EE immediately without needing to call `bootstrap-e2ee` first.
4. **Trust Policy:**
   - Remains strictly **`cross-signed` only** (`TOFU` / `ignore_unverified_devices` is never used). Unencrypted room support and room-creation fallback operate at the room level (`m.room.encryption` presence), requiring zero relaxation of device trust inside encrypted rooms.


