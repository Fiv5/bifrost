import type { IStorage } from '../dao/types';
import type { RequestContext } from '../http';
import {
  sendJson,
  sendError,
  requireAuth,
  parseJsonBody,
} from '../http';
import type {
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

  if (pathname === '/v4/group/invite' && method === 'POST') {
    return handleInvite(ctx, storage);
  }

  const inviteMatch = pathname.match(/^\/v4\/group\/([^/]+)\/invite$/);
  if (inviteMatch && method === 'POST') {
    return handleInvite(ctx, storage, inviteMatch[1]);
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

  const memberMatch = pathname.match(/^\/v4\/group\/([^/]+)\/member\/([^/]+)$/);
  if (memberMatch && method === 'DELETE') {
    return handleRemoveMember(ctx, storage, memberMatch[1], memberMatch[2]);
  }
  if (memberMatch && method === 'PATCH') {
    return handleUpdateMember(ctx, storage, memberMatch[1], memberMatch[2]);
  }

  if (pathname === '/v4/room' && method === 'GET') {
    return handleSearchRoom(ctx, storage);
  }
  if (pathname === '/v4/room' && method === 'PATCH') {
    return handleUpdateRoom(ctx, storage);
  }
  if (pathname === '/v4/room' && method === 'DELETE') {
    return handleRemoveRoom(ctx, storage);
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

async function resolveGroupVisibility(storage: IStorage, group: Group): Promise<Group> {
  const setting = await storage.groupSetting.get(group.id);
  return { ...group, visibility: setting.visibility };
}

async function handleCreate(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const raw = parseJsonBody<Record<string, unknown>>(ctx.body);
  if (!raw?.name) {
    sendError(ctx.res, 400, 'name is required');
    return true;
  }

  const name = raw.name as string;
  const avatar = (raw.avatar as string) ?? '';
  const description = (raw.what as string) ?? (raw.description as string) ?? '';
  const visibility = (raw.visibility as string) ?? 'private';

  const group = await storage.group.create(
    name,
    avatar,
    description,
    visibility,
    ctx.user!.user_id,
  );

  await storage.groupMember.add(group.id, ctx.user!.user_id, 2);
  await storage.groupSetting.init(group.id, visibility);

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
    const resolved = await Promise.all(list.map(g => resolveGroupVisibility(storage, g)));
    sendJson(ctx.res, 200, { code: 0, message: 'ok', data: { list: resolved.map(g => groupToV4(g)), total } });
    return true;
  }

  const memberships = await storage.groupMember.listByUser(ctx.user!.user_id);
  const groups: Array<import('../types').Group & { level: number }> = [];
  for (const m of memberships) {
    const g = await storage.group.findById(m.group_id);
    if (g) {
      const resolved = await resolveGroupVisibility(storage, g);
      groups.push({ ...resolved, level: m.level });
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

  const raw = await storage.group.findById(groupId);
  if (!raw) {
    sendError(ctx.res, 404, `group ${groupId} not found`);
    return true;
  }

  const group = await resolveGroupVisibility(storage, raw);
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

  const raw = parseJsonBody<Record<string, unknown>>(ctx.body);
  if (!raw) {
    sendError(ctx.res, 400, 'invalid JSON body');
    return true;
  }

  const body: UpdateGroupReq = {};
  if (raw.name !== undefined) body.name = raw.name as string;
  if (raw.avatar !== undefined) body.avatar = raw.avatar as string;
  if (raw.what !== undefined) body.description = raw.what as string;
  if (raw.description !== undefined) body.description = raw.description as string;

  if (raw.visibility !== undefined) {
    body.visibility = raw.visibility as string;
  } else if (raw.level !== undefined) {
    body.visibility = (raw.level as number) === 1 ? 'public' : 'private';
  }

  let settingUpdate: UpdateGroupSettingReq | undefined;
  if (raw.status !== undefined || raw.level !== undefined) {
    settingUpdate = {};
    if (raw.status !== undefined) settingUpdate.rules_enabled = !!raw.status;
    if (raw.level !== undefined) settingUpdate.visibility = (raw.level as number) === 1 ? 'public' : 'private';
  }

  if (body.name !== undefined) {
    const existing = await storage.group.findByName(body.name);
    if (existing && existing.id !== groupId) {
      sendError(ctx.res, 409, `group name "${body.name}" already exists`);
      return true;
    }
  }

  const group = await storage.group.update(groupId, body);
  if (!group) {
    sendError(ctx.res, 404, `group ${groupId} not found`);
    return true;
  }

  if (body.visibility !== undefined) {
    await storage.groupSetting.update(groupId, { visibility: body.visibility });
  }

  if (settingUpdate) {
    await storage.groupSetting.update(groupId, settingUpdate);
    if (settingUpdate.visibility !== undefined) {
      await storage.group.update(groupId, { visibility: settingUpdate.visibility });
    }
  }

  const resolved = await resolveGroupVisibility(storage, group);
  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: groupToV4(resolved) });
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
  groupIdFromPath?: string,
): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const body = parseJsonBody<InviteGroupReq>(ctx.body);
  const groupId = body?.group_id ?? groupIdFromPath;
  if (!groupId) {
    sendError(ctx.res, 400, 'group_id is required');
    return true;
  }

  const userIds = body?.user_id ?? (body as unknown as { user_ids?: string[] })?.user_ids;
  if (!userIds || !Array.isArray(userIds) || userIds.length === 0) {
    sendError(ctx.res, 400, 'user_id is required');
    return true;
  }

  const member = await storage.groupMember.findByGroupAndUser(groupId, ctx.user!.user_id);
  if (!member || member.level < 1) {
    sendError(ctx.res, 403, 'access denied');
    return true;
  }

  for (const userId of userIds) {
    const existing = await storage.groupMember.findByGroupAndUser(groupId, userId);
    if (!existing) {
      await storage.groupMember.add(groupId, userId, body?.level ?? 0);
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

  sendJson(ctx.res, 200, {
    code: 0,
    message: 'ok',
    data: {
      status: !!setting.rules_enabled,
      level: setting.visibility === 'public' ? 1 : 0,
    },
  });
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

  const raw = parseJsonBody<Record<string, unknown>>(ctx.body);
  if (!raw) {
    sendError(ctx.res, 400, 'invalid JSON body');
    return true;
  }

  const body: UpdateGroupSettingReq = {};
  if (raw.rules_enabled !== undefined) {
    body.rules_enabled = !!raw.rules_enabled;
  } else if (raw.status !== undefined) {
    body.rules_enabled = !!raw.status;
  }
  if (raw.visibility !== undefined) {
    body.visibility = raw.visibility as string;
  } else if (raw.level !== undefined) {
    body.visibility = raw.level === 1 ? 'public' : 'private';
  }

  await storage.groupSetting.update(groupId, body);

  if (body.visibility !== undefined) {
    await storage.group.update(groupId, { visibility: body.visibility });
  }

  const setting = await storage.groupSetting.get(groupId);
  sendJson(ctx.res, 200, {
    code: 0,
    message: 'ok',
    data: {
      status: !!setting.rules_enabled,
      level: setting.visibility === 'public' ? 1 : 0,
    },
  });
  return true;
}

async function handleSearchRoom(
  ctx: RequestContext,
  storage: IStorage,
): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const groupIds = ctx.url.searchParams.getAll('group_id');
  const userIds = ctx.url.searchParams.getAll('user_id');
  const keyword = ctx.url.searchParams.get('keyword') ?? undefined;
  const offset = parseInt(ctx.url.searchParams.get('offset') ?? '0', 10);
  const limit = parseInt(ctx.url.searchParams.get('limit') ?? '20', 10);

  let results: import('../types').GroupMember[] = [];
  let total = 0;

  if (groupIds.length > 0 && userIds.length > 0) {
    for (const gid of groupIds) {
      for (const uid of userIds) {
        const m = await storage.groupMember.findByGroupAndUser(gid, uid);
        if (m) results.push(m);
      }
    }
    total = results.length;
    results = results.slice(offset, offset + limit);
  } else if (groupIds.length > 0) {
    for (const gid of groupIds) {
      const { list, total: t } = await storage.groupMember.listByGroup(gid, { keyword, offset, limit });
      results.push(...list);
      total += t;
    }
  } else if (userIds.length > 0) {
    for (const uid of userIds) {
      const members = await storage.groupMember.listByUser(uid);
      results.push(...members);
    }
    if (keyword) {
      results = results.filter(m => m.user_id.includes(keyword));
    }
    total = results.length;
    results = results.slice(offset, offset + limit);
  } else {
    sendError(ctx.res, 400, 'group_id or user_id is required');
    return true;
  }

  const list = results.map(m => ({
    id: m.id,
    group_id: m.group_id,
    user_id: m.user_id,
    level: m.level,
    create_time: m.create_time,
    update_time: m.update_time,
  }));

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: { list, total } });
  return true;
}

