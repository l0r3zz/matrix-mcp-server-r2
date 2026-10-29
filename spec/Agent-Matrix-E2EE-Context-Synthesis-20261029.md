# Agent-Matrix Architecture & Operational Synthesis for `002-e2ee-cross-signing`

**Date**: 2026-10-29
**Target Feature**: `matrix-mcp-server-r2` (`002-e2ee-cross-signing`)
**Source Scope**: `agent-matrix-repo/multi-instance-deploy/docs/`, `templates/`, `scripts/`, `am-ctl/`, and `matrix-mcp-server-r2-repo/spec/`

---

## Executive Summary

`matrix-mcp-server-r2` `v0.1.1` (unencrypted v1 parity, 20 tools) is already deployed and running in production across all 5 sovereign Agent-Matrix instances (`agent0-1` .. `agent0-5`) on host `g2s`. Phase 3 (`002-e2ee-cross-signing`) upgrades `matrix-mcp-server-r2` to `matrix-sdk` 0.19.x (Rust >= 1.96) with `e2e-encryption` and `bundled-sqlite`, introducing five `-r2` E2EE tools and enforcing **strict cross-signed device trust** (dropping `TOFU` / `ignore_unverified_devices` completely).

Synthesizing the Agent-Matrix documentation and operational scripts against the `002-e2ee-cross-signing` design reveals **five critical architectural couplings** that must inform upcoming design and implementation decisions:

1. **Reactive vs. Proactive Split (`matrix-bot-rust` is E2EE-blind):** Each agent runs two separate Matrix client processes inside the `agent0-N` container under the same `@agent0-N` Matrix identity: `matrix-bot-rust` (inbound reactive `/sync` listener) and `matrix-mcp-server-r2` (outbound/on-demand MCP tool server). Because `matrix-bot-rust` uses raw `reqwest` HTTP calls without `matrix-sdk` and skips any event where `type != "m.room.message"`, it silently ignores `"type": "m.room.encrypted"` events. Adding E2EE to `matrix-mcp-server-r2` enables **tool-driven (proactive) E2EE**, while reactive push-based replies in encrypted rooms will require either bot delegation to MCP or unifying `/sync` inside `matrix-mcp-server-r2`.
2. **Access Token & Device ID Binding (`finalize-instance.sh` vs. `.device-id`):** In Matrix, an `access_token` is bound 1:1 on the homeserver to a single `device_id` at `/login` or `/register` time. Today, `finalize-instance.sh` creates a single `ACCESS_TOKEN` and writes it to both `matrix-bot/.env` and `matrix-mcp-server/.env`, while `matrix-mcp-server-r2` passes a local `device_id` to `restore_session()`. For `/keys/upload` to succeed on Continuwuity under `002-e2ee-cross-signing`, `$CRYPTO_STORE_PATH/.device-id` must match the server-side `device_id` of `MATRIX_ACCESS_TOKEN` (discovered via `/_matrix/client/v3/account/whoami` or created via a dedicated MCP `/login` session).
3. **Cross-Signing Trust Semantics Across Federation (`is_cross_signed_by_owner` vs. User-Signing):** Dropping TOFU means `matrix-mcp-server-r2` only shares Megolm room keys with cross-signed devices. In a headless multi-homeserver fleet (`agent0-1..5` on Continuwuity + humans on `v-site.net` Synapse), recipient verification must rely on **owner self-signing signatures** (`Device::is_cross_signed_by_owner()` / MSC4153) or automated fleet master-key cross-signing, because headless agents cannot perform interactive SAS emoji comparisons with every peer user.
4. **UIAA Password Availability During First Bootstrap:** Minting a new cross-signing identity on Continuwuity (`/_matrix/client/v3/keys/device_signing/upload`) requires Matrix User-Interactive Authentication (`m.login.password`). `create-instance.sh` already generates `MATRIX_PASSWORD` in `/opt/agent-zero/agent0-N/.env`, so passing a password reference (or file) into `matrix-mcp-server` makes headless first-time bootstrap (`User Story 2, Scenario 2`) fully deterministic.
5. **Day-2 Control Plane (`am-ctl`) & Backup Persistence:** The bind mount `./usr:/a0/usr` already persists `/a0/usr/workdir/matrix-mcp-server/` on the host without Docker Compose changes, but `am-ctl`'s daily backup bundler (`amctl/services/backup.py`) and `am-ctl rebuild --preserve-mhs` must include `crypto-store/` (SQLite DB, `.device-id`, and `0600` `recovery-key`) so rebuilds do not hit `IdentityExistsError` with an unrecoverable identity.

