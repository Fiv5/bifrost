import type { IStorage } from '../dao/types';
import type { RequestContext } from '../http';
import {
  sendJson,
  sendError,
  requireAuth,
  parseJsonBody,
} from '../http';
import type {
  CreateGroupReq,
  UpdateGroupReq,
  InviteGroupReq,
  UpdateGroupSettingReq,
  Group,
} from '../types';

function groupToV4(g: Group & { level?: number }) {
  return {
    id: g.id,
    name: g.name,
    avatar: g.avatar,
    what: g.description ?? '',
    level: g.level ?? 0,
    visibility: g.visibility === 'public' ? 1 : 0,
    created_by: g.created_by,
    create_time: g.create_time,
    update_time: g.update_time,
  };
}

export async function handleGroup(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  const { url, req } = ctx;
  const method = req.method ?? 'GET';
  const pathname = url.pathname.replace(/\/$/, '') || '/';

  if (pathname === '/v4/group' && method === 'POST') {
    return handleCreate(ctx, storage);
  }
  if (pathname === '/v4/group' && method === 'GET') {
    return handleSearch(ctx, storage);
  }

  const leaveMatch = pathname.match(/^\/v4\/group\/([^/]+)\/leave$/);
  if (leaveMatch && method === 'POST') {
    return handleLeave(ctx, storage, leaveMatch[1]);
  }

  const settingMatch = pathname.match(/^\/v4\/group\/([^/]+)\/setting$/);
  if (settingMatch && method === 'GET') {
    return handleGetSetting(ctx, storage, settingMatch[1]);
  }
  if (settingMatch && method === 'PATCH') {
    return handleUpdateSetting(ctx, storage, settingMatch[1]);
  }

  const membersMatch = pathname.match(/^\/v4\/group\/([^/]+)\/members$/);
  if (membersMatch && method === 'GET') {
    return handleListMembers(ctx, storage, membersMatch[1]);
  }

  const inviteMatch = pathname.match(/^\/v4\/group\/([^/]+)\/invite$/);
  if (inviteMatch && method === 'POST') {
    return handleInvite(ctx, storage, inviteMatch[1]);
  }

  const memberMatch = pathname.match(/^\/v4\/group\/([^/]+)\/member\/([^/]+)$/);
  if (memberMatch && method === 'DELETE') {
    return handleRemoveMember(ctx, storage, memberMatch[1], memberMatch[2]);
  }
  if (memberMatch && method === 'PATCH') {
    return handleUpdateMember(ctx, storage, memberMatch[1], memberMatch[2]);
  }

  const idMatch = pathname.match(/^\/v4\/group\/([^/]+)$/);
  if (idMatch && method === 'GET') {
    return handleRead(ctx, storage, idMatch[1]);
  }
  if (idMatch && method === 'PATCH') {
    return handleUpdate(ctx, storage, idMatch[1]);
  }
  if (idMatch && method === 'DELETE') {
    return handleDelete(ctx, storage, idMatch[1]);
  }

  return false;
}

async function handleCreate(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const body = parseJsonBody<CreateGroupReq>(ctx.body);
  if (!body?.name) {
    sendError(ctx.res, 400, 'name is required');
    return true;
  }

  const group = await storage.group.create(
    body.name,
    body.avatar ?? '',
    body.description ?? '',
    body.visibility ?? 'private',
    ctx.user!.user_id,
  );

  await storage.groupMember.add(group.id, ctx.user!.user_id, 2);
  await storage.groupSetting.init(group.id);

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: groupToV4({ ...group, level: 2 }) });
  return true;
}

async function handleSearch(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const keyword = ctx.url.searchParams.get('keyword') ?? undefined;
  const offset = parseInt(ctx.url.searchParams.get('offset') ?? '0', 10);
  const limit = parseInt(ctx.url.searchParams.get('limit') ?? '20', 10);

  if (keyword) {
    const { list, total } = await storage.group.search(
      { keyword, offset, limit },
      ctx.user!.user_id,
    );
    sendJson(ctx.res, 200, { code: 0, message: 'ok', data: { list: list.map(g => groupToV4(g)), total } });
    return true;
  }

  const memberships = await storage.groupMember.listByUser(ctx.user!.user_id);
  const groups: Array<import('../types').Group & { level: number }> = [];
  for (const m of memberships) {
    const g = await storage.group.findById(m.group_id);
    if (g) {
      groups.push({ ...g, level: m.level });
    }
  }

  sendJson(ctx.res, 200, {
    code: 0,
    message: 'ok',
    data: { list: groups.map(g => groupToV4(g)), total: groups.length },
  });
  return true;
}

async function handleRead(ctx: RequestContext, storage: IStorage, groupId: string): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const group = await storage.group.findById(groupId);
  if (!group) {
    sendError(ctx.res, 404, `group ${groupId} not found`);
    return true;
  }

  const member = await storage.groupMember.findByGroupAndUser(groupId, ctx.user!.user_id);
  if (!member && group.visibility !== 'public') {
    sendError(ctx.res, 403, 'access denied');
    return true;
  }

  sendJson(ctx.res, 200, {
    code: 0,
    message: 'ok',
    data: groupToV4({ ...group, level: member?.level ?? 0 }),
  });
  return true;
}

