# OutLayer

**Verifiable compute and custody for AI agents.**

---

## The Problem

AI agents are being handed money, credentials and the authority to act. Today that
means handing a cloud provider all three at once: the private keys, the API tokens,
the agent's own logic — and the ability to change any of it silently.

The security model is "trust us". For a chatbot that is fine. For an autonomous
process that moves funds, signs transactions and talks to a bank, it is not.

Two things have to be true at the same time, and no one delivers both:

- **The keys must be out of everyone's reach** — including the operator's.
- **The code that uses those keys must be out of everyone's reach too.** A guarded
  key is worthless if the program deciding what to sign runs on an ordinary server.

---

## What OutLayer Is

Two products in one attested enclave.

### 1. Agent Custody

A multi-chain wallet an agent can spend but never leak.

- Keys are derived and used **inside an Intel TDX enclave** and never leave it.
- Addresses on **NEAR, EVM and Solana**; deposits and withdrawals reach BTC, ETH,
  SOL and more through NEAR Intents.
- **Gasless and cross-chain**: the agent does not need a native token on every
  network in order to act. For an autonomous process this is not a convenience —
  it is the difference between running and stalling on the first chain where it
  has no gas.
- A **spending policy set by the human owner** — per-transaction caps, velocity
  limits, allowlists, time windows, multisig thresholds, emergency freeze — stored
  encrypted on chain and enforced inside the enclave.
- **A full audit trail.** Every signature leaves a receipt.

Onboarding is one HTTPS call. No browser, no extension, no seed phrase.

### 2. Connectors

Priced operations the agent can call — and the reason the compute layer exists.

A connector is not "run my code". It is a named operation with a published price:
`send_email`, `pay_invoice`, `place_order`. The agent names the operation, pays the
price, gets the result. Prices live on chain; the contract refuses a call that does
not attach the exact price of the operation it names.

Live and in progress today:

| Connector | Status |
|---|---|
| Email (near.email) | Live |
| Bank (Mercury — bill pay and invoicing, under policy) | Built, deploying |
| Hyperliquid | In progress |
| Polymarket | In progress |

Anyone can write one: a connector is an ordinary WASI project published under the
curated namespace, and its author is paid a share of every call, on chain, per
operation.

### Why they belong together

A connector runs **in the same enclave that holds the keys**. The bank token, the
exchange credential and the signing key are all inside the same attested boundary —
so the money and the logic never end up in two different trust zones.

That is the whole architecture in one sentence, and it is what a signing service
structurally cannot copy.

---

## What Makes It Different — Four Claims, Each Independently Checkable

> **The agent's code and the agent's keys live inside the same attested enclave.
> The key's root comes from a decentralised MPC network. The enclave build and the
> spending policy live on a public chain. Every action is signed and can be
> re-verified without our involvement.**

**1. The key root is a decentralised network, not our cloud.**
The master secret is issued by the **NEAR validators' MPC network** via Confidential
Key Derivation. No single party ever holds it whole, and it is delivered into the
enclave — not generated in an account we control.

**2. The enclave build is approved on chain, by a DAO.**
Workers are verified against all five Intel TDX measurements. A worker whose build is
not on the on-chain list cannot take jobs, and that list is readable from any RPC
node.

**3. The spending policy lives encrypted on chain.**
Not a row in a vendor's database. Its existence and every change to it are publicly
auditable; it is decrypted only inside the enclave.

**4. Proof that does not depend on us.**
Every execution returns an enclave-signed attestation binding code hash, input hash
and output hash. It chains to Intel's root certificate, not to any key we hold.
**OutLayer Verify** is an open-source binary your own engineers run on their own
machines — no account, no API key — so the verdict never touches our servers.

Registration keys are post-quantum (**ML-DSA-65**) alongside Ed25519.

---

## You Are Not Moving Your Agent

We are not an agent hosting platform and we are not asking you to migrate.

> **Run your agent wherever you like. When money is involved, it goes through us.**

