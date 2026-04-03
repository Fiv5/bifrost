-- Bifrost Sync Server: MySQL schema
-- Usage: mysql -u root -p bifrost_sync < init-mysql.sql
--
-- CREATE DATABASE IF NOT EXISTS bifrost_sync DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
-- USE bifrost_sync;

CREATE TABLE IF NOT EXISTS bifrost_users (
  id            VARCHAR(32)  NOT NULL PRIMARY KEY,
  user_id       VARCHAR(128) NOT NULL,
  nickname      VARCHAR(255) NOT NULL DEFAULT '',
  avatar        VARCHAR(512) NOT NULL DEFAULT '',
  email         VARCHAR(255) NOT NULL DEFAULT '',
  password_hash VARCHAR(255) NOT NULL DEFAULT '',
  token         VARCHAR(128) DEFAULT NULL,
  create_time   VARCHAR(32)  NOT NULL,
  update_time   VARCHAR(32)  NOT NULL,
  UNIQUE KEY uk_bifrost_user_id (user_id),
  KEY idx_bifrost_users_token (token)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS bifrost_envs (
  id          VARCHAR(32)  NOT NULL PRIMARY KEY,
  user_id     VARCHAR(128) NOT NULL,
  name        VARCHAR(255) NOT NULL,
  rule        LONGTEXT     NOT NULL,
  create_time VARCHAR(32)  NOT NULL,
  update_time VARCHAR(32)  NOT NULL,
  UNIQUE KEY uk_bifrost_user_env (user_id, name),
  KEY idx_bifrost_envs_user_id (user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS bifrost_groups (
  id          VARCHAR(32)  NOT NULL PRIMARY KEY,
  name        VARCHAR(255) NOT NULL,
  avatar      VARCHAR(512) DEFAULT '',
  description TEXT         DEFAULT NULL,
  visibility  VARCHAR(32)  DEFAULT 'private',
  created_by  VARCHAR(128) NOT NULL,
  create_time VARCHAR(32)  NOT NULL,
  update_time VARCHAR(32)  NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS bifrost_group_members (
  id          VARCHAR(32)  NOT NULL PRIMARY KEY,
  group_id    VARCHAR(32)  NOT NULL,
  user_id     VARCHAR(128) NOT NULL,
  level       INT          DEFAULT 0,
  create_time VARCHAR(32)  NOT NULL,
  update_time VARCHAR(32)  NOT NULL,
  UNIQUE KEY uk_bifrost_group_member (group_id, user_id),
  KEY idx_bifrost_group_members_group_id (group_id),
  KEY idx_bifrost_group_members_user_id (user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS bifrost_group_settings (
  group_id       VARCHAR(32) NOT NULL PRIMARY KEY,
  rules_enabled  INT         DEFAULT 1,
  visibility     VARCHAR(32) DEFAULT 'private'
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
