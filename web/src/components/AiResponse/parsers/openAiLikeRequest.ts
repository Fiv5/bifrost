interface OpenAiMessage {
  role: string;
  content: unknown;
  name?: string;
  tool_call_id?: string;
  tool_calls?: unknown[];
  function_call?: unknown;
  refusal?: string;
  [key: string]: unknown;
}

interface OpenAiTool {
  type: string;
  function?: {
    name: string;
    description?: string;
    parameters?: unknown;
  };
  [key: string]: unknown;
}

export interface OpenAiRequestParsed {
  model: string;
  messages: OpenAiMessage[];
  tools: OpenAiTool[];
  stream: boolean | null;
  config: Record<string, unknown>;
  truncated?: boolean;
}

const isRecord = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);

const fromFullParse = (parsed: Record<string, unknown>): OpenAiRequestParsed | null => {
  if (!Array.isArray(parsed.messages)) return null;
  if (typeof parsed.model !== 'string') return null;

  const messages: OpenAiMessage[] = [];
  for (const msg of parsed.messages) {
    if (!isRecord(msg)) continue;
    if (typeof msg.role !== 'string') continue;
    messages.push(msg as OpenAiMessage);
  }
  if (messages.length === 0) return null;

  const tools: OpenAiTool[] = [];
  if (Array.isArray(parsed.tools)) {
    for (const tool of parsed.tools) {
      if (!isRecord(tool)) continue;
      tools.push(tool as OpenAiTool);
    }
  }

  const configKeys = new Set(['model', 'messages', 'tools', 'stream']);
  const config: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(parsed)) {
    if (configKeys.has(key)) continue;
    config[key] = value;
  }

  return {
    model: parsed.model as string,
    messages,
    tools,
    stream: typeof parsed.stream === 'boolean' ? parsed.stream : null,
    config,
  };
};

const RE_MODEL = /"model"\s*:\s*"([^"]{1,200})"/;
const RE_MESSAGES = /"messages"\s*:\s*\[/;
const RE_STREAM = /"stream"\s*:\s*(true|false)/;
const RE_ROLE = /"role"\s*:\s*"(system|user|assistant|tool|function|developer)"/g;
const RE_TOOLS = /"tools"\s*:\s*\[/;
const RE_MAX_TOKENS = /"max_tokens"\s*:\s*(\d+)/;
const RE_TEMPERATURE = /"temperature"\s*:\s*([\d.]+)/;
const RE_TOP_P = /"top_p"\s*:\s*([\d.]+)/;
const RE_FUNC_NAME = /"function"\s*:\s*\{[^}]*?"name"\s*:\s*"([^"]{1,200})"/g;

interface RegexMessageHit {
  offset: number;
  role: string;
}

const scanRoles = (text: string): RegexMessageHit[] => {
  const hits: RegexMessageHit[] = [];
  RE_ROLE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = RE_ROLE.exec(text)) !== null) {
    hits.push({ offset: m.index, role: m[1] });
  }
  return hits;
};

const findContentBetween = (text: string, start: number, end: number): string => {
  const re = /"content"\s*:\s*/g;
  re.lastIndex = 0;
  const slice = text.slice(start, end);
  const m = re.exec(slice);
  if (!m) return '';

  const contentStart = start + m.index + m[0].length;
  return extractJsonValue(text, contentStart);
};

const extractJsonValue = (text: string, offset: number): string => {
  if (offset >= text.length) return '';
  const ch = text[offset];

  if (ch === '"') {
    let i = offset + 1;
    let result = '';
    while (i < text.length) {
      if (text[i] === '\\') {
        const next = text[i + 1];
        if (next === 'n') { result += '\n'; i += 2; continue; }
        if (next === 't') { result += '\t'; i += 2; continue; }
        if (next === '"') { result += '"'; i += 2; continue; }
        if (next === '\\') { result += '\\'; i += 2; continue; }
        if (next === '/') { result += '/'; i += 2; continue; }
        if (next === 'r') { result += '\r'; i += 2; continue; }
        if (next === 'u') {
          const hex = text.slice(i + 2, i + 6);
          result += String.fromCharCode(parseInt(hex, 16));
          i += 6;
          continue;
        }
        result += next;
        i += 2;
        continue;
      }
      if (text[i] === '"') break;
      result += text[i];
      i++;
    }
    return result;
  }

  if (ch === '[' || ch === '{') {
    let bracketDepth = 0;
    let braceDepth = 0;
    let inStr = false;
    let i = offset;
    while (i < text.length) {
      const c = text[i];
      if (inStr) {
        if (c === '\\') { i += 2; continue; }
        if (c === '"') inStr = false;
        i++;
        continue;
      }
      if (c === '"') { inStr = true; i++; continue; }
      if (c === '[') bracketDepth++;
      else if (c === ']') bracketDepth--;
      else if (c === '{') braceDepth++;
      else if (c === '}') braceDepth--;
      i++;
      if (bracketDepth === 0 && braceDepth === 0) break;
    }
    const raw = text.slice(offset, i);
    try {
      return JSON.stringify(JSON.parse(raw), null, 2);
    } catch {
      return raw;
    }
  }

  if (ch === 'n' && text.slice(offset, offset + 4) === 'null') return '';

  const endRe = /[,\s}\]]/;
  let i = offset;
  while (i < text.length && !endRe.test(text[i])) i++;
  return text.slice(offset, i);
};

