#!/usr/bin/env python3
"""
Scan all Bearer-near-origin wallets in the coordinator DB and report their
on-chain balances. Used as a pre-flight check before any wallet-id schema
migration (e.g. v2 wallet_id refactor where Bearer-near wallets get new
wallet_ids and the old rows would be purged).

What it covers:
  - Every `wallet_accounts` row that has `near_pubkey` set AND no
    `wallet_api_keys` entry (= Bearer-near lazy-create, never registered).
  - Mixed wallets (have BOTH a wk_ AND were used via Bearer-near) are
    included with a note.
  - For each, queries native NEAR balance + USDC ft_balance_of + intents.near
    mt_balance_of for USDC and USDT.

What it does NOT cover:
  - Wallets created via Bearer-near WITHOUT ever calling /address —
    `near_pubkey` stays NULL, and we cannot reconstruct the on-chain
    address without keystore-worker access. Pass --derive-via-keystore to
    re-derive from wallet_requests data (needs KEYSTORE_BASE_URL).
  - Vault-scoped Bearer-near where the DB cache holds a different scope's
    pubkey — the cached pubkey reflects whatever was first written. The
    actual money may be at a different (vault-scoped) address. Run with
    --derive-via-keystore to additionally check vault-scoped derivations.

Usage:
    export DATABASE_URL='postgres://user:pass@host/db'
    export NEAR_RPC_URL='https://rpc.mainnet.fastnear.com'
    # optional, for keystore re-derive:
    export KEYSTORE_BASE_URL='https://keystore.outlayer.internal'
    export KEYSTORE_AUTH_TOKEN='...'

    pip install asyncpg aiohttp
    python3 scan_signature_wallet_balances.py [--derive-via-keystore] [--mainnet|--testnet]

Outputs CSV to stdout with: wallet_id, account_id, scope, near_balance,
usdc_balance, intents_usdc, intents_usdt. Logs to stderr.
"""

import argparse
import asyncio
import base64
import csv
import json
import os
import sys
from dataclasses import dataclass, field
from typing import Optional

try:
    import asyncpg  # type: ignore
    import aiohttp  # type: ignore
except ImportError:
    sys.stderr.write("missing deps. install: pip install asyncpg aiohttp\n")
    sys.exit(2)


USDC_MAINNET = "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1"
USDT_MAINNET = "usdt.tether-token.near"
USDC_TESTNET = "usdc.fakes.testnet"  # placeholder — adjust if you use a real testnet USDC
USDT_TESTNET = "usdt.fakes.testnet"


@dataclass
class WalletRow:
    wallet_id: str
    near_pubkey: str
    vault_id: Optional[str]
    has_wk: bool
    extra_scopes: list = field(default_factory=list)  # extra (vault_id) seen in wallet_requests


@dataclass
class BalanceReport:
    wallet_id: str
    account_id: str
    scope: str  # "default-master" or vault id
    near_yocto: str
    usdc: str
    intents_usdc: str
    intents_usdt: str
    note: str = ""


def near_account_from_pubkey(pubkey: str) -> str:
    """ed25519:<hex> → implicit account id (lowercase hex)."""
    if pubkey.startswith("ed25519:"):
        return pubkey[len("ed25519:") :]
    return pubkey


async def fetch_rows(pool, only_bearer_near: bool, vault_only: bool) -> list[WalletRow]:
    """One query: all wallet_accounts with near_pubkey populated, joined to
    wallet_api_keys to determine wk_ status."""
    q = """
    SELECT
        wa.wallet_id,
        wa.near_pubkey,
        wa.vault_id,
        EXISTS (SELECT 1 FROM wallet_api_keys wk WHERE wk.wallet_id = wa.wallet_id) AS has_wk,
        EXISTS (SELECT 1 FROM wallet_requests wr
                WHERE wr.wallet_id = wa.wallet_id AND wr.vault_id IS NOT NULL) AS has_vault_request
    FROM wallet_accounts wa
    WHERE wa.near_pubkey IS NOT NULL
    """
    rows = await pool.fetch(q)
    out = []
    for r in rows:
        if only_bearer_near and r["has_wk"]:
            continue
        if vault_only and r["vault_id"] is None and not r["has_vault_request"]:
            continue
        out.append(WalletRow(
            wallet_id=r["wallet_id"],
            near_pubkey=r["near_pubkey"],
            vault_id=r["vault_id"],
            has_wk=r["has_wk"],
        ))
    return out


