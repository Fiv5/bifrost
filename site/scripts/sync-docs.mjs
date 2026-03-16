import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "../..");
const docsRoot = path.join(repoRoot, "docs");
const outputRoot = path.join(
  repoRoot,
  "site",
  "src",
  "content",
  "docs",
);

const pages = [
  {
    source: "docs/overview.md",
    target: "getting-started/overview.mdx",
    title: "项目概览",
    description: "用几分钟理解 Bifrost 的定位、能力边界和适合的使用场景。",
  },
  {
    source: "docs/getting-started.md",
    target: "getting-started/installation.mdx",
    title: "安装与启动",
    description: "按照 docs/getting-started.md 的最新方式安装和启动 Bifrost。",
  },
  {
    source: "docs/rule.md",
    target: "reference/rule-engine.md",
    title: "规则引擎",
    description: "Bifrost 规则系统的能力全景与配置入口。",
  },
  {
    source: "docs/operation.md",
    target: "reference/operations.md",
    title: "运行操作",
    description: "启动、使用与运行 Bifrost 的常见操作说明。",
  },
  {
    source: "docs/pattern.md",
    target: "reference/patterns.md",
    title: "匹配模式",
    description: "域名、路径、通配符与正则等匹配模式说明。",
  },
  {
    source: "docs/scripts.md",
    target: "reference/scripting.md",
    title: "脚本能力",
    description: "QuickJS 脚本能力、限制与使用方式。",
  },
  {
    source: "docs/rules/README.md",
    target: "reference/rules/index.md",
    title: "Rules 概览",
    description: "Bifrost Rules 语法与能力目录。",
  },
  {
    source: "docs/rules/body-manipulation.md",
    target: "reference/rules/body-manipulation.md",
    title: "Body 改写",
    description: "请求体与响应体改写能力说明。",
  },
  {
    source: "docs/rules/filters.md",
    target: "reference/rules/filters.md",
    title: "过滤器",
    description: "请求与响应过滤条件的配置方式。",
  },
  {
    source: "docs/rules/patterns.md",
    target: "reference/rules/patterns.md",
    title: "规则模式",
    description: "Rules 内的匹配模式与优先级细节。",
  },
  {
    source: "docs/rules/request-modification.md",
    target: "reference/rules/request-modification.md",
    title: "请求改写",
    description: "请求头、请求体与相关字段的改写说明。",
  },
  {
    source: "docs/rules/response-modification.md",
    target: "reference/rules/response-modification.md",
    title: "响应改写",
    description: "响应头、响应体与相关字段的改写说明。",
  },
  {
    source: "docs/rules/routing.md",
    target: "reference/rules/routing.md",
    title: "路由控制",
    description: "规则路由、转发与代理控制说明。",
  },
  {
    source: "docs/rules/rule-priority.md",
    target: "reference/rules/rule-priority.md",
    title: "规则优先级",
    description: "规则冲突与优先级处理方式。",
  },
  {
    source: "docs/rules/scripts.md",
    target: "reference/rules/scripts.md",
    title: "脚本规则",
    description: "在 Rules 中接入脚本能力的方式。",
  },
  {
    source: "docs/rules/status-redirect.md",
    target: "reference/rules/status-redirect.md",
    title: "状态跳转",
    description: "状态码控制、重定向和响应分支能力。",
  },
  {
    source: "docs/rules/timing-throttle.md",
    target: "reference/rules/timing-throttle.md",
    title: "流量控制",
    description: "延迟、限速与相关节流能力说明。",
  },
  {
    source: "docs/rules/url-manipulation.md",
    target: "reference/rules/url-manipulation.md",
    title: "URL 改写",
    description: "URL、路径与查询参数的改写能力。",
  },
  {
    source: "docs/rules/websocket.md",
    target: "reference/rules/websocket.md",
    title: "WebSocket",
    description: "WebSocket 相关规则与使用说明。",
  },
];

const sourceToTarget = new Map(
  pages.map((page) => [normalizePath(page.source), normalizePath(page.target)]),
);

function frontmatter({ title, description, source }) {
  return `---\ntitle: ${title}\ndescription: ${description}\neditUrl: false\n---\n\n> 此页面由 \`${source}\` 自动同步生成。\n\n`;
}

async function ensureDir(dir) {
  await fs.mkdir(dir, { recursive: true });
}

function normalizePath(value) {
  return value.replaceAll(path.sep, "/");
}

function toRoutePath(target) {
  const normalized = normalizePath(target);
  if (normalized.endsWith("/index.md") || normalized.endsWith("/index.mdx")) {
    const dir = normalized.replace(/\/index\.mdx?$/, "");
    return dir.length === 0 ? "./" : `${dir}/`;
  }
  return `${normalized.replace(/\.mdx?$/, "")}/`;
}

function rewriteMarkdownLinks(markdown, page) {
  return markdown.replace(/\]\(([^)#?\s]+\.md)(#[^)]+)?\)/g, (match, rawPath, hash = "") => {
    if (
      rawPath.startsWith("http://") ||
      rawPath.startsWith("https://") ||
      rawPath.startsWith("/")
    ) {
      return match;
    }

    const sourceLink = normalizePath(
      path.join(path.dirname(page.source), rawPath),
    );
    const mappedTarget = sourceToTarget.get(sourceLink);
    if (!mappedTarget) {
      return match;
    }

    const relativeRoute = normalizePath(
      path.relative(path.dirname(page.target), toRoutePath(mappedTarget)),
    );
    const routePath = relativeRoute.startsWith(".")
      ? relativeRoute
      : `./${relativeRoute}`;
    return `](${routePath}${hash})`;
  });
}

async function syncOne(page) {
  const sourcePath = path.join(repoRoot, page.source);
  const targetPath = path.join(outputRoot, page.target);
  const markdown = rewriteMarkdownLinks(
    await fs.readFile(sourcePath, "utf8"),
    page,
  );

  await ensureDir(path.dirname(targetPath));
  await fs.writeFile(
    targetPath,
    `${frontmatter(page)}${markdown.trim()}\n`,
    "utf8",
  );
}

await ensureDir(outputRoot);
await Promise.all(pages.map(syncOne));

const generatedAt = path.join(outputRoot, ".generated-at");
await fs.writeFile(
  generatedAt,
  `Synced from ${path.relative(repoRoot, docsRoot)} at ${new Date().toISOString()}\n`,
  "utf8",
);
