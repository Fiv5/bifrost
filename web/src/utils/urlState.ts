const base64UrlEncode = (bytes: Uint8Array): string => {
  let binary = "";
  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i]!);
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
};

const base64UrlDecode = (input: string): Uint8Array => {
  const base64 = input.replace(/-/g, "+").replace(/_/g, "/");
  const padded = base64 + "=".repeat((4 - (base64.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
};

export const encodeJsonForQueryParam = (value: unknown): string => {
  const json = JSON.stringify(value);
  const bytes = new TextEncoder().encode(json);
  return base64UrlEncode(bytes);
};

const decodeJsonUtf8Base64Url = <T>(input: string): T => {
  const bytes = base64UrlDecode(input);
  const json = new TextDecoder().decode(bytes);
  return JSON.parse(json) as T;
};

const decodeJsonLegacyBase64 = <T>(input: string): T => {
  return JSON.parse(atob(input)) as T;
};

export const decodeJsonFromQueryParam = <T>(input: string): T | null => {
  if (!input) return null;
  try {
    return decodeJsonUtf8Base64Url<T>(input);
  } catch {
    try {
      return decodeJsonLegacyBase64<T>(input);
    } catch {
      return null;
    }
  }
};

