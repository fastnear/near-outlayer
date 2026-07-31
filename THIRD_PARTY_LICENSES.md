# Third-Party Licenses

Inventory of third-party software used by OutLayer, the license elected where a
dependency offers a choice, and the notices required when we redistribute.

**Audit date: 2026-07-31.** The dependency counts below are a snapshot taken on that
date; the license positions themselves do not expire. See "Maintaining this document"
at the end for when to refresh them.

Scope of the audit behind this document: 13 Rust lockfiles across `worker`,
`keystore-worker`, `contract`, `register-contract`, `vault-contract`,
`keystore-dao-contract`, `outlayer-monitor`, `self-hosted-scheduler`,
`dashboard/wasm-verifier`, `sdk/outlayer`, the coordinator, `shared-tee-helpers` and
`attestation-portal` — 1571 unique crate-versions — plus the npm trees of the
dashboard and the SDK, and every container image referenced from a compose file or
Dockerfile.

**No dependency of any first-party component is licensed under GPL-only, AGPL, or
SSPL terms.** Nothing in the dependency tree requires OutLayer source to be
published.

## Rust dependency licenses

| License | Crate-versions |
|---|---|
| MIT and/or Apache-2.0 (in any spelling) | ~1100 |
| Apache-2.0 WITH LLVM-exception (wasmtime, cranelift, pulley) | 120 |
| Unicode-3.0 | 71 |
| BSD-2/3-Clause, ISC, Zlib, CC0-1.0, 0BSD, Unlicense, BSL-1.0, CDLA-Permissive-2.0 | ~280 |
| MPL-2.0 | 10 |
| GPL-only, AGPL, SSPL, BUSL | **0** |

## Elected licenses for dual-licensed dependencies

Where a dependency is offered under a choice of licenses, OutLayer elects the
following. This election is recorded here so it is unambiguous in diligence.

| Dependency | Offered as | **Elected** |
|---|---|---|
| `ittapi`, `ittapi-sys` 0.4.0 | `GPL-2.0-only OR BSD-3-Clause` | **BSD-3-Clause** |
| `r-efi` 5.3.0, 6.0.0 | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | **MIT** |
| `node-forge` 1.3.1 (npm) | `BSD-3-Clause OR GPL-2.0` | **BSD-3-Clause** |
| all `MIT OR Apache-2.0` crates | either | **Apache-2.0** |
| `wasite` | `Apache-2.0 OR BSL-1.0 OR MIT` | **MIT** |

`ittapi` is reachable only through wasmtime's VTune profiling support and is not used
at runtime. Disabling wasmtime's `profiling` feature in `worker/` removes the only
GPL-bearing crate from the tree entirely and is the preferred long-term fix.

## MPL-2.0 dependencies

MPL-2.0 is file-level copyleft: obligations attach only to modified files of the
covered work itself, not to code that merely uses it. OutLayer does not fork or
modify any of these crates, so no source disclosure obligation arises. If any of
these is ever vendored and patched, the modified files must be published under
MPL-2.0.

`brownstone` 1.1.0, `colored` 2.2.0 / 3.1.1, `fastrlp` 0.3.1 / 0.4.0,
`indent_write` 2.2.0, `memory_units` 0.4.0, `nom-supreme` 0.6.0, `option-ext` 0.2.0,
`wee_alloc` 0.4.5.

## Attribution-bearing dependencies of note

These require their notices to travel with any distribution:

- `ed25519-dalek` 2.1 — **BSD-3-Clause** (not MIT, as is commonly assumed).
- `aws-lc-rs` / `aws-lc-sys` — composite:
  `ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND
  (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0)`. The complete text
  bundle ships in the crate and must be reproduced verbatim.
- `ring` — `Apache-2.0 AND ISC`.
- `rustls`, `rustls-native-certs` — `Apache-2.0 OR ISC OR MIT`.
- 71 crates under `Unicode-3.0` (ICU data and derivatives) — permissive, notice required.
- wasmtime / cranelift — `Apache-2.0 WITH LLVM-exception`; the exception must be
  reproduced along with the Apache-2.0 text.

