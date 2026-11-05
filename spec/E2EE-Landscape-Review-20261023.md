# Review: E2EE for matrix-mcp-server-r2

**Date:** 2026-10-23
**Status:** Decisions applied 2026-10-23. Canonical spec is `.specify/specs/002-e2ee-cross-signing/spec.md`. Trust policy is cross-signed. SDK target is matrix-sdk 0.19.x. The "left unchanged" note at the bottom is historical.
**Scope:** Re-check of the April-May 2026 research, Cursor rule, spec-kit spec, and key-management notes, against the repo as it stands and the public Matrix / MCP landscape on 2026-10-22.
**Original question:** Are there Matrix MCP servers that already do E2EE, and are there AI tools that are particularly good at Rust or cryptography, so we can design v2 E2EE for this server?

## Summary

Phase 3 has not moved past design. The server still builds on `matrix-sdk` 0.10 with a hardcoded device id `MCP_SERVER`, and the `e2ee` Cargo feature is unused in source. The May claim that no Matrix MCP server implements E2EE is no longer true: several Rust, TypeScript, and Python servers now encrypt and decrypt with a persistent store. The SDK line we pinned is eight minor versions behind (`0.19.1`, MSRV Rust 1.96), and Matrix spec v1.18 (2026-03-26) tells clients not to share room keys with devices that are not cross-signed. Trust-on-first-use, which the key-management note treats as the default, will fail against clients that follow that recommendation. The next planning step is a version decision, then a rewrite of the May spec, not an implementation pass on the May spec as written.

## Where the repo actually is

| Item | May 2026 assumption | Repo on 2026-10-22 |
|------|---------------------|--------------------|
| Roadmap | Start of Week 4 / Phase 3 design | README says Phase 3 "Started", Phase 4 not started |
| Crate | `matrix-sdk` 0.10, `rmcp` 1.2, MSRV 1.88 | Unchanged in `Cargo.toml` (`version` is `0.1.1`) |
| E2EE code | Not started | Still not started. `e2ee_enabled` and `crypto_store_path` exist in `src/config.rs` only |
| Device identity | One stable device per agent | `src/matrix/client.rs` always restores `device_id: "MCP_SERVER"` |
| Spec-kit | Constitution + feature spec | `.specify/memory/constitution.md` and `.specify/specs/001-e2ee-support/spec.md` exist. No `plan.md`, `research.md`, `data-model.md`, or `tasks.md` |
| Later design note | Not in the May spec | `spec/E2EE-Key-Management-Model.md` locks default trust to `tofu` |

The `e2ee` feature flag still only enables `matrix-sdk/e2e-encryption`. Nothing calls a crypto store, cross-signing, or a `-r2` tool.

## What changed in the landscape

### Finding 1: Other Matrix MCP servers now do E2EE

The May research said no Matrix MCP server, in any language, implemented E2EE. That was fair in April 2026. It is not fair now.

