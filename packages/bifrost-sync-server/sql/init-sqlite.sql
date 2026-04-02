-- Bifrost Sync Server: SQLite schema
-- Usage: sqlite3 bifrost-sync.db < init-sqlite.sql

CREATE TABLE IF NOT EXISTS bifrost_users (
  id            TEXT PRIMARY KEY,
  user_id       TEXT NOT NULL UNIQUE,
  nickname      TEXT NOT NULL DEFAULT '',
  avatar        TEXT NOT NULL DEFAULT '',
  email         TEXT NOT NULL DEFAULT '',
  password_hash TEXT NOT NULL DEFAULT '',
  token         TEXT,
  create_time   TEXT NOT NULL,
  update_time   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS bifrost_envs (
  id          TEXT PRIMARY KEY,
  user_id     TEXT NOT NULL,
  name        TEXT NOT NULL,
  rule        TEXT NOT NULL DEFAULT '',
  create_time TEXT NOT NULL,
  update_time TEXT NOT NULL,
  UNIQUE(user_id, name)
);

CREATE INDEX IF NOT EXISTS idx_bifrost_envs_user_id ON bifrost_envs(user_id);
CREATE INDEX IF NOT EXISTS idx_bifrost_users_token  ON bifrost_users(token);
