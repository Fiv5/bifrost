import { useMemo, useState, memo, useCallback, createElement } from 'react';
import { Typography, Tag, theme, Collapse, Tooltip, Button, Space, Modal } from 'antd';
import {
  RobotOutlined,
  UserOutlined,
  ToolOutlined,
  SettingOutlined,
  CodeOutlined,
  CopyOutlined,
  CheckOutlined,
  ApiOutlined,
  ThunderboltOutlined,
  MessageOutlined,
  BulbOutlined,
  WarningOutlined,
  SwapRightOutlined,
  DashboardOutlined,
  ExperimentOutlined,
  NumberOutlined,
  StopOutlined,
  KeyOutlined,
  SafetyOutlined,
  CloudOutlined,
  DatabaseOutlined,
  ControlOutlined,
  FileTextOutlined,
  InfoCircleOutlined,
  ExpandOutlined,
} from '@ant-design/icons';
import hljs from 'highlight.js/lib/core';
import json from 'highlight.js/lib/languages/json';
import '../../../../styles/hljs-github-theme.css';
import type { OpenAiRequestParsed } from '../../helper/openAiLikeRequest';
import { stringifyContent, formatToolArgs } from '../../helper/openAiLikeRequest';

hljs.registerLanguage('json', json);

const { Text, Paragraph } = Typography;

type Msg = Record<string, unknown>;

interface ToolRoundtrip {
  callId: string;
  name: string;
  args: string;
  response: Msg | null;
}

interface ConversationGroup {
  type: 'message';
  message: Msg;
  index: number;
  toolRoundtrips: ToolRoundtrip[];
}

const buildConversationGroups = (messages: Msg[]): ConversationGroup[] => {
  const toolResponseMap = new Map<string, { msg: Msg; idx: number }>();
  const consumedIndices = new Set<number>();

  for (let i = 0; i < messages.length; i++) {
    const msg = messages[i];
    if (msg.role === 'tool' && typeof msg.tool_call_id === 'string') {
      toolResponseMap.set(msg.tool_call_id, { msg, idx: i });
    }
  }

  const groups: ConversationGroup[] = [];

  for (let i = 0; i < messages.length; i++) {
    if (consumedIndices.has(i)) continue;
    const msg = messages[i];
    const toolCalls = msg.tool_calls as unknown[] | undefined;
    const roundtrips: ToolRoundtrip[] = [];

    if (toolCalls && Array.isArray(toolCalls) && toolCalls.length > 0) {
      for (const tc of toolCalls) {
        const call = tc as Record<string, unknown>;
        const callId = call.id as string | undefined;
        const fn = call.function as Record<string, unknown> | undefined;
        const name = (fn?.name as string) ?? 'unknown';
        const args = formatToolArgs(fn?.arguments);

        let response: Msg | null = null;
        if (callId && toolResponseMap.has(callId)) {
          const entry = toolResponseMap.get(callId)!;
          response = entry.msg;
          consumedIndices.add(entry.idx);
        }

        roundtrips.push({ callId: callId ?? '', name, args, response });
      }
    }

    groups.push({ type: 'message', message: msg, index: i, toolRoundtrips: roundtrips });
  }

  return groups;
};

const ROLE_STYLES: Record<string, { color: string; label: string; bg: string }> = {
  system: { color: '#722ed1', label: 'System', bg: 'rgba(114,46,209,0.06)' },
  user: { color: '#1677ff', label: 'User', bg: 'rgba(22,119,255,0.06)' },
  assistant: { color: '#52c41a', label: 'Assistant', bg: 'rgba(82,196,26,0.06)' },
  tool: { color: '#fa8c16', label: 'Tool', bg: 'rgba(250,140,22,0.06)' },
  function: { color: '#13c2c2', label: 'Function', bg: 'rgba(19,194,194,0.06)' },
  developer: { color: '#eb2f96', label: 'Developer', bg: 'rgba(235,47,150,0.06)' },
};

const ROLE_ICONS: Record<string, React.ComponentType> = {
  system: SettingOutlined,
  user: UserOutlined,
  assistant: RobotOutlined,
  tool: ToolOutlined,
  function: CodeOutlined,
  developer: BulbOutlined,
};

const DEFAULT_ROLE_STYLE = { color: '#8c8c8c', label: 'Unknown', bg: 'rgba(140,140,140,0.06)' };

const getRoleStyle = (role: string) => ROLE_STYLES[role] ?? { ...DEFAULT_ROLE_STYLE, label: role };
const getRoleIcon = (role: string) => ROLE_ICONS[role] ?? MessageOutlined;

const copyToClipboard = async (text: string): Promise<boolean> => {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
};

const formatCharCount = (n: number): string => {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
};