const regexScan = (text: string): OpenAiRequestParsed | null => {
  const modelMatch = RE_MODEL.exec(text);
  if (!modelMatch) return null;
  if (!RE_MESSAGES.test(text)) return null;

  const roleHits = scanRoles(text);
  if (roleHits.length === 0) return null;

  const streamMatch = RE_STREAM.exec(text);
  const stream = streamMatch ? streamMatch[1] === 'true' : null;

  const config: Record<string, unknown> = {};
  const maxTokensMatch = RE_MAX_TOKENS.exec(text);
  if (maxTokensMatch) config.max_tokens = Number(maxTokensMatch[1]);
  const tempMatch = RE_TEMPERATURE.exec(text);
  if (tempMatch) config.temperature = Number(tempMatch[1]);
  const topPMatch = RE_TOP_P.exec(text);
  if (topPMatch) config.top_p = Number(topPMatch[1]);

  const hasTools = RE_TOOLS.test(text);
  const tools: OpenAiTool[] = [];
  if (hasTools) {
    RE_FUNC_NAME.lastIndex = 0;
    const toolNames = new Set<string>();
    let tm: RegExpExecArray | null;
    while ((tm = RE_FUNC_NAME.exec(text)) !== null) {
      toolNames.add(tm[1]);
    }
    for (const name of toolNames) {
      tools.push({ type: 'function', function: { name } });
    }
  }

  const messagesStart = text.search(RE_MESSAGES);
  const sortedHits = roleHits
    .filter(h => h.offset > messagesStart)
    .sort((a, b) => a.offset - b.offset);

  const messages: OpenAiMessage[] = [];
  for (let i = 0; i < sortedHits.length; i++) {
    const hit = sortedHits[i];
    const regionStart = hit.offset;
    const regionEnd = i + 1 < sortedHits.length
      ? sortedHits[i + 1].offset
      : Math.min(text.length, regionStart + 100_000);

    const msg: OpenAiMessage = {
      role: hit.role,
      content: null,
    };

    const contentStr = findContentBetween(text, regionStart, regionEnd);
    if (contentStr) {
      msg.content = contentStr;
    }

    const nameRe = /"name"\s*:\s*"([^"]{1,200})"/;
    const nameSlice = text.slice(regionStart, Math.min(regionEnd, regionStart + 2000));
    const nameMatch = nameRe.exec(nameSlice);
    if (nameMatch) {
      msg.name = nameMatch[1];
    }

    const toolCallIdRe = /"tool_call_id"\s*:\s*"([^"]{1,200})"/;
    const toolCallIdSlice = text.slice(regionStart, Math.min(regionEnd, regionStart + 2000));
    const toolCallIdMatch = toolCallIdRe.exec(toolCallIdSlice);
    if (toolCallIdMatch) {
      msg.tool_call_id = toolCallIdMatch[1];
    }

    messages.push(msg);
  }

  if (messages.length === 0) return null;

  return {
    model: modelMatch[1],
    messages,
    tools,
    stream,
    config,
    truncated: true,
  };
};

const MAX_FULL_PARSE_SIZE = 5 * 1024 * 1024;

export const parseOpenAiLikeRequest = (body: string | null | undefined): OpenAiRequestParsed | null => {
  if (!body) return null;

  if (body.length <= MAX_FULL_PARSE_SIZE) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(body);
    } catch {
      return regexScan(body);
    }
    if (!isRecord(parsed)) return null;
    return fromFullParse(parsed);
  }

  return regexScan(body);
};

export const stringifyContent = (content: unknown): string => {
  if (content === null || content === undefined) return '';
  if (typeof content === 'string') return content;
  return JSON.stringify(content, null, 2);
};

export const formatToolArgs = (args: unknown): string => {
  if (typeof args === 'string') {
    try {
      return JSON.stringify(JSON.parse(args), null, 2);
    } catch {
      return args;
    }
  }
  if (args === null || args === undefined) return '';
  return JSON.stringify(args, null, 2);
};
