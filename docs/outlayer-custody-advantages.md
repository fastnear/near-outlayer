# Agent Custody: Outlayer (TEE) vs. TLA active-signer (on-chain MPC)

An objective side-by-side of the two custody models, followed by notes on the
theses. The two are built for different jobs — this is meant to make the
trade-offs visible, not to declare a winner.

## Comparison

| Dimension | Outlayer custody (TEE) | TLA `active-signer` (on-chain MPC) |
|---|---|---|
| Trust root | Intel TDX enclave, remotely attested (Intel DCAP / PCS) &nbsp;\* | NEAR MPC network + on-chain `active-signer` contract |
| Where the key lives | Sealed in the enclave, never leaves | Held by no one — threshold-shared across MPC nodes |
| Signature latency | Sub-second (in-enclave, ~ms) | ~2–6 s — signer call + MPC yield/resume, multi-block |
| Overhead per signed action | None beyond the action's own transaction | Signer function-call + MPC round + broadcast, on top of the action |
| Gas to produce a signature | None | ~0.003–0.008 NEAR/sig (tens of TGas burned) |
| Chains from one custody | NEAR, Solana (ed25519), EVM (secp256k1) | NEAR — `active-signer` builds NEAR transactions |
| Account onboarding | Implicit account — no `create_account` / `add_key`, no intents key registration | Named account — MPC-derive + `create_account` + `add_key` + funding (~0.002+ NEAR storage), **plus** `add_public_key` on `intents.near`, all on-chain per agent |
| Concurrency | Parallel in-enclave signing | Per-account nonce commit serializes actions |
| Action set | Programmable (capabilities, reject-if-tx guards) | `function_call` + `transfer` only, enforced at the type level |
| Ownership transfer / resale | Not a native primitive | Native — compare-and-swap of the operating key |
| Built-in recovery | Via composition (vaults / external policy) | Native policy engine — timelock, watcher quorum, attestation |
| Human-readable identity | Implicit hex by default | Named (TLA sub-account, e.g. `agent.claude`) |
| Verifiability of authority | Remote attestation of the measured code | On-chain contract logic + MPC network |

\* Both models ultimately root in MPC — the difference is *when* and *how often*.
Outlayer's per-key material is deterministically derived from a master secret
that the keystore-worker obtains **once, at startup, from the NEAR MPC network**.
That derivation runs inside the enclave, is deterministic, and the master never
leaves the enclave. From then on the worker signs locally, with no further MPC
call per action. The `active-signer` model instead calls MPC (`v1.signer`) on
**every** signature. So it is not "TEE vs MPC" — both use MPC — it is one MPC
touch at worker startup to seed an in-enclave master versus one MPC round-trip
per signed action.

Rows 10–13 are where the TLA model is stronger: ownership transfer, a built-in
recovery policy engine, and human-readable names are first-class there and are
not native to Outlayer custody. Rows 3–9 are where the TEE model is stronger,
and they cluster around one thing: signing is local, not an on-chain round-trip.

## Notes on the theses

### Latency and cost per action

Outlayer signs inside the enclave: sub-second, no on-chain round-trip, and no gas
to produce the signature. The on-chain-signer + MPC path pays, for **every**
signed action, a function-call transaction into the signer contract, an MPC
signing round (yield / resume across several blocks, seconds of latency), and a
broadcast. Both models still pay the underlying transaction's own gas when the
action hits the chain; the difference is the signing overhead layered on top. At
wallet cadence — a few transactions a day — that overhead is negligible. At agent
cadence — many actions a minute — it dominates.

**Worked example — an agent doing one action per minute** (1,440 actions/day):

- *TLA `active-signer`:* ~0.003–0.008 NEAR of signing overhead per action →
  roughly **4–12 NEAR/day**, purely in signing overhead, before the underlying
  transactions' own gas — plus 2–6 s of added latency on every action. It scales
  linearly with cadence, so an agent acting every few seconds costs
  proportionally more.
- *Outlayer:* **0 NEAR** of signing overhead and sub-second signing, at any
  cadence.

These figures are estimates — `1 TGas ≈ 0.0001 NEAR` at floor gas price, tens of
TGas burned along the `active-signer` → `v1.signer` → callback chain
(confidence: low–moderate). The point is the order of magnitude and that the cost
accrues **per action**, not the exact number.

### One custody, multiple chains

The same derived-key root signs NEAR (ed25519), Solana (ed25519) and EVM
(secp256k1). There is no per-chain signing contract to deploy or maintain. The
`active-signer` model, as designed, builds NEAR transactions specifically.

### Onboarding: implicit accounts

Agents onboard as NEAR implicit accounts — the account ID *is* the public key —
so there is no `create_account` and no `add_key`. A single off-chain signature is
enough to start transacting, which is what keeps NEAR Intents onboarding to one
signature with no on-chain key registration. The named-account model requires
on-chain account creation, funding for storage, and a key install per agent.

There is a second, intents-specific cost. NEAR Intents authorizes a NEP-413
signature against a public key. For an implicit account the account ID *is* that
key, so Intents can derive it directly and nothing needs to be registered. For a
named account the key cannot be inferred from the name, so the signing key must
be registered with `intents.near` via `add_public_key` before Intents will
recognize the account — an extra on-chain call per agent that the implicit model
skips entirely.

### Programmable policy, co-located with the key

Signing policy runs in the same attested enclave that holds the key: a capability
model per key, canonical-operation signing with reject-if-transaction guards, and
per-customer master-secret isolation. The `active-signer` model takes a different
and also-strong approach to safety — the action set is restricted to
`function_call` + `transfer` at the type level, so dangerous actions cannot be
expressed at all. Ours is programmable; theirs is hard-restricted. Different
tools for different risk models.

### Compute and custody in one environment

Outlayer pairs custody with verifiable compute. An agent's logic and its
signing run in the same TEE, so it can compute privately and sign the result in
one attested environment rather than splitting where it thinks from where it
signs. The TLA model is a custody-and-ownership layer; it has no co-located
compute story, and does not need one.

### Concurrency

There is no per-account on-chain nonce commit serializing an agent's actions; the
enclave signs in parallel. In the `active-signer` model the nonce is committed
per account per action, which serializes concurrent actions from one account.

### Honest trust assumptions

To be precise rather than to overclaim:

- Outlayer trusts the Intel TDX TEE and the DCAP / Intel PCS attestation chain,
  plus on-chain approval of the enclave's measurements. It is a single-vendor
  hardware trust root.
- The TLA model trusts the NEAR MPC network (threshold, no single holder) and the
  correctness of the `active-signer` contract.

These are genuinely different roots — threshold-distributed versus attested
hardware — not one strictly stronger than the other. The latency, cost,
multi-chain and co-located-compute properties above are consequences of the TEE
root; the ownership, recovery and naming primitives are consequences of the
on-chain root.

### Where the two compose

Naming and recovery can sit **on top of** Outlayer custody without changing the
root. A native-mode recovery that targets ordinary NEAR accounts already reaches
Outlayer's implicit accounts, so recovery does not require moving custody or
adopting named accounts. The sensible split is one custody root (TEE, for
agent-cadence signing) with a policy layer — naming, ownership, recovery —
composing above it.
