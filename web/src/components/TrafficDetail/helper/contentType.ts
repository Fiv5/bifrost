import type { RecordContentType } from '../../../types';

export const getContentTypeFromHeader = (
  contentType: string | null | undefined
): RecordContentType => {
  if (!contentType) return 'Other';

  const ct = contentType.toLowerCase();

  if (
    ct.includes('image') ||
    ct.includes('audio') ||
    ct.includes('video') ||
    ct.includes('font')
  )
    return 'Media';
  if (ct.includes('json')) return 'JSON';
  if (ct.includes('html')) return 'HTML';
  if (ct.includes('xml')) return 'XML';
  if (ct.includes('javascript') || ct.includes('ecmascript')) return 'JavaScript';
  if (ct.includes('css')) return 'CSS';

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

export const isImageContentType = (
  contentType: string | null | undefined
): boolean => {
  if (!contentType) return false;
  return contentType.toLowerCase().includes('image/');
};

export const DEFAULT_SHOW_MAX_SIZE = 600 * 1024;
export const MAX_JSON_FORMAT_HIGHLIGHT_LENGTH = 200 * 1024;

export const shouldDisableJsonStructuredView = (
  contentType: RecordContentType,
  data: string | null | undefined
): boolean => {
  if (contentType !== 'JSON' || !data) {
    return false;
  }

  return data.length > MAX_JSON_FORMAT_HIGHLIGHT_LENGTH;
};