async def fetch_extra_scopes(pool, wallet_ids: list[str]) -> dict[str, list[str]]:
    """For each wallet_id, return list of distinct vault_ids observed in
    wallet_requests that AREN'T the cached vault_id. These are extra scopes
    the wallet may have derived addresses under (via Bearer-near vault_id
    that wasn't first-written to wallet_accounts)."""
    if not wallet_ids:
        return {}
    q = """
    SELECT wallet_id, ARRAY_AGG(DISTINCT vault_id) FILTER (WHERE vault_id IS NOT NULL) AS scopes
    FROM wallet_requests
    WHERE wallet_id = ANY($1)
    GROUP BY wallet_id
    """
    rows = await pool.fetch(q, wallet_ids)
    return {r["wallet_id"]: (r["scopes"] or []) for r in rows}


async def rpc_call(session: aiohttp.ClientSession, rpc_url: str, payload: dict) -> dict:
    async with session.post(rpc_url, json=payload, timeout=aiohttp.ClientTimeout(total=10)) as resp:
        return await resp.json()


async def get_near_balance(session, rpc_url: str, account_id: str) -> str:
    p = {
        "jsonrpc": "2.0",
        "id": "b",
        "method": "query",
        "params": {"request_type": "view_account", "finality": "final", "account_id": account_id},
    }
    r = await rpc_call(session, rpc_url, p)
    if r.get("error"):
        return "0"
    return str(r.get("result", {}).get("amount", "0"))


async def get_ft_balance(session, rpc_url: str, ft_contract: str, account_id: str) -> str:
    args = base64.b64encode(json.dumps({"account_id": account_id}).encode()).decode()
    p = {
        "jsonrpc": "2.0",
        "id": "ft",
        "method": "query",
        "params": {
            "request_type": "call_function",
            "finality": "optimistic",
            "account_id": ft_contract,
            "method_name": "ft_balance_of",
            "args_base64": args,
        },
    }
    r = await rpc_call(session, rpc_url, p)
    if r.get("error"):
        return "0"
    raw = r.get("result", {}).get("result", [])
    s = bytes(raw).decode("utf-8", errors="replace") if raw else '"0"'
    try:
        return json.loads(s)
    except Exception:
        return "0"


async def get_intents_balance(session, rpc_url: str, account_id: str, token_contract: str) -> str:
    token_id = f"nep141:{token_contract}"
    args = base64.b64encode(
        json.dumps({"account_id": account_id, "token_id": token_id}).encode()
    ).decode()
    p = {
        "jsonrpc": "2.0",
        "id": "mt",
        "method": "query",
        "params": {
            "request_type": "call_function",
            "finality": "optimistic",
            "account_id": "intents.near",
            "method_name": "mt_balance_of",
            "args_base64": args,
        },
    }
    r = await rpc_call(session, rpc_url, p)
    if r.get("error"):
        return "0"
    raw = r.get("result", {}).get("result", [])
    s = bytes(raw).decode("utf-8", errors="replace") if raw else '"0"'
    try:
        return json.loads(s)
    except Exception:
        return "0"


async def keystore_derive_address(
    session: aiohttp.ClientSession,
    keystore_url: str,
    auth_token: Optional[str],
    wallet_id: str,
    vault_id: Optional[str],
) -> Optional[str]:
    """Returns ed25519:<hex> from keystore's /wallet/derive-address. None on
    error. Used to discover vault-scoped addresses not in DB cache."""
    headers = {}
    if auth_token:
        headers["Authorization"] = f"Bearer {auth_token}"
    if vault_id:
        headers["X-Customer-Vault"] = vault_id
    url = keystore_url.rstrip("/") + "/wallet/derive-address"
    body = {"wallet_id": wallet_id, "chain": "near"}
    try:
        async with session.post(url, json=body, headers=headers,
                                timeout=aiohttp.ClientTimeout(total=10)) as resp:
            if resp.status != 200:
                return None
            j = await resp.json()
            return j.get("public_key")
    except Exception:
        return None


async def report_for_address(
    session, rpc_url, wallet_id, account_id, scope, usdc, usdt, note=""
) -> BalanceReport:
    near = await get_near_balance(session, rpc_url, account_id)
    u = await get_ft_balance(session, rpc_url, usdc, account_id)
    iu = await get_intents_balance(session, rpc_url, account_id, usdc)
    it = await get_intents_balance(session, rpc_url, account_id, usdt)
    return BalanceReport(wallet_id, account_id, scope, near, u, iu, it, note)


