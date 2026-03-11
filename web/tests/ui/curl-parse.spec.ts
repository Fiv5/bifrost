import { test, expect } from "@playwright/test";
import { parseCurl } from "../../src/utils/curl";

test("解析 Chrome cURL 的 -b/--cookie 为 Cookie Header", async () => {
  const parsed = parseCurl(
    "curl 'https://example.com/api' -H 'accept: */*' -b 'a=b; c=d'",
  );
  expect(parsed).not.toBeNull();
  const cookie = parsed!.headers.find((h) => h.key.toLowerCase() === "cookie");
  expect(cookie?.value).toBe("a=b; c=d");
});

test("解析 Chrome cURL 的 $'...' ANSI-C quoting，不保留前缀并正确处理 $ 字符", async () => {
  const parsed = parseCurl(
    "curl 'https://example.com/api' -H 'content-type: application/json' --data-raw $'{\"v\":\"$\"}'",
  );
  expect(parsed).not.toBeNull();
  expect(parsed!.body?.type).toBe("raw");
  expect(parsed!.body?.content).toBe('{"v":"$"}');
});

test("解析 $'...' 内常见转义（\\n、\\t、\\u、\\x）", async () => {
  const parsed = parseCurl(
    "curl 'https://example.com/api' --data-raw $'a\\nb\\tc\\u003d\\x2b'",
  );
  expect(parsed).not.toBeNull();
  expect(parsed!.body?.content).toBe("a\nb\tc=+");
});

test('双引号内非特殊转义保留反斜杠（bash 兼容）', async () => {
  const parsed = parseCurl('curl "https://example.com/api" --data-raw "a\\c"');
  expect(parsed).not.toBeNull();
  expect(parsed!.body?.content).toBe("a\\c");
});

test("解析 -u/--user 为 Basic Authorization Header", async () => {
  const parsed = parseCurl("curl https://example.com/api -u user:pass");
  expect(parsed).not.toBeNull();
  const auth = parsed!.headers.find((h) => h.key.toLowerCase() === "authorization");
  expect(auth?.value).toBe("Basic dXNlcjpwYXNz");
});

test("解析 -G/--get 将 data 追加到 URL query", async () => {
  const parsed = parseCurl("curl https://example.com/api -G --data-urlencode a=b");
  expect(parsed).not.toBeNull();
  expect(parsed!.url).toBe("https://example.com/api?a=b");
  expect(parsed!.body).toBeUndefined();
});

test("Header value 不允许 CRLF 注入（自动折叠为单行）", async () => {
  const parsed = parseCurl("curl https://example.com -H $'X-Test: a\\nb'");
  expect(parsed).not.toBeNull();
  const h = parsed!.headers.find((x) => x.key === "X-Test");
  expect(h?.value).toBe("a b");
});

test("忽略非法 header name（不符合 RFC7230 token）", async () => {
  const parsed = parseCurl("curl https://example.com -H 'Bad Header: v'");
  expect(parsed).not.toBeNull();
  expect(parsed!.headers.some((h) => h.key === "Bad Header")).toBe(false);
});