| Project | Language | What it already does | Why it matters here |
|---------|----------|----------------------|---------------------|
| [qechris/matrix-mcp](https://github.com/qechris/matrix-mcp) | Rust | `rmcp` + `matrix-sdk`, rustls, bundled SQLite, automatic encrypt/decrypt, `MATRIX_STORE_PATH`. No interactive verification or cross-signing. Undecryptable events come back as `unable_to_decrypt: true`. | Closest architecture to this repo. Read this before writing new crypto code. |
| [Just-Insane/matrix-mcp-server](https://github.com/Just-Insane/matrix-mcp-server) | TypeScript | Persistent `matrix-sdk-crypto` state, optional `MATRIX_RECOVERY_KEY` / Secret Storage restore, OAuth. Documents a Dendrite failure mode: encrypted DMs can fail because of device-key and to-device delivery; encrypted group rooms are fine. Synapse is the supported path. | Recovery-key and SSSS pattern, plus a homeserver caveat. This fleet uses Continuwuity, not the Dendrite named in the May spec. |
| [elkimek/matrix-bridge](https://github.com/elkimek/matrix-bridge) | Rust | Five stdio MCP tools, vodozemac, TOFU, static binaries. | Small tool surface and an explicit bot trust policy. |
| [elkimek/matrix-e2ee-bridge](https://github.com/elkimek/matrix-e2ee-bridge) | Python (`matrix-nio`) | Same five tools, store under `~/.matrix-bridge/`, `ignore_unverified_devices`. | Shows the same bot tradeoff in another stack. |
| [jlxq0/matrix-mcp](https://github.com/jlxq0/matrix-mcp) | Rust (remote MCP) | OAuth 2.1, cross-signing, Secret Storage recovery key, history decrypts because `/setup` cross-signs the device. | The pattern that matches spec v1.18, rather than TOFU. |

**Relevance:** v2 is no longer a blank sheet. The useful question is which of these trust models this fleet can live with, not whether E2EE-in-MCP is possible.

### Finding 2: `matrix-sdk` 0.10 is not a viable base for new E2EE work

Current crate, checked 2026-10-22: **`matrix-sdk` 0.19.1** (published 2026-09-18). Default features are `e2e-encryption`, `automatic-room-key-forwarding`, `sqlite`, and `rustls-aws-lc-rs`. MSRV is **Rust 1.96**.

Selected breaks between 0.10 and 0.19 that touch this design:

- **0.16.0 (2025-12-04)** removed the `matrix-sdk-crypto` re-export. Code that reached into that re-export has to change.
- **0.16.1 (2026-05-08)** fixes [GHSA-wfq4-36m3-9g42 / CVE-2026-45056](https://github.com/matrix-org/matrix-rust-sdk/pull/6553): `matrix-sdk-crypto` before 0.16.1 did not check the sender user id on Olm to-device events that carry `sender_device_keys`. A homeserver operator could spoof the sender. This repo's 0.10 line is inside that affected range. There is no workaround. E2EE should not ship on 0.10.
- Current clients set trust and recipient strategy on the builder (`with_decryption_settings`, `with_room_key_recipient_strategy`), not by hand-rolling an `OlmMachine` next to a plain `Client`.
- `StoreConfig` includes a crypto store whenever `e2e-encryption` is on. `sqlite_store` examples in the May `.mdc` rule are from the 0.10-era API.

**Relevance:** Enabling `--features e2ee` on the current crate is the wrong first task. The first task is a planned bump of `matrix-sdk` (and the Rust toolchain to at least 1.96), kept separate from the `rmcp` bump below.

### Finding 3: Spec v1.18 changes the trust model the May docs assume

Matrix v1.18 shipped 2026-03-26. Under [MSC4153 / "invisible crypto"](https://spec.matrix.org/latest/client-server-api/#end-to-end-encryption), clients SHOULD NOT send room keys or other encrypted to-device secrets to devices that are not cross-signed, and SHOULD NOT display messages from those devices. The withholding signal is `m.room_key.withheld` with code `m.unverified`.

Element's default exclusion of those devices was planned for April 2026 and, as of the public security notes checked for this review, is postponed. Element still sends keys to unverified devices unless a Labs flag is on. That delay is why TOFU bots still work today. It is not a promise that they will keep working.

The May spec and `.cursor/rules/rust-e2ee.mdc` treat `TrustRequirement::Untrusted` as acceptable for agent-internal rooms. `spec/E2EE-Key-Management-Model.md` locks the v2 default to `tofu` and defers `strict`. Both need a revision before implementation:

- A device that never cross-signs will not receive room keys from a client that follows v1.18.
- Headless bootstrap has to include cross-signing and Secret Storage (or an explicit, documented decision that this fleet only talks to clients that still share keys with unverified devices).
- `jlxq0/matrix-mcp` and `Just-Insane/matrix-mcp-server` already do recovery-key restore. That is the reference for the second path.

### Finding 4: vodozemac moved, and the February 2026 3DH note was handled upstream

| | May research | 2026-10-22 |
|--|----------------|------------|
| Version | 0.9.0 (Jan 2025) | **0.11.1** (2026-09-30), MSRV 1.89 |
| 0.10.0 (2026-04-13) | Not yet written up | `Curve25519SecretKey::diffie_hellman()` returns `Option`. `Account::create_outbound_session()` and `Session::encrypt()` are fallible. Strict Ed25519 checking is always on. This is the upstream response to the 3DH concern Soatok raised. |
| 0.11.0 | -- | HPKE (X25519 + ChaCha20-Poly1305), used for MSC4388 QR login check codes. Not required for room messaging. |

This server should not depend on `vodozemac` directly. It arrives through `matrix-sdk`. The May security paragraph that "Matrix.org disputes practical exploitability" should be replaced with: use a `matrix-sdk` that vendors vodozemac >= 0.10, which is true of the 0.19 line and not of 0.10.

### Finding 5: `rmcp` jumped from 1.2 to 3.7.0

`rmcp` 3.7.0 was published 2026-10-22. The 3.x line implements MCP `2026-07-28` and has a migration guide. MSRV is 1.88, which this repo already declares, but the API break is real (`#[tool]` macros, transport types, auth).

**Relevance:** Do not fold an `rmcp` 3 migration into the E2EE feature. qechris/matrix-mcp shows a current `rmcp` + `matrix-sdk` pairing if a later upgrade wants a template. E2EE planning should treat the MCP transport as stable and only change Matrix client construction, store, and tool behavior.

### Finding 6: MLS is still not the protocol to implement

MSC4244 (MLS shaped like today's Matrix) is still a draft, last touched February 2025, label `needs-implementation`. MSC4256 (MLS as the state model) is still open, with experimental Synapse and client work from BWI, and was discussed at the Matrix Conference on 2026-10-22. Neither is the Client-Server API this server speaks.

**Relevance:** The May advice still holds. Keep the MCP tool API free of Olm-specific types so a later protocol can sit behind `matrix-sdk`. Do not design v2 around MLS.

### Finding 7: Homeserver assumption is stale

The May spec says "Agent-Matrix Dendrite homeservers support E2EE". The fleet homeserver is Continuwuity. `Just-Insane/matrix-mcp-server` reports that some Dendrite deployments fail Megolm key sharing for encrypted DMs while group rooms work. Continuwuity's to-device and device-key behavior has to be checked on a real agent instance before the spec promises DM decrypt. Group-room encrypt/decrypt is the safer first acceptance test.

### Finding 8: The hardcoded device id conflicts with the key model

`spec/E2EE-Key-Management-Model.md` says one device id per agent instance and "never reuse one device ID across multiple agents". `create_matrix_client` in `src/matrix/client.rs` sets `OwnedDeviceId::from("MCP_SERVER")` for every process. Two agents with that id will collide on the same account, and a rebuilt container that keeps the id but loses the store will look like the same device with new keys. Bootstrap work has to make the device id stable per instance and stored beside the crypto store, not a string literal.

### Finding 9: AI assistants for Rust and crypto

The April survey (Corust for Zed, plus Rust-native agents Omegon, Ryvos, and Koda) was not re-tested in this pass. None of those tools is a substitute for the reference servers above. The practical coding aid that aged well is the project rule `.cursor/rules/rust-e2ee.mdc`, and that rule now contains 0.10-era builder snippets (`sqlite_store`, manual `OlmMachine` locking) that will mislead an agent on 0.19. Update the rule after the version decision, not before.

`specify` CLI 0.0.22 / template 0.8.8 was installed in May. `specify init` could not download GitHub release assets from this environment, so `.specify/` was written by hand. The templates are shortened copies. That is enough to hold a spec. It is not a full Spec Kit install (no slash-command prompts, no `plan.md` yet).

## What to keep from the May artifacts

These points are still right:

- Do not implement Olm or Megolm. Call `matrix-sdk`.
- Gate behavior so a build without the crypto stack still serves the 20 v1 tools.
- v1 tools must not silently drop plaintext into an encrypted room. The `EncryptedRoomError` string in `spec/matrix-mcp-v1-v2.yml` is still the right contract.
- Persist the crypto store on a volume. Losing it means old history is undecryptable.
- Never log keys, passphrases, or decrypted bodies.
- The four-step exchange (keys/upload, keys/query, keys/claim, then `m.room_key` over Olm) is still how Megolm sessions start. The SDK performs it during sync and send.
- Spec Kit order is still constitution, spec, plan, tasks, implement. Plan and tasks for this decision are in `.specify/specs/002-e2ee-cross-signing/`.

## What to revise before any implementation plan

Decisions recorded 2026-10-23. Canonical spec: `.specify/specs/002-e2ee-cross-signing/spec.md`.

1. **Toolchain.** Done in comments and docs: `matrix-sdk` 0.19.x, Rust >= 1.96, recorded in `Cargo.toml`, `.specify/memory/constitution.md`, and `.cursor/rules/rust-e2ee.mdc`. The compiling pin stays 0.10 / 1.88 until task T001, because the current client code does not build against 0.19. `rmcp` stays at 1.2.
2. **Trust.** Done. Policy is cross-signed only. TOFU is not supported. Room keys are not shared with unverified devices. `spec/E2EE-Key-Management-Model.md` no longer defaults to `tofu`.
3. **Device id.** Specified: one id per agent in `$CRYPTO_STORE_PATH/.device-id`. Deleting the `MCP_SERVER` literal is task T002, with the client migration, not a docs-only edit.
4. **Homeserver.** Done in the October spec. First gate is a Continuwuity encrypted group room, restart, still decrypts. DM is the second test.
5. **Prior art.** Done. qechris and jlxq0 are in the October spec. The May "no E2EE MCP reference implementation" line in the research note was replaced.
6. **Security.** Done. Any E2EE build must be on `matrix-sdk-crypto` >= 0.16.1. The 0.19 line satisfies that.

## Suggested order from here

1. Trust policy and SDK version are decided: cross-signing, `matrix-sdk` 0.19.x, Rust >= 1.96.
2. May spec `001-e2ee-support` is marked superseded. The October spec is `002-e2ee-cross-signing`.
3. `plan.md`, `data-model.md`, and `tasks.md` are written next to that spec.
4. Implementation starts at T001 (client construction and store path). New MCP tool schemas come after the 0.19 client builds.

## Sources

- `matrix-sdk` 0.19.1: https://crates.io/crates/matrix-sdk
- `matrix-sdk` changelog (0.16.0 re-export removal, later crypto APIs): https://github.com/matrix-org/matrix-rust-sdk/blob/main/crates/matrix-sdk/CHANGELOG.md
- CVE-2026-45056 / GHSA-wfq4-36m3-9g42, fixed in `matrix-sdk-crypto` 0.16.1: https://github.com/matrix-org/matrix-rust-sdk/pull/6553
- Matrix v1.18 and recommended client behaviour: https://spec.matrix.org/latest/client-server-api/#end-to-end-encryption
- vodozemac 0.11.1: https://crates.io/crates/vodozemac
- `rmcp` 3.7.0: https://crates.io/crates/rmcp
- MSC4244 (still draft): https://github.com/matrix-org/matrix-spec-proposals/pull/4244
- MSC4256 (experimental MLS mode): https://github.com/matrix-org/matrix-spec-proposals/pull/4256
- qechris/matrix-mcp: https://github.com/qechris/matrix-mcp
- Just-Insane/matrix-mcp-server: https://github.com/Just-Insane/matrix-mcp-server
- elkimek/matrix-bridge: https://github.com/elkimek/matrix-bridge
- jlxq0/matrix-mcp: https://github.com/jlxq0/matrix-mcp

## Closeout

The May research note, `.cursor/rules/rust-e2ee.mdc`, the constitution, `001-e2ee-support/spec.md`, and `E2EE-Key-Management-Model.md` were updated on 2026-10-23 to point at `.specify/specs/002-e2ee-cross-signing/`. The body of this review still describes what was true when it was written. The status line at the top is the current briefing. Implementation has not started.