async def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mainnet", action="store_true")
    ap.add_argument("--testnet", action="store_true")
    ap.add_argument("--include-wk", action="store_true",
                    help="include wallets that have a wk_ entry (default: only Bearer-near origin)")
    ap.add_argument("--vault-only", action="store_true",
                    help="only scan wallets that touched any vault scope (vault_id IS NOT NULL in wallet_accounts OR wallet_requests). Use this as the v2-migration pre-flight check.")
    ap.add_argument("--derive-via-keystore", action="store_true",
                    help="re-derive addresses for each known vault scope via keystore (catches stale DB cache)")
    ap.add_argument("--nonzero-only", action="store_true",
                    help="suppress rows where every balance is 0")
    args = ap.parse_args()

    db_url = os.environ.get("DATABASE_URL")
    if not db_url:
        sys.stderr.write("DATABASE_URL env var required (coordinator postgres)\n")
        sys.exit(2)

    rpc_url = os.environ.get("NEAR_RPC_URL")
    if not rpc_url:
        if args.mainnet:
            rpc_url = "https://rpc.mainnet.fastnear.com"
        elif args.testnet:
            rpc_url = "https://rpc.testnet.fastnear.com"
        else:
            sys.stderr.write("NEAR_RPC_URL env var required (or pass --mainnet/--testnet)\n")
            sys.exit(2)

    is_mainnet = "mainnet" in rpc_url or args.mainnet
    usdc = USDC_MAINNET if is_mainnet else USDC_TESTNET
    usdt = USDT_MAINNET if is_mainnet else USDT_TESTNET

    keystore_url = os.environ.get("KEYSTORE_BASE_URL")
    keystore_token = os.environ.get("KEYSTORE_AUTH_TOKEN")
    if args.derive_via_keystore and not keystore_url:
        sys.stderr.write("--derive-via-keystore needs KEYSTORE_BASE_URL\n")
        sys.exit(2)

    sys.stderr.write(f"[scan] DB ready, RPC={rpc_url}, mainnet={is_mainnet}\n")

    pool = await asyncpg.create_pool(db_url, min_size=1, max_size=4)
    try:
        rows = await fetch_rows(
            pool,
            only_bearer_near=(not args.include_wk),
            vault_only=args.vault_only,
        )
        sys.stderr.write(f"[scan] {len(rows)} wallets to check\n")

        wallet_ids = [r.wallet_id for r in rows]
        extras = await fetch_extra_scopes(pool, wallet_ids)
        for r in rows:
            r.extra_scopes = [v for v in extras.get(r.wallet_id, []) if v != r.vault_id]

        reports: list[BalanceReport] = []
        async with aiohttp.ClientSession() as session:
            for r in rows:
                primary_account = near_account_from_pubkey(r.near_pubkey)
                primary_scope = r.vault_id or "default-master"
                note = ""
                if r.has_wk and r.vault_id is None:
                    note = "wk_+bearer mixed"
                elif r.has_wk:
                    note = "wk_-registered (vault)"
                reports.append(await report_for_address(
                    session, rpc_url, r.wallet_id, primary_account, primary_scope,
                    usdc, usdt, note,
                ))

                if args.derive_via_keystore and (r.extra_scopes or r.vault_id is None):
                    # Also probe the OTHER scopes — vault-scoped derivations
                    # that the DB cache doesn't reflect.
                    candidate_scopes = list(r.extra_scopes)
                    # If primary scope was vault, also probe default-master
                    if r.vault_id is not None:
                        candidate_scopes.append(None)
                    seen = {primary_scope}
                    for vs in candidate_scopes:
                        scope_key = vs or "default-master"
                        if scope_key in seen:
                            continue
                        seen.add(scope_key)
                        pubkey = await keystore_derive_address(
                            session, keystore_url, keystore_token, r.wallet_id, vs,
                        )
                        if not pubkey:
                            continue
                        acc = near_account_from_pubkey(pubkey)
                        reports.append(await report_for_address(
                            session, rpc_url, r.wallet_id, acc, scope_key,
                            usdc, usdt, "keystore-derived (not in DB)",
                        ))

        # CSV output to stdout
        w = csv.writer(sys.stdout)
        w.writerow(["wallet_id", "account_id", "scope", "near_yocto", "usdc",
                    "intents_usdc", "intents_usdt", "note"])
        nonzero_total = 0
        for r in reports:
            is_zero = all(x in ("0", "") for x in
                          (r.near_yocto, r.usdc, r.intents_usdc, r.intents_usdt))
            if args.nonzero_only and is_zero:
                continue
            if not is_zero:
                nonzero_total += 1
            w.writerow([r.wallet_id, r.account_id, r.scope, r.near_yocto, r.usdc,
                        r.intents_usdc, r.intents_usdt, r.note])

        sys.stderr.write(
            f"[scan] {len(reports)} address probes, {nonzero_total} with non-zero balance\n"
        )
    finally:
        await pool.close()


if __name__ == "__main__":
    asyncio.run(main())
