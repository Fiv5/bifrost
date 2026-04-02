import crypto from 'crypto';
import mysql, { type Pool, type RowDataPacket, type ResultSetHeader } from 'mysql2/promise';
import { nanoid } from 'nanoid';
import type { Env, User, CreateEnvReq, UpdateEnvReq, SearchEnvQuery, MysqlConfig } from '../types';
import type { IUserDao, IEnvDao, IStorage } from './types';

function rowToUser(row: RowDataPacket): User {
  return {
    id: row.id,
    user_id: row.user_id,
    nickname: row.nickname,
    avatar: row.avatar,
    email: row.email,
    password_hash: row.password_hash,
    token: row.token,
    create_time: row.create_time,
    update_time: row.update_time,
  };
}

function rowToEnv(row: RowDataPacket): Env {
  return {
    id: row.id,
    user_id: row.user_id,
    name: row.name,
    rule: row.rule,
    create_time: row.create_time,
    update_time: row.update_time,
  };
}

export class MysqlUserDao implements IUserDao {
  constructor(private pool: Pool) {}

  async findByToken(token: string): Promise<User | undefined> {
    const [rows] = await this.pool.execute<RowDataPacket[]>(
      'SELECT * FROM bifrost_users WHERE token = ?',
      [token],
    );
    return rows.length > 0 ? rowToUser(rows[0]) : undefined;
  }

  async findByUserId(userId: string): Promise<User | undefined> {
    const [rows] = await this.pool.execute<RowDataPacket[]>(
      'SELECT * FROM bifrost_users WHERE user_id = ?',
      [userId],
    );
    return rows.length > 0 ? rowToUser(rows[0]) : undefined;
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

    await this.pool.execute(
      `INSERT INTO bifrost_users (id, user_id, nickname, avatar, email, password_hash, create_time, update_time)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      [id, userId, fields.nickname ?? '', fields.avatar ?? '', fields.email ?? '', passwordHash, now, now],
    );
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
    await this.pool.execute(
      'UPDATE bifrost_users SET token = ?, update_time = ? WHERE user_id = ?',
      [token, new Date().toISOString(), userId],
    );
  }

  async clearToken(userId: string): Promise<void> {
    await this.pool.execute(
      'UPDATE bifrost_users SET token = NULL, update_time = ? WHERE user_id = ?',
      [new Date().toISOString(), userId],
    );
  }
}

export class MysqlEnvDao implements IEnvDao {
  constructor(private pool: Pool) {}

  async findById(id: string): Promise<Env | undefined> {
    const [rows] = await this.pool.execute<RowDataPacket[]>(
      'SELECT * FROM bifrost_envs WHERE id = ?',
      [id],
    );
    return rows.length > 0 ? rowToEnv(rows[0]) : undefined;
  }

  async findByUserAndName(userId: string, name: string): Promise<Env | undefined> {
    const [rows] = await this.pool.execute<RowDataPacket[]>(
      'SELECT * FROM bifrost_envs WHERE user_id = ? AND name = ?',
      [userId, name],
    );
    return rows.length > 0 ? rowToEnv(rows[0]) : undefined;
  }

  async create(req: CreateEnvReq): Promise<Env> {
    const now = new Date().toISOString();
    const id = nanoid();
    await this.pool.execute(
      'INSERT INTO bifrost_envs (id, user_id, name, rule, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?)',
      [id, req.user_id, req.name, req.rule ?? '', now, now],
    );
    return (await this.findById(id))!;
  }

  async update(id: string, fields: UpdateEnvReq): Promise<Env | undefined> {
    const existing = await this.findById(id);
    if (!existing) return undefined;
    const now = new Date().toISOString();
    await this.pool.execute(
      'UPDATE bifrost_envs SET user_id = ?, name = ?, rule = ?, update_time = ? WHERE id = ?',
      [fields.user_id ?? existing.user_id, fields.name ?? existing.name, fields.rule ?? existing.rule, now, id],
    );
    return (await this.findById(id))!;
  }

  async delete(id: string): Promise<boolean> {
    const [result] = await this.pool.execute<ResultSetHeader>(
      'DELETE FROM bifrost_envs WHERE id = ?',
      [id],
    );
    return result.affectedRows > 0;
  }

  async search(query: SearchEnvQuery): Promise<{ list: Env[]; total: number }> {
    const conditions: string[] = [];
    const params: (string | number)[] = [];

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

    const [countRows] = await this.pool.execute<RowDataPacket[]>(
      `SELECT COUNT(*) as total FROM bifrost_envs ${where}`,
      params,
    );
    const total = countRows[0].total as number;

    const [rows] = await this.pool.execute<RowDataPacket[]>(
      `SELECT * FROM bifrost_envs ${where} ORDER BY update_time DESC LIMIT ? OFFSET ?`,
      [...params, limit, offset],
    );

    return { list: rows.map(rowToEnv), total };
  }
}

export class MysqlStorage implements IStorage {
  public user: MysqlUserDao;
  public env: MysqlEnvDao;
  private pool: Pool;

  constructor(config: MysqlConfig) {
    this.pool = mysql.createPool({
      host: config.host,
      port: config.port,
      user: config.user,
      password: config.password,
      database: config.database,
      waitForConnections: true,
      connectionLimit: 10,
    });
    this.user = new MysqlUserDao(this.pool);
    this.env = new MysqlEnvDao(this.pool);
  }

  async close(): Promise<void> {
    await this.pool.end();
  }
}
