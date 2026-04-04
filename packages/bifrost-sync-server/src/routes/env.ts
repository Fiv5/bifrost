import type { IStorage } from '../dao/types';
import type { RequestContext } from '../http';
import {
  sendJson,
  sendError,
  requireAuth,
  parseJsonBody,
  extractPathParam,
  parseQueryAll,
} from '../http';
import type { CreateEnvReq, UpdateEnvReq, SearchEnvQuery } from '../types';

async function checkEditable(userId: string, currentUserId: string, storage: IStorage): Promise<boolean> {
  if (userId === currentUserId) return true;
  const group = await storage.group.findByName(userId);
  if (!group) return false;
  const member = await storage.groupMember.findByGroupAndUser(group.id, currentUserId);
  return !!member && member.level >= 1;
}

async function checkReadable(userId: string, currentUserId: string, storage: IStorage): Promise<boolean> {
  if (userId === currentUserId) return true;
  const group = await storage.group.findByName(userId);
  if (!group) return false;
  const setting = await storage.groupSetting.get(group.id);
  if (setting.visibility === 'public') return true;
  const member = await storage.groupMember.findByGroupAndUser(group.id, currentUserId);
  return !!member;
}

export async function handleEnv(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  const { url, req } = ctx;
  const method = req.method ?? 'GET';
  const pathname = url.pathname.replace(/\/$/, '') || '/';

  if (pathname === '/v4/env/sync' && method === 'POST') {
    return handleSync(ctx, storage);
  }
  if (pathname === '/v4/env_search_name' && method === 'GET') {
    return handleSearchByName(ctx, storage);
  }
  if (pathname === '/v4/env' && method === 'GET') {
    return handleSearch(ctx, storage);
  }
  if (pathname === '/v4/env' && method === 'POST') {
    return handleCreate(ctx, storage);
  }
  if (pathname.startsWith('/v4/env/') && method === 'PATCH') {
    return handleUpdate(ctx, storage);
  }
  if (pathname.startsWith('/v4/env/') && method === 'DELETE') {
    return handleDelete(ctx, storage);
  }
  if (pathname.startsWith('/v4/env/') && method === 'GET') {
    return handleRead(ctx, storage);
  }

  return false;
}

async function handleSearch(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const userIds = parseQueryAll(ctx.url, 'user_id');
  const keyword = ctx.url.searchParams.get('keyword') ?? undefined;
  const offset = parseInt(ctx.url.searchParams.get('offset') ?? '0', 10);
  const limit = parseInt(ctx.url.searchParams.get('limit') ?? '500', 10);

  const effectiveUserIds = userIds.length > 0 ? userIds : [ctx.user!.user_id];
  for (const uid of effectiveUserIds) {
    if (!(await checkReadable(uid, ctx.user!.user_id, storage))) {
      sendError(ctx.res, 403, `read ${uid} denied`);
      return true;
    }
  }

  const query: SearchEnvQuery = {
    user_id: effectiveUserIds,
    keyword,
    offset,
    limit,
  };

  const { list } = await storage.env.search(query);
  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: { list } });
  return true;
}

async function handleSearchByName(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const keyword = ctx.url.searchParams.get('keyword') ?? '';
  if (keyword.length < 1) {
    sendError(ctx.res, 403, 'keyword not found');
    return true;
  }

  const parts = keyword.split('/');
  const userId = parts.shift() ?? '';
  const name = parts.join('/');
  const offset = parseInt(ctx.url.searchParams.get('offset') ?? '0', 10);
  const limit = parseInt(ctx.url.searchParams.get('limit') ?? '10', 10);

  let query: SearchEnvQuery;
  if (name) {
    query = { user_id: userId, keyword: name, offset, limit };
  } else {
    query = { keyword: userId, offset, limit };
  }

  const { list, total } = await storage.env.search(query);
  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: { list, total } });
  return true;
}

async function handleCreate(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const body = parseJsonBody<CreateEnvReq>(ctx.body);
  if (!body?.user_id || !body?.name) {
    sendError(ctx.res, 400, 'user_id and name are required');
    return true;
  }

  if (!(await checkEditable(body.user_id, ctx.user!.user_id, storage))) {
    sendError(ctx.res, 403, `access ${body.user_id} denied`);
    return true;
  }

  const existing = await storage.env.findByUserAndName(body.user_id, body.name);
  if (existing) {
    sendJson(ctx.res, 200, { code: 0, message: 'ok', data: existing });
    return true;
  }

  const env = await storage.env.create(body);
  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: env });
  return true;
}

