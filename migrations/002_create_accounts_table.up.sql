-- Create accounts table
CREATE TABLE IF NOT EXISTS accounts (
    ida INTEGER PRIMARY KEY AUTOINCREMENT,
    idu INTEGER NOT NULL,
    kind TEXT NOT NULL,
    token TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (idu) REFERENCES users(idu)
);

-- Create index on idu for faster lookups
CREATE INDEX IF NOT EXISTS idx_accounts_idu ON accounts(idu);
CREATE INDEX IF NOT EXISTS idx_accounts_kind ON accounts(kind);

