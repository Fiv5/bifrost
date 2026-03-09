let clientId: string | null = null;
const SESSION_KEY = 'bifrost_x_client_id';

export function getClientId(): string {
  if (clientId) return clientId;
  try {
    const stored = sessionStorage.getItem(SESSION_KEY);
    if (stored) {
      clientId = stored;
      return stored;
    }
  } catch {
    void 0;
  }
  const cryptoObj = globalThis.crypto as Crypto | undefined;
  const uuid = cryptoObj?.randomUUID?.();
  clientId = uuid ?? `cid_${Date.now()}_${Math.random().toString(16).slice(2)}`;
  try {
    sessionStorage.setItem(SESSION_KEY, clientId);
  } catch {
    void 0;
  }
  return clientId;
}
