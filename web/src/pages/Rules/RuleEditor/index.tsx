import { useCallback, useEffect, useRef, useState } from "react";
import { editor as MonacoEditor, KeyCode, KeyMod } from "monaco-editor";
import { Empty, Spin, message } from "antd";
import BifrostEditor, {
  THEME_DARK,
  THEME_LIGHT,
} from "../../../components/BifrostEditor";
import { useRulesStore } from "../../../stores/useRulesStore";
import { useThemeStore } from "../../../stores/useThemeStore";
import styles from "./index.module.css";

export default function RuleEditor() {
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

  const editorRef = useRef<MonacoEditor.IStandaloneCodeEditor | null>(null);
  const modelRef = useRef<MonacoEditor.ITextModel | null>(null);
  const saveRef = useRef<typeof saveCurrentRule | null>(null);
  const currentRuleRef = useRef<{
    currentRule: typeof currentRule;
    selectedRuleName: typeof selectedRuleName;
    editingContent: typeof editingContent;
  } | null>(null);
  const [containerElement, setContainerElement] =
    useState<HTMLDivElement | null>(null);

  useEffect(() => {
    saveRef.current = saveCurrentRule;
  }, [saveCurrentRule]);

  useEffect(() => {
    currentRuleRef.current = { currentRule, selectedRuleName, editingContent };
  }, [currentRule, selectedRuleName, editingContent]);

  const handleChange = useCallback(() => {
    if (!modelRef.current || modelRef.current.isDisposed()) return;
    const selectedName = currentRuleRef.current?.selectedRuleName;
    if (!selectedName) return;

    const content = modelRef.current.getValue();
    setEditingContent(selectedName, content);
  }, [setEditingContent]);

  const handleSave = useCallback(async () => {
    if (!saveRef.current) return;
    const success = await saveRef.current();
    if (success) {
      message.success("Saved");
    }
  }, []);

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

    editorRef.current = ed;
    modelRef.current = model;

    return () => {
      changeDisposable.dispose();
      model.dispose();
      ed.dispose();
      editorRef.current = null;
      modelRef.current = null;
    };
  }, [containerElement, handleChange, handleSave, resolvedTheme]);

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
      modelRef.current.setValue("");
      return;
    }

    const edited = editingContent[currentRule.name];
    const content = edited !== undefined ? edited : currentRule.content || "";
    const currentContent = modelRef.current.getValue();

    if (currentContent !== content) {
      modelRef.current.setValue(content);
      editorRef.current.setScrollTop(0);
      editorRef.current.setScrollLeft(0);
    }
  }, [currentRule, editingContent, containerElement]);

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

  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <span className={styles.title}>{currentRule?.name}</span>
        {saving && <Spin size="small" style={{ marginLeft: 8 }} />}
      </div>
      <div className={styles.editorContainer} ref={setContainerElement} />
    </div>
  );
}