## Key upstream projects

| Project | License |
|---|---|
| nearcore (`near-primitives`, `near-crypto`, `near-jsonrpc-client`) | MIT OR Apache-2.0 |
| near-sdk-rs | Apache-2.0 |
| near/mpc | MIT |
| wasmtime, wasmi, bollard, WasmEdge | Apache-2.0 |
| Dstack-TEE/dstack | Apache-2.0 |
| Phala `dcap-qvl`, `dstack-sdk` | MIT |
| PostgreSQL | PostgreSQL License |
| Docker Engine | Apache-2.0 |

Note on nearcore: GitHub's license detector reports GPL-3.0 for `near/nearcore`.
That is a false positive triggered by the GPL text quoted inside the repository's
`ATTRIBUTIONS.md`. The authoritative license is `MIT OR Apache-2.0`, declared in the
workspace `Cargo.toml` and in `licenses/`. That same `ATTRIBUTIONS.md` does state
that nearcore incorporates some code from OpenEthereum and Parity Substrate, both
GPL-3.0 — an unresolved ambiguity upstream at NEAR, inherited by everyone who links
`near-primitives`, and not one OutLayer can resolve on its own.

## npm

Dashboard (447 packages) and `sdk-js` (166 packages) are permissive throughout, with
two items worth recording:

- `@img/sharp-libvips-*` — **LGPL-3.0-or-later**, pulled in transitively by `next`
  for image optimization. The dashboard is operated as a hosted service and is not
  distributed, so no LGPL obligation is triggered. If the dashboard is ever shipped
  as a public container image, LGPL §4 applies; it is satisfied in practice because
  the library is loaded as a separate dynamic `.node` module that a recipient can
  replace.
- `@near-wallet-selector/*` 10.1.0, `@meteorwallet/sdk` 1.0.24,
  `text-encoding-utf-8` 1.0.2 — published **without a `license` field**, which
  formally defaults to all-rights-reserved. Upstream `near-wallet-selector` is MIT in
  its repository; the package metadata simply lost it. These appear only in example
  frontends, not in the production dashboard. Upstream fixes should be requested; in
  the meantime the upstream repository licenses are relied upon.

## Container images

### GPL components inside published container images

OutLayer publishes container images to Docker Hub
(`outlayer/near-outlayer-worker`, `outlayer/near-outlayer-keystore`). These are built
`FROM alpine:3.19` and therefore contain third-party packages licensed under the GNU
General Public License, version 2 — notably **BusyBox** (part of every Alpine image)
and **git** (installed for repository cloning) — plus `libgcc` under GPL-3.0 with the
GCC Runtime Library Exception.

Two things follow, and they are separate:

1. **No effect on OutLayer's own code.** These packages are separate executables. They
   are not linked into the OutLayer binary and are invoked as subprocesses, which is
   mere aggregation. Nothing about their presence requires OutLayer source to be
   published, and the images may be distributed commercially.

2. **They are unmodified upstream Alpine Linux builds.** OutLayer does not patch,
   fork, or statically link any of them. The corresponding sources for the exact
   versions in any image are published by Alpine Linux at
   https://git.alpinelinux.org/aports/ ; the versions themselves can be listed with
   `apk info -v` inside the image.

OutLayer makes no separate source-distribution undertaking of its own for these
packages, relying instead on the upstream availability described above. This is the
prevailing practice for distro-based container images, but note that it is not one of
the three mechanisms GPL-2.0 §3 spells out verbatim, so it is a disclosed position
rather than a belt-and-braces one. The obligation can be removed outright rather than
managed, by not shipping GPL binaries at all: `git` is the only GPL tool installed
deliberately (replacing it with an in-process Rust implementation would drop it), and
BusyBox goes away with a `scratch`/distroless base image, since the worker and
keystore are statically linked musl binaries.

### Infrastructure images

