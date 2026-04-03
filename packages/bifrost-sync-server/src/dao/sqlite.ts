import Database from 'better-sqlite3';
import crypto from 'crypto';
import path from 'path';
import fs from 'fs';
import { nanoid } from 'nanoid';
import type {
  Env, User, CreateEnvReq, UpdateEnvReq, SearchEnvQuery,
  Group, GroupMember, GroupSetting, UpdateGroupReq, SearchGroupQuery, UpdateGroupSettingReq,
} from '../types';
import type { IUserDao, IEnvDao, IGroupDao, IGroupMemberDao, IGroupSettingDao, IStorage } from './types';

export class SqliteUserDao implements IUserDao {
  constructor(private db: Database.Database) {}

  async findByToken(token: string): Promise<User | undefined> {
    return this.db
      .prepare('SELECT * FROM bifrost_users WHERE token = ?')
      .get(token) as User | undefined;
  }

  async findByUserId(userId: string): Promise<User | undefined> {
    return this.db
      .prepare('SELECT * FROM bifrost_users WHERE user_id = ?')
      .get(userId) as User | undefined;
  }

  async register(
    userId: string,
    password: string,
    fields: Partial<Pick<User, 'nickname' | 'avatar' | 'email'>>,
  ): Promise<User> {
    const now = new Date().toISOString();
    const id = nanoid();
    const salt = crypto.randomBytes(16).toString('hex');
    const hash = crypto.scryptSync(password, salt, 64).toString('hex');
    const passwordHash = `${salt}:${hash}`;

    this.db
      .prepare(
        `INSERT INTO bifrost_users (id, user_id, nickname, avatar, email, password_hash, create_time, update_time)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(id, userId, fields.nickname ?? '', fields.avatar ?? '', fields.email ?? '', passwordHash, now, now);
    return (await this.findByUserId(userId))!;
  }

  async verifyPassword(userId: string, password: string): Promise<boolean> {
    const user = await this.findByUserId(userId);
    if (!user || !user.password_hash) return false;
    const [salt, storedHash] = user.password_hash.split(':');
    const hash = crypto.scryptSync(password, salt, 64).toString('hex');
    const a = Buffer.from(hash, 'hex');
    const b = Buffer.from(storedHash, 'hex');
    if (a.length !== b.length) return false;
    return crypto.timingSafeEqual(a, b);
  }

  async saveToken(userId: string, token: string): Promise<void> {
    this.db
      .prepare('UPDATE bifrost_users SET token = ?, update_time = ? WHERE user_id = ?')
      .run(token, new Date().toISOString(), userId);
  }

  async clearToken(userId: string): Promise<void> {
    this.db
      .prepare('UPDATE bifrost_users SET token = NULL, update_time = ? WHERE user_id = ?')
      .run(new Date().toISOString(), userId);
  }
}

export class SqliteEnvDao implements IEnvDao {
  constructor(private db: Database.Database) {}

  async findById(id: string): Promise<Env | undefined> {
    return this.db.prepare('SELECT * FROM bifrost_envs WHERE id = ?').get(id) as Env | undefined;
  }

  async findByUserAndName(userId: string, name: string): Promise<Env | undefined> {
    return this.db
      .prepare('SELECT * FROM bifrost_envs WHERE user_id = ? AND name = ?')
      .get(userId, name) as Env | undefined;
  }

  async create(req: CreateEnvReq): Promise<Env> {
    const now = new Date().toISOString();
    const id = nanoid();
    this.db
      .prepare(
        'INSERT INTO bifrost_envs (id, user_id, name, rule, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?)',
      )
      .run(id, req.user_id, req.name, req.rule ?? '', now, now);
    return (await this.findById(id))!;
  }

  async update(id: string, fields: UpdateEnvReq): Promise<Env | undefined> {
    const existing = await this.findById(id);
    if (!existing) return undefined;
    const now = new Date().toISOString();
    this.db
      .prepare('UPDATE bifrost_envs SET user_id = ?, name = ?, rule = ?, update_time = ? WHERE id = ?')
      .run(
        fields.user_id ?? existing.user_id,
        fields.name ?? existing.name,
        fields.rule ?? existing.rule,
        now,
        id,
      );
    return (await this.findById(id))!;
  }

  async delete(id: string): Promise<boolean> {
    const result = this.db.prepare('DELETE FROM bifrost_envs WHERE id = ?').run(id);
    return result.changes > 0;
  }

  async deleteByUserId(userId: string): Promise<number> {
    const result = this.db.prepare('DELETE FROM bifrost_envs WHERE user_id = ?').run(userId);
    return result.changes;
  }

  async search(query: SearchEnvQuery): Promise<{ list: Env[]; total: number }> {
    const conditions: string[] = [];
    const params: unknown[] = [];

    if (query.user_id) {
      const userIds = Array.isArray(query.user_id) ? query.user_id : [query.user_id];
      conditions.push(`user_id IN (${userIds.map(() => '?').join(', ')})`);
      params.push(...userIds);
    }
    if (query.keyword) {
      conditions.push('name LIKE ?');
      params.push(`%${query.keyword}%`);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const offset = query.offset ?? 0;
    const limit = query.limit ?? 500;

    const countRow = this.db
      .prepare(`SELECT COUNT(*) as total FROM bifrost_envs ${where}`)
      .get(...params) as { total: number };
    const list = this.db
      .prepare(`SELECT * FROM bifrost_envs ${where} ORDER BY update_time DESC LIMIT ? OFFSET ?`)
      .all(...params, limit, offset) as Env[];

    return { list, total: countRow.total };
  }
}

export class SqliteGroupDao implements IGroupDao {
  constructor(private db: Database.Database) {}

  async create(
    name: string,
    avatar: string,
    description: string,
    visibility: string,
    createdBy: string,
  ): Promise<Group> {
    const now = new Date().toISOString();
    const id = nanoid();
    this.db
      .prepare(
        `INSERT INTO bifrost_groups (id, name, avatar, description, visibility, created_by, create_time, update_time)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(id, name, avatar, description, visibility, createdBy, now, now);
    return (await this.findById(id))!;
  }

  async findById(id: string): Promise<Group | undefined> {
    return this.db
      .prepare('SELECT * FROM bifrost_groups WHERE id = ?')
      .get(id) as Group | undefined;
  }

  async findByName(name: string): Promise<Group | undefined> {
    return this.db.prepare('SELECT * FROM bifrost_groups WHERE name = ?').get(name) as Group | undefined;
  }

  async update(id: string, fields: UpdateGroupReq): Promise<Group | undefined> {
    const existing = await this.findById(id);
    if (!existing) return undefined;
    const now = new Date().toISOString();
    const sets: string[] = [];
    const params: unknown[] = [];
    if (fields.name !== undefined) {
      sets.push('name = ?');
      params.push(fields.name);
    }
    if (fields.avatar !== undefined) {
      sets.push('avatar = ?');
      params.push(fields.avatar);
    }
    if (fields.description !== undefined) {
      sets.push('description = ?');
      params.push(fields.description);
    }
    if (sets.length === 0) return existing;
    sets.push('update_time = ?');
    params.push(now, id);
    this.db
      .prepare(`UPDATE bifrost_groups SET ${sets.join(', ')} WHERE id = ?`)
      .run(...params);
    return (await this.findById(id))!;
  }

  async delete(id: string): Promise<boolean> {
    this.db.prepare('DELETE FROM bifrost_group_members WHERE group_id = ?').run(id);
    this.db.prepare('DELETE FROM bifrost_group_settings WHERE group_id = ?').run(id);
    const result = this.db.prepare('DELETE FROM bifrost_groups WHERE id = ?').run(id);
    return result.changes > 0;
  }

  async search(
    query: SearchGroupQuery,
    userId?: string,
  ): Promise<{ list: Group[]; total: number }> {
    const offset = query.offset ?? 0;
    const limit = query.limit ?? 500;
    const uid = query.user_id ?? userId;

    if (query.keyword) {
      const countRow = this.db
        .prepare(
          `SELECT COUNT(*) as total FROM bifrost_groups g
           WHERE g.name LIKE ?
           AND (g.visibility = 'public' OR EXISTS (
             SELECT 1 FROM bifrost_group_members m WHERE m.group_id = g.id AND m.user_id = ?
           ))`,
        )
        .get(`%${query.keyword}%`, uid ?? '') as { total: number };
      const list = this.db
        .prepare(
          `SELECT g.*, (SELECT m.level FROM bifrost_group_members m WHERE m.group_id = g.id AND m.user_id = ?) as level
           FROM bifrost_groups g
           WHERE g.name LIKE ?
           AND (g.visibility = 'public' OR EXISTS (
             SELECT 1 FROM bifrost_group_members m WHERE m.group_id = g.id AND m.user_id = ?
           ))
           ORDER BY g.update_time DESC LIMIT ? OFFSET ?`,
        )
        .all(uid ?? '', `%${query.keyword}%`, uid ?? '', limit, offset) as Group[];
      return { list, total: countRow.total };
    }

    if (uid) {
      const countRow = this.db
        .prepare(
          `SELECT COUNT(*) as total FROM bifrost_groups g
           INNER JOIN bifrost_group_members m ON g.id = m.group_id
           WHERE m.user_id = ?`,
        )
        .get(uid) as { total: number };
      const list = this.db
        .prepare(
          `SELECT g.*, m.level FROM bifrost_groups g
           INNER JOIN bifrost_group_members m ON g.id = m.group_id
           WHERE m.user_id = ?
           ORDER BY g.update_time DESC LIMIT ? OFFSET ?`,
        )
        .all(uid, limit, offset) as Group[];
      return { list, total: countRow.total };
    }

    const countRow = this.db
      .prepare('SELECT COUNT(*) as total FROM bifrost_groups')
      .get() as { total: number };
    const list = this.db
      .prepare('SELECT * FROM bifrost_groups ORDER BY update_time DESC LIMIT ? OFFSET ?')
      .all(limit, offset) as Group[];
    return { list, total: countRow.total };
  }
}

export class SqliteGroupMemberDao implements IGroupMemberDao {
  constructor(private db: Database.Database) {}

