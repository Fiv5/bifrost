import { editor, languages, Position } from 'monaco-editor';
import type { IRange } from 'monaco-editor';

interface Operator extends Partial<languages.CompletionItem> {
  snippet: string[];
}

const inline_tpl_small = '(${1:key}=${2:value})';
const inline_tpl_long = '(${1:key1}=${2:value1}&${3:key2}=${4:value2})';
const block_tpl = `{\${1:block_var}}
\`\`\`\${1:block_var}
\${2:data}
\`\`\`
`;

const genSnippet = (operator: string) => {
  return [
    operator + '://' + inline_tpl_small,
    operator + '://' + inline_tpl_long,
    operator + '://' + block_tpl,
    operator + '://{${1:block_var}}',
  ];
};
const genSnippetBlock = (operator: string) => {
  return [
    operator + '://' + '(${1:value})',
    operator + '://' + block_tpl,
    operator + '://{${1:block_var}}',
  ];
};
const genSnippetReplace = (operator: string) => {
  return [
    operator + '://{${1:block_var}}',
    operator +
      `://{\${1:block_var}}
\`\`\`\${1:block_var}
\${2:str}: \${3:replacement}
/user=([^&])/ig: user=\\$1\\$1
\`\`\`
`,
  ];
};
const genSnippetInlineAndVar = (operator: string) => {
  return [
    operator + '://(${1:inline_value})',
    operator + '://' + block_tpl,
    operator + '://{${1:block_var}}',
  ];
};
const genSnippetFilenameAndVar = (operator: string) => {
  return [
    operator + '://${1:filename}',
    operator + '://' + block_tpl,
    operator + '://{${1:block_var}}',
  ];
};

