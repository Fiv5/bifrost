import { useEffect, useRef } from 'react';
import { message, theme } from 'antd';
import SplitPane from '../../components/SplitPane';
import RuleList from './RuleList';
import RuleEditor from './RuleEditor';
import { useRulesStore } from '../../stores/useRulesStore';

export default function Rules() {
  const { token } = theme.useToken();
  const error = useRulesStore((state) => state.error);
  const clearError = useRulesStore((state) => state.clearError);
  const rules = useRulesStore((state) => state.rules);
  const selectedRuleName = useRulesStore((state) => state.selectedRuleName);
  const selectRule = useRulesStore((state) => state.selectRule);

  const initRef = useRef(false);

  useEffect(() => {
    if (initRef.current) return;
    if (rules.length > 0 && !selectedRuleName) {
      initRef.current = true;
      selectRule(rules[0].name);
    }
  }, [rules, selectedRuleName, selectRule]);

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
        left={<RuleList />}
        right={<RuleEditor />}
        defaultLeftWidth="250px"
        minLeftWidth={200}
        minRightWidth={400}
      />
    </div>
  );
}
