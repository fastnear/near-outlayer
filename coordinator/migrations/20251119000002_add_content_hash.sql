-- Add content_hash column to wasm_cache for file integrity verification
-- content_hash is SHA256 of actual WASM bytes, while checksum is SHA256(repo:commit:target)

ALTER TABLE wasm_cache ADD COLUMN content_hash TEXT;

-- Index for content hash lookups (optional, mainly for debugging)
CREATE INDEX idx_cache_content_hash ON wasm_cache(content_hash);