async function handleRead(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const id = extractPathParam(ctx.url.pathname, '/v4/env/');
  const env = await storage.env.findById(id);
  if (!env) {
    sendError(ctx.res, 404, `env ${id} not found`);
    return true;
  }

  if (!(await checkReadable(env.user_id, ctx.user!.user_id, storage))) {
    sendError(ctx.res, 403, `read ${env.user_id} denied`);
    return true;
  }

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: env });
  return true;
}

async function handleUpdate(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const id = extractPathParam(ctx.url.pathname, '/v4/env/');
  const body = parseJsonBody<UpdateEnvReq>(ctx.body);
  if (!body) {
    sendError(ctx.res, 400, 'invalid JSON body');
    return true;
  }

  const existing = await storage.env.findById(id);
  if (!existing) {
    sendError(ctx.res, 404, `env ${id} not found`);
    return true;
  }

  if (!(await checkEditable(existing.user_id, ctx.user!.user_id, storage))) {
    sendError(ctx.res, 403, `access ${existing.user_id} denied`);
    return true;
  }

  const env = await storage.env.update(id, body);
  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: env });
  return true;
}

async function handleDelete(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const id = extractPathParam(ctx.url.pathname, '/v4/env/');
  const existing = await storage.env.findById(id);
  if (!existing) {
    sendError(ctx.res, 404, `env ${id} not found`);
    return true;
  }

  if (!(await checkEditable(existing.user_id, ctx.user!.user_id, storage))) {
    sendError(ctx.res, 403, `access ${existing.user_id} denied`);
    return true;
  }

  await storage.env.delete(id);
  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: 1 });
  return true;
}

interface SyncEnvReq {
  user_ids: string[];
  check_list: Array<{ id: string; user_id: string; update_time: string; hash: string }>;
  update_list: Array<{
    user_id: string;
    id: string;
    name: string;
    rule?: string;
    sort_order?: number;
    update_time: string;
  }>;
  delete_list: Array<{ user_id: string; id: string; delete_time: string }>;
}

async function handleSync(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const body = parseJsonBody<SyncEnvReq>(ctx.body);
  if (!body) {
    sendError(ctx.res, 400, 'invalid JSON body');
    return true;
  }

  const resultList: Array<{
    type: number;
    status: number;
    msg?: string;
    user_id?: string;
    id?: string;
    name?: string;
    rule?: string;
    create_time?: string;
    update_time?: string;
  }> = [];
  const localUpdateList: unknown[] = [];
  const localDeleteList: string[] = [];

  for (const item of body.delete_list ?? []) {
    try {
      if (!(await checkEditable(item.user_id, ctx.user!.user_id, storage))) {
        resultList.push({ type: 0, user_id: item.user_id, id: item.id, status: 1, msg: `access ${item.user_id} denied` });
        continue;
      }
      const env = await storage.env.findById(item.id);
      if (env) {
        await storage.env.delete(item.id);
      }
      resultList.push({ type: 0, user_id: item.user_id, id: item.id, status: 0 });
    } catch (e: unknown) {
      resultList.push({
        type: 0,
        user_id: item.user_id,
        id: item.id,
        status: 1,
        msg: e instanceof Error ? e.message : 'unknown error',
      });
    }
  }

  for (const item of body.update_list ?? []) {
    try {
      if (!(await checkEditable(item.user_id, ctx.user!.user_id, storage))) {
        resultList.push({ type: 1, id: item.id, user_id: item.user_id, status: 1, msg: `access ${item.user_id} denied` });
        continue;
      }
      const existing = await storage.env.findById(item.id);
      if (existing) {
        const updated = await storage.env.update(item.id, {
          name: item.name,
          rule: item.rule,
          sort_order: item.sort_order,
          user_id: item.user_id,
        });
        if (updated) {
          resultList.push({ type: 1, status: 0, ...updated });
        }
      } else {
        const created = await storage.env.create({
          user_id: item.user_id,
          name: item.name,
          rule: item.rule,
          sort_order: item.sort_order,
        });
        resultList.push({ type: 3, status: 0, ...created });
      }
    } catch (e: unknown) {
      resultList.push({
        type: 1,
        id: item.id,
        user_id: item.user_id,
        status: 1,
        msg: e instanceof Error ? e.message : 'unknown error',
      });
    }
  }

  for (const item of body.check_list ?? []) {
    try {
      const env = await storage.env.findById(item.id);
      if (env) {
        if (!(await checkReadable(env.user_id, ctx.user!.user_id, storage))) {
          continue;
        }
        if (env.update_time !== item.update_time) {
          localUpdateList.push(env);
        }
      } else {
        localDeleteList.push(item.id);
      }
    } catch {
      localDeleteList.push(item.id);
    }
  }

  sendJson(ctx.res, 200, {
    code: 0,
    message: 'ok',
    data: {
      result_list: resultList,
      local_update_list: localUpdateList,
      local_delete_list: localDeleteList,
    },
  });
  return true;
}
