-- Bootstrap credentials are authoritative Worker secrets, never D1 data.
-- Retain the required legacy column for compatibility with the data-model schema.
UPDATE users SET password_hash = '';