---

## 1. Fleet Architecture & The Sovereign Agent Triplet

### 1.1 The 3-Container Triplet per Agent
Every agent instance `N` (`1..5` deployed on host `g2s` at `172.23.100.121`, scaling up to `99` on `/24` or `999` in `am-ctl` v2) is provisioned as an isolated 3-container Docker Compose stack (`agent-matrix-design.md:99-128`, `theory-of-operations.md:139-145`, `templates/docker-compose.yml.template:1-85`):

| Container | Image | Network & Addressing | Exposed Ports | Role & Supervised Processes |
|---|---|---|---|---|
| `agent0-N` | `agent0ai/agent-zero` | `macvlan-172-23` (`172.23.88.N`) + `bridge-local` | Host `5000N:80`, `5002(N+1):22`; Container `:80` (A0 UI), `:3000` (MCP), `:8080` (Wiki) | Runs `supervisord` managing Agent Zero (`run_ui.py`), `run_mcp` (`matrix-mcp-server-r2`), and `run_bot` (`matrix-bot-rust`). |
| `agent0-N-continuwuity` | `ghcr.io/continuwuity/continuwuity:latest` | `bridge-local` only (isolated from LAN) | `:6167` (HTTP internal CS + Federation) | Per-agent Rust Matrix homeserver (embedded RocksDB, ~20-50 MB RAM) serving `agent0-N-mhs.cybertribe.com`. |
| `agent0-N-mhs` | `caddy:2-alpine` | `macvlan-172-23` (`172.23.89.N`) + `bridge-local` | `:8008` (HTTP CS API), `:8448` (HTTPS Federation with Step-CA TLS) | Reverse proxy terminating internal HTTP (`:8008`) and federated TLS (`:8448`) into `agent0-N-continuwuity:6167`. |

### 1.2 Networking, Federation Hub, and Shared Services
- **LAN & Macvlan Subnets (`theory-of-operations.md:109-159`):**
  - `172.23.88.0/24`: Agent Zero runtime containers (`172.23.88.1` .. `172.23.88.5`), reachable from host `g2s` via the `mac0` shim (`172.23.88.254/32`, where `am-ctl` listens on `:8077`).
  - `172.23.89.0/24`: Caddy Matrix Homeserver proxies (`172.23.89.1` .. `172.23.89.5`).
  - `172.23.90.0/24`: Shared services segment (`172.23.90.1:5432` PostgreSQL 16 + pgvector hosting `openbrain` and `fleet_ops`; `172.23.90.2:3100` `open-brain-mcp`).
- **Public Federation Hub (`theory-of-operations.md:210-333`):**
  - A 3-node Contabo Kubernetes cluster (`matrix.v-site.net` / `v-site.net`) runs Synapse 1.147.1 + Element Web, linked to the home lab edge router (`kama`) via an OpenVPN sidecar (`172.23.200.0/24`).
  - Synapse resolves `agent0-N-mhs.cybertribe.com` -> `172.23.89.N` via pod `hostAliases`, validates Caddy's `:8448` TLS certificates against the CyberTribe Step-CA root + intermediate bundle (`/etc/synapse/certs/home-lab-ca.pem`), and serves as the trusted notary key server (`CONTINUWUITY_TRUSTED_SERVERS: '["v-site.net"]'`) for all agent Continuwuity instances.

### 1.3 Host Volume Layout per Agent
Each instance resides at `/opt/agent-zero/agent0-N/` on `g2s` (`templates/docker-compose.yml.template:30-31, 56-71`):
- `./usr:/a0/usr` (mounted RW into `agent0-N`):
  - `/a0/usr/settings.json` -- Agent Zero settings containing the `mcp_servers` configuration.
  - `/a0/usr/workdir/matrix-bot/` -- `matrix-bot-rust` binary, `.env`, `.a0-api-key` (`0600`), `room_contexts.json`, `sync_cursor.json`.
  - `/a0/usr/workdir/matrix-mcp-server/` -- `matrix-mcp-server-r2` binary, `run-mcp-wrapper.sh`, `.env`, and the target location for the E2EE SQLite crypto store (`crypto-store/`).
- `./mhs/continuwuity-data:/var/lib/continuwuity` (mounted RW into `agent0-N-continuwuity`):
  - RocksDB database storing local room DAGs, user accounts, uploaded device keys, cross-signing keys, Secret Storage (`account_data`), and to-device message queues.

---

## 2. Matrix Client & Identity Topology per Agent

