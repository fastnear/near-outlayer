#![forbid(unsafe_code)]
#![deny(warnings)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::dbg_macro
)]

//! QuickJS demo-mode executor for OutLayer.
//! - No custom syscalls/imports: everything is done via a preopened /work directory.
//! - JS loader creates a `near` shim with storage backed by /work/state.json (deterministic).
//! - Contract source is provided as `contract.js`; loader evaluates it and calls the requested function.
//!
//! Usage: construct with a QuickJS WASM binary (Vec<u8>), then call `execute`.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;

const LOADER_NAME: &str = "loader.mjs";
const CONTRACT_NAME: &str = "contract.js";
const STATE_NAME: &str = "state.json";
const ARGS_NAME: &str = "args.json";
const OUT_NAME: &str = "out.json";

/// Input for a single invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invocation<'a> {
    /// JavaScript contract source code.
    pub contract_source: &'a str,
    /// Function to call (e.g. "increment").
    pub function: &'a str,
    /// Positional arguments to pass to the function (JSON-serializable).
    #[serde(default)]
    pub args: serde_json::Value,
    /// Prior state blob (JSON string bytes). If empty, `{}` will be used.
    pub prior_state_json: &'a [u8],
}

/// Output of a single invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationResult {
    /// New state JSON bytes written by the contract (pretty JSON).
    pub new_state_json: Vec<u8>,
    /// Function result as JSON, if any.
    pub result: serde_json::Value,
    /// Captured loader log lines (stderr), if you decide to plumb them later.
    #[serde(default)]
    pub logs: Vec<String>,
}

/// Config for the QuickJS executor.
#[derive(Debug, Clone)]
pub struct QuickJsConfig {
    /// Epoch-based wall-clock budget.
    pub max_wall_time: Duration,
    /// Fuel budget (instruction-ish accounting).
    pub max_fuel: u64,
}

/// Demo-mode QuickJS executor (WASI, no custom syscalls).
pub struct QuickJsExecutor {
    engine: Engine,
    module: Module,
    cfg: QuickJsConfig,
}

impl QuickJsExecutor {
    /// Create a new executor from QuickJS WASM bytes.
    pub fn new(quickjs_wasm: &[u8], cfg: QuickJsConfig) -> Result<Self> {
        let mut wcfg = wasmtime::Config::new();
        wcfg.consume_fuel(true);
        wcfg.epoch_interruption(true);
        wcfg.debug_info(false);
        let engine = Engine::new(&wcfg).context("create wasmtime engine")?;
        let module = Module::new(&engine, quickjs_wasm).context("compile quickjs module")?;
        Ok(Self { engine, module, cfg })
    }

    /// Execute a contract function. Deterministic within the JS you supply.
    ///
    /// - Writes files into a temp dir preopened as `/work`
    /// - Launches qjs with `-m /work/loader.mjs`
    /// - Loader evaluates `/work/contract.js`, calls `args.function`, persists `/work/state.json`,
    ///   and writes `/work/out.json` with `{ ok, result }`
    pub fn execute(&self, inv: &Invocation) -> Result<InvocationResult> {
        let tmp = TempDir::new().context("create temp dir")?;
        let work = tmp.path();

        // 1) Materialize /work payload
        self.write_loader(work)?;
        self.write_contract(work, inv.contract_source)?;
        self.write_state(work, inv.prior_state_json)?;
        self.write_args(work, inv.function, &inv.args)?;

        // 2) WASI store with preopened /work
        use wasmtime_wasi::{DirPerms, FilePerms};

        let mut wasi_builder = WasiCtxBuilder::new();
        wasi_builder.inherit_stdout();
        wasi_builder.inherit_stderr();

        // Preopen /work directory with read/write permissions
        // Signature in Wasmtime 27: preopened_dir(host_path: impl AsRef<Path>, guest_path: impl AsRef<str>, dir_perms: DirPerms, file_perms: FilePerms)
        wasi_builder.preopened_dir(work, "/work", DirPerms::all(), FilePerms::all())?;

        // Set argv for QuickJS
        wasi_builder.arg("qjs");
        wasi_builder.arg("-m");
        wasi_builder.arg("/work/loader.mjs");

        let wasi_ctx = wasi_builder.build_p1();
        let mut store = Store::new(&self.engine, wasi_ctx);
        store.set_fuel(self.cfg.max_fuel).context("set fuel")?;

        // Epoch watchdog (simple, cooperative)
        let engine = self.engine.clone();
        let deadline = self.cfg.max_wall_time;
        let _ticker = std::thread::spawn(move || {
            let start = std::time::Instant::now();
            while start.elapsed() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(5));
                engine.increment_epoch();
            }
        });

        // 3) Linker + instantiate + run
        let mut linker = Linker::new(&self.engine);
        preview1::add_to_linker_sync(&mut linker, |cx: &mut WasiP1Ctx| cx).context("link wasi")?;
        let instance = linker
            .instantiate(&mut store, &self.module)
            .context("instantiate quickjs")?;

        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .context("missing _start")?;
        start
            .call(&mut store, ())
            .map_err(|e| anyhow!("quickjs _start failed: {e}"))?;

        drop(store); // release any handles to /work

        // 4) Read results
        let out_path = work.join(OUT_NAME);
        if !out_path.exists() {
            return Err(anyhow!("loader did not write {OUT_NAME}"));
        }
        let out_bytes = fs::read(&out_path).context("read out.json")?;
        let parsed: serde_json::Value = serde_json::from_slice(&out_bytes)
            .context("parse out.json")?;
        let result = parsed.get("result").cloned().unwrap_or(json!(null));

        let new_state = fs::read(work.join(STATE_NAME)).context("read state.json")?;

        Ok(InvocationResult {
            new_state_json: new_state,
            result,
            logs: Vec::new(),
        })
    }

    fn write_loader(&self, work: &Path) -> Result<()> {
        let loader = LOADER_MJS;
        let p = work.join(LOADER_NAME);
        fs::write(&p, loader).with_context(|| format!("write {p:?}"))
    }

    fn write_contract(&self, work: &Path, src: &str) -> Result<()> {
        let p = work.join(CONTRACT_NAME);
        fs::write(&p, src).with_context(|| format!("write {p:?}"))
    }

    fn write_state(&self, work: &Path, state_json: &[u8]) -> Result<()> {
        let p = work.join(STATE_NAME);
        if state_json.is_empty() {
            fs::write(&p, b"{}").with_context(|| format!("write {p:?}"))
        } else {
            // Validate it's JSON to avoid surprising parse failures in loader.
            let _v: serde_json::Value =
                serde_json::from_slice(state_json).context("prior_state is not valid JSON")?;
            fs::write(&p, state_json).with_context(|| format!("write {p:?}"))
        }
    }

    fn write_args(&self, work: &Path, function: &str, args: &serde_json::Value) -> Result<()> {
        let p = work.join(ARGS_NAME);
        let obj = json!({ "function": function, "args": args });
        let s = serde_json::to_vec_pretty(&obj)?;
        fs::write(&p, s).with_context(|| format!("write {p:?}"))
    }
}

