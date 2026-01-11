-- Drop api_keys tables (no longer used - replaced by payment_keys for HTTPS API)
-- The api_keys system was for attestation API access control but was never fully implemented
-- Attestations are now public endpoints with IP rate limiting

DROP TABLE IF EXISTS api_key_usage;
DROP TABLE IF EXISTS api_keys;
