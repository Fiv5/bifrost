import type { SSEEvent, RecordContentType } from '../../../types';

export interface DouBaoLikeSseAssembly {
  contentType: RecordContentType;
  body: string;
}

type JsonRecord = Record<string, unknown>;

const isRecord = (value: unknown): value is JsonRecord =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const DOUBAO_EVENT_TYPES = new Set([
  'SSE_HEARTBEAT',
  'SSE_ACK',
  'FULL_MSG_NOTIFY',
  'STREAM_MSG_NOTIFY',
  'STREAM_CHUNK',
  'CHUNK_DELTA',
  'SSE_REPLY_END',
]);

export const isDouBaoLikeStream = (events: SSEEvent[]): boolean => {
  if (events.length < 3) return false;
  let matched = 0;
  let total = 0;
  for (const ev of events) {
    const name = (ev.event ?? '').trim();
    if (!name) continue;
    total++;
    if (DOUBAO_EVENT_TYPES.has(name)) matched++;
  }
  return total > 0 && matched / total > 0.5;
};

interface ContentBlock {
  block_type: number;
  block_id?: string;
  parent_id?: string;
  is_finish?: boolean;
  patch_type?: number;
  content?: {
    text_block?: { text?: string; summary?: string; icon_url?: string };
    thinking_block?: {
      finish_title?: string;
      streaming_title?: string;
      unfold_streaming_title?: string;
    };
    search_query_result_block?: {
      summary?: string;
      queries?: string[];
      result_list?: unknown[];
    };
    [key: string]: unknown;
  };
}

interface PatchOp {
  patch_object?: number;
  patch_type?: number;
  patch_value?: {
    content_block?: ContentBlock[];
    [key: string]: unknown;
  };
}

export const assembleDouBaoLikeSse = (events: SSEEvent[]): DouBaoLikeSseAssembly | null => {
  if (!isDouBaoLikeStream(events)) return null;

  let streamMsgContent: JsonRecord | null = null;
  let streamMsgMeta: JsonRecord | null = null;

  const thinkingSteps: string[] = [];
  const searchQueries: string[] = [];
  const contentTexts: string[] = [];
  const chunkDeltaTexts: string[] = [];

  let thinkingTitle = '';
  let thinkingFinishTitle = '';

  const replyEnds: JsonRecord[] = [];

  let agentName = '';
  let modelType = '';
  let isDeepThinking = false;
  let conversationId = '';
  let messageId = '';

  for (const ev of events) {
    const eventType = (ev.event ?? '').trim();
    const dataStr = ev.data ?? '';
    if (!eventType || !dataStr) continue;

    let payload: unknown;
    try {
      payload = JSON.parse(dataStr);
    } catch {
      continue;
    }
    if (!isRecord(payload)) continue;

    switch (eventType) {
      case 'SSE_ACK':
        break;

      case 'FULL_MSG_NOTIFY': {
        const msg = payload.message as JsonRecord | undefined;
        if (msg) {
          const ext = msg.ext as JsonRecord | undefined;
          if (ext) {
            if (typeof ext.model_type === 'string') modelType = ext.model_type;
            if (typeof ext.llm_model_type === 'string') modelType = ext.llm_model_type;
            if (ext.use_deep_think === '1') isDeepThinking = true;
          }
        }
        break;
      }

      case 'STREAM_MSG_NOTIFY': {
        streamMsgContent = payload.content as JsonRecord | undefined ?? null;
        streamMsgMeta = payload.meta as JsonRecord | undefined ?? null;
        const ext = streamMsgContent?.ext as JsonRecord | undefined;
        if (ext) {
          if (typeof ext.agent_name === 'string') agentName = ext.agent_name;
          if (ext.is_deep_thinking === '1') isDeepThinking = true;
          if (typeof ext.llm_model_type === 'string') modelType = ext.llm_model_type;
        }
        if (streamMsgMeta) {
          if (typeof streamMsgMeta.conversation_id === 'string') conversationId = streamMsgMeta.conversation_id;
          if (typeof streamMsgMeta.message_id === 'string') messageId = streamMsgMeta.message_id;
        }
        break;
      }

      case 'STREAM_CHUNK': {
        const patchOps = payload.patch_op as PatchOp[] | undefined;
        if (!Array.isArray(patchOps)) break;
        for (const op of patchOps) {
          const blocks = op.patch_value?.content_block;
          if (!Array.isArray(blocks)) continue;
          for (const block of blocks) {
            const bt = block.block_type;
            if (bt === 10040) {
              const tb = block.content?.thinking_block;
              if (tb?.streaming_title) thinkingTitle = tb.streaming_title;
              if (tb?.finish_title) thinkingFinishTitle = tb.finish_title;
              if (tb?.unfold_streaming_title && !thinkingTitle) thinkingTitle = tb.unfold_streaming_title;
            } else if (bt === 10000) {
              const text = block.content?.text_block?.text;
              const summary = block.content?.text_block?.summary;
              if (block.parent_id && text) {
                thinkingSteps.push(text);
              } else if (text) {
                contentTexts.push(text);
              }
              if (summary) {
                thinkingSteps.push(`[${summary}]`);
              }
            } else if (bt === 10025) {
              const sqrb = block.content?.search_query_result_block;
              if (sqrb?.queries) {
                for (const q of sqrb.queries) {
                  if (typeof q === 'string' && !searchQueries.includes(q)) {
                    searchQueries.push(q);
                  }
                }
              }
            }
          }
        }
        break;
      }

      case 'CHUNK_DELTA': {
        if (typeof payload.text === 'string') {
          chunkDeltaTexts.push(payload.text);
        }
        break;
      }

      case 'SSE_REPLY_END':
        replyEnds.push(payload);
        break;
    }
  }

  const thought = chunkDeltaTexts.join('');
  const contentFromBlocks = contentTexts.join('');

  let brief = '';
  let finishType = '';
  for (const end of replyEnds) {
    const et = end.end_type;
    if (et === 1) {
      const attr = end.msg_finish_attr as JsonRecord | undefined;
      if (attr && typeof attr.brief === 'string') brief = attr.brief;
      finishType = 'msg_finish';
    } else if (et === 2) {
      finishType = finishType || 'answer_finish';
    } else if (et === 3) {
      finishType = finishType || 'end';
    }
  }

  const assembled: JsonRecord = {
    type: 'doubao',
    agent_name: agentName || undefined,
    model_type: modelType || undefined,
    is_deep_thinking: isDeepThinking,
    conversation_id: conversationId || undefined,
    message_id: messageId || undefined,
    thinking: {
      title: thinkingTitle || undefined,
      finish_title: thinkingFinishTitle || undefined,
      steps: thinkingSteps.length > 0 ? thinkingSteps : undefined,
    },
    search_queries: searchQueries.length > 0 ? searchQueries : undefined,
    message: {
      role: 'assistant',
      content: thought || contentFromBlocks || brief || '',
    },
    brief: brief || undefined,
    finish_type: finishType || undefined,
    reply_ends: replyEnds.length > 0 ? replyEnds : undefined,
  };

  if (streamMsgMeta) {
    assembled.meta = streamMsgMeta;
  }

  return {
    contentType: 'JSON',
    body: JSON.stringify(assembled, null, 2),
  };
};
