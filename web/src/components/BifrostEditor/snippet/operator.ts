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
    label: 'G',
    detail: 'G://<group_name> - Rule group',
    snippet: ['G://${1:group_name}'],
  },
  {
    label: 'style',
    detail: 'style://<style_options>',
    snippet: [
      'style://color=${1:red}',
      'style://color=${1:red}&fontStyle=bold',
      'style://(color=${1:red})',
    ],
  },
  {
    label: 'rule',
    detail: 'rule://<rule_name>',
    snippet: ['rule://${1:rule_name}'],
  },
  {
    label: 'pipe',
    detail: 'pipe://<target>',
    snippet: ['pipe://${1:target}'],
  },

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
    label: 'params',
    detail: 'params://<value> - Alias for reqMerge',
    snippet: genSnippet('params'),
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
    label: 'urlReplace',
    detail: 'urlReplace://<pattern>',
    snippet: genSnippetReplace('urlReplace'),
  },
  {
    label: 'pathReplace',
    detail: 'pathReplace://<pattern> - Alias for urlReplace',
    snippet: genSnippetReplace('pathReplace'),
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
    label: 'status',
    detail: 'status://<code> - Alias for statusCode',
    snippet: (() => {
      const prefix = 'status://';
      return [200, 301, 302, 400, 401, 403, 404, 500, 502, 503].map(
        (v) => prefix + v
      );
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
    label: 'locationHref',
    detail: 'locationHref://<uri>',
    snippet: ['locationHref://(${1:uri})'],
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
    label: 'attachment',
    detail: 'attachment://<filename>',
    snippet: ['attachment://${1:filename}'],
  },
  {
    label: 'download',
    detail: 'download://<filename> - Alias for attachment',
    snippet: ['download://${1:filename}'],
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
    label: 'htmlBody',
    detail: 'htmlBody://<filepath>',
    snippet: genSnippetFilenameAndVar('htmlBody'),
  },
  {
    label: 'html',
    detail: 'html://<filepath> - Alias for htmlAppend',
    snippet: genSnippetFilenameAndVar('html'),
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
    label: 'cssBody',
    detail: 'cssBody://<filepath>',
    snippet: genSnippetFilenameAndVar('cssBody'),
  },
  {
    label: 'css',
    detail: 'css://<filepath> - Alias for cssAppend',
    snippet: genSnippetFilenameAndVar('css'),
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
    label: 'jsBody',
    detail: 'jsBody://<filepath>',
    snippet: genSnippetFilenameAndVar('jsBody'),
  },
  {
    label: 'js',
    detail: 'js://<filepath> - Alias for jsAppend',
    snippet: genSnippetFilenameAndVar('js'),
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
    snippet: ['file://${1:path}', 'file://{${1:block_var}}'],
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
    label: 'tpl',
    detail: 'tpl://<path> - Template file',
    snippet: ['tpl://${1:path}', 'tpl://{${1:block_var}}'],
  },

  {
    label: 'http',
    detail: 'http://<target>',
    snippet: ['http://${1:ip}:${2:port}', 'http://${1:hostname}'],
  },
  {
    label: 'https',
    detail: 'https://<target>',
    snippet: ['https://${1:ip}:${2:port}', 'https://${1:hostname}'],
  },
  {
    label: 'ws',
    detail: 'ws://<target>',
    snippet: ['ws://${1:ip}:${2:port}', 'ws://${1:hostname}'],
  },
  {
    label: 'wss',
    detail: 'wss://<target>',
    snippet: ['wss://${1:ip}:${2:port}', 'wss://${1:hostname}'],
  },

  {
    label: 'dns',
    detail: 'dns://<ip>',
    snippet: ['dns://${1:ip}'],
  },
  {
    label: 'host',
    detail: 'host://<ip:port>',
    snippet: ['host://${1:ip}:${2:port}', 'host://${1:ip}'],
  },
  {
    label: 'hosts',
    detail: 'hosts://<ip:port> - Alias for host',
    snippet: ['hosts://${1:ip}:${2:port}', 'hosts://${1:ip}'],
  },
  {
    label: 'xhost',
    detail: 'xhost://<ip:port>',
    snippet: ['xhost://${1:ip}:${2:port}'],
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
    detail: 'http-proxy://<ip>:<port> - Alias for proxy',
    snippet: ['http-proxy://${1:ip}:${2:port}'],
  },
  {
    label: 'https2http-proxy',
    detail: 'https2http-proxy://<ip>:<port>',
    snippet: ['https2http-proxy://${1:ip}:${2:port}'],
  },
  {
    label: 'http2https-proxy',
    detail: 'http2https-proxy://<ip>:<port>',
    snippet: ['http2https-proxy://${1:ip}:${2:port}'],
  },
  {
    label: 'internal-proxy',
    detail: 'internal-proxy://<ip>:<port>',
    snippet: ['internal-proxy://${1:ip}:${2:port}'],
  },
  {
    label: 'pac',
    detail: 'pac://<uri>',
    snippet: ['pac://(${1:uri})', 'pac://${1:filepath}'],
  },

  {
    label: 'trailers',
    detail: 'trailers://<value>',
    snippet: genSnippet('trailers'),
  },

  {
    label: 'tlsIntercept',
    detail: 'tlsIntercept:// - Enable TLS interception',
    snippet: ['tlsIntercept://'],
  },
  {
    label: 'tlsPassthrough',
    detail: 'tlsPassthrough:// - Disable TLS interception',
    snippet: ['tlsPassthrough://'],
  },
  {
    label: 'cipher',
    detail: 'cipher://<options> - TLS cipher options',
    snippet: [
      'cipher://${1:options}',
      `cipher://{\${1:block_var}}
\`\`\`\${1:block_var}
minVersion: TLSv1.2
maxVersion: TLSv1.3
\`\`\`
`,
    ],
  },
  {
    label: 'tlsOptions',
    detail: 'tlsOptions://<options> - Alias for cipher',
    snippet: ['tlsOptions://${1:options}'],
  },
  {
    label: 'sniCallback',
    detail: 'sniCallback://<value>',
    snippet: ['sniCallback://${1:value}'],
  },

  {
    label: 'rulesFile',
    detail: 'rulesFile://<filepath>',
    snippet: ['rulesFile://${1:filepath}'],
  },
  {
    label: 'ruleFile',
    detail: 'ruleFile://<filepath> - Alias for rulesFile',
    snippet: ['ruleFile://${1:filepath}'],
  },
  {
    label: 'reqScript',
    detail: 'reqScript://<filepath> - Alias for rulesFile',
    snippet: ['reqScript://${1:filepath}'],
  },
  {
    label: 'reqRules',
    detail: 'reqRules://<filepath> - Alias for rulesFile',
    snippet: ['reqRules://${1:filepath}'],
  },
  {
    label: 'resScript',
    detail: 'resScript://<filepath>',
    snippet: ['resScript://${1:filepath}'],
  },
  {
    label: 'resRules',
    detail: 'resRules://<filepath> - Alias for resScript',
    snippet: ['resRules://${1:filepath}'],
  },
  {
    label: 'frameScript',
    detail: 'frameScript://<filepath>',
    snippet: ['frameScript://${1:filepath}'],
  },

  {
    label: 'log',
    detail: 'log://<message>',
    snippet: ['log://${1:message}', 'log://(${1:message})'],
  },
  {
    label: 'weinre',
    detail: 'weinre://<id>',
    snippet: ['weinre://${1:id}'],
  },
  {
    label: 'plugin',
    detail: 'plugin://<name>',
    snippet: ['plugin://${1:name}'],
  },

  {
    label: 'includeFilter',
    detail: 'includeFilter://<options>',
    snippet: (() => {
      const prefix = 'includeFilter://';
      return [
        'm:${1:GET}',
        'm:GET,POST',
        's:${1:200}',
        's:200-299',
        's:200,201,204',
        's:4xx',
        's:5xx',
        'h:${1:header-name}',
        'reqH:${1:header-name}=${2:value}',
        'reqH:content-type=/json/',
        'resH:${1:header-name}=${2:value}',
        'resH:content-type=/html/',
        'b:${1:body-pattern}',
        'b:/error/',
        'i:${1:client-ip}',
        'i:192.168.0.0/16',
        '/${1:path-pattern}/',
        '/api/',
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
        'm:${1:GET}',
        'm:OPTIONS',
        's:${1:200}',
        's:200-299',
        'h:${1:header-name}',
        'reqH:${1:header-name}=${2:value}',
        'resH:${1:header-name}=${2:value}',
        'b:${1:body-pattern}',
        'i:${1:client-ip}',
        '/${1:path-pattern}/',
        '/admin/',
        '/health',
        '*',
      ].map((v) => prefix + v);
    })(),
  },
  {
    label: 'filter',
    detail: 'filter://<options>',
    snippet: ['filter://${1:options}'],
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
    label: 'skip',
    detail: 'skip://[operators] - Alias for ignore',
    snippet: ['skip://${1:operator}', 'skip://*'],
  },
  {
    label: 'enable',
    detail: 'enable://[operators]',
    snippet: [
      'enable://intercept',
      'enable://h2',
      'enable://capture',
      'enable://abort',
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
      'disable://https',
      'disable://abort',
      'disable://${1:operator}',
      'disable://${1:operator1}|${2:operator2}',
    ],
  },
  {
    label: 'delete',
    detail: 'delete://<value>',
    snippet: [
      'delete://(${1:value})',
      'delete://req.headers.${1:key}',
      'delete://req.body',
      'delete://res.headers.${1:key}',
      'delete://res.body',
      'delete://body',
    ],
  },

  {
    label: 'lineProps',
    detail: 'lineProps://<options>',
    snippet: [
      'lineProps://important',
      'lineProps://disabled',
      'lineProps://important,disabled',
    ],
  },
];

const templateVariables: Operator[] = [
  {
    label: '${now}',
    detail: 'Current timestamp in milliseconds',
    snippet: ['${now}'],
  },
  {
    label: '${random}',
    detail: 'Random float number',
    snippet: ['${random}'],
  },
  {
    label: '${randomInt(N)}',
    detail: 'Random integer from 0 to N',
    snippet: ['${randomInt(${1:100})}', '${randomInt(${1:1}-${2:100})}'],
  },
  {
    label: '${randomUUID}',
    detail: 'Random UUID v4',
    snippet: ['${randomUUID}'],
  },
  {
    label: '${version}',
    detail: 'Bifrost version',
    snippet: ['${version}'],
  },
  {
    label: '${id}',
    detail: 'Request ID',
    snippet: ['${id}', '${reqId}'],
  },
  {
    label: '${url}',
    detail: 'Full request URL',
    snippet: ['${url}'],
  },
  {
    label: '${host}',
    detail: 'Request host with port',
    snippet: ['${host}'],
  },
  {
    label: '${hostname}',
    detail: 'Request hostname without port',
    snippet: ['${hostname}'],
  },
  {
    label: '${port}',
    detail: 'Request port',
    snippet: ['${port}'],
  },
  {
    label: '${path}',
    detail: 'Request path with query string',
    snippet: ['${path}'],
  },
  {
    label: '${pathname}',
    detail: 'Request path without query string',
    snippet: ['${pathname}'],
  },
  {
    label: '${query}',
    detail: 'Query string or specific query param',
    snippet: ['${query}', '${query.${1:param}}'],
  },
  {
    label: '${search}',
    detail: 'Query string with ? prefix',
    snippet: ['${search}'],
  },
  {
    label: '${method}',
    detail: 'HTTP method',
    snippet: ['${method}'],
  },
  {
    label: '${statusCode}',
    detail: 'Response status code',
    snippet: ['${statusCode}', '${status}'],
  },
  {
    label: '${clientIp}',
    detail: 'Client IP address',
    snippet: ['${clientIp}', '${ip}'],
  },
  {
    label: '${clientPort}',
    detail: 'Client port',
    snippet: ['${clientPort}'],
  },
  {
    label: '${serverIp}',
    detail: 'Server IP address',
    snippet: ['${serverIp}'],
  },
  {
    label: '${serverPort}',
    detail: 'Server port',
    snippet: ['${serverPort}'],
  },
  {
    label: '${realUrl}',
    detail: 'Real target URL after rewrite',
    snippet: ['${realUrl}'],
  },
  {
    label: '${realHost}',
    detail: 'Real target host after rewrite',
    snippet: ['${realHost}'],
  },
  {
    label: '${realPort}',
    detail: 'Real target port after rewrite',
    snippet: ['${realPort}'],
  },
  {
    label: '${reqHeaders}',
    detail: 'Request headers',
    snippet: ['${reqHeaders.${1:header-name}}', '${reqH.${1:header-name}}'],
  },
  {
    label: '${resHeaders}',
    detail: 'Response headers',
    snippet: ['${resHeaders.${1:header-name}}', '${resH.${1:header-name}}'],
  },
  {
    label: '${reqCookies}',
    detail: 'Request cookies',
    snippet: ['${reqCookies.${1:cookie-name}}'],
  },
  {
    label: '${resCookies}',
    detail: 'Response cookies',
    snippet: ['${resCookies.${1:cookie-name}}'],
  },
  {
    label: '${env}',
    detail: 'Environment variable',
    snippet: ['${env.${1:VAR_NAME}}'],
  },
  {
    label: '${{url-encode}}',
    detail: 'URL encode the value',
    snippet: ['${{${1:hostname}}}'],
  },
  {
    label: '$${escape}',
    detail: 'Escape $ to prevent variable expansion',
    snippet: ['$${${1:varname}}'],
  },
  {
    label: '${.replace()}',
    detail: 'Replace pattern in variable value',
    snippet: [
      '${hostname.replace(${1:pattern},${2:replacement})}',
      '${path.replace(/api/,api/v2)}',
      '${hostname.replace(/\\./g,-)}',
    ],
  },
  {
    label: '$1 $2',
    detail: 'Regex capture groups',
    snippet: ['$1', '$2', '$3'],
  },
  {
    label: '${clientId}',
    detail: 'Client identifier',
    snippet: ['${clientId}', '${localClientId}'],
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

const createTemplateVars = (range: IRange): languages.CompletionItem[] => {
  const list: languages.CompletionItem[] = [];

  const base = {
    kind: languages.CompletionItemKind.Variable,
    insertTextRules: languages.CompletionItemInsertTextRule.InsertAsSnippet,
    range,
  };

  templateVariables.forEach(({ snippet, label, ...other }: Operator) => {
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
  triggerCharacters: ['$', '{', ':'],
  provideCompletionItems: (model: editor.ITextModel, position: Position) => {
    let suggestions: languages.CompletionItem[] = [];
    if (model.isDisposed()) {
      return { suggestions };
    }
    const text = model.getLineContent(position.lineNumber);
    const textBeforeCursor = text.substring(0, position.column - 1);

    if (text.trim()[0] !== '@') {
      const word = model.getWordUntilPosition(position);
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      };

      if (
        textBeforeCursor.endsWith('$') ||
        textBeforeCursor.endsWith('${') ||
        textBeforeCursor.match(/\$\{[\w.]*$/)
      ) {
        const varRange = {
          ...range,
          startColumn: Math.max(
            1,
            position.column - (textBeforeCursor.match(/\$\{?[\w.]*$/)?.[0]?.length || 0)
          ),
        };
        suggestions = createTemplateVars(varRange);
      } else {
        suggestions = [...create(range), ...createTemplateVars(range)];
      }
    }
    return {
      suggestions,
    };
  },
};

export default provider;
