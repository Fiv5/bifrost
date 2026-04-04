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
  sort_order  INTEGER NOT NULL DEFAULT 0,
  create_time TEXT NOT NULL,
  update_time TEXT NOT NULL,
  UNIQUE(user_id, name)
);

CREATE TABLE IF NOT EXISTS bifrost_groups (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  avatar      TEXT DEFAULT '',
  description TEXT DEFAULT '',
  visibility  TEXT DEFAULT 'private',
  created_by  TEXT NOT NULL,
  create_time TEXT NOT NULL,
  update_time TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS bifrost_group_members (
  id          TEXT PRIMARY KEY,
  group_id    TEXT NOT NULL,
  user_id     TEXT NOT NULL,
  level       INTEGER DEFAULT 0,
  create_time TEXT NOT NULL,
  update_time TEXT NOT NULL,
  UNIQUE(group_id, user_id)
);

CREATE TABLE IF NOT EXISTS bifrost_group_settings (
  group_id       TEXT PRIMARY KEY,
  rules_enabled  INTEGER DEFAULT 1,
  visibility     TEXT DEFAULT 'private'
);

CREATE INDEX IF NOT EXISTS idx_bifrost_envs_user_id ON bifrost_envs(user_id);
CREATE INDEX IF NOT EXISTS idx_bifrost_users_token  ON bifrost_users(token);
CREATE INDEX IF NOT EXISTS idx_bifrost_group_members_group_id ON bifrost_group_members(group_id);
CREATE INDEX IF NOT EXISTS idx_bifrost_group_members_user_id  ON bifrost_group_members(user_id);
