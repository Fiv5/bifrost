import { useEffect, useRef, useCallback } from 'react';
import { theme } from 'antd';
import SplitPane from '../../components/SplitPane';
import RuleList from './RuleList';
import RuleEditor from './RuleEditor';
import RulesDynamicIsland from './RulesDynamicIsland';
import { useRulesStore } from '../../stores/useRulesStore';
import { useValuesStore } from '../../stores/useValuesStore';
import { notifyApiBusinessError } from '../../api/client';
import pushService from '../../services/pushService';

export default function Rules() {
  const { token } = theme.useToken();
  const error = useRulesStore((state) => state.error);
  const clearError = useRulesStore((state) => state.clearError);
  const rules = useRulesStore((state) => state.rules);
  const selectedRuleName = useRulesStore((state) => state.selectedRuleName);
  const selectRule = useRulesStore((state) => state.selectRule);
  const fetchRules = useRulesStore((state) => state.fetchRules);
  const setActiveGroupId = useRulesStore((state) => state.setActiveGroupId);
  const activeGroupId = useRulesStore((state) => state.activeGroupId);
  const applyValuesSnapshot = useValuesStore((state) => state.applyValuesSnapshot);

  const initRef = useRef(false);
  const loadedRef = useRef(false);

  useEffect(() => {
    if (loadedRef.current) return;
    loadedRef.current = true;

    if (rules.length === 0) {
      void fetchRules();
    }
    pushService.connect({ need_values: true });
    const unsubscribe = pushService.onValuesUpdate((data) => {
      applyValuesSnapshot(data.values);
    });

    return () => {
      unsubscribe();
      pushService.updateSubscription({ need_values: false });
      pushService.disconnectIfIdle();
    };
  }, [applyValuesSnapshot, fetchRules, rules.length]);

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

  const handleNavigateRule = useCallback(
    async (name: string, groupId: string | null) => {
      const targetGroupId = groupId ?? null;
      if (targetGroupId !== activeGroupId) {
        setActiveGroupId(targetGroupId);
        await fetchRules();
      }
      selectRule(name);
    },
    [activeGroupId, setActiveGroupId, fetchRules, selectRule],
  );

  const containerStyle = {
    width: '100%',
    height: '100%',
    overflow: 'hidden',
    display: 'flex' as const,
    flexDirection: 'column' as const,
    backgroundColor: token.colorBgContainer,
    position: 'relative' as const,
  };

  return (
    <div style={containerStyle}>
      <RulesDynamicIsland onNavigateRule={handleNavigateRule} />
      <div style={{ flex: 1, overflow: 'hidden' }}>
        <SplitPane
          left={<RuleList />}
          right={<RuleEditor />}
          defaultLeftWidth="250px"
          minLeftWidth={200}
          minRightWidth={400}
        />
      </div>
    </div>
  );
}