const computeMessageCharCount = (message: Msg): number => {
  let total = 0;
  const content = message.content;
  if (typeof content === 'string') {
    total += content.length;
  } else if (Array.isArray(content)) {
    for (const part of content) {
      if (typeof part === 'string') {
        total += part.length;
      } else if (typeof part === 'object' && part !== null) {
        const p = part as Record<string, unknown>;
        if (typeof p.text === 'string') total += p.text.length;
        else total += JSON.stringify(p).length;
      }
    }
  } else if (content !== null && content !== undefined) {
    total += JSON.stringify(content).length;
  }

  if (message.tool_calls && Array.isArray(message.tool_calls)) {
    total += JSON.stringify(message.tool_calls).length;
  }
  if (message.function_call) {
    total += JSON.stringify(message.function_call).length;
  }
  if (typeof message.reasoning_content === 'string') {
    total += (message.reasoning_content as string).length;
  }
  return total;
};

const MAX_HIGHLIGHT_SIZE = 50_000;
const MAX_TEXT_DISPLAY_SIZE = 100_000;
const INVISIBLE_CHARS_RE = /[\s\u200B-\u200D\u2060\u2063\uFEFF]/g;

const CopyButton = memo(({ text }: { text: string }) => {
  const [copied, setCopied] = useState(false);
  const { token } = theme.useToken();
  const handleCopy = useCallback(async () => {
    const ok = await copyToClipboard(text);
    if (ok) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }
  }, [text]);
  return (
    <Tooltip title={copied ? 'Copied!' : 'Copy'}>
      <Button
        type="text"
        size="small"
        icon={copied ? <CheckOutlined style={{ color: token.colorSuccess }} /> : <CopyOutlined />}
        onClick={handleCopy}
        style={{ width: 24, height: 24 }}
      />
    </Tooltip>
  );
});

const JsonBlock = memo(({ data, label }: { data: string; label?: string }) => {
  const { token } = theme.useToken();
  const highlighted = useMemo(() => {
    if (data.length > MAX_HIGHLIGHT_SIZE) return null;
    try {
      return hljs.highlight(data, { language: 'json' }).value;
    } catch {
      return null;
    }
  }, [data]);

  return (
    <div style={{ position: 'relative', marginTop: label ? 0 : 4 }}>
      {label && (
        <Text type="secondary" style={{ fontSize: 11, display: 'block', marginBottom: 2 }}>
          {label}
        </Text>
      )}
      <div style={{ position: 'relative' }}>
        <div style={{ position: 'absolute', top: 4, right: 4, zIndex: 1 }}>
          <CopyButton text={data} />
        </div>
        <pre
          style={{
            margin: 0,
            padding: '8px 32px 8px 8px',
            fontSize: 12,
            fontFamily: 'monospace',
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-all',
            lineHeight: 1.5,
            backgroundColor: token.colorFillQuaternary,
            borderRadius: 6,
            border: `1px solid ${token.colorBorderSecondary}`,
            maxHeight: 300,
            overflow: 'auto',
          }}
        >
          {highlighted
            ? <code className="hljs" dangerouslySetInnerHTML={{ __html: highlighted }} />
            : <code>{data}</code>
          }
        </pre>
      </div>
    </div>
  );
});

const TruncatedText = memo(({ text, token: themeToken }: { text: string; token: { colorText: string } }) => {
  const [expanded, setExpanded] = useState(false);
  const needsTruncation = text.length > MAX_TEXT_DISPLAY_SIZE;
  const displayText = needsTruncation && !expanded ? text.slice(0, MAX_TEXT_DISPLAY_SIZE) : text;

  return (
    <div style={{ position: 'relative' }}>
      <div style={{ position: 'absolute', top: 0, right: 0, zIndex: 1 }}>
        <CopyButton text={text} />
      </div>
      <Paragraph
        style={{
          margin: 0,
          fontSize: 13,
          lineHeight: 1.7,
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-word',
          paddingRight: 28,
          color: themeToken.colorText,
        }}
      >
        {displayText}
      </Paragraph>
      {needsTruncation && !expanded && (
        <Button type="link" size="small" onClick={() => setExpanded(true)} style={{ padding: 0, fontSize: 12, height: 'auto' }}>
          Show all ({formatCharCount(text.length)} chars)
        </Button>
      )}
    </div>
  );
});

const ContentBlock = memo(({ content }: { content: unknown }) => {
  const { token } = theme.useToken();
  if (content === null || content === undefined) {
    return <Text type="secondary" italic style={{ fontSize: 12 }}>(empty)</Text>;
  }

  if (typeof content === 'string') {
    if (!content.trim()) {
      return <Text type="secondary" italic style={{ fontSize: 12 }}>(empty)</Text>;
    }
    return <TruncatedText text={content} token={token} />;
  }

  if (Array.isArray(content)) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        {content.map((part, i) => {
          if (typeof part === 'string') {
            return <ContentBlock key={i} content={part} />;
          }
          if (typeof part === 'object' && part !== null) {
            const p = part as Record<string, unknown>;
            if (p.type === 'text' && typeof p.text === 'string') {
              return <ContentBlock key={i} content={p.text} />;
            }
            if (p.type === 'image_url') {
              const url = typeof p.image_url === 'object' && p.image_url !== null
                ? (p.image_url as Record<string, unknown>).url
                : p.image_url;
              return (
                <Tag key={i} color="purple" style={{ margin: 0 }}>
                  🖼 Image: {typeof url === 'string' ? (url.length > 60 ? url.slice(0, 60) + '…' : url) : 'embedded'}
                </Tag>
              );
            }
            return <JsonBlock key={i} data={JSON.stringify(p, null, 2)} />;
          }
          return <Text key={i}>{String(part)}</Text>;
        })}
      </div>
    );
  }

  return <JsonBlock data={JSON.stringify(content, null, 2)} />;
});

