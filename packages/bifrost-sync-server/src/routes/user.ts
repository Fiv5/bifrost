import type { IStorage } from '../dao/types';
import type { RequestContext } from '../http';
import { sendJson, requireAuth } from '../http';

export async function handleUser(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  const { url, req } = ctx;
  const method = req.method ?? 'GET';

  if (url.pathname === '/v4/user/peer' && method === 'GET') {
    return handlePeer(ctx, storage);
  }

  return false;
}

interface PeerEntry {
  user_id: string;
  channel: number;
  group_id: string | null;
  editable: boolean;
  nickname: string;
  avatar: string;
  email: string;
}

async function handlePeer(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;

  const offset = parseInt(ctx.url.searchParams.get('offset') ?? '0', 10);
  const limit = parseInt(ctx.url.searchParams.get('limit') ?? '500', 10);
  const keyword = ctx.url.searchParams.get('keyword') ?? '';
  const currentUserId = ctx.user!.user_id;

  const peers: PeerEntry[] = [];

  if (!keyword || currentUserId.includes(keyword)) {
    peers.push({
      user_id: currentUserId,
      channel: 1,
      group_id: null,
      editable: true,
      nickname: ctx.user!.nickname ?? '',
      avatar: ctx.user!.avatar ?? '',
      email: ctx.user!.email ?? '',
    });
  }

  const memberships = await storage.groupMember.listByUser(currentUserId);
  for (const membership of memberships) {
    const group = await storage.group.findById(membership.group_id);
    if (!group) continue;
    if (keyword && !group.name.includes(keyword)) continue;

    peers.push({
      user_id: group.name,
      channel: 3,
      group_id: group.id,
      editable: membership.level >= 1,
      nickname: group.name,
      avatar: group.avatar ?? '',
      email: '',
    });
  }

  const paged = peers.slice(offset, offset + limit);

  sendJson(ctx.res, 200, {
    code: 0,
    message: 'ok',
    data: { list: paged, total: peers.length },
  });
  return true;
}