Custody is a feature you can adopt on its own, over plain HTTPS, from any framework.

---

## What Runs On It Today

In production on NEAR mainnet since late 2025. Contract: `outlayer.near`.

- **market.near.ai** — NEAR AI's agent marketplace; its agents operate through
  confidential intents under OutLayer custody, via a vault.
- **near.email** — every NEAR account gets a mailbox; mail is encrypted on arrival
  and decrypted only inside the TEE.
- **TEE Price Oracle** — ~15 sources, median computed in-enclave. Built as NEAR's
  oracle RFP answer; **RHEA Finance was going to build their own and launched on
  OutLayer instead**, and integration into their oracle is agreed.
- **near.fm** — AI music marketplace; cross-chain tipping through custody wallets.
- **Voulai** — private trading agent on confidential intents (closed beta).
- **EAS attestor** — TEE-attested balance attestations for Ethereum networks.

Scale so far: **2,500+ agent wallets registered** and **23,600+ custody operations** —
swaps, transfers and cross-chain moves, not test pings. Execution success rate across
ten months of production: **99.85%**.

---

## How It Is Paid For

- **Agent custody is free.** It is how the audience grows.
- **Connectors are priced per operation**, on chain, with a share going to the
  connector's author.
- **Subscriptions** turn per-call charges into a flat allowance for a key.
  *(Both currently live on testnet.)*
- **Integrations pay directly** — RHEA pays per call to the TEE oracle.

---

## Why Now, and Why NEAR

- **NEAR Intents** gives agents gasless, cross-chain settlement and confidential
  swaps — the rails an autonomous wallet actually needs.
- **NEAR's MPC network** provides a key root nobody holds, including us.
- **`yield/resume`** lets a smart contract pause on an off-chain result and resume
  with it verified.
- **Intel TDX** is mature, and remote attestation is a standard an auditor already
  accepts.

---

## Where We Sit In The Market

**Against agent-wallet providers** (Turnkey, Privy, Coinbase Agentic Wallets,
Crossmint): they sign; they do not execute. The agent's logic — and the secrets that
logic uses — run on the customer's own server. Their key root is a company cloud.
Keys-in-a-TEE is table stakes now; claims 1–4 above are what sits above that line.

**Against TEE clouds** (Phala, Oasis, Marlin, Acurast): they sell a property of the
hardware and leave the buyer to invent the use case. We sell the job: a wallet with
policy, and operations with prices.

**Against verifiable-compute-for-contracts** (Chainlink Functions, Gelato, ZK
coprocessors): narrow by construction — data only, or no secrets, no network calls,
no signing.

---

## Honest Trust Assumptions

We would rather state this than have it discovered.

- OutLayer trusts the **Intel TDX** hardware and the DCAP / Intel PCS attestation
  chain — a single-vendor hardware root — plus on-chain approval of the enclave's
  measurements.
- The key root is distributed (NEAR MPC), the enclave root is not. Those are
  different guarantees and we do not blur them.
- **If we shut down, you keep custody.** A user or partner operating through a vault
  can export every secret **after OutLayer stops** and restore custody themselves —
  locally or in someone else's TEE. The procedure is documented, not theoretical.

---

## Links

| | |
|---|---|
| Site | [outlayer.ai](https://outlayer.ai) |
| App & docs | [app.outlayer.ai](https://app.outlayer.ai) · [/docs](https://app.outlayer.ai/docs) |
| API | `https://api.outlayer.ai` |
| Contracts | `outlayer.near` · `outlayer.testnet` |
| Attestation portal | [workers.outlayer.ai](https://workers.outlayer.ai) |
| Agent skill file | [skills.outlayer.ai/agent-custody/SKILL.md](https://skills.outlayer.ai/agent-custody/SKILL.md) |
| Source | [github.com/fastnear/near-outlayer](https://github.com/fastnear/near-outlayer) |
| X | [@out_layer](https://x.com/out_layer) |

---

*OutLayer — the agent's wallet is the account, connectors are what it can do, and
both live in the same enclave.*
