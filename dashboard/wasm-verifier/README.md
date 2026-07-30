# attestation verifier (WebAssembly)

`dcap-qvl` compiled to WebAssembly so the dashboard can verify a TDX quote in the visitor's own
browser: Intel's signature chain, the TCB status, and the measurements decoded from the verified
quote.

It is the same crate, at the same version, that the register-contract uses to verify worker
registration on chain. That is the point — one implementation, so a verdict shown in the browser
cannot drift from the one the contract reached.

## Rebuilding

The build output is committed (`dashboard/lib/attestation-wasm/` plus
`dashboard/public/attestation-verifier.wasm`), so the dashboard builds with Node alone and no Rust
toolchain in CI. Rebuild only when the `dcap-qvl` version or this crate changes:

```bash
cargo install wasm-pack            # once
cd dashboard/wasm-verifier
wasm-pack build --target web --release --out-dir ../lib/attestation-wasm --out-name verifier
rm -f ../lib/attestation-wasm/.gitignore          # wasm-pack writes one that would hide the output
cp ../lib/attestation-wasm/verifier_bg.wasm ../public/attestation-verifier.wasm
```

The `.wasm` is loaded from `/attestation-verifier.wasm` by explicit URL rather than through bundler
asset handling, which keeps it out of the initial page load — it is fetched the first time someone
actually verifies an attestation.

## Feature choices

`ring` is off: it does not build for `wasm32-unknown-unknown`. `rustcrypto` provides the same
primitives in pure Rust and satisfies the `_anycrypto` marker that `dcap_qvl::verify::verify`
requires. `js` is dcap-qvl's own wasm-bindgen feature.
