# connector-probe

A connector that does nothing useful, so that everything **around** a connector
can be tested.

near.email is the only real connector and it is mainnet-only, which leaves the
whole connector path unexercisable on testnet: pricing, the fixed fee per
firing, the per-operation caps, the owner's secret, the outbound allowlist. This
one is published like a connector, priced like a connector and metered like a
connector — and reports back what it saw.

**Testnet only.** The registry lists it on testnet and nowhere else
(`ConnectorProbe::networks` in the coordinator), so on mainnet nothing at
`connectors.outlayer.near/connector-probe` is a connector at all. A test
connector reachable in production would be an extra door into a worker that
holds keys, opened for nobody's benefit.

## Publishing

```bash
./build.sh                      # checks the manifest section is present, prints the SHA256
outlayer deploy                 # or upload the wasm and publish a version by hash
```

The project id must be **`connectors.outlayer.testnet/connector-probe`**. Every
connector is deployed under that one account, so its id is `{namespace}/{name}`
and it is called like any other project:

```
POST /call/connectors.outlayer.testnet/connector-probe
```

A different owner is a different project: the registry only recognises the name
under the namespace, so a probe published anywhere else is an ordinary project
with no price and no fee.

## Operations

| `operation` | Price | What it proves |
|---|---|---|
| `ping` | free | a free operation is still a REAL price: it runs only on a key that can pay, and is refused when the key cannot |
| `whoami` | $0.01, share 0 | what the guest was TOLD about its caller — injected by the worker, not taken from the request. Priced with the whole fee staying with the platform, which is the shape of every connector we own ourselves |
| `secret` | $0.01 | the owner's secret reached the guest, without printing it |
| `burn` | $0.01 | compute costs something: `{"operation":"burn","rounds":50}` burns instructions on demand |
| `fetch` | $0.015 | the declared host (`rpc.testnet.fastnear.com`) is reachable |
| `forbidden_fetch` | $0.015 | an undeclared host (`example.com`) is NOT |
| `unpriced` | — | absent from the price table AND unimplemented here: must be refused before anything runs |

`forbidden_fetch` **passes when it fails**: `ok: false` with an `http_error` is
the expected result. A success means an undeclared host was reachable from
inside a TEE that holds keys, and the manifest allowlist is not being enforced.

## The secret

Store it under the AGENT's own account, which is what the keystore compares
against the caller:

```
accessor: Project("connectors.outlayer.testnet/connector-probe")
profile:  <agent account>      # the 64-hex custody wallet account
owner:    <agent account>
```

Use `POST /wallet/v1/agent-secret/prepare` (the author pays, the agent needs no
NEAR) or `POST /wallet/v1/agent-secret` (the agent's own wallet signs). Keys the
probe looks for: `PROBE_TOKEN`, `PROBE_SECOND`.

Then call with `X-Use-Owner-Secret: 1`. Without that header no secret is looked
up at all — which is itself worth testing: `operation: "secret"` should then report
`found: false` for everything.

## What it never does

It never returns a secret's value, and never takes a host to fetch from the
caller. A test tool that could be pointed at an arbitrary host would be an SSRF
gadget with a TEE's network access; one that echoed secrets would turn every
test run into a leak.

## Where the full runbook is

`docs/TESTNET_RUNBOOK.md` in the coordinator repository — the ordered list of
calls that checks each limit and each refusal, with what a pass looks like.
