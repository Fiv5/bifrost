import { useEffect, useRef } from 'react';
import { useSearchParams } from 'react-router-dom';
import { message, theme } from 'antd';
import SplitPane from '../../components/SplitPane';
import RuleList from './RuleList';
import RuleEditor from './RuleEditor';
import { useRulesStore } from '../../stores/useRulesStore';

const RULE_PARAM = 'rule';

export default function Rules() {
  const { token } = theme.useToken();
  const [searchParams, setSearchParams] = useSearchParams();
  const {
    error,
    clearError,
    rules,
    selectedRuleName,
    selectRule,
    fetchRules,
  } = useRulesStore();

  const initializedRef = useRef(false);
  const isUpdatingUrlRef = useRef(false);

  useEffect(() => {
    fetchRules();
  }, [fetchRules]);

  useEffect(() => {
    if (rules.length === 0) return;
    if (initializedRef.current) return;

    const ruleFromUrl = searchParams.get(RULE_PARAM);
    if (ruleFromUrl) {
      const exists = rules.some((r) => r.name === ruleFromUrl);
      if (exists) {
        selectRule(ruleFromUrl);
      } else {
        selectRule(rules[0].name);
        isUpdatingUrlRef.current = true;
        setSearchParams(
          (prev) => {
            prev.set(RULE_PARAM, rules[0].name);
            return prev;
          },
          { replace: true }
        );
      }
    } else {
      selectRule(rules[0].name);
      isUpdatingUrlRef.current = true;
      setSearchParams(
        (prev) => {
          prev.set(RULE_PARAM, rules[0].name);
          return prev;
        },
        { replace: true }
      );
    }
    initializedRef.current = true;
  }, [rules, searchParams, selectRule, setSearchParams]);

  useEffect(() => {
    if (!initializedRef.current) return;
    if (isUpdatingUrlRef.current) {
      isUpdatingUrlRef.current = false;
      return;
    }
    if (!selectedRuleName) return;

    const currentUrlParam = searchParams.get(RULE_PARAM);
    if (currentUrlParam !== selectedRuleName) {
      setSearchParams(
        (prev) => {
          prev.set(RULE_PARAM, selectedRuleName);
          return prev;
        },
        { replace: true }
      );
    }
  }, [selectedRuleName, searchParams, setSearchParams]);

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
