use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use wasmtime::component::{Component, Linker as ComponentLinker};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

#[derive(Parser)]
#[command(name = "wasi-test")]
#[command(about = "Test runner for WASI modules for NEAR OutLayer compatibility")]
struct Args {
    /// Path to WASM file
    #[arg(short, long)]
    wasm: PathBuf,

    /// Input JSON data (or use --input-file)
    #[arg(short, long, conflicts_with = "input_file")]
    input: Option<String>,

    /// Path to input JSON file
    #[arg(long)]
    input_file: Option<PathBuf>,

    /// Maximum instructions (fuel limit)
    #[arg(long, default_value = "10000000000")]
    max_instructions: u64,

    /// Maximum memory in MB
    #[arg(long, default_value = "128")]
    max_memory_mb: u64,

    /// Environment variables (format: KEY=value, can be specified multiple times)
    #[arg(short, long)]
    env: Vec<String>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

struct HostState {
    wasi_ctx: WasiCtx,
    wasi_http_ctx: WasiHttpCtx,
    table: ResourceTable,
}

impl WasiView for HostState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

impl WasiHttpView for HostState {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.wasi_http_ctx
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Read input data
    let input_data = if let Some(ref input) = args.input {
        input.clone()
    } else if let Some(ref path) = args.input_file {
        std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read input file: {}", path.display()))?
    } else {
        "{}".to_string() // Empty JSON by default
    };

    // Validate input is valid JSON
    serde_json::from_str::<serde_json::Value>(&input_data)
        .context("Input is not valid JSON")?;

    // Read WASM bytes
    let wasm_bytes = std::fs::read(&args.wasm)
        .with_context(|| format!("Failed to read WASM file: {}", args.wasm.display()))?;

    println!("🔍 Testing WASM module: {}", args.wasm.display());
    println!("📝 Input: {}", input_data);
    println!("⚙️  Max instructions: {}", args.max_instructions);
    println!("💾 Max memory: {} MB", args.max_memory_mb);
    if !args.env.is_empty() {
        println!("🔑 Environment variables: {}", args.env.len());
    }
    println!();

    // Try to execute
    match execute_wasm(&wasm_bytes, &input_data, &args).await {
        Ok((output, fuel_consumed)) => {
            println!("✅ Execution successful!");
            println!();
            println!("📊 Results:");
            println!("  - Fuel consumed: {} instructions", fuel_consumed);
            println!("  - Output size: {} bytes", output.len());
            println!();
            println!("📤 Output:");
            println!("{}", String::from_utf8_lossy(&output));
            println!();

            // Validate output
            validate_output(&output)?;

            println!("✅ All checks passed! Module is compatible with NEAR OutLayer.");
            Ok(())
        }
        Err(e) => {
            println!("❌ Execution failed!");
            println!();
            println!("Error: {}", e);
            println!();
            println!("💡 Common issues:");
            println!("  - Make sure you're using [[bin]] format, not [lib]");
            println!("  - Check that you have fn main() as entry point");
            println!("  - Verify you're reading from stdin and writing to stdout");
            println!("  - Use correct build target (wasm32-wasip1 or wasm32-wasip2)");
            println!();
            println!("📚 See WASI_TUTORIAL.md for detailed guide");
            std::process::exit(1);
        }
    }
}

async fn execute_wasm(
    wasm_bytes: &[u8],
    input_data: &str,
    args: &Args,
) -> Result<(Vec<u8>, u64)> {
    // Try WASI P2 Component first
    if args.verbose {
        println!("🔄 Trying WASI Preview 2 component...");
    }

    let mut config = Config::new();
    config.wasm_component_model(true);
    config.async_support(true);
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;

    if let Ok(component) = Component::from_binary(&engine, wasm_bytes) {
        println!("✓ Detected: WASI Preview 2 Component");
        return execute_wasi_p2(&engine, &component, input_data, args).await;
    }

    // Try WASI P1 Module
    if args.verbose {
        println!("🔄 Trying WASI Preview 1 module...");
    }

    let mut module_config = Config::new();
    module_config.async_support(true);
    module_config.consume_fuel(true);
    let module_engine = Engine::new(&module_config)?;

    if let Ok(module) = wasmtime::Module::from_binary(&module_engine, wasm_bytes) {
        println!("✓ Detected: WASI Preview 1 Module");
        return execute_wasi_p1(&module_engine, &module, input_data, args).await;
    }

    anyhow::bail!(
        "Failed to load WASM binary. Not a valid WASI P1 module or P2 component.\n\
         Make sure you compiled with --target wasm32-wasip1 or wasm32-wasip2"
    )
}

async fn execute_wasi_p2(
    engine: &Engine,
    component: &Component,
    input_data: &str,
    args: &Args,
) -> Result<(Vec<u8>, u64)> {
    use wasmtime::component::Linker;
    use wasmtime_wasi::bindings::Command;

    let mut linker = Linker::new(engine);
    wasmtime_wasi::add_to_linker_async(&mut linker)?;
    wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker)?;