const TOOL_RESULT_MAX_LINES = 10;

const ToolResultContent = memo(({ content }: { content: unknown }) => {
  const [modalOpen, setModalOpen] = useState(false);
  const text = useMemo(() => stringifyContent(content), [content]);
  const lines = useMemo(() => text.split('\n'), [text]);
  const needsTruncation = lines.length > TOOL_RESULT_MAX_LINES;
  const previewText = needsTruncation ? lines.slice(0, TOOL_RESULT_MAX_LINES).join('\n') : text;
  const { token } = theme.useToken();

  if (content === null || content === undefined || !text.trim()) {
    return <Text type="secondary" italic style={{ fontSize: 12 }}>(empty)</Text>;
  }

  return (
    <>
      <div style={{ position: 'relative' }}>
        <pre
          style={{
            margin: 0,
            padding: '6px 8px',
            fontSize: 12,
            fontFamily: 'monospace',
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-all',
            lineHeight: 1.5,
            backgroundColor: token.colorFillQuaternary,
            borderRadius: 6,
            border: `1px solid ${token.colorBorderSecondary}`,
            color: token.colorText,
          }}
        >
          {previewText}
          {needsTruncation && (
            <span style={{ color: token.colorTextSecondary }}>{'\n'}…</span>
          )}
        </pre>
        {needsTruncation && (
          <Button
            type="link"
            size="small"
            icon={<ExpandOutlined />}
            onClick={() => setModalOpen(true)}
            style={{ padding: 0, fontSize: 11, height: 'auto', marginTop: 2 }}
          >
            Show all ({lines.length} lines)
          </Button>
        )}
      </div>
      {needsTruncation && (
        <Modal
          title={
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', paddingRight: 24 }}>
              <span>Tool Result ({lines.length} lines)</span>
              <CopyButton text={text} />
            </div>
          }
          open={modalOpen}
          onCancel={() => setModalOpen(false)}
          footer={null}
          width="80vw"
          styles={{ body: { maxHeight: '70vh', overflow: 'auto', padding: 0 } }}
        >
          <pre
            style={{
              margin: 0,
              padding: 12,
              fontSize: 12,
              fontFamily: 'monospace',
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-all',
              lineHeight: 1.6,
            }}
          >
            {text}
          </pre>
        </Modal>
      )}
    </>
  );
});

const ToolRoundtripCard = memo(({ roundtrip }: { roundtrip: ToolRoundtrip }) => {
  const { token } = theme.useToken();
  const { name, args, callId, response } = roundtrip;
  const responseContent = response?.content;
  const responseName = response?.name as string | undefined;

  return (
    <div
      style={{
        borderRadius: 8,
        border: `1px solid rgba(250,140,22,0.3)`,
        overflow: 'hidden',
        backgroundColor: token.colorBgContainer,
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 6,
          padding: '5px 10px',
          backgroundColor: 'rgba(250,140,22,0.06)',
          borderBottom: `1px solid rgba(250,140,22,0.2)`,
        }}
      >
        <ApiOutlined style={{ color: '#fa8c16', fontSize: 12 }} />
        <Text strong style={{ fontSize: 12, color: '#fa8c16' }}>{name}</Text>
        {callId && (
          <Text type="secondary" style={{ fontSize: 10, fontFamily: 'monospace', opacity: 0.7 }}>
            {callId}
          </Text>
        )}
      </div>

      <div style={{ display: 'flex', flexDirection: 'column' }}>
        {args && (
          <div style={{ padding: '6px 10px', borderBottom: response ? `1px dashed ${token.colorBorderSecondary}` : undefined }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 4 }}>
              <SwapRightOutlined style={{ fontSize: 10, color: '#1677ff' }} />
              <Text type="secondary" style={{ fontSize: 11 }}>Arguments</Text>
            </div>
            <JsonBlock data={args} />
          </div>
        )}

        {response ? (
          <div style={{ padding: '6px 10px', backgroundColor: 'rgba(82,196,26,0.03)' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 4 }}>
              <CheckOutlined style={{ fontSize: 10, color: '#52c41a' }} />
              <Text type="secondary" style={{ fontSize: 11 }}>Result</Text>
              {responseName && responseName !== name && (
                <Tag style={{ margin: 0, fontSize: 10 }}>{responseName}</Tag>
              )}
            </div>
            <ToolResultContent content={responseContent} />
          </div>
        ) : (
          <div style={{ padding: '4px 10px', backgroundColor: 'rgba(140,140,140,0.04)' }}>
            <Text type="secondary" italic style={{ fontSize: 11 }}>No response in context</Text>
          </div>
        )}
      </div>
    </div>
  );
});

