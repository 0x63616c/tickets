-- Bootstrap credentials are authoritative Worker secrets, never D1 data (ADR 0003).
ALTER TABLE users DROP COLUMN password_hash;