  async add(groupId: string, userId: string, level: number): Promise<GroupMember> {
    const now = new Date().toISOString();
    const id = nanoid();
    this.db
      .prepare(
        `INSERT INTO bifrost_group_members (id, group_id, user_id, level, create_time, update_time)
         VALUES (?, ?, ?, ?, ?, ?)`,
      )
      .run(id, groupId, userId, level, now, now);
    return (await this.findByGroupAndUser(groupId, userId))!;
  }

  async remove(groupId: string, userId: string): Promise<boolean> {
    const result = this.db
      .prepare('DELETE FROM bifrost_group_members WHERE group_id = ? AND user_id = ?')
      .run(groupId, userId);
    return result.changes > 0;
  }

  async updateLevel(groupId: string, userId: string, level: number): Promise<boolean> {
    const now = new Date().toISOString();
    const result = this.db
      .prepare('UPDATE bifrost_group_members SET level = ?, update_time = ? WHERE group_id = ? AND user_id = ?')
      .run(level, now, groupId, userId);
    return result.changes > 0;
  }

  async findByGroupAndUser(groupId: string, userId: string): Promise<GroupMember | undefined> {
    return this.db
      .prepare(
        `SELECT m.id, m.group_id, m.user_id, m.level, m.create_time, m.update_time,
                u.nickname, u.avatar, u.email
         FROM bifrost_group_members m
         LEFT JOIN bifrost_users u ON m.user_id = u.user_id
         WHERE m.group_id = ? AND m.user_id = ?`,
      )
      .get(groupId, userId) as GroupMember | undefined;
  }

