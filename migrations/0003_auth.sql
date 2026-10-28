-- Bounded fixed-window counters; addresses are hashed before persistence.
CREATE TABLE login_attempts (
 ip_hash TEXT PRIMARY KEY NOT NULL,
 window_start INTEGER NOT NULL,
 attempts INTEGER NOT NULL
);
