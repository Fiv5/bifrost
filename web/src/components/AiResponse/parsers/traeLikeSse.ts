import type { SSEEvent, RecordContentType } from '../../../types';

export interface TraeLikeSseAssembly {
  contentType: RecordContentType;
  body: string;
}

type JsonRecord = Record<string, unknown>;

const isRecord = (value: unknown): value is JsonRecord =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const parseJsonRecord = (text: string): JsonRecord | null => {
  try {
    const parsed = JSON.parse(text);
    return isRecord(parsed) ? parsed : null;
  } catch {
    return null;
  }
};

const getString = (obj: JsonRecord, key: string): string | undefined => {
  const v = obj[key];
  return typeof v === 'string' ? v : undefined;
};

const getBool = (obj: JsonRecord, key: string): boolean | undefined => {
  const v = obj[key];
  return typeof v === 'boolean' ? v : undefined;
};

const TRAE_AGENT_EVENT_TYPES = new Set([
  'history',
  'progress_notice',
  'metadata',
  'timing_cost',
  'thought',
  'tool_call',
  'token_usage',
  'required_context',
  'turn_completion',
  'task_created',
]);

const TRAE_COMPLETION_EVENT_TYPES = new Set([
  'meta',
  'output',
  'token_usage',
  'time_cost',
  'done',
]);

interface ParsedEvent {
  event: SSEEvent;
  eventType: string;
  payload: JsonRecord | string | null;
}

const isTraeLikeStream = (events: ParsedEvent[]): 'agent' | 'completion' | false => {
  if (events.length === 0) return false;

  let agentCount = 0;
  let completionCount = 0;
  for (const { eventType } of events) {
    if (TRAE_AGENT_EVENT_TYPES.has(eventType)) agentCount++;
    if (TRAE_COMPLETION_EVENT_TYPES.has(eventType)) completionCount++;
  }

  if (agentCount >= 2 && agentCount / events.length > 0.3) return 'agent';
  if (completionCount >= 2 && completionCount / events.length > 0.3) return 'completion';
  return false;
};

interface ToolCallState {
  toolcallId: string;
  toolName: string;
  argParts: string[];
  requireLocalExecution: boolean;
  agentType?: string;
  agentName?: string;
}

const mergeToolCallArgChunks = (parts: string[]): JsonRecord => {
  if (parts.length === 0) return {};

  const merged: JsonRecord = {};
  for (const part of parts) {
    const parsed = parseJsonRecord(part);
    if (!parsed) continue;
    for (const [key, value] of Object.entries(parsed)) {
      if (typeof value === 'string' && typeof merged[key] === 'string') {
        merged[key] = (merged[key] as string) + value;
      } else if (merged[key] === undefined) {
        merged[key] = value;
      }
    }
  }

  return merged;
};

const removeUndefined = (obj: JsonRecord): JsonRecord => {
  const result: JsonRecord = {};
  for (const [k, v] of Object.entries(obj)) {
    if (v !== undefined) result[k] = v;
  }
  return result;
};

export const assembleTraeLikeSse = (
  events: SSEEvent[],
): TraeLikeSseAssembly | null => {
  if (events.length === 0) return null;

  const parsed: ParsedEvent[] = events.map((event) => {
    const eventType = (event.event ?? 'message').trim();
    const trimmed = event.data.trim();
    let payload: JsonRecord | string | null = parseJsonRecord(trimmed);
    if (payload === null) {
      try {
        const strVal = JSON.parse(trimmed);
        if (typeof strVal === 'string') {
          payload = strVal;
        }
      } catch {
        payload = trimmed;
      }
    }
    return { event, eventType, payload };
  });

  const streamType = isTraeLikeStream(parsed);
  if (!streamType) return null;

  if (streamType === 'completion') {
    return assembleCompletionProtocol(parsed);
  }

  return assembleAgentProtocol(parsed);
};

const assembleCompletionProtocol = (parsed: ParsedEvent[]): TraeLikeSseAssembly => {
  let meta: JsonRecord | null = null;
  let tokenUsage: JsonRecord | null = null;
  let timeCost: JsonRecord | null = null;
  let done: JsonRecord | null = null;
  const choiceTexts = new Map<number, string[]>();
  let lastFinishReason: string | null = null;
  let lastCreated: number | undefined;

  for (const { eventType, payload } of parsed) {
    if (eventType === 'meta' && isRecord(payload)) {
      meta = payload;
      continue;
    }

    if (eventType === 'output' && isRecord(payload)) {
      if (typeof payload.created === 'number') lastCreated = payload.created as number;
      const choices = payload.choices;
      if (Array.isArray(choices)) {
        for (const c of choices as JsonRecord[]) {
          if (!isRecord(c)) continue;
          const idx = typeof c.index === 'number' ? c.index : 0;
          const text = getString(c, 'text');
          if (text) {
            if (!choiceTexts.has(idx)) choiceTexts.set(idx, []);
            choiceTexts.get(idx)!.push(text);
          }
          const fr = getString(c, 'finish_reason');
          if (fr && fr !== '') lastFinishReason = fr;
        }
      }
      continue;
    }

    if (eventType === 'token_usage' && isRecord(payload)) {
      tokenUsage = payload;
      continue;
    }

    if (eventType === 'time_cost' && isRecord(payload)) {
      timeCost = payload;
      continue;
    }

    if (eventType === 'done' && isRecord(payload)) {
      done = payload;
      continue;
    }
  }

  const choices = Array.from(choiceTexts.entries())
    .sort(([a], [b]) => a - b)
    .map(([index, texts]) => ({
      index,
      text: texts.join(''),
      finish_reason: lastFinishReason,
    }));

  const assembled: JsonRecord = {};

  if (meta) {
    assembled.metadata = meta;
  }

  if (timeCost) {
    assembled.timing_cost = timeCost;
  }

  if (lastCreated !== undefined) {
    assembled.created = lastCreated;
  }

  assembled.message = {
    role: 'assistant',
    content: choices.length > 0 ? choices[0].text : '',
    choices: choices.length > 1 ? choices : undefined,
  };

  if (choices.length > 0 && lastFinishReason) {
    assembled.finish_reason = lastFinishReason;
  }

  if (tokenUsage) {
    assembled.usage = tokenUsage;
  }

  if (done) {
    assembled.done = done;
  }

  return {
    contentType: 'JSON',
    body: JSON.stringify(assembled, null, 2),
  };
};

