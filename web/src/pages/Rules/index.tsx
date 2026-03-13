import { useEffect, useRef } from 'react';
import { theme } from 'antd';
import SplitPane from '../../components/SplitPane';
import RuleList from './RuleList';
import RuleEditor from './RuleEditor';
import { useRulesStore } from '../../stores/useRulesStore';
import { useValuesStore } from '../../stores/useValuesStore';
import { notifyApiBusinessError } from '../../api/client';

export default function Rules() {
  const { token } = theme.useToken();
  const error = useRulesStore((state) => state.error);
  const clearError = useRulesStore((state) => state.clearError);
  const rules = useRulesStore((state) => state.rules);
  const selectedRuleName = useRulesStore((state) => state.selectedRuleName);
  const selectRule = useRulesStore((state) => state.selectRule);
  const fetchRules = useRulesStore((state) => state.fetchRules);
  const values = useValuesStore((state) => state.values);
  const fetchValues = useValuesStore((state) => state.fetchValues);

  const initRef = useRef(false);
  const loadedRef = useRef(false);

  useEffect(() => {
    if (loadedRef.current) return;
    loadedRef.current = true;

    if (rules.length === 0) {
      void fetchRules();
    }
    if (values.length === 0) {
      void fetchValues();
    }
  }, [fetchRules, fetchValues, rules.length, values.length]);

  useEffect(() => {
    if (initRef.current) return;
    if (rules.length > 0 && !selectedRuleName) {
      initRef.current = true;
      selectRule(rules[0].name);
    }
  }, [rules, selectedRuleName, selectRule]);

  useEffect(() => {
    if (error) {
      notifyApiBusinessError(new Error(error), error);
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
