import { isDesktopShell } from '../runtime';

async function nativeClipboardWrite(text: string): Promise<boolean> {
  const invoke = window.__TAURI__?.core?.invoke;
  if (!invoke) return false;
  try {
    await invoke('write_clipboard', { text });
    return true;
  } catch {
    return false;
  }
}

export async function copyToClipboard(text: string): Promise<boolean> {
  if (isDesktopShell()) {
    return nativeClipboardWrite(text);
  }

  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // fallback: execCommand for environments where clipboard API
    // is denied after async operations lose the user-gesture context.
  }

  const textarea = document.createElement('textarea');
  textarea.value = text;
  textarea.style.position = 'fixed';
  textarea.style.left = '-9999px';
  textarea.style.top = '-9999px';
  textarea.style.opacity = '0';
  document.body.appendChild(textarea);
  textarea.select();
  try {
    const ok = document.execCommand('copy');
    return ok;
  } catch {
    return false;
  } finally {
    document.body.removeChild(textarea);
  }
}