const MessageCard = memo(({ group }: { group: ConversationGroup }) => {
  const { token } = theme.useToken();
  const { message, index, toolRoundtrips } = group;
  const role = (message.role as string) ?? 'unknown';
  const style = getRoleStyle(role);
  const roleIcon = useMemo(() => createElement(getRoleIcon(role)), [role]);
  const content = message.content;
  const functionCall = message.function_call as Record<string, unknown> | undefined;
  const toolCallId = message.tool_call_id as string | undefined;
  const name = message.name as string | undefined;
  const reasoningContent = message.reasoning_content as string | undefined;
  const refusal = message.refusal as string | undefined;

  const charCount = useMemo(() => computeMessageCharCount(message), [message]);

  const extras = useMemo(() => {
    const extraKeys = new Set([
      'role', 'content', 'tool_calls', 'function_call',
      'tool_call_id', 'name', 'reasoning_content', 'refusal',
    ]);
    const result: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(message)) {
      if (!extraKeys.has(k) && v !== undefined) result[k] = v;
    }
    return result;
  }, [message]);
  const hasExtras = Object.keys(extras).length > 0;

  const hasContent = content !== null && content !== undefined && (typeof content !== 'string' || content.trim().length > 0);
  const hasVisibleReasoning = !!reasoningContent && reasoningContent.replace(INVISIBLE_CHARS_RE, '').length > 0;

  return (
    <div
      style={{
        borderRadius: 8,
        border: `1px solid ${token.colorBorderSecondary}`,
        overflow: 'hidden',
        backgroundColor: style.bg,
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '6px 10px',
          borderBottom: `1px solid ${token.colorBorderSecondary}`,
        }}
      >
        <Space size={6} align="center">
          <Text type="secondary" style={{ fontSize: 10, fontFamily: 'monospace' }}>#{index + 1}</Text>
          <Tag
            color={style.color}
            style={{ margin: 0, fontSize: 11, lineHeight: '18px', padding: '0 6px' }}
            icon={roleIcon}
          >
            {style.label}
          </Tag>
          {name && (
            <Tag style={{ margin: 0, fontSize: 10 }}>{name}</Tag>
          )}
          {toolCallId && (
            <Text type="secondary" style={{ fontSize: 10, fontFamily: 'monospace' }}>
              ← {toolCallId}
            </Text>
          )}
          {toolRoundtrips.length > 0 && (
            <Tag color="orange" style={{ margin: 0, fontSize: 10, lineHeight: '16px', padding: '0 4px' }}>
              🔧 {toolRoundtrips.length} tool{toolRoundtrips.length > 1 ? 's' : ''}
            </Tag>
          )}
        </Space>
        <Space size={4} align="center">
          <Tooltip title={`${charCount.toLocaleString()} chars`}>
            <Tag
              style={{
                margin: 0,
                fontSize: 10,
                lineHeight: '16px',
                padding: '0 4px',
                fontFamily: 'monospace',
                opacity: 0.7,
                cursor: 'default',
              }}
            >
              {formatCharCount(charCount)}
            </Tag>
          </Tooltip>
          {hasContent && <CopyButton text={stringifyContent(content)} />}
        </Space>
      </div>

      <div style={{ padding: '8px 10px', display: 'flex', flexDirection: 'column', gap: 8 }}>
        {hasVisibleReasoning && (
          <div
            style={{
              padding: '6px 8px',
              backgroundColor: 'rgba(114,46,209,0.06)',
              borderRadius: 6,
              borderLeft: '3px solid #722ed1',
            }}
          >
            <Text type="secondary" style={{ fontSize: 11, display: 'block', marginBottom: 2 }}>
              💭 Reasoning
            </Text>
            <Paragraph
              style={{
                margin: 0,
                fontSize: 12,
                lineHeight: 1.6,
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-word',
                color: token.colorTextSecondary,
              }}
            >
              {reasoningContent}
            </Paragraph>
          </div>
        )}

        {hasContent && <ContentBlock content={content} />}

        {refusal && (
          <div
            style={{
              padding: '6px 8px',
              backgroundColor: 'rgba(255,77,79,0.06)',
              borderRadius: 6,
              borderLeft: '3px solid #ff4d4f',
            }}
          >
            <Text type="secondary" style={{ fontSize: 11, display: 'block', marginBottom: 2 }}>
              🚫 Refusal
            </Text>
            <Text style={{ fontSize: 12 }}>{refusal}</Text>
          </div>
        )}

        {functionCall && (
          <div>
            <Text type="secondary" style={{ fontSize: 11, display: 'block', marginBottom: 4 }}>
              ⚡ Function Call
            </Text>
            <ToolRoundtripCard
              roundtrip={{
                callId: '',
                name: (functionCall.name as string) ?? 'unknown',
                args: formatToolArgs(functionCall.arguments),
                response: null,
              }}
            />
          </div>
        )}

        {toolRoundtrips.length > 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
              <ToolOutlined style={{ fontSize: 12, color: '#fa8c16' }} />
              <Text strong style={{ fontSize: 12, color: '#fa8c16' }}>
                Tool Execution ({toolRoundtrips.length})
              </Text>
            </div>
            {toolRoundtrips.map((rt, i) => (
              <ToolRoundtripCard key={rt.callId || i} roundtrip={rt} />
            ))}
          </div>
        )}

        {hasExtras && (
          <Collapse
            size="small"
            items={[{
              key: 'extras',
              label: <Text type="secondary" style={{ fontSize: 11 }}>Extra fields</Text>,
              children: <JsonBlock data={JSON.stringify(extras, null, 2)} />,
            }]}
            style={{ backgroundColor: 'transparent' }}
          />
        )}
      </div>
    </div>
  );
});

