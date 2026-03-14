import { Typography, theme } from 'antd';
import type { CSSProperties } from 'react';
import type {
  RecordContentType,
  SessionTargetSearchState,
  DisplayFormat,
} from '../../../../types';
import { DisplayFormat as DF } from '../../../../types';
import { HighLightBody } from './HighLightBody';
import { HexView } from './HexView';
import { TreeView } from './TreeView';

const { Text } = Typography;

const styles: Record<string, CSSProperties> = {
  mediaContainer: {
    padding: 12,
    backgroundColor: 'transparent',
    borderRadius: 4,
    textAlign: 'center',
    height: '100%',
    overflow: 'auto',
  },
  mediaImage: {
    maxWidth: '100%',
    maxHeight: '100%',
    objectFit: 'contain',
    borderRadius: 4,
  },
};

interface BodyProps {
  data?: string | null;
  contentType: RecordContentType;
  rawContentType?: string | null;
  mediaSrc?: string | null;
  searchValue: SessionTargetSearchState;
  displayFormat: DisplayFormat;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
}

export const Body = ({
  data,
  contentType,
  rawContentType,
  mediaSrc,
  searchValue,
  displayFormat,
  onSearch,
}: BodyProps) => {
  const { token } = theme.useToken();
  const normalizedRawContentType = rawContentType?.toLowerCase() ?? '';
  const isImageMedia = contentType === 'Media' && normalizedRawContentType.includes('image/');

  if (displayFormat === DF.Media) {
    if (contentType === 'Media') {
      if (isImageMedia && mediaSrc) {
        return (
          <div
            style={{
              ...styles.mediaContainer,
              backgroundColor: token.colorBgLayout,
            }}
          >
            <img
              src={mediaSrc}
              alt={rawContentType || 'Response preview'}
              style={styles.mediaImage}
            />
          </div>
        );
      }
      return (
        <div
          style={{
            ...styles.mediaContainer,
            backgroundColor: token.colorBgLayout,
          }}
        >
          <Text type="secondary">Media preview not supported</Text>
        </div>
      );
    }
    return (
      <Text type="secondary" style={{ padding: 12, display: 'block' }}>
        Not a media type
      </Text>
    );
  }

  if (!data) {
    return (
      <Text type="secondary" style={{ padding: 8, display: 'block' }}>
        No body content
      </Text>
    );
  }

  if (displayFormat === DF.Hex) {
    return <HexView data={data} searchValue={searchValue} onSearch={onSearch} />;
  }

  if (displayFormat === DF.Tree) {
    return <TreeView data={data} searchValue={searchValue} onSearch={onSearch} />;
  }

  return (
    <HighLightBody
      data={data}
      contentType={contentType}
      searchValue={searchValue}
      onSearch={onSearch}
    />
  );
};

export { BodyTypeMenu } from './BodyTypeMenu';
