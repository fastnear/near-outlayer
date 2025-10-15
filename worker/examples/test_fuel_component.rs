use wasmtime::*;
use wasmtime::component::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wasm = std::fs::read("../wasi-examples/ai-ark/target/wasm32-wasip2/release/ai-ark.wasm")?;
    
    let mut config = Config::new();
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;
    
    match Component::from_binary(&engine, &wasm) {
        Ok(_component) => {
            println!("✅ Component loaded with fuel enabled!");
            let mut store = Store::new(&engine, ());
            match store.set_fuel(1_000_000) {
                Ok(_) => println!("✅ Fuel metering WORKS with component model!"),
                Err(e) => println!("❌ Set fuel failed: {}", e),
            }
        }
        Err(e) => println!("❌ Load component failed: {}", e),
    }
    Ok(())
}
