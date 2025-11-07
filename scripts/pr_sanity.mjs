#!/usr/bin/env node
/**
 * PR sanity gate:
 * - Fails on raw TODO/FIXME/XXX/HACK without issue-tag (allow: TODO(#123) or TODO(owner:note))
 * - Fails on panic!/unwrap!/expect() outside tests/examples/benches
 * - Fails on console.log in src (allowed in scripts/** and tests/**)
 * - Fails on eval/new Function in JS/TS anywhere
 * - Ignores: research/**, node_modules/**, target/**, .git/**, dist/**, build/**, coverage/**
 *
 * Exit code: 0 = OK, 1 = violations found.
 */
import fs from "fs";
import path from "path";
const ROOT = process.cwd();

const IGNORE_DIRS = new Set([
  "node_modules", "target", ".git", "dist", "build", "coverage", ".turbo",
  ".husky", ".github", ".vscode"
]);
const IGNORE_GLOBS = [
  /^research\//, /^logs\//, /^\.yarn\//, /^\.pnpm-.*\//, /^\.cache\//,
  /^wasi-examples\//, /^browser-worker\//, /^clients\//, /^dashboard\//,
  /^mike-suggestions\//, /^outlayer-quickjs-executor\//,
  /\.md$/, // Ignore all markdown files (docs)
];
const TEXT_EXT = new Set([
  ".rs",".toml",".lock",".md",".yml",".yaml",".json",".js",".ts",".tsx",".jsx",
  ".mjs",".cjs",".css",".scss",".html"
]);

const ALLOW_TODO = /^TODO\(([^)]+)\)/; // e.g., TODO(#123) or TODO(owner:reason)
const TODO_PATTERNS = [/\bTODO\b/, /\bFIXME\b/, /\bXXX\b/, /\bHACK\b/];
const JS_EVAL_PATTERNS = [/\beval\s*\(/, /\bnew\s+Function\s*\(/, /\bsetTimeout\s*\(/, /\bsetInterval\s*\(/];
const RS_FORBID_PATTERNS = [/\bpanic!\s*\(/, /\bunwrap\s*\(/, /\bexpect\s*\(/, /\bdbg!\s*\(/];
const CONSOLE_PATTERN = /\bconsole\.(log|debug|info|warn|error)\s*\(/;

const TODO_WHITELIST = new Set(
  (fs.existsSync("TODO_WHITELIST.txt") ? fs.readFileSync("TODO_WHITELIST.txt","utf8") : "")
    .split(/\r?\n/).map(s => s.trim()).filter(Boolean)
);

function isIgnored(rel) {
  if (IGNORE_GLOBS.some(re => re.test(rel))) return true;
  const parts = rel.split(/[\\/]/);
  return parts.some(p => IGNORE_DIRS.has(p));
}
function isTextFile(p) {
  const ext = path.extname(p).toLowerCase();
  return TEXT_EXT.has(ext);
}
function isTestPath(rel) {
  return /(^|\/)(tests?|__tests__|examples|benches)\//.test(rel) || /\.test\.(t|j)sx?$/.test(rel);
}
function isScriptPath(rel) {
  return /^scripts\//.test(rel);
}
function walk(dir) {
  const out = [];
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    const rel = path.relative(ROOT, p).replace(/\\/g, "/");
    if (isIgnored(rel)) continue;
    if (e.isDirectory()) out.push(...walk(p));
    else out.push(rel);
  }
  return out;
}
function* scanFile(rel, text) {
  const lines = text.split(/\r?\n/);
  const ext = path.extname(rel).toLowerCase();
  const rust = ext === ".rs";
  const jsLike = [".js",".ts",".tsx",".jsx",".mjs",".cjs"].includes(ext);

  // TODO/FIXME without tag, excluding whitelist
  for (let i=0;i<lines.length;i++) {
    const line = lines[i];
    if (TODO_PATTERNS.some(re => re.test(line))) {
      if (!ALLOW_TODO.test(line) && !TODO_WHITELIST.has(line.trim())) {
        yield { rel, line: i+1, msg: "Raw TODO/FIXME/XXX/HACK without tag", snippet: line.trim() };
      }
    }
  }
  if (rust && !isTestPath(rel)) {
    for (let i=0;i<lines.length;i++) {
      const line = lines[i];
      if (RS_FORBID_PATTERNS.some(re => re.test(line))) {
        yield { rel, line: i+1, msg: "panic/unwrap/expect/dbg forbidden outside tests", snippet: line.trim() };
      }
    }
  }
  if (jsLike) {
    for (let i=0;i<lines.length;i++) {
      const line = lines[i];
      if (JS_EVAL_PATTERNS.some(re => re.test(line))) {
        yield { rel, line: i+1, msg: "eval/new Function/timers forbidden", snippet: line.trim() };
      }
      if (!isTestPath(rel) && !isScriptPath(rel) && CONSOLE_PATTERN.test(line)) {
        yield { rel, line: i+1, msg: "console.* forbidden outside scripts/tests", snippet: line.trim() };
      }
    }
  }
}

function main() {
  const files = walk(ROOT).filter(isTextFile);
  const violations = [];
  for (const rel of files) {
    const text = fs.readFileSync(rel, "utf8");
    for (const v of scanFile(rel, text)) violations.push(v);
  }
  if (violations.length) {
    console.error("❌ PR sanity failed. Resolve or whitelist these issues:\n");
    for (const v of violations) {
      console.error(` - ${v.rel}:${v.line}  ${v.msg}\n     ${v.snippet}`);
    }
    console.error("\nAllowed TODO forms: TODO(#123) or TODO(owner:note) or exact line in TODO_WHITELIST.txt");
    process.exit(1);
  }
  console.log("✅ PR sanity ok");
}
main();
