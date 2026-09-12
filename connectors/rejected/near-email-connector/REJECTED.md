# Rejected for the mainnet release — kept for reference

This is a complete, tested connector for near.email: seven priced operations,
the mailbox sealed to the caller's own ephemeral key, the author's master key
taken from the manifest. It builds and its tests pass. It is not published, and
the reason is not the code.

**Why it cannot be deployed as written.** Every mailbox is sealed to a key
derived from one master private key, and that key lives as `PROTECTED_MASTER_KEY`
in a secret whose encryption key the keystore derives from
`project:zavodil.near/near-email:zavodil.near`. `PROTECTED_` means nobody can
read the value back — not us, not its owner. So the key exists for that project
and for no other, and a connector published under `connectors.outlayer.near`
would start from a different master: it would read nothing that exists and write
what the website cannot read.

**The routes that were considered and why each was dropped.**

* *Re-encrypt the secret for the new project.* The keystore can do it, and that
  is exactly the problem: an owner could point the re-encryption at a project
  whose code they write, print the value, and the `PROTECTED_` guarantee would be
  worth nothing. Rejected as a platform hole, not as an inconvenience.
* *Publish the connector at `zavodil.near/near-email`.* Pricing is per project,
  so the website's users would start paying per operation; the contract then
  requires `operation` in every request and an attached deposit on the on-chain
  path, and a name defined by both the manifest and the caller's `secrets_ref`
  refuses the run. Three changes to the website and a product decision about who
  pays, to price a showcase.
* *A connector under the namespace that aliases a pinned version of the old
  project.* The cleanest of the three: the pin makes the approved code explicit,
  pricing lands on the alias, the website is untouched. It needs two project
  identities in one run — secrets and storage following the code, price and
  quota following the alias — and it is invisible to the on-chain path, because
  a chain request carries its own code source. A day of careful work in the
  coordinator plus the storage test, for a product with no billing pressure.

**What near.email does instead.** It stays an ordinary project, exactly as it was
before any of this: `zavodil.near/near-email`, no manifest, no per-operation
price, the website unchanged.

**What is worth taking from here.** The sealed-reply pattern (a reply encrypted
to an ephemeral key the caller generates per call), the rule that the priced
operation is checked only AFTER a sealed payload is opened, and the split between
the service's wire protocol and the connector's own door. The Gmail connector
reuses all three.