// Loader script (module mode) — deterministic, file-backed `near` shim.
const LOADER_MJS: &str = r#"
import * as std from 'std';
import * as os from 'os';

// basic file helpers
function readFile(path) {
  const f = std.open(path, 'rb');
  if (!f) throw new Error('open failed: ' + path);
  const s = f.readAsString();
  f.close();
  return s;
}
function writeFile(path, content) {
  const f = std.open(path, 'wb');
  if (!f) throw new Error('open failed: ' + path);
  f.puts(content);
  f.close();
}
function loadJSON(path, defVal) {
  try { return JSON.parse(readFile(path)); } catch (e) { return defVal; }
}

const WORK = '/work';
const STATE_PATH = WORK + '/state.json';
const ARGS_PATH  = WORK + '/args.json';
const CONTRACT_PATH = WORK + '/contract.js';
const OUT_PATH = WORK + '/out.json';

// state & args
let state = loadJSON(STATE_PATH, {});
const args  = loadJSON(ARGS_PATH, { function: 'main', args: [] });
const fnName = args.function || 'main';
const fnArgs = Array.isArray(args.args) ? args.args : [];

// demo-mode near shim (deterministic)
globalThis.near = {
  storageRead: (k) => state[k],
  storageWrite: (k, v) => { state[k] = v; },
  log: (...a) => std.err.puts(a.join(' ') + '\n'),
};

// evaluate the contract in a clean function scope
const src = readFile(CONTRACT_PATH);
// Allow either ESM "export function x()" or UMD-ish attaching to globalThis.
// We evaluate source, then grab from globalThis if exported.
(function () {
  // Wrap source to allow both `export function` and plain functions on globalThis.
  // If ESM `export` is used, QuickJS module parser is required. We are in module mode (-m),
  // so top-level exports get attached to module namespace. We expose them via globalThis.
  // Easiest: eval inside a Function and rely on globalThis; devs can assign explicitly.
  (1, eval)(src);
})();

// Resolve callable
let target = globalThis[fnName];
if (typeof target !== 'function') {
  // try default export pattern
  if (globalThis.default && typeof globalThis.default[fnName] === 'function') {
    target = globalThis.default[fnName];
  }
}
if (typeof target !== 'function') {
  writeFile(OUT_PATH, JSON.stringify({ ok:false, error:`function ${fnName} not found` }));
  std.exit(0);
}

// Call
let result = null;
try {
  result = target.apply(null, fnArgs);
} catch (e) {
  writeFile(OUT_PATH, JSON.stringify({ ok:false, error: String(e) }));
  std.exit(0);
}

// Persist state and result
writeFile(STATE_PATH, JSON.stringify(state, null, 2));
writeFile(OUT_PATH, JSON.stringify({ ok:true, result }, null, 2));
"#;