async function handleUpdateRoom(
  ctx: RequestContext,
  storage: IStorage,
): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const body = parseJsonBody<{ group_id: string; user_id: string; level: number }>(ctx.body);
  if (!body?.group_id || !body?.user_id || body.level === undefined) {
    sendError(ctx.res, 400, 'group_id, user_id and level are required');
    return true;
  }

  const member = await storage.groupMember.findByGroupAndUser(body.group_id, ctx.user!.user_id);
  if (!member || member.level < 1) {
    sendError(ctx.res, 403, 'access denied');
    return true;
  }

  const updated = await storage.groupMember.updateLevel(body.group_id, body.user_id, body.level);
  if (!updated) {
    sendError(ctx.res, 404, 'room not found');
    return true;
  }

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: 1 });
  return true;
}

async function handleRemoveRoom(
  ctx: RequestContext,
  storage: IStorage,
): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const groupId = ctx.url.searchParams.get('group_id');
  const userId = ctx.url.searchParams.get('user_id');
  if (!groupId || !userId) {
    sendError(ctx.res, 400, 'group_id and user_id are required');
    return true;
  }

  const member = await storage.groupMember.findByGroupAndUser(groupId, ctx.user!.user_id);
  if (!member || member.level < 1) {
    sendError(ctx.res, 403, 'access denied');
    return true;
  }

  await storage.groupMember.remove(groupId, userId);

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: 1 });
  return true;
}
