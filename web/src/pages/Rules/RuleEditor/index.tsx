import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { editor as MonacoEditor, KeyCode, KeyMod } from "monaco-editor";
import { Empty, Spin, message, Button, Tooltip, Space, theme } from "antd";
import { SaveOutlined, CopyOutlined } from "@ant-design/icons";
import BifrostEditor, {
  THEME_DARK,
  THEME_LIGHT,
  createDebouncedValidator,
  clearValidationMarkers,
  setNavigateCallback,
  setLocalVariables,
  setLocalVariablesGetter,
  type DebouncedValidator,
  type ReferenceLocation,
} from "../../../components/BifrostEditor";
import { useRulesStore } from "../../../stores/useRulesStore";
import { useThemeStore } from "../../../stores/useThemeStore";
import { useValuesStore } from "../../../stores/useValuesStore";
import styles from "./index.module.css";

export default function RuleEditor() {
  const { token } = theme.useToken();
  const navigate = useNavigate();
  const {
    currentRule,
    selectedRuleName,
    editingContent,
    loading,
    saving,
    setEditingContent,
    saveCurrentRule,
  } = useRulesStore();
  const { resolvedTheme } = useThemeStore();
  const { values } = useValuesStore();

  const editorRef = useRef<MonacoEditor.IStandaloneCodeEditor | null>(null);
  const modelRef = useRef<MonacoEditor.ITextModel | null>(null);
  const saveRef = useRef<typeof saveCurrentRule | null>(null);
  const validatorRef = useRef<DebouncedValidator | null>(null);
  const isSettingValueRef = useRef(false);
  const valuesRef = useRef(values);
  const localVariablesRef = useRef<Array<{ name: string; line: number }>>([]);
  const currentRuleRef = useRef<{
    currentRule: typeof currentRule;
    selectedRuleName: typeof selectedRuleName;
    editingContent: typeof editingContent;
  } | null>(null);
  const [containerElement, setContainerElement] =
    useState<HTMLDivElement | null>(null);

  const handleValidationComplete = useCallback(
    (result: { defined_variables?: { name: string; defined_at?: number | null }[] }) => {
      const localVars = (result.defined_variables || [])
        .filter((v) => v.defined_at != null)
        .map((v) => ({ name: v.name, line: v.defined_at! }));
      setLocalVariables(localVars);
      localVariablesRef.current = localVars;
    },
    []
  );

  useEffect(() => {
    setLocalVariablesGetter(() => localVariablesRef.current);
    return () => {
      setLocalVariablesGetter(() => []);
    };
  }, []);

  useEffect(() => {
    saveRef.current = saveCurrentRule;
  }, [saveCurrentRule]);

  useEffect(() => {
    valuesRef.current = values;
  }, [values]);

  useEffect(() => {
    currentRuleRef.current = { currentRule, selectedRuleName, editingContent };
  }, [currentRule, selectedRuleName, editingContent]);

  const handleChange = useCallback(() => {
    if (isSettingValueRef.current) return;
    if (!modelRef.current || modelRef.current.isDisposed()) return;
    const selectedName = currentRuleRef.current?.selectedRuleName;
    if (!selectedName) return;

    const content = modelRef.current.getValue();
    setEditingContent(selectedName, content);

    if (validatorRef.current && modelRef.current) {
      validatorRef.current.validate(modelRef.current, handleValidationComplete);
    }
  }, [setEditingContent, handleValidationComplete]);

  const handleSave = useCallback(async () => {
    if (!saveRef.current) return;
    const success = await saveRef.current();
    if (success) {
      message.success("Saved");
    }
  }, []);

  const handleCopy = useCallback(async () => {
    if (!modelRef.current || modelRef.current.isDisposed()) return;
    const content = modelRef.current.getValue();
    try {
      await navigator.clipboard.writeText(content);
      message.success("Copied");
    } catch {
      message.error("Failed to copy");
    }
  }, []);

  const handleNavigate = useCallback(
    (location: ReferenceLocation) => {
      if (location.navigationType === "page" && location.uri) {
        navigate(location.uri);
      }
    },
    [navigate]
  );

  useEffect(() => {
    setNavigateCallback(handleNavigate);
    return () => {
      setNavigateCallback(null);
    };
  }, [handleNavigate]);

  useEffect(() => {
    if (!containerElement) return;
    if (editorRef.current) return;

    const editorTheme = resolvedTheme === "dark" ? THEME_DARK : THEME_LIGHT;
    const rule = currentRuleRef.current?.currentRule;
    const edited = rule
      ? currentRuleRef.current?.editingContent[rule.name]
      : undefined;
    const initialContent = rule
      ? edited !== undefined
        ? edited
        : rule.content || ""
      : "";
    const model = BifrostEditor.createModel(initialContent);
    const ed = BifrostEditor.create(containerElement, {
      theme: editorTheme,
      readOnly: false,
    });

    ed.setModel(model);
    ed.addCommand(KeyMod.CtrlCmd | KeyCode.KeyS, () => {
      handleSave();
    });

    const changeDisposable = model.onDidChangeContent(() => {
      handleChange();
    });

    const getGlobalValues = () => {
      const result: Record<string, string> = {};
      for (const v of valuesRef.current) {
        result[v.name] = v.value;
      }
      return result;
    };

    const validator = createDebouncedValidator(500, getGlobalValues);
    validatorRef.current = validator;

    if (initialContent) {
      validator.validate(model, handleValidationComplete);
    }

    editorRef.current = ed;
    modelRef.current = model;

    return () => {
      validator.cancel();
      validatorRef.current = null;
      clearValidationMarkers(model);
      setLocalVariables([]);
      changeDisposable.dispose();
      model.dispose();
      ed.dispose();
      editorRef.current = null;
      modelRef.current = null;
    };
  }, [containerElement, handleChange, handleSave, resolvedTheme, handleValidationComplete]);

  useEffect(() => {
    if (!editorRef.current) return;
    const editorTheme = resolvedTheme === "dark" ? THEME_DARK : THEME_LIGHT;
    editorRef.current.updateOptions({ theme: editorTheme });
  }, [resolvedTheme]);

  useEffect(() => {
    if (
      !modelRef.current ||
      !editorRef.current ||
      modelRef.current.isDisposed()
    ) {
      return;
    }

    if (!currentRule) {
      isSettingValueRef.current = true;
      modelRef.current.setValue("");
      isSettingValueRef.current = false;
      return;
    }

    const edited = editingContent[currentRule.name];
    const content = edited !== undefined ? edited : currentRule.content || "";
    const currentContent = modelRef.current.getValue();

    if (currentContent !== content) {
      isSettingValueRef.current = true;
      modelRef.current.setValue(content);
      isSettingValueRef.current = false;
      editorRef.current.setScrollTop(0);
      editorRef.current.setScrollLeft(0);

      if (validatorRef.current && modelRef.current) {
        validatorRef.current.validate(modelRef.current, handleValidationComplete);
      }
    }
  }, [currentRule, editingContent, containerElement, handleValidationComplete]);

  if (!selectedRuleName) {
    return (
      <div className={styles.empty}>
        <Empty description="Select a rule to edit" />
      </div>
    );
  }

  if (loading && !currentRule) {
    return (
      <div className={styles.loading}>
        <Spin size="large" />
      </div>
    );
  }

  const hasChanges =
    selectedRuleName && editingContent[selectedRuleName] !== undefined;

  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <div className={styles.titleSection}>
          <span className={styles.title}>{currentRule?.name}</span>
          {saving && <Spin size="small" style={{ marginLeft: 8 }} />}
        </div>
        <Space size={4}>
          <Tooltip title="Copy">
            <Button
              type="text"
              size="small"
              icon={<CopyOutlined />}
              onClick={handleCopy}
            />
          </Tooltip>
          <Tooltip title="Save (Cmd+S)">
            <Button
              type="text"
              size="small"
              icon={<SaveOutlined />}
              onClick={handleSave}
              disabled={!hasChanges}
              style={{ color: hasChanges ? token.colorPrimary : undefined }}
            />
          </Tooltip>
        </Space>
      </div>
      <div className={styles.editorContainer} ref={setContainerElement} />
    </div>
  );
}
