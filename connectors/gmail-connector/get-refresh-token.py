#!/usr/bin/env python3
"""Mint a Gmail refresh token for this connector, on your own machine.

Google hands out a refresh token only at the end of a consent flow, and the flow
needs a browser and a redirect back to somewhere. This script is that somewhere:
it listens on localhost, prints the URL to open, catches the code Google sends
back, exchanges it for tokens, and prints the one value you then store.

Nothing here reaches OutLayer, and nothing is written to disk. The token is
printed once, to this terminal — treat it as the password to the mailbox, because
that is what it is.

    python3 get-refresh-token.py --client-id … --client-secret …

Before running it, once, in the Google Cloud console:

  1. Create a project (or pick one) at console.cloud.google.com.
  2. APIs & Services → Library → enable **Gmail API**.
  3. APIs & Services → OAuth consent screen → External. Add the mailbox you are
     connecting as a Test user.
  4. **Publishing status: In production.** A client left in Testing issues
     refresh tokens that expire in days, and the agent then stops working every
     week with no explanation.
  5. Credentials → Create credentials → OAuth client ID → **Desktop app**. Copy
     the client id and client secret into the command above.
"""
import argparse
import http.server
import json
import secrets
import socketserver
import sys
import time
import urllib.parse
import urllib.request
import webbrowser

AUTH = "https://accounts.google.com/o/oauth2/v2/auth"
TOKEN = "https://oauth2.googleapis.com/token"
# Send as the account, and nothing else: the connector has no operation that
# reads, and a scope it does not use is a scope Google will not approve.
SCOPES = "https://www.googleapis.com/auth/gmail.send"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--client-id", required=True)
    parser.add_argument("--client-secret", required=True)
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--no-browser", action="store_true")
    args = parser.parse_args()

    redirect = f"http://localhost:{args.port}"
    state = secrets.token_urlsafe(16)
    url = f"{AUTH}?" + urllib.parse.urlencode(
        {
            "client_id": args.client_id,
            "redirect_uri": redirect,
            "response_type": "code",
            "scope": SCOPES,
            # Offline is what makes Google return a refresh token at all, and
            # consent forces a fresh one even if this client was approved before.
            "access_type": "offline",
            "prompt": "consent",
            "state": state,
        }
    )

    print("Open this URL, sign in as the mailbox you are connecting, and approve:\n")
    print(url + "\n")
    if not args.no_browser:
        try:
            webbrowser.open(url)
        except Exception:
            pass

    code = serve_once(args.port, state)
    if not code:
        print("No code came back. Run it again.", file=sys.stderr)
        return 1

    body = urllib.parse.urlencode(
        {
            "code": code,
            "client_id": args.client_id,
            "client_secret": args.client_secret,
            "redirect_uri": redirect,
            "grant_type": "authorization_code",
        }
    ).encode()
    request = urllib.request.Request(TOKEN, data=body, headers={"Content-Type": "application/x-www-form-urlencoded"})
    try:
        answer = json.load(urllib.request.urlopen(request, timeout=30))
    except urllib.error.HTTPError as e:
        print(f"Google refused the exchange: {e.read().decode()[:400]}", file=sys.stderr)
        return 1

    refresh = answer.get("refresh_token")
    if not refresh:
        print(
            "Google returned no refresh_token. That happens when this client has "
            "already been consented to without `prompt=consent`, or when the OAuth "
            "client is not a Desktop app. Revoke the app at "
            "myaccount.google.com/permissions and run this again.",
            file=sys.stderr,
        )
        return 1

    print("\nStore these three for the connector, and nowhere else:\n")
    print(json.dumps({
        "GMAIL_CLIENT_ID": args.client_id,
        "GMAIL_CLIENT_SECRET": args.client_secret,
        "GMAIL_REFRESH_TOKEN": refresh,
    }, indent=2))
    print("\n  outlayer secrets set-for-agent '<the JSON above>' \\")
    print("    --project connectors.outlayer.near/gmail --api-key wk_…\n")
    print(f"Scopes granted: {answer.get('scope', '?')}")
    return 0


def serve_once(port: int, expected_state: str, timeout_seconds: int = 300):
    """Wait for Google's redirect — the one carrying our `state` — then stop.

    Anything else that reaches the port first (a browser's favicon request, a
    preconnect, another tab) is answered and ignored rather than taken for the
    redirect, so it cannot end the flow early. Gives up after `timeout_seconds`.
    """
    caught = {}

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):  # noqa: N802
            query = urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query)
            # The state is checked because this port is open to anything on the
            # machine while the flow runs.
            ours = query.get("state", [""])[0] == expected_state
            if ours:
                caught["code"] = query.get("code", [""])[0]
                caught["error"] = query.get("error", [""])[0]
            self.send_response(200 if ours else 404)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.end_headers()
            if ours:
                self.wfile.write("Done. Close this tab and look at the terminal.\n".encode())

        def log_message(self, *_):
            pass

    deadline = time.monotonic() + timeout_seconds
    with socketserver.TCPServer(("127.0.0.1", port), Handler) as server:
        while "code" not in caught and "error" not in caught:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                print(f"No redirect from Google within {timeout_seconds} seconds.", file=sys.stderr)
                return None
            server.timeout = remaining
            server.handle_request()
    if caught.get("error"):
        print(f"Google reported: {caught['error']}", file=sys.stderr)
    return caught.get("code") or None


if __name__ == "__main__":
    raise SystemExit(main())
