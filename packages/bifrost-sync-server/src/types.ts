export interface Env {
  id: string;
  user_id: string;
  name: string;
  rule: string;
  create_time: string;
  update_time: string;
}

export interface User {
  id: string;
  user_id: string;
  nickname: string;
  avatar: string;
  email: string;
  password_hash: string;
  token: string;
  create_time: string;
  update_time: string;
}

export interface CreateEnvReq {
  user_id: string;
  name: string;
  rule?: string;
}

export interface UpdateEnvReq {
  id?: string;
  user_id?: string;
  name?: string;
  rule?: string;
}

export interface SearchEnvQuery {
  user_id?: string | string[];
  keyword?: string;
  offset?: number;
  limit?: number;
}

export interface ApiResponse<T = unknown> {
  code: number;
  message: string;
  data?: T;
}

export interface MysqlConfig {
  host: string;
  port: number;
  user: string;
  password: string;
  database: string;
}

export interface OAuth2Config {
  client_id: string;
  client_secret: string;
  authorize_url: string;
  token_url: string;
  userinfo_url: string;
  scopes: string[];
  redirect_uri?: string;
  user_id_field?: string;
  nickname_field?: string;
  email_field?: string;
  avatar_field?: string;
}

export interface AuthConfig {
  mode: 'password' | 'oauth2';
  oauth2?: OAuth2Config;
}

export interface StorageConfig {
  type: 'sqlite' | 'mysql';
  sqlite?: { data_dir: string };
  mysql?: MysqlConfig;
}

export interface ServerConfig {
  port: number;
  host: string;
}

export interface SyncServerConfig {
  server: ServerConfig;
  storage: StorageConfig;
  auth: AuthConfig;
}

export interface Group {
  id: string;
  name: string;
  avatar: string;
  description: string;
  visibility: string;
  created_by: string;
  create_time: string;
  update_time: string;
}

export interface GroupMember {
  id: string;
  group_id: string;
  user_id: string;
  level: number;
  nickname: string;
  avatar: string;
  email: string;
  create_time: string;
  update_time: string;
}

export interface GroupSetting {
  group_id: string;
  rules_enabled: number;
  visibility: string;
}

export interface CreateGroupReq {
  name: string;
  avatar?: string;
  description?: string;
  visibility?: string;
}

export interface UpdateGroupReq {
  name?: string;
  avatar?: string;
  description?: string;
}

export interface SearchGroupQuery {
  keyword?: string;
  user_id?: string;
  offset?: number;
  limit?: number;
}

export interface InviteGroupReq {
  user_ids: string[];
  level?: number;
}

export interface UpdateGroupSettingReq {
  rules_enabled?: boolean;
  visibility?: string;
}