const ToolDefinitionCard = memo(({ tool, index }: { tool: Record<string, unknown>; index: number }) => {
  const { token } = theme.useToken();
  const fn = tool.function as Record<string, unknown> | undefined;
  const name = fn?.name as string | undefined;
  const description = fn?.description as string | undefined;
  const parameters = fn?.parameters;
  const formattedParams = useMemo(
    () => parameters ? JSON.stringify(parameters, null, 2) : null,
    [parameters],
  );

  return (
    <div
      style={{
        borderRadius: 6,
        border: `1px solid ${token.colorBorderSecondary}`,
        overflow: 'hidden',
        backgroundColor: token.colorBgContainer,
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 6,
          padding: '4px 8px',
          backgroundColor: 'rgba(19,194,194,0.06)',
          borderBottom: `1px solid ${token.colorBorderSecondary}`,
        }}
      >
        <CodeOutlined style={{ color: '#13c2c2', fontSize: 12 }} />
        <Text strong style={{ fontSize: 12 }}>
          {name ?? `tool[${index}]`}
        </Text>
        <Tag style={{ margin: 0, fontSize: 10 }}>{(tool.type as string) ?? 'function'}</Tag>
      </div>
      <div style={{ padding: '6px 8px', display: 'flex', flexDirection: 'column', gap: 4 }}>
        {description && (
          <Text type="secondary" style={{ fontSize: 12, lineHeight: 1.5 }}>{description}</Text>
        )}
        {formattedParams && (
          <Collapse
            size="small"
            items={[{
              key: 'params',
              label: <Text type="secondary" style={{ fontSize: 11 }}>Parameters Schema</Text>,
              children: <JsonBlock data={formattedParams} />,
            }]}
            style={{ backgroundColor: 'transparent' }}
          />
        )}
      </div>
    </div>
  );
});

interface ParamMeta {
  iconType: React.ComponentType<{ style?: React.CSSProperties }>;
  iconColor: string;
  label: string;
  desc: string;
  render?: (value: unknown) => React.ReactNode;
}

const boolTag = (val: unknown): React.ReactNode => {
  const b = Boolean(val);
  return <Tag color={b ? 'green' : 'default'} style={{ margin: 0, fontSize: 11 }}>{b ? 'true' : 'false'}</Tag>;
};

const numTag = (val: unknown): React.ReactNode => (
  <Text strong style={{ fontFamily: 'monospace', fontSize: 12 }}>{String(val)}</Text>
);

const jsonTag = (val: unknown): React.ReactNode => {
  const str = typeof val === 'string' ? val : JSON.stringify(val, null, 2);
  return (
    <pre style={{ margin: 0, fontSize: 11, fontFamily: 'monospace', whiteSpace: 'pre-wrap', wordBreak: 'break-all', lineHeight: 1.4 }}>
      {str}
    </pre>
  );
};

const strTag = (val: unknown): React.ReactNode => (
  <Text style={{ fontFamily: 'monospace', fontSize: 12 }}>{String(val)}</Text>
);

