import { Typography, theme } from 'antd';
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

interface BodyProps {
  data?: string | null;
  contentType: RecordContentType;
  searchValue: SessionTargetSearchState;
  displayFormat: DisplayFormat;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
}

export const Body = ({
  data,
  contentType,
  searchValue,
  displayFormat,
  onSearch,
}: BodyProps) => {
  const { token } = theme.useToken();

  if (!data) {
    return (
      <Text type="secondary" style={{ padding: 8, display: 'block' }}>
        No body content
      </Text>
    );
  }

  if (displayFormat === DF.Media) {
    if (contentType === 'Media') {
      return (
        <div
          style={{
            padding: 12,
            backgroundColor: token.colorBgLayout,
            borderRadius: 4,
            textAlign: 'center',
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
