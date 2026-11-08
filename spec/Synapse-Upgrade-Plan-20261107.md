# Synapse upgrade plan for the 003 trusted invite

Date: 2026-11-07

This plan upgrades Synapse on `matrix.v-site.net` so a trusted invite can reach the agent bot. The 003 bot code stays as it is. Continuwuity stays at `26.9.1`.

Task: T019 in `.specify/specs/003-integrated-matrix-bot/tasks.md`.

## Why

Continuwuity `26.9.1` rejects an invite from Synapse `1.147.1`. The error is `M_INVALID_PARAM`: `PDU in invite state (index 0) violates the room event format`. The bot never sees the invite. The same invite fails for the old bot on agent0-2.

Synapse `1.162.0` sends full invite events over federation. Continuwuity has required that format since `26.6.1`. Source: [Continuwuity issue 1971](https://forgejo.ellis.link/continuwuation/continuwuity/issues/1971) and [Synapse 1.162.0](https://github.com/element-hq/synapse/blob/v1.164.2/CHANGES.md).

## What is installed

| Item | Value |
| --- | --- |
| Cluster | `v-site-cluster`, namespace `matrix` |
| Helm release | `matrix-synapse` |
| Chart | `matrix` `2.9.14` (zekker6). Label `helm.sh/chart: matrix-2.9.14` on ConfigMap `matrix-synapse`. |
| App version on that chart | `1.147.1` |
| Container | `matrix` in Deployment `matrix-synapse` |
| Expected image | `matrixdotorg/synapse:v1.147.1` |
| Sidecar | OpenVPN in the same pod. Leave it. |
| Database | PostgreSQL StatefulSet in namespace `matrix` |
| Config | ConfigMap `matrix-synapse`. The process reads the merged file `/tmp/synapse.yaml`. |
| Workers | None |
| Live version | `1.147.1` from `https://matrix.v-site.net/_matrix/federation/v1/version` on 2026-11-07 |

Confirm the live image before the change:

```bash
kubectl -n matrix get deploy matrix-synapse \
  -o jsonpath='{range .spec.template.spec.containers[*]}{.name}{" "}{.image}{"\n"}{end}'
```

## Target image

Use `matrixdotorg/synapse:v1.164.2`.

That tag is the newest stable release, published 2026-11-02. It includes the `1.162.0` invite fix. `1.164.0` release candidates are older than this tag.

Do not move the Helm chart from `2.9.14` to a current chart. The current zekker6 chart is past `2.81.0` and also owns Element, mail, and Postgres. An image change on the `matrix` container is the drop-in.

## What else changes

These items come from the Synapse upgrade notes between `1.147.1` and `1.164.2`.

- New rooms default to room version 12, starting in `1.162.0`. Existing rooms keep their current version. Continuwuity `26.9.1` lists room version 12 as stable.
- Synapse does not strictly reject incoming old invites until 2027-06-01. An invite from Continuwuity to Synapse keeps working.
- `1.164` turns on the stable mention push rules and drops the legacy mention rules. The agent bot matches message text. It does not use those push rules.
- `1.163.0` refuses to start when `limit_profile_requests_to_users_who_share_rooms` is true and `require_auth_for_profile_requests` is unset. The saved ConfigMap does not set either key.
- `1.157.0` drops MSC3861 auth delegation. The saved ConfigMap does not use it.
- `1.152.0` adds a worker stream for media quarantine. This deploy has no workers.
- `1.162.0` limits one-time keys to 500 per device per algorithm. Each agent uses one Matrix device.

The database schema moves forward during startup. Synapse will not serve traffic until that migration finishes. A one-replica Deployment has a short gap during the roll. Putting `v1.147.1` back after a successful start can fail, because the new schema may not load in the old image.

## Steps

Run these on the cluster admin host. Do not run them until this plan is accepted.

1. Save the live Helm values and a Postgres backup.

```bash
helm get values matrix-synapse -n matrix > /tmp/matrix-synapse-values-20261107.yaml
kubectl -n matrix exec sts/<postgres-pod> -- \
  pg_dump -U matrix matrix > /tmp/matrix-synapse-20261107.sql
```

Replace `<postgres-pod>` with the Postgres pod name from `kubectl -n matrix get pods`.

2. Confirm the `matrix` container image is `matrixdotorg/synapse:v1.147.1`.
3. Set the new image. Kubernetes rolls the Deployment. Leave the OpenVPN container on its current image.

```bash
kubectl -n matrix set image deployment/matrix-synapse \
  matrix=matrixdotorg/synapse:v1.164.2
kubectl -n matrix rollout status deployment/matrix-synapse
```

4. Read the version.

```bash
curl -s https://matrix.v-site.net/_matrix/federation/v1/version
```

Pass is `"version":"1.164.2"`.

5. From `@admin:v-site.net`, invite `@agent0-5:agent0-5-mhs.cybertribe.com` to a new room. The Element desktop app may show the member as `@agent0-5`. Confirm the server name is `agent0-5-mhs.cybertribe.com`.
6. Pass is one join and one greeting: `Hello! I'm Agent0-5, an AI agent powered by Agent Zero.`
7. Write the version and the greeting into `spec/T018-Agent0-5-Result-20261107.md`.

Leave agent0-5 in maintenance during this check. `run_bot` stays stopped.

## Rollback

Use the saved image only if `1.164.2` fails before the database migration commits.

```bash
kubectl -n matrix set image deployment/matrix-synapse \
  matrix=matrixdotorg/synapse:v1.147.1
kubectl -n matrix rollout status deployment/matrix-synapse
```

If the new pod started and then failed, restore the Postgres dump into a new database and point the Deployment at that database before starting `v1.147.1`. Do not drop the live database.
