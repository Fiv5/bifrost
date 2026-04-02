import type { Env, User, CreateEnvReq, UpdateEnvReq, SearchEnvQuery } from '../types';

export interface IUserDao {
  findByToken(token: string): Promise<User | undefined>;
  findByUserId(userId: string): Promise<User | undefined>;
  register(
    userId: string,
    password: string,
    fields: Partial<Pick<User, 'nickname' | 'avatar' | 'email'>>,
  ): Promise<User>;
  verifyPassword(userId: string, password: string): Promise<boolean>;
  saveToken(userId: string, token: string): Promise<void>;
  clearToken(userId: string): Promise<void>;
}

export interface IEnvDao {
  findById(id: string): Promise<Env | undefined>;
  findByUserAndName(userId: string, name: string): Promise<Env | undefined>;
  create(req: CreateEnvReq): Promise<Env>;
  update(id: string, fields: UpdateEnvReq): Promise<Env | undefined>;
  delete(id: string): Promise<boolean>;
  search(query: SearchEnvQuery): Promise<{ list: Env[]; total: number }>;
}

export interface IStorage {
  user: IUserDao;
  env: IEnvDao;
  close(): Promise<void>;
}