  async listByGroup(
    groupId: string,
    query?: { keyword?: string; offset?: number; limit?: number },
  ): Promise<{ list: GroupMember[]; total: number }> {
    const offset = query?.offset ?? 0;
    const limit = query?.limit ?? 500;
    const conditions: string[] = ['m.group_id = ?'];
    const params: unknown[] = [groupId];

    if (query?.keyword) {
      conditions.push('(m.user_id LIKE ? OR u.nickname LIKE ?)');
      params.push(`%${query.keyword}%`, `%${query.keyword}%`);
    }

    const where = conditions.join(' AND ');

    const countRow = this.db
      .prepare(
        `SELECT COUNT(*) as total FROM bifrost_group_members m
         LEFT JOIN bifrost_users u ON m.user_id = u.user_id
         WHERE ${where}`,
      )
      .get(...params) as { total: number };
    const list = this.db
      .prepare(
        `SELECT m.id, m.group_id, m.user_id, m.level, m.create_time, m.update_time,
                u.nickname, u.avatar, u.email
         FROM bifrost_group_members m
         LEFT JOIN bifrost_users u ON m.user_id = u.user_id
         WHERE ${where}
         ORDER BY m.create_time ASC LIMIT ? OFFSET ?`,
      )
      .all(...params, limit, offset) as GroupMember[];

    return { list, total: countRow.total };
  }

