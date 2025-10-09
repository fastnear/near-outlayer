# Тестирование Worker

## 1. Тест WASM execution

Запускает executor изолированно для проверки правильности выполнения WASM.

### Подготовка:
```bash
# Собрать test-wasm
cd ../test-wasm
cargo build --release --target wasm32-unknown-unknown

# Вернуться в worker
cd ../worker
```

### Запуск теста:
```bash
cargo test test_wasm_execution -- --nocapture
```

### Ожидаемый результат:
```
✅ Loaded WASM: 75109 bytes
⚙️  Executing WASM...
✅ Execution result:
   Success: true
   Time: XXms
   Output: Some([...])
   Error: None
   Output as string: {"random_number": 42}
```

### Если тест падает:
- Проверь что test-wasm собран
- Проверь что WASM файл существует
- Проверь логи executor на детали ошибки

## 2. Просмотр логов основного worker

Сейчас в логах ты увидишь:

```
INFO  Received task: Compile { request_id: 16, data_id: "...", ... }
INFO  🔨 Starting compilation for request_id=16
INFO  ✅ Compilation successful: checksum=940bc...
INFO  📥 Downloading compiled WASM: checksum=940bc...
INFO  ✅ Downloaded WASM: 75109 bytes
INFO  ⚙️  Executing WASM for request_id=16 (size=75109 bytes)
INFO  WASM execution failed: WASM execution returned error code: -1
INFO  ✅ Execution completed: success=false, error=Some("...")
INFO  📤 Submitting result to NEAR contract
INFO  📡 Submitting execution result: data_id=..., success=false
INFO  📦 data_id bytes (first 8): [199, 207, ...]
INFO  📦 data_id as base58: ESygK5a7n...
INFO  📤 Full args for resolve_execution: {"data_id":"...","response":{...}}
INFO  🔗 Sending transaction:
INFO     Contract: offchainvm.testnet
INFO     Signer: worker.offchainvm.testnet
INFO     Method: resolve_execution
INFO     Gas: 100 TGas
ERROR ❌ Failed to submit result to NEAR: ...
ERROR Full error chain:
ERROR   [0] Failed to call resolve_execution
ERROR   [1] <детали ошибки от NEAR RPC>
```

## 3. Проверка конкретных проблем

### Проблема 1: WASM execution error -1
```bash
cargo test test_wasm_execution -- --nocapture
```
Это покажет детали ошибки wasmi.

### Проблема 2: resolve_execution failed
Смотри в логах:
- Какой signer используется (должен быть operator)
- Какие args отправляются (формат data_id)
- Полную цепочку ошибок от NEAR

## 4. Debug executor

Если нужно больше логов от executor:
```bash
RUST_LOG=offchainvm_worker=debug cargo test test_wasm_execution -- --nocapture
```

Или в основном worker:
```bash
RUST_LOG=offchainvm_worker=debug,offchainvm_worker::executor=trace cargo run
```
