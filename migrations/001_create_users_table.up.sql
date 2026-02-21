-- Create users table
CREATE TABLE IF NOT EXISTS users (
    idu INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create index on idu for faster lookups
CREATE INDEX IF NOT EXISTS idx_users_idu ON users(idu);