const config: Operator[] = [
  {
    label: 'reqHeaders',
    detail: 'reqHeaders://<value>',
    snippet: genSnippet('reqHeaders'),
  },
  {
    label: 'resHeaders',
    detail: 'resHeaders://<value>',
    snippet: genSnippet('resHeaders'),
  },

  {
    label: 'reqCookies',
    detail: 'reqCookies://<value>',
    snippet: genSnippetInlineAndVar('reqCookies'),
  },
  {
    label: 'resCookies',
    detail: 'resCookies://<value>',
    snippet: genSnippetInlineAndVar('resCookies'),
  },

  {
    label: 'reqType',
    detail: 'reqType://<mime-type>',
    snippet: (() => {
      const prefix = 'reqType://';
      return [
        'urlencoded',
        'form',
        'json',
        'xml',
        'text',
        'upload',
        'multipart',
        'defaultType',
      ].map((v) => prefix + v);
    })(),
  },
  {
    label: 'resType',
    detail: 'resType://<mime-type>',
    snippet: (() => {
      const prefix = 'resType://';
      return [
        'urlencoded',
        'form',
        'json',
        'xml',
        'text',
        'upload',
        'multipart',
        'defaultType',
      ].map((v) => prefix + v);
    })(),
  },

  {
    label: 'reqCharset',
    detail: 'reqCharset://<content-type>',
    snippet: ['reqCharset://(utf-8)', 'reqCharset://(${1:value})'],
  },
  {
    label: 'resCharset',
    detail: 'resCharset://<content-type>',
    snippet: ['resCharset://(utf-8)', 'resCharset://(${1:value})'],
  },

  {
    label: 'reqCors',
    detail: 'reqCors://<headers>',
    snippet: [
      'reqCors://*',
      'reqCors://enable',
      `reqCors://{\${1:block_var}}
\`\`\`\${1:block_var}
origin: *
methods: \${2:POST}
headers: \${3:x-customer-header-key}
credentials: true
maxAge: 300000
\`\`\`
`,
    ],
  },
  {
    label: 'resCors',
    detail: 'resCors://<headers>',
    snippet: [
      'resCors://*',
      'resCors://enable',
      `resCors://{\${1:block_var}}
\`\`\`\${1:block_var}
origin: *
methods: \${2:POST}
headers: \${3:x-customer-header-key}
credentials: true
maxAge: 300000
\`\`\`
`,
    ],
  },

  {
    label: 'urlParams',
    detail: 'urlParams://<value>',
    snippet: genSnippet('urlParams'),
  },
  {
    label: 'reqBody',
    detail: 'reqBody://<value>',
    snippet: genSnippetBlock('reqBody'),
  },
  {
    label: 'reqMerge',
    detail: 'reqMerge://<value>',
    snippet: genSnippet('reqMerge'),
  },
  {
    label: 'reqReplace',
    detail: 'reqReplace://<value>',
    snippet: genSnippetReplace('reqReplace'),
  },
  {
    label: 'resBody',
    detail: 'resBody://<value>',
    snippet: genSnippetBlock('resBody'),
  },
  {
    label: 'resMerge',
    detail: 'resMerge://<value>',
    snippet: genSnippet('resMerge'),
  },
  {
    label: 'resReplace',
    detail: 'resReplace://<value>',
    snippet: genSnippetReplace('resReplace'),
  },

  {
    label: 'method',
    detail: 'method://<value>',
    snippet: (() => {
      const prefix = 'method://';
      return ['get', 'post', 'patch', 'put', 'delete', 'options', 'head'].map(
        (v) => prefix + v
      );
    })(),
  },
  {
    label: 'statusCode',
    detail: 'statusCode://<code>',
    snippet: (() => {
      const prefix = 'statusCode://';
      return [
        100, 101, 200, 204, 206, 301, 302, 304, 400, 401, 403, 404, 405, 408,
        409, 500, 502, 503, 504,
      ].map((v) => prefix + v);
    })(),
  },
  {
    label: 'replaceStatus',
    detail: 'replaceStatus://<code>',
    snippet: ['replaceStatus://${1:code}'],
  },

  {
    label: 'ua',
    detail: 'ua://<user-agent>',
    snippet: genSnippetInlineAndVar('ua'),
  },
  {
    label: 'referer',
    detail: 'referer://<uri>',
    snippet: ['referer://(${1:uri})'],
  },
  {
    label: 'auth',
    detail: 'auth://<username>:<password>',
    snippet: ['auth://${1:username}:${2:password}'],
  },

  {
    label: 'cache',
    detail: 'cache://<second>',
    snippet: [
      'cache://${1:second}',
      'cache://60',
      'cache://no',
      'cache://no-cache',
      'cache://no-store',
    ],
  },
  {
    label: 'reqDelay',
    detail: 'reqDelay://<ms>',
    snippet: ['reqDelay://${1:ms}'],
  },
  {
    label: 'resDelay',
    detail: 'resDelay://<ms>',
    snippet: ['resDelay://${1:ms}'],
  },
  {
    label: 'reqSpeed',
    detail: 'reqSpeed://<kbs>',
    snippet: ['reqSpeed://${1:kbs}'],
  },
  {
    label: 'resSpeed',
    detail: 'resSpeed://<kbs>',
    snippet: ['resSpeed://${1:kbs}'],
  },

  {
    label: 'forwardedFor',
    detail: 'forwardedFor://<ip>',
    snippet: ['forwardedFor://${1:ip}'],
  },
  {
    label: 'responseFor',
    detail: 'responseFor://<ip>',
    snippet: ['responseFor://${1:ip}'],
  },
  {
    label: 'redirect',
    detail: 'redirect://<uri>',
    snippet: ['redirect://(${1:uri})'],
  },

  {
    label: 'headerReplace',
    detail: 'headerReplace://<pattern>',
    snippet: [
      'headerReplace://req.${1:header-name}:${2:pattern1}=${3:replacement1}',
      'headerReplace://res.${1:header-name}:${2:pattern1}=${3:replacement1}',
      'headerReplace://(json_string)',
      'headerReplace://' + block_tpl,
    ],
  },
  {
    label: 'pathReplace',
    detail: 'pathReplace://<filepath>',
    snippet: genSnippetReplace('pathReplace'),
  },

  {
    label: 'attachment',
    detail: 'attachment://<filename>',
    snippet: ['attachment://${1:filename}'],
  },
  {
    label: 'reqPrepend',
    detail: 'reqPrepend://<filename>',
    snippet: genSnippetFilenameAndVar('reqPrepend'),
  },
  {
    label: 'resPrepend',
    detail: 'resPrepend://<filename>',
    snippet: genSnippetFilenameAndVar('resPrepend'),
  },
  {
    label: 'reqAppend',
    detail: 'reqAppend://<filename>',
    snippet: genSnippetFilenameAndVar('reqAppend'),
  },
  {
    label: 'resAppend',
    detail: 'resAppend://<filename>',
    snippet: genSnippetFilenameAndVar('resAppend'),
  },

  {
    label: 'htmlPrepend',
    detail: 'htmlPrepend://<filepath>',
    snippet: genSnippetFilenameAndVar('htmlPrepend'),
  },
  {
    label: 'htmlAppend',
    detail: 'htmlAppend://<filepath>',
    snippet: genSnippetFilenameAndVar('htmlAppend'),
  },
  {
    label: 'cssPrepend',
    detail: 'cssPrepend://<filepath>',
    snippet: genSnippetFilenameAndVar('cssPrepend'),
  },
  {
    label: 'cssAppend',
    detail: 'cssAppend://<filepath>',
    snippet: genSnippetFilenameAndVar('cssAppend'),
  },
  {
    label: 'jsPrepend',
    detail: 'jsPrepend://<filepath>',
    snippet: genSnippetFilenameAndVar('jsPrepend'),
  },
  {
    label: 'jsAppend',
    detail: 'jsAppend://<filepath>',
    snippet: genSnippetFilenameAndVar('jsAppend'),
  },

  {
    label: 'htmlBody',
    detail: 'htmlBody://<filepath>',
    snippet: genSnippetFilenameAndVar('htmlBody'),
  },
  {
    label: 'cssBody',
    detail: 'cssBody://<filepath>',
    snippet: genSnippetFilenameAndVar('cssBody'),
  },
  {
    label: 'jsBody',
    detail: 'jsBody://<filepath>',
    snippet: genSnippetFilenameAndVar('jsBody'),
  },

  {
    label: 'reqWrite',
    detail: 'reqWrite://<filepath>',
    snippet: ['reqWrite://${1:filepath}'],
  },
  {
    label: 'resWrite',
    detail: 'resWrite://<filepath>',
    snippet: ['resWrite://${1:filepath}'],
  },
  {
    label: 'reqWriteRaw',
    detail: 'reqWriteRaw://<filepath>',
    snippet: ['reqWriteRaw://${1:filepath}'],
  },
  {
    label: 'resWriteRaw',
    detail: 'resWriteRaw://<filepath>',
    snippet: ['resWriteRaw://${1:filepath}'],
  },
  {
    label: 'file',
    detail: 'file://<path>',
    snippet: ['file://${1:path}'],
  },
  {
    label: 'xfile',
    detail: 'xfile://<path>',
    snippet: ['xfile://${1:path}'],
  },
  {
    label: 'rawfile',
    detail: 'rawfile://<path>',
    snippet: ['rawfile://${1:path}'],
  },
  {
    label: 'xrawfile',
    detail: 'xrawfile://<path>',
    snippet: ['xrawfile://${1:path}'],
  },

  {
    label: 'http',
    detail: 'https?://',
    snippet: ['http://', 'https://'],
  },
  {
    label: 'ws',
    detail: 'wss?://',
    snippet: ['ws://', 'wss://'],
  },
  {
    label: 'tunnel',
    detail: 'tunnel://',
    snippet: ['tunnel://'],
  },
  {
    label: 'dns',
    detail: 'dns://<ip>',
    snippet: ['dns://${1:ip}'],
  },
  {
    label: 'host',
    detail: 'host://<ip>',
    snippet: ['host://${1:ip:port}'],
  },
  {
    label: 'proxy',
    detail: 'proxy://[username]:[password]@<ip>:<port>',
    snippet: [
      'proxy://${1:ip}:${2:port}',
      'proxy://${1:username}:${2:password}@${3:ip}:${4:port}',
    ],
  },
  {
    label: 'http-proxy',
    detail: 'http-proxy://<ip>',
    snippet: ['http-proxy://${1:ip}'],
  },
  {
    label: 'pac',
    detail: 'pac://<uri>',
    snippet: ['pac://(${1:uri})'],
  },

  {
    label: 'includeFilter',
    detail: 'includeFilter://<options>',
    snippet: (() => {
      const prefix = 'includeFilter://';
      return [
        'm:${1:method}',
        's:${1:statusCode}',
        'h:${1:key}=${2:value}',
        'reqH:${1:key}=${2:value}',
        'resH:${1:key}=${2:value}',
        'b:${1:body}',
        'i:${1:ip}',
        'clientIp:${1:ip}',
        'serverIp:${1:ip}',
        '*',
      ].map((v) => prefix + v);
    })(),
  },
  {
    label: 'excludeFilter',
    detail: 'excludeFilter://<options>',
    snippet: (() => {
      const prefix = 'excludeFilter://';
      return [
        'm:${1:method}',
        's:${1:statusCode}',
        'h:${1:key}=${2:value}',
        'reqH:${1:key}=${2:value}',
        'resH:${1:key}=${2:value}',
        'b:${1:body}',
        'i:${1:ip}',
        'clientIp:${1:ip}',
        'serverIp:${1:ip}',
        '*',
      ].map((v) => prefix + v);
    })(),
  },
  {
    label: 'ignore',
    detail: 'ignore://[operators]',
    snippet: [
      'ignore://${1:operator}',
      'ignore://${1:operator1}|${2:operator2}',
      'ignore://*',
      'ignore://-host|-rule',
      'ignore://htmlAppend|htmlPrepend',
    ],
  },
  {
    label: 'enable',
    detail: 'enable://[operators]',
    snippet: [
      'enable://intercept',
      'enable://h2',
      'enable://${1:operator}',
      'enable://${1:operator1}|${2:operator2}',
    ],
  },
  {
    label: 'disable',
    detail: 'disable://[operators]',
    snippet: [
      'disable://intercept',
      'disable://ignore',
      'disable://csp',
      'disable://ua',
      'disable://${1:operator}',
      'disable://${1:operator1}|${2:operator2}',
    ],
  },
  {
    label: 'delete',
    detail: 'delete://<value>',
    snippet: [
      'delete://(${1:value})',
      'delete://${1:req}.headers.${2:key}',
      'delete://${1:req}.body',
      'delete://${1:res}.headers.${2:key}',
      'delete://${1:res}.body',
      'delete://body',
    ],
  },
];

const clean = (token: string) => {
  return token.replace(/\${\d:([\w-]+)}/g, (_, $1) => $1);
};

const create = (range: IRange): languages.CompletionItem[] => {
  const list: languages.CompletionItem[] = [];

  const base = {
    kind: languages.CompletionItemKind.Snippet,
    insertTextRules: languages.CompletionItemInsertTextRule.InsertAsSnippet,
    range,
  };

  config.forEach(({ snippet, label, ...other }: Operator) => {
    const token = !label ? '' : clean(label as string);

    snippet.map((v: string) => {
      list.push({
        ...base,
        ...other,
        label: snippet.length > 1 ? clean(v) : token,
        insertText: v,
      } as languages.CompletionItem);
    });
  });

  return list;
};

const provider: languages.CompletionItemProvider = {
  provideCompletionItems: (model: editor.ITextModel, position: Position) => {
    let suggestions: languages.CompletionItem[] = [];
    if (model.isDisposed()) {
      return { suggestions };
    }
    const text = model.getLineContent(position.lineNumber);
    if (text.trim()[0] !== '@') {
      const word = model.getWordUntilPosition(position);
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      };
      suggestions = create(range);
    }
    return {
      suggestions,
    };
  },
};

export default provider;
