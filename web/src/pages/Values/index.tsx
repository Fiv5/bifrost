import { useEffect, useRef } from 'react';
import { useSearchParams } from 'react-router-dom';
import { message, theme } from 'antd';
import SplitPane from '../../components/SplitPane';
import ValueList from './ValueList';
import ValueEditor from './ValueEditor';
import { useValuesStore } from '../../stores/useValuesStore';

export default function Values() {
  const { token } = theme.useToken();
  const [searchParams, setSearchParams] = useSearchParams();
  const error = useValuesStore((state) => state.error);
  const clearError = useValuesStore((state) => state.clearError);
  const values = useValuesStore((state) => state.values);
  const selectedValueName = useValuesStore((state) => state.selectedValueName);
  const selectValue = useValuesStore((state) => state.selectValue);

  const initRef = useRef(false);
  const urlParamRef = useRef(false);

  useEffect(() => {
    const nameParam = searchParams.get('name');
    if (nameParam && values.length > 0 && !urlParamRef.current) {
      urlParamRef.current = true;
      const exists = values.some(v => v.name === nameParam);
      if (exists) {
        selectValue(nameParam);
        setSearchParams({}, { replace: true });
      }
    }
  }, [searchParams, values, selectValue, setSearchParams]);

  useEffect(() => {
    if (initRef.current || urlParamRef.current) return;
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
