# Review: E2EE for matrix-mcp-server-r2 (October design)

**Date:** 2026-10-23
**Status:** Current briefing. Decisions below are locked. Implementation has not started.
**Supersedes for planning:** `spec/E2EE-Landscape-Review-20261023.md`. That file is the May-era review and is left unchanged. Where the two disagree, this file wins.
**Canonical spec:** `.specify/specs/002-e2ee-cross-signing/spec.md` (plan, data model, and tasks sit beside it). The May spec `.specify/specs/001-e2ee-support/spec.md` is superseded.
**Role:** `matrix-mcp-server-r2` is the primary Matrix MCP for the Agent-Matrix fleet. The fleet is a secure network, not the public Matrix network.

## Locked decisions

| Decision | Choice |
|----------|--------|
| Trust policy | Cross-signing only. TOFU is not supported |
| Room keys | Do not share room keys, or other encrypted to-device secrets, with a device that is not cross-signed. Withhold with `m.room_key.withheld` / `m.unverified` |
| SDK | `matrix-sdk` 0.19.x (or newer), features `e2e-encryption` and `bundled-sqlite` |
| Crypto floor | `matrix-sdk-crypto` >= 0.16.1 (CVE-2026-45056). The 0.19 line includes this |
| Rust | >= 1.96 |
| MCP transport | `rmcp` stays at 1.2. The 3.x migration is a different project |
| Homeserver | Continuwuity. First acceptance test is an encrypted group room, restart, still decrypts. A DM is the second test |
| Device id | One id per agent, in `$CRYPTO_STORE_PATH/.device-id`. The literal `MCP_SERVER` is not allowed |
| Recovery secret | Secret-manager reference, then env-var name, then `MATRIX_RECOVERY_KEY_FILE`. Never a raw secret in a tool argument, log line, or tool result |
| Identity reset | If the account already has cross-signing and no recovery secret resolves, fail. Do not create a second identity |
| Protocol | Olm + Megolm through `matrix-sdk`. Do not implement the ratchets. Do not design v2 around MLS |

The compiling tree is still `matrix-sdk` 0.10 and Rust 1.88. That pin stays until task T001 in `.specify/specs/002-e2ee-cross-signing/tasks.md`, because the current client does not build against 0.19. Comments in `Cargo.toml` record the target. Flipping the version string alone would leave the tree uncompilable.

## Where the repo is

| Item | October design | Code on 2026-10-23 |
|------|----------------|--------------------|
| Spec | `.specify/specs/002-e2ee-cross-signing/` (`spec.md`, `plan.md`, `data-model.md`, `tasks.md`) | May spec kept, marked superseded |
| Trust | Cross-signed only. Recorded in the October spec, `spec/E2EE-Key-Management-Model.md`, `spec/matrix-mcp-v1-v2.yml`, and the constitution | No crypto store, no cross-signing, no `-r2` tools |
| SDK | 0.19.x, Rust >= 1.96 | `Cargo.toml` still pins 0.10 / Rust 1.88 so `cargo build` works |
| Device id | File beside the store | `src/matrix/client.rs` still restores `device_id: "MCP_SERVER"` for every process. Removal is task T002 |
| Feature flag | `e2ee` gates the crypto build. A build without it still serves the 20 v1 tools | The flag only enables `matrix-sdk/e2e-encryption`. Nothing calls it |
| Homeserver | Continuwuity group room first | Not yet tested |

`src/config.rs` already has `E2EE_ENABLED` and `CRYPTO_STORE_PATH`. Those names stay. Bootstrap behavior around them does not exist yet.

## What the other E2EE MCP servers contribute

Several servers now encrypt and decrypt. The May claim that no reference implementation existed is withdrawn. The question that remains is which pieces match a secure fleet.

