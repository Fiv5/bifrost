import { useEffect, useRef } from 'react';
import { message, theme } from 'antd';
import SplitPane from '../../components/SplitPane';
import ValueList from './ValueList';
import ValueEditor from './ValueEditor';
import { useValuesStore } from '../../stores/useValuesStore';

export default function Values() {
  const { token } = theme.useToken();
  const error = useValuesStore((state) => state.error);
  const clearError = useValuesStore((state) => state.clearError);
  const values = useValuesStore((state) => state.values);
  const selectedValueName = useValuesStore((state) => state.selectedValueName);
  const selectValue = useValuesStore((state) => state.selectValue);

  const initRef = useRef(false);

  useEffect(() => {
    if (initRef.current) return;
    if (values.length > 0 && !selectedValueName) {
      initRef.current = true;
      selectValue(values[0].name);
    }
  }, [values, selectedValueName, selectValue]);

  useEffect(() => {
    if (error) {
      message.error(error);
      clearError();
    }
  }, [error, clearError]);

  const containerStyle = {
    width: '100%',
    height: '100%',
    overflow: 'hidden',
    backgroundColor: token.colorBgContainer,
  };

  return (
    <div style={containerStyle}>
      <SplitPane
        left={<ValueList />}
        right={<ValueEditor />}
        defaultLeftWidth="250px"
        minLeftWidth={200}
        minRightWidth={400}
      />
    </div>
  );
}