  async listByUser(userId: string): Promise<GroupMember[]> {
    return this.db
      .prepare(
        `SELECT m.id, m.group_id, m.user_id, m.level, m.create_time, m.update_time,
                u.nickname, u.avatar, u.email
         FROM bifrost_group_members m
         LEFT JOIN bifrost_users u ON m.user_id = u.user_id
         WHERE m.user_id = ?
         ORDER BY m.create_time ASC`,
      )
      .all(userId) as GroupMember[];
  }
}

export class SqliteGroupSettingDao implements IGroupSettingDao {
  constructor(private db: Database.Database) {}

  async init(groupId: string): Promise<void> {
    this.db
      .prepare(
        `INSERT OR IGNORE INTO bifrost_group_settings (group_id, rules_enabled, visibility)
         VALUES (?, 1, 'private')`,
      )
      .run(groupId);
  }

  async get(groupId: string): Promise<GroupSetting> {
    const row = this.db
      .prepare('SELECT * FROM bifrost_group_settings WHERE group_id = ?')
      .get(groupId) as GroupSetting | undefined;
    if (row) return row;
    return { group_id: groupId, rules_enabled: 1, visibility: 'private' };
  }

  async update(groupId: string, fields: UpdateGroupSettingReq): Promise<void> {
    const sets: string[] = [];
    const params: unknown[] = [];
    if (fields.rules_enabled !== undefined) {
      sets.push('rules_enabled = ?');
      params.push(fields.rules_enabled ? 1 : 0);
    }
    if (fields.visibility !== undefined) {
      sets.push('visibility = ?');
      params.push(fields.visibility);
    }
    if (sets.length === 0) return;
    params.push(groupId);
    this.db
      .prepare(`UPDATE bifrost_group_settings SET ${sets.join(', ')} WHERE group_id = ?`)
      .run(...params);
  }
}

export class SqliteStorage implements IStorage {
  public user: SqliteUserDao;
  public env: SqliteEnvDao;
  public group: SqliteGroupDao;
  public groupMember: SqliteGroupMemberDao;
  public groupSetting: SqliteGroupSettingDao;
  private db: Database.Database;

  constructor(dataDir: string) {
    fs.mkdirSync(dataDir, { recursive: true });
    const dbPath = path.join(dataDir, 'bifrost-sync.db');
    this.db = new Database(dbPath);
    this.db.pragma('journal_mode = WAL');
    this.db.pragma('foreign_keys = ON');
    this.migrate();
    this.user = new SqliteUserDao(this.db);
    this.env = new SqliteEnvDao(this.db);
    this.group = new SqliteGroupDao(this.db);
    this.groupMember = new SqliteGroupMemberDao(this.db);
    this.groupSetting = new SqliteGroupSettingDao(this.db);
  }

  private migrate() {
    this.db.exec(`
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
      CREATE INDEX IF NOT EXISTS idx_bifrost_group_members_group_id ON bifrost_group_members(group_id);
      CREATE INDEX IF NOT EXISTS idx_bifrost_group_members_user_id  ON bifrost_group_members(user_id);
      CREATE TABLE IF NOT EXISTS bifrost_group_settings (
        group_id       TEXT PRIMARY KEY,
        rules_enabled  INTEGER DEFAULT 1,
        visibility     TEXT DEFAULT 'private'
      );
    `);
  }

  async close(): Promise<void> {
    this.db.close();
  }
}