| Image | License | Status |
|---|---|---|
| `postgres:16-alpine` | PostgreSQL License | OK |
| `grafana/grafana-oss:11.5.2` | **AGPL-3.0** | See below |
| `redis:7.4.6-alpine` (digest-pinned) | **RSALv2** elected (offered as RSALv2 or SSPLv1) | See below |

**Grafana** is AGPL-3.0. It is deployed unmodified, as a separate process, alongside
OutLayer; this is mere aggregation and imposes no obligation on OutLayer code, and
provisioned dashboard JSON is not a derivative work of Grafana. **Standing policy:
Grafana is never patched or forked.** Modifying Grafana while exposing it over a
network would trigger AGPL §13 and require publishing those modifications to every
user of that endpoint.

**Redis — RSALv2 elected.** Redis Open Source 7.2 and earlier are BSD-3-Clause, as
stated in Redis's own `LICENSE.txt`: *"Redis Open Source 7.2 and prior releases remain
subject to the BSDv3 clause license."* Redis 7.4 is offered under a choice of the
**Redis Source Available License v2 (RSALv2)** or the **Server Side Public License v1
(SSPLv1)**; Redis 8 adds AGPLv3 as a third option. None of RSALv2 or SSPLv1 is
OSI-approved.

Production was verified on 2026-07-31 to be running **Redis 7.4.6**.

> **OutLayer elects RSALv2, and never SSPLv1.** RSALv2 permits internal use and
> prohibits providing the licensed software itself to third parties as a managed
> service whose value is substantially the software's own functionality. Redis is
> used here strictly as internal infrastructure — task queue, distributed locks,
> usage counters, short-lived JWT state — and is never exposed to customers as a
> Redis service. SSPLv1's service-source-disclosure clause is therefore never
> engaged, because it is not the license under which the software is taken.

The defect corrected here was not the version but the **floating tag**: all four
coordinator compose files referenced `redis:7-alpine`, so the license terms governing
the running software were determined by the date of the last `docker pull`. They are
now pinned by immutable digest to
`redis:7.4.6-alpine@sha256:3b73847e72874be07e6657b129a94761662b79bc0f679273757d4218573b2a98`.

Downgrading to the BSD-licensed 7.2 line was considered and rejected. Redis 7.4
writes RDB version 12, which 7.2 cannot read, so a downgrade requires discarding the
data volume — and that volume holds state that is not reconstructible
(`topup:task_id_counter`, whose reset would re-issue already-used task identifiers;
`daily:`/`hourly:`/`monthly:` usage aggregates; pending `topup:`, `approve:` and
`reject:` entries). Valkey (`valkey/valkey:8-alpine`, BSD-3-Clause, Linux Foundation
fork of Redis 7.2.4) shares the same RDB constraint and so does not avoid that
trade-off. Revisit if the coordinator ever moves this state into PostgreSQL.

The Rust client crate `redis` 0.24 is BSD-3-Clause and is unaffected by any of this.

## Derived first-party code

- `register-contract/` incorporates code from the NEAR MPC contract
  (https://github.com/near/mpc), MIT licensed. See `register-contract/NOTICE`.

## Maintaining this document

Most of this document is policy and does not go stale: the elected licenses, the
MPL-2.0 list, the Grafana and Redis positions, the container-image disclosure. The
only part that ages is the dependency census in "Rust dependency licenses" and the
npm package counts, which are a snapshot of the audit date given at the top.

Re-run the check and update the counts when any of these happen — not on a schedule:

- a dependency is added or a major version bumped in any component;
- a container base image or infrastructure image changes;
- before handing this document to an investor, acquirer or customer as part of
  diligence.

The check itself is `deny.toml` in the repository root, run manually:

    cargo install cargo-deny --locked        # once
    cargo deny --manifest-path worker/Cargo.toml --config deny.toml check licenses

Repeat per component; the full list of workspaces is in `LICENSING.md`. As of the
audit date all of them report `licenses ok`. This is deliberately not wired into CI —
the dependency tree changes rarely, and a manual run before diligence is the point at
which the answer actually matters.