### 2.1 Single Canonical User Principal (`@agent0-N`), Multiple Processes
- Following the July/October 2026 identity drift remediation (`matrix-bot-auth-membership-incident-report.md:20, 79-81, 266-272`), every agent has **one canonical Matrix user ID** on its local Continuwuity server (`@agent0-N:agent0-N-mhs.cybertribe.com`, with `@agent:agent0-1-mhs.cybertribe.com` as legacy alias on `agent0-1`).
- Inside each `agent0-N` container, two supervised Rust binaries connect to `http://agent0-N-mhs:8008` as `@agent0-N`:
  1. **`matrix-bot-rust` (`[program:run_bot]`):** Reactive inbound listener polling `/_matrix/client/v3/sync` (`templates/matrix-bot/rust/src/main.rs:398-415`) and forwarding triggered room messages to Agent Zero's HTTP endpoint `/api/api_message`.
  2. **`matrix-mcp-server-r2` (`[program:run_mcp]`):** Proactive MCP server listening on `http://localhost:3000/mcp` with its own background `matrix-sdk` sync loop (`src/matrix/client.rs:60-74`).
- `matrix-mcp-server-r2` `v0.1.1` is **already the active MCP server on all 5 agents** (`permanent-repair-progress-gates.md:820`).

### 2.2 The Shared Access Token & Device ID Collision
Inspecting `scripts/finalize-instance.sh:160-248` alongside `OpenBrain` operational history (specifically Galadriel's 2026-07-06 token fix on `agent0-1`) reveals a crucial detail for E2EE:
- **What `finalize-instance.sh` does on new instances:** It performs a **single** registration/login call against Continuwuity, captures one `ACCESS_TOKEN`, and writes that **same token** to both `matrix-bot/.env` and `matrix-mcp-server/.env`.
- **What happened on `agent0-1` on 2026-07-06:** When `matrix-mcp-server-r2` experienced token drift, a fresh login was performed specifically for MCP with `device_id: "matrix-mcp-server-device"`, giving `matrix-mcp-server-r2` and `matrix-bot` **separate access tokens and separate device IDs** on `agent0-1`.
- **Why this matters for `002-e2ee-cross-signing` (`T004`/`T005`):**
  - In Matrix, every `access_token` is bound on the homeserver to the exact `device_id` created at `/login` or `/register` time.
  - If `matrix-mcp-server-r2` generates a brand-new local string in `$CRYPTO_STORE_PATH/.device-id` that does **not** match the `device_id` bound to `MATRIX_ACCESS_TOKEN` on Continuwuity, calling `/_matrix/client/v3/keys/upload` will fail because the homeserver rejects key uploads for a `device_id` different from the token's session device!
  - **Design Requirement:** On first startup with an empty `$CRYPTO_STORE_PATH/.device-id`, `matrix-mcp-server-r2` should query `/_matrix/client/v3/account/whoami` using `MATRIX_ACCESS_TOKEN` to discover the token's authoritative server-side `device_id` and persist that into `$CRYPTO_STORE_PATH/.device-id` (matching the `qechris/matrix-mcp` lesson recorded in `.specify/specs/002-e2ee-cross-signing/spec.md:16`: *"device id taken from the session rather than a shared constant"*). In parallel, `finalize-instance.sh` (and `fleet-token-healthcheck.sh` / `am-ctl`) should mint **separate login tokens** for `matrix-bot` (`device_id: "AgentZeroBot"`) and `matrix-mcp-server-r2` (`device_id: "agent0-N-mcp-<suffix>"`).

---

## 3. Continuwuity Homeserver & Federation Characteristics

1. **Server Key Auto-Rotation (`matrix-bot-auth-membership-incident-report.md:634-640`):**
   - Continuwuity rotates its federation signing keys automatically; `am-ctl` (`amctl/services/matrix.py:136-182`) and `federation-key-refresh.sh` query Synapse's notary endpoint `/_matrix/key/v2/query/{fqdn}` every 6 hours so federated room actions and cross-server `/keys/query` calls do not fail with `401 M_UNAUTHORIZED`.
2. **Federated Joins Must Use Room ID (`!roomId:domain`), Never Alias (`#alias:domain`):**
   - Continuwuity returns HTTP 500 when joining federated rooms by alias (`operations-manual.md:969-976`). All `-r2` room tools and tests must operate on canonical `!roomId:domain` identifiers.
3. **Group Rooms First, 1:1 DMs Second (`.specify/specs/002-e2ee-cross-signing/spec.md:61-65, 86, 109`):**
   - Because Conduit/Continuwuity lineages have historically exhibited edge cases with federated to-device key delivery in 1:1 DMs (`isDirect: true`), `002-e2ee-cross-signing` makes **encrypted group rooms** (`create-room-r2` with `encryption_mode: e2ee`) the primary release gate (`SC-002`) and treats 1:1 encrypted DMs as a secondary verification gate.

---

## 4. Communication Planes & The `matrix-bot-rust` E2EE Blind Spot

### 4.1 Where E2EE Fits in the Multi-Plane Architecture
Agent-Matrix separates traffic across three planes (`agent-matrix-design.md:246-261`, `galadriel-agent-design.md:88-108`, `rust-matrix-mcp-server-plan.md:47, 161-162`):
- **Unencrypted Broadcast / Control Rooms (e.g., Fleet HQ Clean `!gahcrWFWAUJQPPkuQy:v-site.net`):** Remain unencrypted so all agents, humans, and `am-ctl` health checks can observe fleet-wide operations using the 20 v1 tools.
- **Encrypted Agent-Internal Scratchpad & Audit Rooms:** Single-agent private rooms created via `create-room-r2` (`encryption_mode: e2ee`) for sensitive reasoning logs, credentials audit trails, and Open Brain memory staging (`agent-matrix-open-brain-design.md:738-740`).
- **Encrypted Task Rooms & Confidential Human-Agent Rooms:** Multi-party or 1:1 rooms created via `create-room-r2` where messages are exchanged via `send-message-r2` and `get-room-messages-r2`.

### 4.2 The Inbound (`matrix-bot-rust`) vs. Outbound (`matrix-mcp-server-r2`) Split
Inspecting `agent-matrix-repo/multi-instance-deploy/templates/matrix-bot/rust/src/main.rs` (`lines 343-415, 887-995`) exposes a critical system boundary:
- `matrix-bot-rust` does **not** link `matrix-sdk`. It uses `reqwest` to poll `/_matrix/client/v3/sync` and explicitly skips any timeline event where `event_type != "m.room.message"` (`main.rs:896`).
- In an encrypted room, incoming messages arrive over `/sync` as `"type": "m.room.encrypted"`.
- **Consequence:**
  - When `002-e2ee-cross-signing` is completed in `matrix-mcp-server-r2`, **all proactive / tool-driven E2EE workflows work immediately** (e.g., Agent Zero or Cursor calling `bootstrap-e2ee-r2`, `create-room-r2`, `send-message-r2`, `get-room-messages-r2`, `get-encryption-status-r2`).
  - Furthermore, because `matrix-bot-rust` never uploads Olm/Megolm device keys to Continuwuity, its non-crypto session (`AgentZeroBot`) **will not block or interfere with** `matrix-mcp-server-r2`'s cross-signing or Megolm key sharing!
  - **However**, `matrix-bot-rust` will not automatically wake Agent Zero on incoming `m.room.encrypted` events until a follow-on bot integration is added. When that follow-on work happens, `matrix-bot-rust` must **not** open `matrix-mcp-server-r2`'s `CRYPTO_STORE_PATH` (which violates the single-process store rule in `spec.md:85`). Instead, `matrix-bot-rust` should either:
    1. Detect `m.room.encrypted` in `/sync` and notify Agent Zero to fetch/decrypt and reply using `get-room-messages-r2` and `send-message-r2` via MCP, **or**
    2. Move the reactive `/sync` -> `/api/api_message` webhook dispatcher directly into `matrix-mcp-server-r2` so a single `matrix-sdk` 0.19 client and single SQLite crypto store per agent handles both inbound triggers and MCP tools.

---

## 5. Implications of Losing TOFU & Enforcing Strict Cross-Signing

Dropping `TOFU` (`ignore_unverified_devices`) in favor of strict `cross-signed` trust (`spec.md:10-20, 50-60, 93-95`) has four concrete consequences for `matrix-mcp-server-r2` design decisions:

### 5.1 Recipient Verification Strategy in `matrix-sdk` 0.19 (`T008`)
- In Matrix Cross-Signing, each user account (`@agent0-N:...` or `@l0r3zz:v-site.net`) has a 3-key hierarchy in Secret Storage: **Master Key (MSK)** -> **Self-Signing Key (SSK)** (signs that user's own devices) + **User-Signing Key (USK)** (signs *other* users' Master Keys after interactive SAS verification).
- When `@agent0-1` sends an encrypted message (`send-message-r2`) in a room containing `@agent0-2` and `@l0r3zz:v-site.net`:
  - Each participant's device has been signed by **its own owner's Self-Signing Key** (`device.is_cross_signed_by_owner()`).
  - However, `@agent0-1` has **not** performed an interactive emoji SAS verification with `@agent0-2` or `@l0r3zz` to sign their Master Keys with `@agent0-1`'s `User-Signing Key`.
- **Actionable Design Rule for `T008`:**
  - When configuring `matrix-sdk` 0.19's `CollectStrategy` / recipient filtering in `src/matrix/client.rs` and `send-message-r2`, verify how `matrix-sdk` 0.19 evaluates recipient devices across users:
    - A recipient device qualifies as **cross-signed** when it has a valid cross-signing signature chain from its owner's Self-Signing / Master Key (`device.is_cross_signed_by_owner()`, matching MSC4153 and `spec/E2EE-Landscape-Review-October-20261023.md:67-72`: *"Three published facts have to agree... the account's cross-signing public keys, this device's Ed25519 and Curve25519 keys... and a self-signing signature on that device"*), while any device **without** a valid owner cross-signing signature (an unverified login session) is withheld (`m.room_key.withheld` / `m.unverified`).
    - Conversely, if `CollectStrategy::IdentityBasedStrategy` in `matrix-sdk` 0.19 requires the *sender* (`@agent0-1`) to have pinned/signed the *recipient user's* Master Key (`UserIdentity::is_verified()`), then either `matrix-mcp-server-r2` must use a device filter that checks `device.is_cross_signed_by_owner()` (or auto-signs peer Master Keys for known fleet domains `*.cybertribe.com` / `v-site.net` during room encryption), or provide an administrative user-verification mechanism. Testing this exact `matrix-sdk` 0.19 behavior during `T008` is top priority.

### 5.2 First-Time Cross-Signing Bootstrap & UIAA Password (`T006` / `T007`)
- When `bootstrap-e2ee-r2` runs on an agent account that has **no** existing cross-signing keys on Continuwuity (`User Story 2, Scenario 2`), `encryption().bootstrap_cross_signing(...)` uploads master/self/user signing keys to `/_matrix/client/v3/keys/device_signing/upload`.
- Matrix homeservers (including Continuwuity) require **User-Interactive Authentication (`m.login.password`)** for `/keys/device_signing/upload`.
- Because `scripts/create-instance.sh:133` already generates `MATRIX_PASSWORD` in `/opt/agent-zero/agent0-N/.env`, `matrix-mcp-server-r2` should support an optional password env var or file reference (never returned in tool output or logged, per `spec.md:19`) solely to satisfy UIAA when initializing cross-signing on a virgin account.

### 5.3 Token Refresh Scripts Must Never Invalidate the Cross-Signed Device
- Look at `fleet-token-healthcheck.sh` (captured in OpenBrain thought `aabf1e3b`): if `/whoami` ever returns `401`, the script calls `/login` to mint a new token.
- **Critical Operational Invariant:** Once `matrix-mcp-server-r2` is cross-signed under `$CRYPTO_STORE_PATH/.device-id`, any re-login or token refresh for `matrix-mcp-server-r2` **must pass the existing `device_id` from `$CRYPTO_STORE_PATH/.device-id`** (or avoid re-logging in unless the store is also rotated), because logging in with a new `device_id` while keeping the old SQLite crypto store creates a device-ID/key mismatch!

### 5.4 `am-ctl` Backup, Rebuild (`--preserve-mhs`), and Health Integration
1. **Backups (`amctl/services/backup.py:94-106`):** Add `agent0-{i}/usr/workdir/matrix-mcp-server/crypto-store` (containing the SQLite DB, `.device-id`, and `0600` `recovery-key`) to `_bundle_infra_configs()`.
2. **Rebuilds (`am-ctl rebuild --preserve-mhs`):** When rebuilding an agent container while preserving `mhs/continuwuity-data`, the agent's cross-signing identity remains on Continuwuity. Therefore, `--preserve-mhs` must preserve `crypto-store/` (or at minimum `recovery-key`) so `bootstrap-e2ee-r2` can unlock Secret Storage via `MATRIX_RECOVERY_KEY_FILE` (`User Story 2, Scenario 1`) instead of failing with `IdentityExistsError` (`Scenario 3`).
3. **Health Endpoint (`FR-009`):** Extending `GET /health` on `matrix-mcp-server-r2` to report `"e2ee": true` and `"crossSigned": true` allows `am-ctl`'s 8-check health monitor (`amctl/services/health.py`) to continuously verify fleet E2EE readiness without invoking MCP tools.