const KNOWN_PARAMS: Record<string, ParamMeta> = {
  temperature: { iconType: DashboardOutlined, iconColor: '#fa541c', label: 'Temperature', desc: 'Sampling randomness (0-2). Higher = more creative, lower = more deterministic', render: numTag },
  top_p: { iconType: ExperimentOutlined, iconColor: '#13c2c2', label: 'Top P', desc: 'Nucleus sampling. Model considers tokens with top_p cumulative probability', render: numTag },
  max_tokens: { iconType: NumberOutlined, iconColor: '#1677ff', label: 'Max Tokens', desc: 'Maximum number of tokens to generate in the completion', render: numTag },
  max_completion_tokens: { iconType: NumberOutlined, iconColor: '#1677ff', label: 'Max Completion Tokens', desc: 'Upper bound on tokens generated, including reasoning tokens', render: numTag },
  n: { iconType: NumberOutlined, iconColor: '#722ed1', label: 'N (Completions)', desc: 'Number of completions to generate for each prompt', render: numTag },
  stop: {
    iconType: StopOutlined, iconColor: '#ff4d4f', label: 'Stop Sequences', desc: 'Sequences where the model will stop generating further tokens',
    render: (val) => {
      if (Array.isArray(val)) {
        return <Space size={4} wrap>{val.map((s, i) => <Tag key={i} style={{ margin: 0, fontSize: 11, fontFamily: 'monospace' }}>{String(s)}</Tag>)}</Space>;
      }
      return strTag(val);
    },
  },
  presence_penalty: { iconType: ControlOutlined, iconColor: '#eb2f96', label: 'Presence Penalty', desc: 'Penalize new tokens based on whether they appear in text so far (-2 to 2)', render: numTag },
  frequency_penalty: { iconType: ControlOutlined, iconColor: '#fa8c16', label: 'Frequency Penalty', desc: 'Penalize new tokens based on their frequency in text so far (-2 to 2)', render: numTag },
  logit_bias: { iconType: ControlOutlined, iconColor: '#8c8c8c', label: 'Logit Bias', desc: 'Modify likelihood of specified tokens appearing in completion', render: jsonTag },
  logprobs: { iconType: DatabaseOutlined, iconColor: '#722ed1', label: 'Logprobs', desc: 'Whether to return log probabilities of output tokens', render: boolTag },
  top_logprobs: { iconType: DatabaseOutlined, iconColor: '#722ed1', label: 'Top Logprobs', desc: 'Number of most likely tokens to return at each position (0-20)', render: numTag },
  response_format: {
    iconType: FileTextOutlined, iconColor: '#52c41a', label: 'Response Format', desc: 'Output format: text, json_object, or json_schema',
    render: (val) => {
      if (typeof val === 'object' && val !== null) {
        const v = val as Record<string, unknown>;
        if (v.type) {
          return <Space size={4}><Tag color="blue" style={{ margin: 0, fontSize: 11 }}>{String(v.type)}</Tag>{!!v.json_schema && <Text type="secondary" style={{ fontSize: 10 }}>(with schema)</Text>}</Space>;
        }
      }
      return jsonTag(val);
    },
  },
  seed: { iconType: KeyOutlined, iconColor: '#faad14', label: 'Seed', desc: 'Deterministic sampling seed for reproducible outputs', render: numTag },
  user: { iconType: UserOutlined, iconColor: '#8c8c8c', label: 'User', desc: 'Unique identifier for the end-user, for abuse monitoring', render: strTag },
  reasoning: {
    iconType: BulbOutlined, iconColor: '#722ed1', label: 'Reasoning', desc: 'Reasoning/thinking configuration for o-series and reasoning models',
    render: (val) => {
      if (typeof val === 'object' && val !== null) {
        const v = val as Record<string, unknown>;
        if (v.effort) {
          const c: Record<string, string> = { low: 'default', medium: 'blue', high: 'orange' };
          return <Tag color={c[String(v.effort)] ?? 'default'} style={{ margin: 0, fontSize: 11 }}>{String(v.effort)}</Tag>;
        }
      }
      return jsonTag(val);
    },
  },
  reasoning_effort: {
    iconType: BulbOutlined, iconColor: '#722ed1', label: 'Reasoning Effort', desc: 'How much effort the model should put into reasoning (low/medium/high)',
    render: (val) => {
      const c: Record<string, string> = { low: 'default', medium: 'blue', high: 'orange' };
      return <Tag color={c[String(val)] ?? 'default'} style={{ margin: 0, fontSize: 11 }}>{String(val)}</Tag>;
    },
  },
  tool_choice: {
    iconType: ToolOutlined, iconColor: '#fa8c16', label: 'Tool Choice', desc: 'Controls which tool the model calls: auto, none, required, or specific function',
    render: (val) => {
      if (typeof val === 'string') {
        const c: Record<string, string> = { auto: 'blue', none: 'default', required: 'orange' };
        return <Tag color={c[val] ?? 'default'} style={{ margin: 0, fontSize: 11 }}>{val}</Tag>;
      }
      return jsonTag(val);
    },
  },
  parallel_tool_calls: { iconType: ThunderboltOutlined, iconColor: '#fa8c16', label: 'Parallel Tool Calls', desc: 'Whether the model can make multiple tool calls in one turn', render: boolTag },
  stream_options: {
    iconType: CloudOutlined, iconColor: '#1677ff', label: 'Stream Options', desc: 'Streaming configuration (e.g. include_usage for token counting)',
    render: (val) => {
      if (typeof val === 'object' && val !== null) {
        const entries = Object.entries(val as Record<string, unknown>);
        return <Space size={4} wrap>{entries.map(([k, v]) => <Tag key={k} style={{ margin: 0, fontSize: 10 }}>{k}: {String(v)}</Tag>)}</Space>;
      }
      return jsonTag(val);
    },
  },
  service_tier: { iconType: SafetyOutlined, iconColor: '#52c41a', label: 'Service Tier', desc: 'Quality of service tier (auto, default, flex)', render: (val) => <Tag style={{ margin: 0, fontSize: 11 }}>{String(val)}</Tag> },
  store: { iconType: DatabaseOutlined, iconColor: '#8c8c8c', label: 'Store', desc: 'Whether to store the output for model distillation or evals', render: boolTag },
  metadata: { iconType: InfoCircleOutlined, iconColor: '#8c8c8c', label: 'Metadata', desc: 'Developer-defined tags and values for filtering completions', render: jsonTag },
  prediction: { iconType: ExperimentOutlined, iconColor: '#722ed1', label: 'Prediction', desc: 'Predicted output to speed up generation with speculative decoding', render: jsonTag },
  audio: { iconType: SettingOutlined, iconColor: '#13c2c2', label: 'Audio', desc: 'Audio output configuration (voice, format)', render: jsonTag },
  modalities: {
    iconType: SettingOutlined, iconColor: '#eb2f96', label: 'Modalities', desc: 'Output modalities to generate (text, audio)',
    render: (val) => {
      if (Array.isArray(val)) return <Space size={4}>{val.map((v, i) => <Tag key={i} style={{ margin: 0, fontSize: 11 }}>{String(v)}</Tag>)}</Space>;
      return strTag(val);
    },
  },
  functions: { iconType: CodeOutlined, iconColor: '#13c2c2', label: 'Functions (Deprecated)', desc: 'Legacy function definitions (deprecated, use tools instead)', render: (val) => <Tag style={{ margin: 0, fontSize: 10 }}>{Array.isArray(val) ? `${val.length} functions` : 'set'}</Tag> },
  function_call: {
    iconType: CodeOutlined, iconColor: '#13c2c2', label: 'Function Call (Deprecated)', desc: 'Legacy function call mode (deprecated, use tool_choice instead)',
    render: (val) => {
      if (typeof val === 'string') return <Tag style={{ margin: 0, fontSize: 11 }}>{val}</Tag>;
      return jsonTag(val);
    },
  },
};

