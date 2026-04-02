import Database from 'better-sqlite3';
import crypto from 'crypto';
import path from 'path';
import fs from 'fs';
import { nanoid } from 'nanoid';
import type { Env, User, CreateEnvReq, UpdateEnvReq, SearchEnvQuery } from '../types';
import type { IUserDao, IEnvDao, IStorage } from './types';

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

export class SqliteStorage implements IStorage {
  public user: SqliteUserDao;
  public env: SqliteEnvDao;
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
    `);
  }

  async close(): Promise<void> {
    this.db.close();
  }
}
