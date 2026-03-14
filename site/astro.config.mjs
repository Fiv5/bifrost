import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

const site = process.env.SITE_URL ?? "https://bifrost-proxy.github.io/bifrost";
const siteUrl = new URL(site);
const base =
  process.env.BASE_PATH ??
  (siteUrl.hostname.endsWith("github.io") ? siteUrl.pathname || "/" : "/");

export default defineConfig({
  site,
  base,
  integrations: [
    starlight({
      title: "Bifrost",
      description: "高性能 HTTP/HTTPS/SOCKS5 代理服务器的官网与文档站",
      disable404Route: true,
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/bifrost-proxy/bifrost",
        },
      ],
      editLink: {
        baseUrl:
          "https://github.com/bifrost-proxy/bifrost/edit/main/site/src/content/docs/",
      },
      customCss: ["./src/styles/starlight.css"],
      sidebar: [
        {
          label: "开始使用",
          items: [
            {
              label: "项目概览",
              slug: "getting-started/overview",
            },
            {
              label: "安装与启动",
              slug: "getting-started/installation",
            },
          ],
        },
        {
          label: "参考文档",
          items: [
            { label: "总览", slug: "reference" },
            { label: "规则引擎", slug: "reference/rule-engine" },
            { label: "运行操作", slug: "reference/operations" },
            { label: "匹配模式", slug: "reference/patterns" },
            { label: "脚本能力", slug: "reference/scripting" },
            { label: "Rules 概览", slug: "reference/rules" },
            {
              label: "请求改写",
              slug: "reference/rules/request-modification",
            },
            {
              label: "响应改写",
              slug: "reference/rules/response-modification",
            },
            {
              label: "URL 改写",
              slug: "reference/rules/url-manipulation",
            },
            { label: "流量控制", slug: "reference/rules/timing-throttle" },
            { label: "状态跳转", slug: "reference/rules/status-redirect" },
            { label: "过滤器", slug: "reference/rules/filters" },
            { label: "WebSocket", slug: "reference/rules/websocket" },
            { label: "脚本规则", slug: "reference/rules/scripts" },
          ],
        },
      ],
    }),
  ],
  vite: {
    server: {
      fs: {
        allow: [".."],
      },
    },
  },
});