const ConfigRow = memo(({ label, desc, iconType: Icon, iconColor, value, render }: {
  label: string;
  desc: string;
  iconType: React.ComponentType<{ style?: React.CSSProperties }>;
  iconColor: string;
  value: unknown;
  render?: (value: unknown) => React.ReactNode;
}) => {
  const { token } = theme.useToken();
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: '5px 0',
        borderBottom: `1px solid ${token.colorFillQuaternary}`,
        gap: 8,
      }}
    >
      <Space size={6} style={{ flexShrink: 0 }}>
        <Icon style={{ color: iconColor, fontSize: 12 }} />
        <Tooltip title={desc}>
          <Text type="secondary" style={{ fontSize: 12, cursor: 'help', borderBottom: '1px dashed', borderColor: token.colorBorderSecondary }}>
            {label}
          </Text>
        </Tooltip>
      </Space>
      <div style={{ textAlign: 'right', minWidth: 0, overflow: 'hidden' }}>
        {render ? render(value) : strTag(value)}
      </div>
    </div>
  );
});

const ConfigPanel = memo(({ model, stream, config }: { model: string; stream: boolean | null; config: Record<string, unknown> }) => {
  const { token } = theme.useToken();

  const { knownItems, unknownItems } = useMemo(() => {
    const known: { key: string; meta: ParamMeta; value: unknown }[] = [];
    const unknown: { key: string; value: unknown }[] = [];
    for (const [key, value] of Object.entries(config)) {
      const meta = KNOWN_PARAMS[key];
      if (meta) {
        known.push({ key, meta, value });
      } else {
        unknown.push({ key, value });
      }
    }
    return { knownItems: known, unknownItems: unknown };
  }, [config]);

  return (
    <div
      style={{
        borderRadius: 8,
        border: `1px solid ${token.colorBorderSecondary}`,
        overflow: 'hidden',
        backgroundColor: token.colorBgContainer,
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 6,
          padding: '6px 10px',
          backgroundColor: 'rgba(22,119,255,0.06)',
          borderBottom: `1px solid ${token.colorBorderSecondary}`,
        }}
      >
        <ThunderboltOutlined style={{ color: '#1677ff', fontSize: 13 }} />
        <Text strong style={{ fontSize: 13 }}>Model Configuration</Text>
      </div>

      <div style={{ padding: '6px 10px', display: 'flex', flexDirection: 'column', gap: 0 }}>
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: '5px 0',
            borderBottom: `1px solid ${token.colorFillQuaternary}`,
          }}
        >
          <Space size={6}>
            <RobotOutlined style={{ color: '#52c41a', fontSize: 12 }} />
            <Text type="secondary" style={{ fontSize: 12 }}>Model</Text>
          </Space>
          <Text strong style={{ fontFamily: 'monospace', fontSize: 12 }}>{model}</Text>
        </div>

        {stream !== null && (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              padding: '5px 0',
              borderBottom: `1px solid ${token.colorFillQuaternary}`,
            }}
          >
            <Space size={6}>
              <CloudOutlined style={{ color: '#1677ff', fontSize: 12 }} />
              <Tooltip title="Whether to stream partial responses as server-sent events">
                <Text type="secondary" style={{ fontSize: 12, cursor: 'help', borderBottom: '1px dashed', borderColor: token.colorBorderSecondary }}>
                  Stream
                </Text>
              </Tooltip>
            </Space>
            <Tag color={stream ? 'green' : 'default'} style={{ margin: 0, fontSize: 11 }}>
              {stream ? 'Enabled' : 'Disabled'}
            </Tag>
          </div>
        )}

        {knownItems.map(({ key, meta, value }) => (
          <ConfigRow
            key={key}
            label={meta.label}
            desc={meta.desc}
            iconType={meta.iconType}
            iconColor={meta.iconColor}
            value={value}
            render={meta.render}
          />
        ))}

        {unknownItems.map(({ key, value }) => {
          const isObj = typeof value === 'object' && value !== null;
          const display = isObj ? JSON.stringify(value) : String(value);
          return (
            <div
              key={key}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '5px 0',
                borderBottom: `1px solid ${token.colorFillQuaternary}`,
                gap: 8,
              }}
            >
              <Space size={6} style={{ flexShrink: 0 }}>
                <InfoCircleOutlined style={{ color: '#8c8c8c', fontSize: 12 }} />
                <Text type="secondary" style={{ fontSize: 12, fontFamily: 'monospace' }}>{key}</Text>
              </Space>
              <Text
                style={{
                  fontSize: 12,
                  fontFamily: 'monospace',
                  wordBreak: 'break-all',
                  textAlign: 'right',
                  maxWidth: '60%',
                }}
              >
                {display.length > 120 ? display.slice(0, 120) + '…' : display}
              </Text>
            </div>
          );
        })}
      </div>
    </div>
  );
});

