#!/usr/bin/env node
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  ListResourcesRequestSchema,
  ReadResourceRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { readFile } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { DOC_FILES } from "./docs.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
// dist/index.js -> mcp-server/ -> repo root
const REPO_ROOT = resolve(__dirname, "..", "..");
const URI_SCHEME = "outlayer-docs";

function uriFor(path: string): string {
  return `${URI_SCHEME}://${path}`;
}

function pathFromUri(uri: string): string | undefined {
  if (!uri.startsWith(`${URI_SCHEME}://`)) return undefined;
  return uri.slice(`${URI_SCHEME}://`.length);
}

const server = new Server(
  { name: "outlayer-docs-mcp-server", version: "0.1.0" },
  { capabilities: { resources: {} } },
);

server.setRequestHandler(ListResourcesRequestSchema, async () => ({
  resources: DOC_FILES.map((doc) => ({
    uri: uriFor(doc.path),
    name: doc.path,
    description: doc.description,
    mimeType: "text/markdown",
  })),
}));

server.setRequestHandler(ReadResourceRequestSchema, async (request) => {
  const docPath = pathFromUri(request.params.uri);
  const doc = DOC_FILES.find((d) => d.path === docPath);
  if (!doc) {
    throw new Error(`Unknown resource: ${request.params.uri}`);
  }

  const text = await readFile(resolve(REPO_ROOT, doc.path), "utf-8");
  return {
    contents: [
      {
        uri: request.params.uri,
        mimeType: "text/markdown",
        text,
      },
    ],
  };
});

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("outlayer-docs-mcp-server running on stdio");
}

main().catch((error) => {
  console.error("Fatal error running outlayer-docs-mcp-server:", error);
  process.exit(1);
});
