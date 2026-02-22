-- Create accounts table
CREATE TABLE IF NOT EXISTS accounts
(
    ida        INTEGER PRIMARY KEY AUTOINCREMENT,
    idu        INTEGER     NOT NULL,
    kind       TEXT        NOT NULL,
    token      TEXT UNIQUE NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (idu) REFERENCES users (idu)
);

-- Create index for faster lookups
CREATE INDEX IF NOT EXISTS idx_accounts_idu ON accounts (idu);
CREATE INDEX IF NOT EXISTS idx_accounts_kind ON accounts (kind);

-- Create accounts_monitor table
CREATE TABLE IF NOT EXISTS accounts_monitor
(
    ida         INTEGER,
    external_id TEXT PRIMARY KEY,
    updated_at  DATETIME,
    FOREIGN KEY (ida) REFERENCES accounts (ida)
);

-- Create index on idu for faster lookups
CREATE INDEX IF NOT EXISTS idx_accounts_monitor_external_id ON accounts_monitor (external_id);

