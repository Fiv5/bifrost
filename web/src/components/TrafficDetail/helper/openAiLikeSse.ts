import type { SSEEvent, RecordContentType } from '../../../types';

interface ChatCompletionChoiceState {
  index: number;
  role?: string;
  content: string;
  reasoning_content: string;
  refusal: string;
  finish_reason?: string | null;
  extra: JsonRecord;
  messageExtra: JsonRecord;
  function_call?: {
    name?: string;
    arguments: string;
    extra: JsonRecord;
  };
  tool_calls: Array<{
    index: number;
    id?: string;
    type?: string;
    extra: JsonRecord;
    function?: {
      name?: string;
      arguments: string;
      extra: JsonRecord;
    };
  }>;
}

interface ResponsesTextState {
  key: string;
  item_id?: string;
  output_index?: number;
  content_index?: number;
  text: string;
}

export interface OpenAiLikeSseAssembly {
  mode: 'chat.completions' | 'responses';
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

const getEventName = (event: SSEEvent, payload: JsonRecord): string => {
  if (typeof payload.type === 'string' && payload.type.trim()) {
    return payload.type;
  }
  if (typeof event.event === 'string' && event.event.trim()) {
    return event.event;
  }
  return 'message';
};

const appendString = (
  base: string,
  value: unknown,
): string => {
  if (typeof value !== 'string' || !value) {
    return base;
  }
  return `${base}${value}`;
};

const mergeJsonRecord = (
  target: JsonRecord,
  source: JsonRecord,
  excludedKeys: string[] = [],
) => {
  const excluded = new Set(excludedKeys);

  for (const [key, value] of Object.entries(source)) {
    if (excluded.has(key) || value === undefined) {
      continue;
    }

    if (isRecord(value) && isRecord(target[key])) {
      mergeJsonRecord(target[key] as JsonRecord, value);
      continue;
    }

    target[key] = value;
  }
};

const deepEqual = (left: unknown, right: unknown): boolean => {
  if (left === right) {
    return true;
  }

  if (Array.isArray(left) && Array.isArray(right)) {
    if (left.length !== right.length) {
      return false;
    }
    for (let index = 0; index < left.length; index += 1) {
      if (!deepEqual(left[index], right[index])) {
        return false;
      }
    }
    return true;
  }

  if (isRecord(left) && isRecord(right)) {
    const leftKeys = Object.keys(left);
    const rightKeys = Object.keys(right);
    if (leftKeys.length !== rightKeys.length) {
      return false;
    }
    for (const key of leftKeys) {
      if (!deepEqual(left[key], right[key])) {
        return false;
      }
    }
    return true;
  }

  return false;
};

const stripDuplicateKeys = (record: JsonRecord, parent: JsonRecord): JsonRecord => {
  const next: JsonRecord = {};

  for (const [key, value] of Object.entries(record)) {
    if (parent[key] !== undefined && deepEqual(value, parent[key])) {
      continue;
    }
    next[key] = value;
  }

  return next;
};

const extractChatDeltaParts = (
  delta: JsonRecord,
): { content: string; reasoningContent: string; refusal: string } => {
  let content = '';
  let reasoningContent = '';
  let refusal = '';

  if (typeof delta.content === 'string') {
    content += delta.content;
  }

  if (Array.isArray(delta.content)) {
    for (const part of delta.content) {
      if (typeof part === 'string') {
        content += part;
        continue;
      }
      if (!isRecord(part)) continue;
      const partText =
        typeof part.text === 'string'
          ? part.text
          : typeof part.content === 'string'
            ? part.content
            : '';
      const partType = typeof part.type === 'string' ? part.type : '';

      if (partType.includes('reasoning')) {
        reasoningContent += partText;
        continue;
      }
      if (partType.includes('refusal')) {
        refusal += partText;
        continue;
      }
      content += partText;
    }
  }

  if (typeof delta.reasoning_content === 'string') {
    reasoningContent += delta.reasoning_content;
  }

  if (typeof delta.refusal === 'string') {
    refusal += delta.refusal;
  }

  return { content, reasoningContent, refusal };
};

const ensureChoiceState = (
  state: Map<number, ChatCompletionChoiceState>,
  index: number,
): ChatCompletionChoiceState => {
  const existing = state.get(index);
  if (existing) {
    return existing;
  }

  const created: ChatCompletionChoiceState = {
    index,
    content: '',
    reasoning_content: '',
    refusal: '',
    extra: {},
    messageExtra: {},
    tool_calls: [],
  };
  state.set(index, created);
  return created;
};

const assembleChatCompletions = (
  parsedEvents: Array<{ event: SSEEvent; payload: JsonRecord }>,
): OpenAiLikeSseAssembly | null => {
  const choices = new Map<number, ChatCompletionChoiceState>();
  const topLevel: JsonRecord = {};
  let recognized = false;

  for (const { payload } of parsedEvents) {
    if (!Array.isArray(payload.choices)) continue;
    recognized = true;
    mergeJsonRecord(topLevel, payload, ['choices']);

    for (const rawChoice of payload.choices) {
      if (!isRecord(rawChoice)) continue;
      const index = typeof rawChoice.index === 'number' ? rawChoice.index : 0;
      const choice = ensureChoiceState(choices, index);
      mergeJsonRecord(choice.extra, rawChoice, ['delta', 'message', 'index']);

      if (rawChoice.finish_reason !== undefined) {
        choice.finish_reason =
          rawChoice.finish_reason === null || typeof rawChoice.finish_reason === 'string'
            ? rawChoice.finish_reason
            : choice.finish_reason;
      }

      if (isRecord(rawChoice.delta)) {
        const delta = rawChoice.delta;
        if (typeof delta.role === 'string') {
          choice.role = delta.role;
        }
        mergeJsonRecord(choice.messageExtra, delta, [
          'role',
          'content',
          'reasoning_content',
          'refusal',
          'function_call',
          'tool_calls',
        ]);
        const deltaParts = extractChatDeltaParts(delta);
        choice.content = appendString(choice.content, deltaParts.content);
        choice.reasoning_content = appendString(
          choice.reasoning_content,
          deltaParts.reasoningContent,
        );
        choice.refusal = appendString(choice.refusal, deltaParts.refusal);

        if (isRecord(delta.function_call)) {
          choice.function_call = choice.function_call ?? { arguments: '', extra: {} };
          mergeJsonRecord(choice.function_call.extra, delta.function_call, [
            'name',
            'arguments',
          ]);
          choice.function_call.name =
            typeof delta.function_call.name === 'string'
              ? delta.function_call.name
              : choice.function_call.name;
          choice.function_call.arguments = appendString(
            choice.function_call.arguments,
            delta.function_call.arguments,
          );
        }

        if (Array.isArray(delta.tool_calls)) {
          for (const rawToolCall of delta.tool_calls) {
            if (!isRecord(rawToolCall)) continue;
            const toolIndex =
              typeof rawToolCall.index === 'number' ? rawToolCall.index : choice.tool_calls.length;
            const toolCall =
              choice.tool_calls.find((item) => item.index === toolIndex) ??
              (() => {
                const created: ChatCompletionChoiceState['tool_calls'][number] = {
                  index: toolIndex,
                  extra: {},
                  function: { arguments: '', extra: {} },
                };
                choice.tool_calls.push(created);
                return created;
              })();

            mergeJsonRecord(toolCall.extra, rawToolCall, ['index', 'id', 'type', 'function']);
            if (typeof rawToolCall.id === 'string') {
              toolCall.id = rawToolCall.id;
            }
            if (typeof rawToolCall.type === 'string') {
              toolCall.type = rawToolCall.type;
            }
            if (isRecord(rawToolCall.function)) {
              toolCall.function = toolCall.function ?? { arguments: '', extra: {} };
              mergeJsonRecord(toolCall.function.extra, rawToolCall.function, [
                'name',
                'arguments',
              ]);
              toolCall.function.name =
                typeof rawToolCall.function.name === 'string'
                  ? rawToolCall.function.name
                  : toolCall.function.name;
              toolCall.function.arguments = appendString(
                toolCall.function.arguments,
                rawToolCall.function.arguments,
              );
            }
          }
        }
      }

      if (isRecord(rawChoice.message)) {
        const message = rawChoice.message;
        mergeJsonRecord(choice.messageExtra, message, [
          'role',
          'content',
          'reasoning_content',
          'refusal',
          'function_call',
          'tool_calls',
        ]);
        if (typeof message.role === 'string') {
          choice.role = message.role;
        }
        choice.content = appendString(choice.content, message.content);
        choice.reasoning_content = appendString(
          choice.reasoning_content,
          message.reasoning_content,
        );
        choice.refusal = appendString(choice.refusal, message.refusal);

        if (isRecord(message.function_call)) {
          choice.function_call = choice.function_call ?? { arguments: '', extra: {} };
          mergeJsonRecord(choice.function_call.extra, message.function_call, [
            'name',
            'arguments',
          ]);
          choice.function_call.name =
            typeof message.function_call.name === 'string'
              ? message.function_call.name
              : choice.function_call.name;
          choice.function_call.arguments = appendString(
            choice.function_call.arguments,
            message.function_call.arguments,
          );
        }

        if (Array.isArray(message.tool_calls)) {
          for (const [toolIndex, rawToolCall] of message.tool_calls.entries()) {
            if (!isRecord(rawToolCall)) continue;
            const callIndex =
              typeof rawToolCall.index === 'number' ? rawToolCall.index : toolIndex;
            const toolCall =
              choice.tool_calls.find((item) => item.index === callIndex) ??
              (() => {
                const created: ChatCompletionChoiceState['tool_calls'][number] = {
                  index: callIndex,
                  extra: {},
                  function: { arguments: '', extra: {} },
                };
                choice.tool_calls.push(created);
                return created;
              })();

            mergeJsonRecord(toolCall.extra, rawToolCall, ['index', 'id', 'type', 'function']);
            if (typeof rawToolCall.id === 'string') {
              toolCall.id = rawToolCall.id;
            }
            if (typeof rawToolCall.type === 'string') {
              toolCall.type = rawToolCall.type;
            }
            if (isRecord(rawToolCall.function)) {
              toolCall.function = toolCall.function ?? { arguments: '', extra: {} };
              mergeJsonRecord(toolCall.function.extra, rawToolCall.function, [
                'name',
                'arguments',
              ]);
              toolCall.function.name =
                typeof rawToolCall.function.name === 'string'
                  ? rawToolCall.function.name
                  : toolCall.function.name;
              toolCall.function.arguments = appendString(
                toolCall.function.arguments,
                rawToolCall.function.arguments,
              );
            }
          }
        }
      }
    }
  }

  if (!recognized || choices.size === 0) {
    return null;
  }

  const assembledChoices = Array.from(choices.values())
    .sort((a, b) => a.index - b.index)
    .map((choice) => {
      const message: JsonRecord = {
        ...choice.messageExtra,
        role: choice.role ?? 'assistant',
      };
      if (choice.content) {
        message.content = choice.content;
      }
      if (choice.reasoning_content) {
        message.reasoning_content = choice.reasoning_content;
      }
      if (choice.refusal) {
        message.refusal = choice.refusal;
      }

      if (choice.function_call && (choice.function_call.name || choice.function_call.arguments)) {
        message.function_call = {
          ...choice.function_call.extra,
          ...(choice.function_call.name ? { name: choice.function_call.name } : {}),
          arguments: choice.function_call.arguments,
        };
      }

      if (choice.tool_calls.length > 0) {
        message.tool_calls = choice.tool_calls
          .sort((a, b) => a.index - b.index)
          .map((toolCall) => ({
            ...toolCall.extra,
            ...(toolCall.id ? { id: toolCall.id } : {}),
            ...(toolCall.type ? { type: toolCall.type } : {}),
            ...(toolCall.function &&
            (toolCall.function.name || toolCall.function.arguments)
              ? {
                  function: {
                    ...toolCall.function.extra,
                    ...(toolCall.function.name ? { name: toolCall.function.name } : {}),
                    arguments: toolCall.function.arguments,
                  },
                }
              : {}),
          }));
      }

      const choiceExtra = stripDuplicateKeys(choice.extra, topLevel);

      return {
        ...choiceExtra,
        index: choice.index,
        message,
        finish_reason: choice.finish_reason ?? null,
      };
    });

  topLevel.object =
    topLevel.object === 'chat.completion.chunk' ? 'chat.completion' : topLevel.object;

  return {
    mode: 'chat.completions',
    contentType: 'JSON',
    body: JSON.stringify(
      {
        ...topLevel,
        object: 'chat.completion',
        choices: assembledChoices,
      },
      null,
      2,
    ),
  };
};

const buildResponsesTextKey = (payload: JsonRecord): string => {
  const itemId = typeof payload.item_id === 'string' ? payload.item_id : 'message';
  const outputIndex = typeof payload.output_index === 'number' ? payload.output_index : 0;
  const contentIndex = typeof payload.content_index === 'number' ? payload.content_index : 0;
  return `${itemId}:${outputIndex}:${contentIndex}`;
};

const extractResponseMeta = (payload: JsonRecord): JsonRecord | null => {
  if (isRecord(payload.response)) {
    return payload.response;
  }
  if (typeof payload.object === 'string' && payload.object.startsWith('response')) {
    return payload;
  }
  return null;
};

const assembleResponses = (
  parsedEvents: Array<{ event: SSEEvent; payload: JsonRecord }>,
): OpenAiLikeSseAssembly | null => {
  const textByKey = new Map<string, ResponsesTextState>();
  let meta: JsonRecord | null = null;
  let recognized = false;

  for (const { event, payload } of parsedEvents) {
    const eventName = getEventName(event, payload);
    const nextMeta = extractResponseMeta(payload);
    if (nextMeta) {
      meta = nextMeta;
    }

    if (!eventName.startsWith('response.')) {
      continue;
    }

    recognized = true;

    if (eventName === 'response.output_text.delta') {
      const key = buildResponsesTextKey(payload);
      const existing = textByKey.get(key) ?? {
        key,
        item_id: typeof payload.item_id === 'string' ? payload.item_id : undefined,
        output_index: typeof payload.output_index === 'number' ? payload.output_index : 0,
        content_index: typeof payload.content_index === 'number' ? payload.content_index : 0,
        text: '',
      };
      existing.text = appendString(existing.text, payload.delta);
      textByKey.set(key, existing);
      continue;
    }

    if (eventName === 'response.output_text.done') {
      const key = buildResponsesTextKey(payload);
      const existing = textByKey.get(key) ?? {
        key,
        item_id: typeof payload.item_id === 'string' ? payload.item_id : undefined,
        output_index: typeof payload.output_index === 'number' ? payload.output_index : 0,
        content_index: typeof payload.content_index === 'number' ? payload.content_index : 0,
        text: '',
      };
      if (typeof payload.text === 'string') {
        existing.text = payload.text;
      }
      textByKey.set(key, existing);
    }
  }

  if (!recognized) {
    return null;
  }

  const assembledTextParts = Array.from(textByKey.values())
    .sort((a, b) => {
      const outputDiff = (a.output_index ?? 0) - (b.output_index ?? 0);
      if (outputDiff !== 0) return outputDiff;
      return (a.content_index ?? 0) - (b.content_index ?? 0);
    })
    .map((part) => part.text)
    .filter(Boolean);

  const assembledText = assembledTextParts.join('');
  const responseObject: JsonRecord = {
    ...(meta ?? {}),
  };

  if (assembledText) {
    responseObject.output_text = assembledText;
    responseObject.output = [
      {
        type: 'message',
        role: 'assistant',
        content: [
          {
            type: 'output_text',
            text: assembledText,
          },
        ],
      },
    ];
  }

  if (Object.keys(responseObject).length > 0) {
    if (!responseObject.object) {
      responseObject.object = 'response';
    }
    return {
      mode: 'responses',
      contentType: 'JSON',
      body: JSON.stringify(responseObject, null, 2),
    };
  }

  if (!assembledText) {
    return null;
  }

  return {
    mode: 'responses',
    contentType: 'Other',
    body: assembledText,
  };
};

export const assembleOpenAiLikeSse = (
  events: SSEEvent[],
): OpenAiLikeSseAssembly | null => {
  if (events.length === 0) {
    return null;
  }

  const parsedEvents = events
    .map((event) => ({
      event,
      payload: event.data.trim() === '[DONE]' ? null : parseJsonRecord(event.data),
    }))
    .filter(
      (
        item,
      ): item is {
        event: SSEEvent;
        payload: JsonRecord;
      } => item.payload !== null,
    );

  if (parsedEvents.length === 0) {
    return null;
  }

  return assembleChatCompletions(parsedEvents) ?? assembleResponses(parsedEvents);
};
