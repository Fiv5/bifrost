import type { RecordContentType } from '../../../types';

export const getContentTypeFromHeader = (
  contentType: string | null | undefined
): RecordContentType => {
  if (!contentType) return 'Other';

  const ct = contentType.toLowerCase();

  if (ct.includes('json')) return 'JSON';
  if (ct.includes('html')) return 'HTML';
  if (ct.includes('xml')) return 'XML';
  if (ct.includes('javascript') || ct.includes('ecmascript')) return 'JavaScript';
  if (ct.includes('css')) return 'CSS';
  if (
    ct.includes('image') ||
    ct.includes('audio') ||
    ct.includes('video') ||
    ct.includes('font')
  )
    return 'Media';

  return 'Other';
};

export const getHighlightLanguage = (contentType: RecordContentType): string => {
  switch (contentType) {
    case 'JSON':
      return 'json';
    case 'HTML':
      return 'html';
    case 'XML':
      return 'xml';
    case 'JavaScript':
      return 'javascript';
    case 'CSS':
      return 'css';
    default:
      return 'plaintext';
  }
};

export const DEFAULT_SHOW_MAX_SIZE = 600 * 1024;
