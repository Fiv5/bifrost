import type { SSEEvent, WebSocketMessage, WebSocketFrame } from '../../types';

export type MessageDirection = 'send' | 'receive' | 'event';

export type MessageType = 
  | 'text' 
  | 'binary' 
  | 'ping' 
  | 'pong' 
  | 'close' 
  | 'continuation'
  | 'sse' 
  | 'json';

export interface MessageItem {
  id: string | number;
  timestamp: number;
  direction: MessageDirection;
  type: MessageType;
  data: string;
  metadata?: {
    eventId?: string;
    eventType?: string;
    retry?: number;
    isJson?: boolean;
    isMasked?: boolean;
    isFin?: boolean;
    payloadSize?: number;
  };
  searchText?: string;
}

export interface MessageSearchOptions {
  query: string;
  caseSensitive?: boolean;
  matchMode: 'highlight' | 'filter';
}

export interface MessageSearchState extends MessageSearchOptions {
  matchedIndices: number[];
  currentIndex: number;
  total: number;
}

export interface MessageViewerState {
  search: MessageSearchState;
  fullscreen: boolean;
  followTail: boolean;
}

function parseSsePayload(payload: string): {
  data: string;
  eventId?: string;
  eventType?: string;
  retry?: number;
} {
  const lines = payload.split(/\r?\n/);
  const dataLines: string[] = [];
  let eventId: string | undefined;
  let eventType: string | undefined;
  let retry: number | undefined;

  for (const rawLine of lines) {
    const line = rawLine.trimEnd();
    if (!line) continue;

    if (line.startsWith('data:')) {
      dataLines.push(line.slice(5).replace(/^ /, ''));
      continue;
    }
    if (line.startsWith('event:')) {
      const v = line.slice(6).replace(/^ /, '');
      if (v) eventType = v;
      continue;
    }
    if (line.startsWith('id:')) {
      const v = line.slice(3).replace(/^ /, '');
      if (v) eventId = v;
      continue;
    }
    if (line.startsWith('retry:')) {
      const v = line.slice(6).replace(/^ /, '');
      const n = Number.parseInt(v, 10);
      if (!Number.isNaN(n)) retry = n;
      continue;
    }
  }

  const data = dataLines.length > 0 ? dataLines.join('\n') : payload;
  return { data, eventId, eventType, retry };
}

export function normalizeSSEEvent(event: SSEEvent, index?: number): MessageItem {
  const eventData = event.data || '';
  const isJson = (() => {
    try {
      JSON.parse(eventData);
      return true;
    } catch {
      return false;
    }
  })();

  const searchText = [
    event.event || 'message',
    event.id || '',
    eventData
  ].filter(Boolean).join(' ').toLowerCase();

  return {
    id: event.id || `sse-${event.timestamp}-${index || 0}`,
    timestamp: event.timestamp,
    direction: 'event',
    type: isJson ? 'json' : 'text',
    data: eventData,
    metadata: {
      eventId: event.id,
      eventType: event.event,
      isJson,
    },
    searchText,
  };
}

export function normalizeWSMessage(message: WebSocketMessage): MessageItem {
  const isJson = (() => {
    if (message.type !== 'text') return false;
    try {
      JSON.parse(message.data);
      return true;
    } catch {
      return false;
    }
  })();

  const searchText = [
    message.direction,
    message.type,
    message.data
  ].filter(Boolean).join(' ').toLowerCase();

  return {
    id: message.id,
    timestamp: message.timestamp,
    direction: message.direction,
    type: isJson ? 'json' : message.type,
    data: message.data,
    metadata: {
      isJson,
    },
    searchText,
  };
}

export function normalizeWSFrame(frame: WebSocketFrame): MessageItem {
  const rawPreview = frame.payload_preview || '';
  const sseParsed = frame.frame_type === 'sse' ? parseSsePayload(rawPreview) : null;
  const data = sseParsed?.data ?? rawPreview;
  const isJson = (() => {
    if (frame.frame_type !== 'text' && frame.frame_type !== 'sse') return false;
    try {
      JSON.parse(data);
      return true;
    } catch {
      return false;
    }
  })();

  const searchText = [
    frame.direction,
    frame.frame_type,
    sseParsed?.eventType || '',
    sseParsed?.eventId || '',
    data
  ].filter(Boolean).join(' ').toLowerCase();

  return {
    id: frame.frame_id,
    timestamp: frame.timestamp,
    direction: frame.direction,
    type: frame.frame_type === 'sse' ? 'sse' : (isJson ? 'json' : frame.frame_type),
    data,
    metadata: {
      eventId: sseParsed?.eventId,
      eventType: sseParsed?.eventType,
      retry: sseParsed?.retry,
      isJson,
      isMasked: frame.is_masked,
      isFin: frame.is_fin,
      payloadSize: frame.payload_size,
    },
    searchText,
  };
}