interface OpenAiChatViewProps {
  parsed: OpenAiRequestParsed;
}

export const OpenAiChatView = memo(({ parsed }: OpenAiChatViewProps) => {
  const { token } = theme.useToken();

  const groups = useMemo(
    () => buildConversationGroups(parsed.messages as Msg[]),
    [parsed.messages],
  );

  const totalChars = useMemo(
    () => parsed.messages.reduce((sum, msg) => sum + computeMessageCharCount(msg as Msg), 0),
    [parsed.messages],
  );

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        padding: '8px 4px',
      }}
    >
      {parsed.truncated && (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 6,
            padding: '6px 10px',
            borderRadius: 6,
            backgroundColor: 'rgba(250,173,20,0.08)',
            border: `1px solid rgba(250,173,20,0.3)`,
          }}
        >
          <WarningOutlined style={{ color: '#faad14', fontSize: 13 }} />
          <Text type="secondary" style={{ fontSize: 12 }}>
            Body too large for full JSON parse — extracted via regex scan. Some fields may be incomplete.
          </Text>
        </div>
      )}

      <ConfigPanel model={parsed.model} stream={parsed.stream} config={parsed.config} />

      {parsed.tools.length > 0 && (
        <Collapse
          size="small"
          defaultActiveKey={parsed.tools.length <= 3 ? ['tools'] : []}
          items={[{
            key: 'tools',
            label: (
              <Space size={6}>
                <ToolOutlined style={{ color: '#13c2c2' }} />
                <Text strong style={{ fontSize: 13 }}>Tool Definitions</Text>
                <Tag style={{ margin: 0, fontSize: 11 }}>{parsed.tools.length}</Tag>
              </Space>
            ),
            children: (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                {parsed.tools.map((tool, i) => (
                  <ToolDefinitionCard key={i} tool={tool as Record<string, unknown>} index={i} />
                ))}
              </div>
            ),
          }]}
          style={{
            borderRadius: 8,
            border: `1px solid ${token.colorBorderSecondary}`,
            backgroundColor: token.colorBgContainer,
          }}
        />
      )}

      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '4px 0',
        }}
      >
        <Space size={6}>
          <MessageOutlined style={{ color: token.colorTextSecondary, fontSize: 13 }} />
          <Text strong style={{ fontSize: 13 }}>Conversation</Text>
          <Tag style={{ margin: 0, fontSize: 11 }}>{parsed.messages.length}</Tag>
        </Space>
        <Tooltip title={`Total: ${totalChars.toLocaleString()} chars`}>
          <Tag style={{ margin: 0, fontSize: 10, fontFamily: 'monospace', cursor: 'default' }}>
            Σ {formatCharCount(totalChars)} chars
          </Tag>
        </Tooltip>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        {groups.map((g) => (
          <MessageCard key={g.index} group={g} />
        ))}
      </div>
    </div>
  );
});