| Project | Take | Reject |
|---------|------|--------|
| [qechris/matrix-mcp](https://github.com/qechris/matrix-mcp) | Closest architecture: Rust, `matrix-sdk`, rustls, bundled SQLite, `MATRIX_STORE_PATH`, one store per device, `unable_to_decrypt` instead of a crash, `restore_key_backup`. Device id comes from the session, not a shared constant | It will send and receive before the device is verified. Interactive SAS is a human-operator path, not agent bootstrap. `logout` deletes the store. Do not copy either |
| [jlxq0/matrix-mcp](https://github.com/jlxq0/matrix-mcp) | Cross-signing is required before peers share Megolm keys. Device id lives in a file next to the store and rotates when the volume is wiped. `bootstrap_cross_signing` creates an identity only when none exists. One client per device id. Setup runs after the device-bound client exists | Synapse + MAS is a prerequisite there. This fleet does not require MAS. Do not copy that gate |
| [Just-Insane/matrix-mcp-server](https://github.com/Just-Insane/matrix-mcp-server) | Absolute crypto directory, `MATRIX_RECOVERY_KEY` / `MATRIX_RECOVERY_KEY_FILE`, restore key backup, write a newly created recovery key to a mode-0600 file, password only to create or repair Secret Storage | A password or recovery key in a tool result would land in the agent transcript. Do not return it |
| [elkimek/matrix-bridge](https://github.com/elkimek/matrix-bridge) and [elkimek/matrix-e2ee-bridge](https://github.com/elkimek/matrix-e2ee-bridge) | A lost store cannot decrypt old sessions | Default trust is TOFU / `ignore_unverified_devices`. Out of policy. The Python bridge also uses libolm, which was deprecated in August 2024 |

## SDK facts that still govern implementation

Checked 2026-10-22. Current crate: **`matrix-sdk` 0.19.1** (published 2026-09-18). Default features include `e2e-encryption`, `automatic-room-key-forwarding`, `sqlite`, and `rustls-aws-lc-rs`. MSRV is **Rust 1.96**. This project turns default features off and opts into `e2e-encryption` plus `bundled-sqlite`, so the container image does not depend on a system SQLite.

Breaks between 0.10 and 0.19 that this design has to absorb in T001 and T002:

- **0.16.0** removed the `matrix-sdk-crypto` re-export. Do not copy 0.10 examples that reach through it.
- **0.16.1 (2026-05-08)** fixes [GHSA-wfq4-36m3-9g42 / CVE-2026-45056](https://github.com/matrix-org/matrix-rust-sdk/pull/6553). Before that release, Olm to-device events carrying `sender_device_keys` did not check the sender user id. A homeserver operator could spoof the sender. The 0.10 pin is inside the affected range. There is no workaround. Do not ship E2EE on 0.10.
- Trust and recipient strategy are builder settings (`with_decryption_settings`, `with_room_key_recipient_strategy`). Do not construct a standalone `OlmMachine` beside `Client`.
- `vodozemac` 0.11.1 (2026-09-30) arrives through `matrix-sdk`. Do not depend on it directly. 0.10.0 made `diffie_hellman()` return `Option` (the upstream response to the 3DH note). The 0.19 line vendors vodozemac >= 0.10. The 0.10 line does not.

`rmcp` 3.7.0 (published 2026-10-22) implements MCP `2026-07-28` and breaks the 1.2 API. Leave it. qechris is a later template if that migration happens on its own.

MLS is not this protocol. [MSC4244](https://github.com/matrix-org/matrix-spec-proposals/pull/4244) is still a draft. [MSC4256](https://github.com/matrix-org/matrix-spec-proposals/pull/4256) is still experimental. Keep the MCP tool API free of Olm-specific types so a later protocol can sit behind `matrix-sdk`.

## Trust, as the spec states it

Matrix spec v1.18 (2026-03-26) recommends that clients not send room keys or other encrypted to-device secrets to devices that are not cross-signed, and not display messages from those devices. The signal is `m.room_key.withheld` with code `m.unverified`. The proposal is [MSC4153](https://github.com/matrix-org/matrix-spec-proposals/blob/main/proposals/4153-invisible-crypto.md).

Element's default exclusion of those devices was postponed. Until that ships, Element still sends keys to unverified devices unless a Labs flag is on. That delay is why TOFU bots still interoperate with Element today. This project does not depend on that delay. A device that never cross-signs will not receive room keys from this server.

Three published facts have to agree before another client will share a Megolm key with this device: the account's cross-signing public keys, this device's Ed25519 and Curve25519 keys uploaded under the advertised device id, and a self-signing signature on that device. Bootstrap order is: open the store, resolve the device id, restore the session with that id, upload keys, then import or create cross-signing. Signing a device the homeserver has never registered produces a signature for a device that does not exist.

## External documents

These are the Matrix protocol and ecosystem documents. They are not project specs. Read them to understand how encryption works. Implementation calls `matrix-sdk`. It does not reimplement the ratchets.

### The protocol contract

The end-to-end encryption module is the contract for device keys, the `keys/upload` then `keys/query` then `keys/claim` handshake, `m.room_key`, Megolm room events, cross-signing, and server-side key backup.

- Readable source: [end_to_end_encryption.md](https://github.com/matrix-org/matrix-spec/blob/main/content/client-server-api/modules/end_to_end_encryption.md)
- Rendered spec: [Client-Server API: End-to-end encryption](https://spec.matrix.org/latest/client-server-api/#end-to-end-encryption)
- v1.18 changelog entry for the trust recommendation: [v1.18 changelog](https://spec.matrix.org/v1.18/changelog/v1.18/)
- Proposal text: [MSC4153](https://github.com/matrix-org/matrix-spec-proposals/blob/main/proposals/4153-invisible-crypto.md)

Sections inside that module that decide this design:

- **Cross-signing.** Three Ed25519 keys (master, self-signing, user-signing). A device is trusted when the owner's self-signing key has signed it.
- **Server-side key backups.** Encrypted Megolm sessions stored on the homeserver. The spec name for the secret that decrypts them is the backup decryption key (`m.megolm_backup.v1`).
- **Recommended client behaviour.** Do not send room keys to a device that is not cross-signed.

Secret Storage (SSSS, also called 4S) holds the cross-signing private keys and the backup decryption key, locked by one default key (`m.secret_storage.default_key`). It is the [Secret Storage](https://spec.matrix.org/latest/client-server-api/#secret-storage) module.

Naming trap: the spec stopped calling the backup decryption key a "recovery key", because Element and most other clients use "recovery key" for the Secret Storage key. Those are two secrets. This project's `MATRIX_RECOVERY_KEY` / `MATRIX_RECOVERY_KEY_FILE` means the Secret Storage key, in the Element sense. Unlocking it is what reveals the backup decryption key.

[Matrix.org's implementation guide](https://matrix.org/docs/matrix-concepts/end-to-end-encryption/) (last updated 2023-02-08) is the short walkthrough of creating a Megolm session, sharing the inbound key, and encrypting the event. It still lists libolm as a usable library, and it predates the v1.18 trust rule. Use it for the message flow. Use the spec for trust.

### The ratchets

Olm is the 1:1 double ratchet. Megolm is the group ratchet. The inbound Megolm session is what gets shared as a room key.

- [olm.md](https://gitlab.matrix.org/matrix-org/olm/-/blob/master/docs/olm.md)
- [megolm.md](https://gitlab.matrix.org/matrix-org/olm/-/blob/master/docs/megolm.md)

The implementation in current clients is [vodozemac](https://github.com/matrix-org/vodozemac). libolm was deprecated in August 2024.

- Crate overview: [vodozemac](https://matrix-org.github.io/vodozemac/vodozemac/index.html)
- Account and session model: [vodozemac::olm](https://matrix-org.github.io/vodozemac/vodozemac/olm/index.html)
- Independent review: [Trail of Bits audit, May 2022](https://matrix.org/blog/2022/05/16/independent-public-audit-of-vodozemac-a-native-rust-implementation-of-matrix-end-to-end-encryption)

### How a Rust client calls this

[matrix_sdk::encryption](https://docs.rs/matrix-sdk/latest/matrix_sdk/encryption/index.html) matches `matrix-sdk` 0.19. A device is an "end". A room key is shared once and then reused. The room's `m.room.encryption` state decides rotation. The default is 100 messages or one week, whichever comes first. Feature flags are on the [crate page](https://docs.rs/matrix-sdk/latest/matrix_sdk/).

[matrix-sdk-crypto](https://docs.rs/matrix-sdk-crypto/latest/matrix_sdk_crypto/) is the layer `Client` drives. Its tutorial builds an `OlmMachine` by hand. That is the inside of the SDK. An application that already has a `Client` does not construct a second machine beside it. The strategy that shares keys only with cross-signed devices is [share_strategy.rs](https://matrix-org.github.io/matrix-rust-sdk/src/matrix_sdk_crypto/session_manager/group_sessions/share_strategy.rs.html).

Two version documents matter before writing against 0.19:

- [matrix-sdk changelog](https://github.com/matrix-org/matrix-rust-sdk/blob/main/crates/matrix-sdk/CHANGELOG.md). The 0.16 line removed the `matrix-sdk-crypto` re-export.
- [GHSA-wfq4-36m3-9g42](https://github.com/matrix-org/matrix-rust-sdk/pull/6553) (CVE-2026-45056), fixed in `matrix-sdk-crypto` 0.16.1.

### Not the system to implement

[MSC4244](https://github.com/matrix-org/matrix-spec-proposals/pull/4244) and [MSC4256](https://github.com/matrix-org/matrix-spec-proposals/pull/4256) are the MLS proposals. They are still drafts. Current rooms are Olm plus Megolm.

## What stays from the May artifacts

- Do not implement Olm or Megolm. Call `matrix-sdk`.
- A build without the crypto stack still serves the 20 v1 tools.
- v1 tools on an encrypted room return `room is encrypted; use *-r2 tools instead`.
- The crypto store is a volume. Losing it means sessions that were not in key backup cannot be decrypted.
- Never log keys, passphrases, or decrypted bodies.
- The four-step exchange (keys/upload, keys/query, keys/claim, then `m.room_key` over Olm) is still how a Megolm session starts. The SDK performs it during sync and send.

## What happens next

Design for this decision is written. Code is not.

1. T001 bumps `matrix-sdk` to 0.19 and `rust-version` to 1.96 together with the client call sites. `rmcp` stays at 1.2.
2. T002 removes the `MCP_SERVER` device id. The id comes from the store file, not a constant.
3. Store, cross-signing bootstrap, and recipient strategy come before new `-r2` tool schemas.
4. The Continuwuity group-room gate is last: encrypt, round-trip with Element, restart, history still decrypts. Record the DM result separately. A DM failure does not relax the trust policy.

## Sources

- October spec: `.specify/specs/002-e2ee-cross-signing/spec.md`
- Key model: `spec/E2EE-Key-Management-Model.md`
- Tool contract: `spec/matrix-mcp-v1-v2.yml`
- Earlier review, left unchanged: `spec/E2EE-Landscape-Review-20261023.md`
- `matrix-sdk` 0.19.1: https://crates.io/crates/matrix-sdk
- `matrix-sdk` changelog: https://github.com/matrix-org/matrix-rust-sdk/blob/main/crates/matrix-sdk/CHANGELOG.md
- CVE-2026-45056 / GHSA-wfq4-36m3-9g42: https://github.com/matrix-org/matrix-rust-sdk/pull/6553
- Matrix E2EE module (source): https://github.com/matrix-org/matrix-spec/blob/main/content/client-server-api/modules/end_to_end_encryption.md
- Matrix E2EE module (rendered): https://spec.matrix.org/latest/client-server-api/#end-to-end-encryption
- Secret Storage: https://spec.matrix.org/latest/client-server-api/#secret-storage
- Spec v1.18 changelog: https://spec.matrix.org/v1.18/changelog/v1.18/
- MSC4153: https://github.com/matrix-org/matrix-spec-proposals/blob/main/proposals/4153-invisible-crypto.md
- Olm: https://gitlab.matrix.org/matrix-org/olm/-/blob/master/docs/olm.md
- Megolm: https://gitlab.matrix.org/matrix-org/olm/-/blob/master/docs/megolm.md
- vodozemac: https://github.com/matrix-org/vodozemac
- Trail of Bits audit (2022): https://matrix.org/blog/2022/05/16/independent-public-audit-of-vodozemac-a-native-rust-implementation-of-matrix-end-to-end-encryption
- matrix-sdk encryption guide: https://docs.rs/matrix-sdk/latest/matrix_sdk/encryption/index.html
- Room-key share strategy: https://matrix-org.github.io/matrix-rust-sdk/src/matrix_sdk_crypto/session_manager/group_sessions/share_strategy.rs.html
- Matrix.org implementation guide (2023, predates v1.18): https://matrix.org/docs/matrix-concepts/end-to-end-encryption/
- MSC4244: https://github.com/matrix-org/matrix-spec-proposals/pull/4244
- MSC4256: https://github.com/matrix-org/matrix-spec-proposals/pull/4256
- qechris/matrix-mcp: https://github.com/qechris/matrix-mcp
- jlxq0/matrix-mcp: https://github.com/jlxq0/matrix-mcp
- Just-Insane/matrix-mcp-server: https://github.com/Just-Insane/matrix-mcp-server
- elkimek/matrix-bridge: https://github.com/elkimek/matrix-bridge
- elkimek/matrix-e2ee-bridge: https://github.com/elkimek/matrix-e2ee-bridge
