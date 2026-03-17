import { message } from "antd";

export const TLS_RECONNECT_NOTICE =
  "Restart the target app and reopen the target domain to establish a new connection.";

export function showTlsWhitelistChangeSuccess(content: string) {
  message.success({
    content: `${content}. ${TLS_RECONNECT_NOTICE}`,
    duration: 5,
  });
}