async function handleUpdate(ctx: RequestContext, storage: IStorage, groupId: string): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const member = await storage.groupMember.findByGroupAndUser(groupId, ctx.user!.user_id);
  if (!member || member.level < 1) {
    sendError(ctx.res, 403, 'access denied');
    return true;
  }

  const body = parseJsonBody<UpdateGroupReq>(ctx.body);
  if (!body) {
    sendError(ctx.res, 400, 'invalid JSON body');
    return true;
  }

  const group = await storage.group.update(groupId, body);
  if (!group) {
    sendError(ctx.res, 404, `group ${groupId} not found`);
    return true;
  }

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: groupToV4(group) });
  return true;
}

async function handleDelete(ctx: RequestContext, storage: IStorage, groupId: string): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const member = await storage.groupMember.findByGroupAndUser(groupId, ctx.user!.user_id);
  if (!member || member.level < 2) {
    sendError(ctx.res, 403, 'access denied');
    return true;
  }

  const group = await storage.group.findById(groupId);
  if (group) {
    await storage.env.deleteByUserId(group.name);
  }

  await storage.group.delete(groupId);

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: 1 });
  return true;
}

async function handleListMembers(
  ctx: RequestContext,
  storage: IStorage,
  groupId: string,
): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const member = await storage.groupMember.findByGroupAndUser(groupId, ctx.user!.user_id);
  if (!member) {
    sendError(ctx.res, 403, 'access denied');
    return true;
  }

  const keyword = ctx.url.searchParams.get('keyword') ?? undefined;
  const offset = parseInt(ctx.url.searchParams.get('offset') ?? '0', 10);
  const limit = parseInt(ctx.url.searchParams.get('limit') ?? '20', 10);

  const { list, total } = await storage.groupMember.listByGroup(groupId, {
    keyword,
    offset,
    limit,
  });

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: { list, total } });
  return true;
}

async function handleInvite(
  ctx: RequestContext,
  storage: IStorage,
  groupId: string,
): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const member = await storage.groupMember.findByGroupAndUser(groupId, ctx.user!.user_id);
  if (!member || member.level < 1) {
    sendError(ctx.res, 403, 'access denied');
    return true;
  }

  const body = parseJsonBody<InviteGroupReq>(ctx.body);
  if (!body?.user_ids || !Array.isArray(body.user_ids)) {
    sendError(ctx.res, 400, 'user_ids is required');
    return true;
  }

  for (const userId of body.user_ids) {
    const existing = await storage.groupMember.findByGroupAndUser(groupId, userId);
    if (!existing) {
      await storage.groupMember.add(groupId, userId, body.level ?? 0);
    }
  }

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: 1 });
  return true;
}

async function handleRemoveMember(
  ctx: RequestContext,
  storage: IStorage,
  groupId: string,
  userId: string,
): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const member = await storage.groupMember.findByGroupAndUser(groupId, ctx.user!.user_id);
  if (!member || member.level < 1) {
    sendError(ctx.res, 403, 'access denied');
    return true;
  }

  if (userId === ctx.user!.user_id) {
    sendError(ctx.res, 400, 'cannot remove self');
    return true;
  }

  await storage.groupMember.remove(groupId, userId);

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: 1 });
  return true;
}

async function handleUpdateMember(
  ctx: RequestContext,
  storage: IStorage,
  groupId: string,
  userId: string,
): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const member = await storage.groupMember.findByGroupAndUser(groupId, ctx.user!.user_id);
  if (!member || member.level < 1) {
    sendError(ctx.res, 403, 'access denied');
    return true;
  }

  if (userId === ctx.user!.user_id) {
    sendError(ctx.res, 400, 'cannot change own level');
    return true;
  }

  const body = parseJsonBody<{ level: number }>(ctx.body);
  if (body?.level === undefined) {
    sendError(ctx.res, 400, 'level is required');
    return true;
  }

  await storage.groupMember.updateLevel(groupId, userId, body.level);

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: 1 });
  return true;
}

async function handleLeave(
  ctx: RequestContext,
  storage: IStorage,
  groupId: string,
): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const member = await storage.groupMember.findByGroupAndUser(groupId, ctx.user!.user_id);
  if (!member) {
    sendError(ctx.res, 403, 'not a member');
    return true;
  }

  if (member.level === 2) {
    const { list } = await storage.groupMember.listByGroup(groupId);
    const owners = list.filter((m) => m.level === 2);
    if (owners.length <= 1) {
      sendError(ctx.res, 400, 'cannot leave as the only owner');
      return true;
    }
  }

  await storage.groupMember.remove(groupId, ctx.user!.user_id);

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: 1 });
  return true;
}

async function handleGetSetting(
  ctx: RequestContext,
  storage: IStorage,
  groupId: string,
): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const member = await storage.groupMember.findByGroupAndUser(groupId, ctx.user!.user_id);
  if (!member || member.level < 1) {
    sendError(ctx.res, 403, 'access denied');
    return true;
  }

  const setting = await storage.groupSetting.get(groupId);

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: setting });
  return true;
}

async function handleUpdateSetting(
  ctx: RequestContext,
  storage: IStorage,
  groupId: string,
): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const member = await storage.groupMember.findByGroupAndUser(groupId, ctx.user!.user_id);
  if (!member || member.level < 1) {
    sendError(ctx.res, 403, 'access denied');
    return true;
  }

  const body = parseJsonBody<UpdateGroupSettingReq>(ctx.body);
  if (!body) {
    sendError(ctx.res, 400, 'invalid JSON body');
    return true;
  }

  await storage.groupSetting.update(groupId, body);

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: 1 });
  return true;
}
