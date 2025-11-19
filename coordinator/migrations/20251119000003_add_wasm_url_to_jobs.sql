-- Add wasm_url and wasm_hash columns to jobs table for WasmUrl code sources
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS wasm_url TEXT;
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS wasm_content_hash VARCHAR(64);

-- Add build_target to preserve the original value
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS build_target VARCHAR(50);

COMMENT ON COLUMN jobs.wasm_url IS 'URL for pre-compiled WASM (WasmUrl code source)';
COMMENT ON COLUMN jobs.wasm_content_hash IS 'SHA256 hash of WASM content for verification (WasmUrl code source)';
COMMENT ON COLUMN jobs.build_target IS 'Build target (wasm32-wasip1 or wasm32-wasip2)';
