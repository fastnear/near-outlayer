# outlayer-docs-mcp-server

An [MCP](https://modelcontextprotocol.io) server that exposes NEAR OutLayer's developer
documentation as resources, so AI coding assistants (Claude Code, Claude Desktop, and other
MCP-compatible clients) can pull accurate, up-to-date OutLayer docs directly into context
instead of relying on training data or manual copy-pasting.

## Why

There was no MCP server for OutLayer's docs — devs building against OutLayer with an AI
assistant had no structured way to give it the docs short of pasting files in by hand. This
closes that gap for the markdown docs already in this repo (API reference, auth, vaults, VRF,
custody, worker/keystore/contract READMEs, WASI tutorials, etc.).

This is a docs-only server (v1): it exposes files as read-only MCP **resources**. It does not
call the live HTTPS API — see [`@outlayer/sdk`](https://www.npmjs.com/package/@outlayer/sdk) or
[API.md](../API.md) for that today. Wiring API calls in as MCP **tools** is a natural follow-up.

## Install & build

```bash
cd mcp-server
npm install
npm run build
```

## Run

```bash
npm start
# or, for local iteration without building:
npm run dev
```

The server communicates over stdio, per the MCP spec.

## Use with Claude Code / Claude Desktop

Add to your MCP client config (e.g. `~/.claude/config.json` or Claude Desktop's
`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "outlayer-docs": {
      "command": "node",
      "args": ["/absolute/path/to/near-outlayer/mcp-server/dist/index.js"]
    }
  }
}
```

Once connected, the client can list and read resources like `outlayer-docs://API.md` or
`outlayer-docs://docs/CLI.md`.

## Adding a doc

Add an entry (`path` relative to the repo root, plus a short `description`) to
[`src/docs.ts`](src/docs.ts). Business/marketing docs and internal AI-assistant instructions
(`CLAUDE.md`) are intentionally excluded — keep this list to developer-facing technical docs.
