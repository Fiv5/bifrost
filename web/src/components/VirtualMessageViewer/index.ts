export type {
  MessageItem,
  MessageDirection,
  MessageType,
  MessageSearchOptions,
  MessageSearchState,
  MessageViewerState,
} from './types';

export {
  parseSseTextToEvents,
  normalizeSSEEvent,
  normalizeWSMessage,
  normalizeWSFrame,
} from './types';

export { VirtualMessageList } from './VirtualMessageList';
export type { VirtualMessageListProps } from './VirtualMessageList';
export { useVirtualMessageList } from './useVirtualMessageList';
export type { UseVirtualMessageListOptions, UseVirtualMessageListReturn } from './useVirtualMessageList';

export { useMessageSearch, highlightText } from './useMessageSearch';
export type { UseMessageSearchOptions, UseMessageSearchReturn } from './useMessageSearch';

export { MessageItemCard } from './MessageItemCard';
export type { MessageItemCardProps } from './MessageItemCard';

export { FullscreenMessageViewer } from './FullscreenMessageViewer';
export type { FullscreenMessageViewerProps } from './FullscreenMessageViewer';