const assembleAgentProtocol = (parsed: ParsedEvent[]): TraeLikeSseAssembly => {

  let thought = '';
  let reasoningContent = '';
  const toolCallChunks = new Map<string, ToolCallState>();
  const toolCallOrder: string[] = [];
  let metadata: JsonRecord | null = null;
  let timingCost: JsonRecord | null = null;
  let tokenUsage: JsonRecord | null = null;
  let turnCompletion: JsonRecord | null = null;
  let taskCreated: JsonRecord | null = null;
  const requiredContexts: string[] = [];
  const progressNotices: string[] = [];
  let agentType: string | undefined;
  let agentName: string | undefined;
  let agentRunId: string | undefined;

  for (const { eventType, payload } of parsed) {
    if (eventType === 'thought' && isRecord(payload)) {
      const t = getString(payload, 'thought');
      if (t) thought += t;
      const r = getString(payload, 'reasoning_content');
      if (r) reasoningContent += r;
      if (!agentType) agentType = getString(payload, 'agent_type');
      if (!agentName) agentName = getString(payload, 'agent_name');
      if (!agentRunId) agentRunId = getString(payload, 'agent_run_id');
      continue;
    }

    if (eventType === 'tool_call' && isRecord(payload)) {
      const tid = getString(payload, 'toolcall_id') ?? '';
      if (!toolCallChunks.has(tid)) {
        toolCallChunks.set(tid, {
          toolcallId: tid,
          toolName: getString(payload, 'tool_name') ?? 'unknown',
          argParts: [],
          requireLocalExecution: getBool(payload, 'require_local_execution') ?? false,
          agentType: getString(payload, 'agent_type'),
          agentName: getString(payload, 'agent_name'),
        });
        toolCallOrder.push(tid);
      }
      const chunk = toolCallChunks.get(tid)!;
      const args = getString(payload, 'arguments');
      if (args) chunk.argParts.push(args);
      if (!agentType) agentType = getString(payload, 'agent_type');
      if (!agentName) agentName = getString(payload, 'agent_name');
      if (!agentRunId) agentRunId = getString(payload, 'agent_run_id');
      continue;
    }

    if (eventType === 'metadata' && isRecord(payload)) {
      metadata = payload;
      continue;
    }

    if (eventType === 'timing_cost' && isRecord(payload)) {
      timingCost = payload;
      continue;
    }

    if (eventType === 'token_usage' && isRecord(payload)) {
      tokenUsage = payload;
      continue;
    }

    if (eventType === 'turn_completion' && isRecord(payload)) {
      turnCompletion = payload;
      continue;
    }

    if (eventType === 'task_created' && isRecord(payload)) {
      taskCreated = payload;
      continue;
    }

    if (eventType === 'required_context' && isRecord(payload)) {
      const contexts = payload.contexts;
      if (Array.isArray(contexts)) {
        for (const c of contexts) {
          if (typeof c === 'string') requiredContexts.push(c);
        }
      }
      continue;
    }

    if (eventType === 'progress_notice') {
      if (typeof payload === 'string') {
        progressNotices.push(payload);
      } else if (isRecord(payload)) {
        progressNotices.push(JSON.stringify(payload));
      }
      continue;
    }
  }

  const toolCalls = toolCallOrder.map((tid) => {
    const state = toolCallChunks.get(tid)!;
    const mergedArgs = mergeToolCallArgChunks(state.argParts);

    return removeUndefined({
      id: state.toolcallId,
      tool_name: state.toolName,
      arguments: mergedArgs,
      require_local_execution: state.requireLocalExecution || undefined,
      agent_type: state.agentType,
      agent_name: state.agentName,
    });
  });

  const message: JsonRecord = removeUndefined({
    role: 'assistant',
    agent_type: agentType,
    agent_name: agentName,
    agent_run_id: agentRunId,
    thought: thought || undefined,
    reasoning_content: reasoningContent || undefined,
    tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
  });

  const assembled: JsonRecord = {};

  if (taskCreated) {
    assembled.task = taskCreated;
  }

  if (metadata) {
    assembled.metadata = metadata;
  }

  if (timingCost) {
    assembled.timing_cost = timingCost;
  }

  assembled.message = message;

  if (tokenUsage) {
    assembled.usage = tokenUsage;
  }

  if (requiredContexts.length > 0) {
    assembled.required_contexts = requiredContexts;
  }

  if (turnCompletion) {
    assembled.turn_completion = turnCompletion;
  }

  if (progressNotices.length > 0) {
    assembled.progress_notices = progressNotices;
  }

  return {
    contentType: 'JSON',
    body: JSON.stringify(assembled, null, 2),
  };
};