    let stdin_pipe = wasmtime_wasi::pipe::MemoryInputPipe::new(input_data.as_bytes().to_vec());
    let stdout_pipe =
        wasmtime_wasi::pipe::MemoryOutputPipe::new((args.max_memory_mb as usize) * 1024 * 1024);

    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder.stdin(stdin_pipe);
    wasi_builder.stdout(stdout_pipe.clone());
    wasi_builder.stderr(wasmtime_wasi::pipe::MemoryOutputPipe::new(1024 * 1024));
    wasi_builder.preopened_dir(
        "/tmp",
        ".",
        wasmtime_wasi::DirPerms::all(),
        wasmtime_wasi::FilePerms::all(),
    )?;

    // Add environment variables
    for env_var in &args.env {
        if let Some((key, value)) = env_var.split_once('=') {
            wasi_builder.env(key, value);
        }
    }

    let host_state = HostState {
        wasi_ctx: wasi_builder.build(),
        wasi_http_ctx: WasiHttpCtx::new(),
        table: ResourceTable::new(),
    };

    let mut store = Store::new(engine, host_state);
    store.set_fuel(args.max_instructions)?;

    if args.verbose {
        println!("🔄 Instantiating component...");
    }
    let command = Command::instantiate_async(&mut store, component, &linker).await?;

    if args.verbose {
        println!("🔄 Running command...");
    }
    command
        .wasi_cli_run()
        .call_run(&mut store)
        .await?
        .map_err(|_| anyhow::anyhow!("Command failed"))?;

    let fuel_consumed = args.max_instructions - store.get_fuel().unwrap_or(0);
    let output = stdout_pipe.contents().to_vec();

    Ok((output, fuel_consumed))
}

async fn execute_wasi_p1(
    engine: &Engine,
    module: &wasmtime::Module,
    input_data: &str,
    args: &Args,
) -> Result<(Vec<u8>, u64)> {
    let mut linker = wasmtime::Linker::new(engine);
    preview1::add_to_linker_async(&mut linker, |t: &mut WasiP1Ctx| t)?;

    let stdin_pipe = wasmtime_wasi::pipe::MemoryInputPipe::new(input_data.as_bytes().to_vec());
    let stdout_pipe =
        wasmtime_wasi::pipe::MemoryOutputPipe::new((args.max_memory_mb as usize) * 1024 * 1024);

    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder.stdin(stdin_pipe);
    wasi_builder.stdout(stdout_pipe.clone());
    wasi_builder.stderr(wasmtime_wasi::pipe::MemoryOutputPipe::new(1024 * 1024));

    // Add environment variables
    for env_var in &args.env {
        if let Some((key, value)) = env_var.split_once('=') {
            wasi_builder.env(key, value);
        }
    }

    let wasi_p1_ctx = wasi_builder.build_p1();

    let mut store = Store::new(engine, wasi_p1_ctx);
    store.set_fuel(args.max_instructions)?;

    if args.verbose {
        println!("🔄 Instantiating module...");
    }
    let instance = linker.instantiate_async(&mut store, module).await?;

    if args.verbose {
        println!("🔄 Calling _start...");
    }
    let start = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .context("Failed to find _start function. Make sure you're using [[bin]] format with fn main()")?;

    start.call_async(&mut store, ()).await?;

    let fuel_consumed = args.max_instructions - store.get_fuel().unwrap_or(0);
    let output = stdout_pipe.contents().to_vec();

    Ok((output, fuel_consumed))
}

fn validate_output(output: &[u8]) -> Result<()> {
    // Check output size
    if output.is_empty() {
        anyhow::bail!(
            "❌ Output is empty!\n\
             Make sure you:\n\
             - Write to stdout (use print!() or println!())\n\
             - Flush stdout: io::stdout().flush()?"
        );
    }

    if output.len() > 900 {
        println!(
            "⚠️  Warning: Output is {} bytes (limit is 900 bytes for NEAR)",
            output.len()
        );
        println!("   Consider truncating your output to fit the limit.");
    }

    // Try to parse as JSON
    let output_str = String::from_utf8_lossy(output);
    match serde_json::from_str::<serde_json::Value>(&output_str) {
        Ok(_) => {
            println!("✓ Output is valid JSON");
        }
        Err(e) => {
            println!("⚠️  Warning: Output is not valid JSON: {}", e);
            println!("   While not required, JSON output is recommended.");
        }
    }

    Ok(())
}
