# Worker Examples

Development examples and standalone test programs.

## test_fuel_component.rs

**Purpose**: Standalone test for WASI P2 component fuel metering.

**What it does**:
- Loads a WASI P2 component (ai-ark)
- Executes with wasmtime
- Measures fuel consumption
- Demonstrates component instantiation

**Usage**:
```bash
# Build ai-ark component first
cd ../../wasi-examples/ai-ark
cargo build --release --target wasm32-wasip2

# Run example
cd ../../worker
cargo run --example test_fuel_component
```

**Expected output**:
```
Component size: 500000 bytes
Fuel consumed: 8663 instructions
Output: Error: OPENAI_API_KEY not found...
```

**When to use**:
- Debugging component loading issues
- Testing fuel metering accuracy
- Isolated component execution testing
- Development of new component features

**Note**: This is a development tool, not part of the automated test suite.

## Adding New Examples

Create `examples/your_example.rs`:
```rust
fn main() {
    println!("Example logic here");
}
```

Run with:
```bash
cargo run --example your_example
```

---

For production tests, see [/tests/README.md](../../tests/README.md)
