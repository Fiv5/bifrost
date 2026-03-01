import {
  useCallback,
  useEffect,
  useRef,
  useMemo,
  useState,
  type CSSProperties,
} from "react";
import { editor as MonacoEditor, KeyCode, KeyMod } from "monaco-editor";
import { Empty, Spin, message, theme, Button, Tooltip, Space } from "antd";
import {
  FormatPainterOutlined,
  CopyOutlined,
  SaveOutlined,
} from "@ant-design/icons";
import { useValuesStore } from "../../../stores/useValuesStore";
import { useThemeStore } from "../../../stores/useThemeStore";
import styles from "./index.module.css";

function detectLanguage(content: string): "json" | "xml" | "plaintext" {
  const trimmed = content.trim();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    try {
      JSON.parse(trimmed);
      return "json";
    } catch {
      return "plaintext";
    }
  }
  if (trimmed.startsWith("<") && trimmed.endsWith(">")) {
    return "xml";
  }
  return "plaintext";
}

function formatJSON(content: string): string {
  try {
    const parsed = JSON.parse(content);
    return JSON.stringify(parsed, null, 2);
  } catch {
    return content;
  }
}

function formatXML(content: string): string {
  try {
    let formatted = "";
    let indent = 0;
    const pad = "  ";
    content.split(/>\s*</).forEach((node, index) => {
      if (node.match(/^\/\w/)) {
        indent--;
      }
      formatted +=
        (index > 0 ? "\n" : "") +
        pad.repeat(Math.max(0, indent)) +
        (index > 0 ? "<" : "") +
        node +
        (index < content.split(/>\s*</).length - 1 ? ">" : "");
      if (
        node.match(/^<?\w[^>]*[^/]$/) &&
        !node.startsWith("?") &&
        !node.startsWith("!")
      ) {
        indent++;
      }
    });
    return formatted;
  } catch {
    return content;
  }
}

export default function ValueEditor() {
  const { token } = theme.useToken();
  const {
    currentValue,
    selectedValueName,
    editingContent,
    loading,
    saving,
    setEditingContent,
    saveCurrentValue,
  } = useValuesStore();
  const { resolvedTheme } = useThemeStore();

  const [containerElement, setContainerElement] =
    useState<HTMLDivElement | null>(null);
  const editorRef = useRef<MonacoEditor.IStandaloneCodeEditor | null>(null);
  const saveRef = useRef<typeof saveCurrentValue | null>(null);
  const isSettingValueRef = useRef(false);
  const currentValueRef = useRef<{
    currentValue: typeof currentValue;
    selectedValueName: typeof selectedValueName;
    editingContent: typeof editingContent;
  } | null>(null);

  useEffect(() => {
    saveRef.current = saveCurrentValue;
  }, [saveCurrentValue]);

  useEffect(() => {
    currentValueRef.current = {
      currentValue,
      selectedValueName,
      editingContent,
    };
  }, [currentValue, selectedValueName, editingContent]);

  const handleChange = useCallback(() => {
    if (isSettingValueRef.current) return;
    if (!editorRef.current) return;
    const selectedName = currentValueRef.current?.selectedValueName;
    if (!selectedName) return;

    const content = editorRef.current.getValue();
    setEditingContent(selectedName, content);
  }, [setEditingContent]);

  const handleSave = useCallback(async () => {
    if (!saveRef.current) return;
    const success = await saveRef.current();
    if (success) {
      message.success("Saved");
    }
  }, []);

  const currentContent = useMemo(() => {
    if (!currentValue) return "";
    const edited = editingContent[currentValue.name];
    return edited !== undefined ? edited : currentValue.value || "";
  }, [currentValue, editingContent]);

  const detectedLanguage = useMemo(
    () => detectLanguage(currentContent),
    [currentContent],
  );

  useEffect(() => {
    if (!containerElement) return;
    if (editorRef.current) return;

    const ed = MonacoEditor.create(containerElement, {
      value: currentValueRef.current?.currentValue?.value || "",
      language: detectLanguage(
        currentValueRef.current?.currentValue?.value || "",
      ),
      theme: resolvedTheme === "dark" ? "vs-dark" : "vs",
      minimap: { enabled: false },
      automaticLayout: true,
      scrollBeyondLastLine: false,
      fontSize: 12,
      lineHeight: 20,
      tabSize: 2,
      wordWrap: "on",
      lineNumbers: "on",
      folding: true,
      renderLineHighlight: "all",
      scrollbar: {
        vertical: "auto",
        horizontal: "auto",
        verticalScrollbarSize: 8,
        horizontalScrollbarSize: 8,
      },
      padding: { top: 8, bottom: 8 },
    });

    ed.addCommand(KeyMod.CtrlCmd | KeyCode.KeyS, () => {
      handleSave();
    });

    ed.onDidChangeModelContent(() => {
      handleChange();
    });

    editorRef.current = ed;

    return () => {
      ed.dispose();
      editorRef.current = null;
    };
  }, [containerElement, handleChange, handleSave, resolvedTheme]);

  useEffect(() => {
    if (!editorRef.current) return;
    editorRef.current.updateOptions({
      theme: resolvedTheme === "dark" ? "vs-dark" : "vs",
    });
  }, [resolvedTheme]);

  useEffect(() => {
    if (!editorRef.current) return;

    if (!currentValue) {
      isSettingValueRef.current = true;
      editorRef.current.setValue("");
      isSettingValueRef.current = false;
      return;
    }

    const edited = editingContent[currentValue.name];
    const content = edited !== undefined ? edited : currentValue.value || "";
    const editorContent = editorRef.current.getValue();

    if (editorContent !== content) {
      isSettingValueRef.current = true;
      editorRef.current.setValue(content);
      isSettingValueRef.current = false;
      editorRef.current.setScrollTop(0);
      editorRef.current.setScrollLeft(0);
    }

    const model = editorRef.current.getModel();
    if (model) {
      const newLang = detectLanguage(content);
      MonacoEditor.setModelLanguage(model, newLang);
    }
  }, [currentValue, editingContent]);

  const handleFormat = useCallback(() => {
    if (!editorRef.current) return;
    const content = editorRef.current.getValue();
    if (!content.trim()) return;

    const lang = detectLanguage(content);
    let formatted = content;
    if (lang === "json") {
      formatted = formatJSON(content);
    } else if (lang === "xml") {
      formatted = formatXML(content);
    }

    if (formatted !== content) {
      isSettingValueRef.current = true;
      editorRef.current.setValue(formatted);
      isSettingValueRef.current = false;
      handleChange();
      message.success("Formatted");
    }
  }, [handleChange]);

  const handleCopy = useCallback(async () => {
    if (!editorRef.current) return;
    const content = editorRef.current.getValue();
    try {
      await navigator.clipboard.writeText(content);
      message.success("Copied");
    } catch {
      message.error("Failed to copy");
    }
  }, []);

  const toolbarStyles: Record<string, CSSProperties> = {
    toolbar: {
      display: "flex",
      alignItems: "center",
      justifyContent: "space-between",
      padding: "4px 8px",
      backgroundColor: token.colorBgLayout,
      borderBottom: `1px solid ${token.colorBorderSecondary}`,
    },
  };

  if (!selectedValueName) {
    return (
      <div className={styles.empty}>
        <Empty description="Select a value to edit" />
        <div className={styles.usageGuide}>
          <h4>What are Values?</h4>
          <p>
            Values are reusable variables that can be referenced in your rules.
            Store sensitive data like API keys, tokens, or frequently used
            content here.
          </p>
          <h4>How to Use</h4>
          <ul>
            <li>
              Create a value with a unique name (e.g., <code>api_key</code>)
            </li>
            <li>
              Reference it in rules using <code>{"{name}"}</code> syntax
            </li>
            <li>
              Example: <code>{"{api_key}"}</code> will be replaced with the
              actual value
            </li>
          </ul>
          <h4>Tips</h4>
          <ul>
            <li>Use descriptive names for easy identification</li>
            <li>
              JSON and XML content will be auto-detected and syntax highlighted
            </li>
            <li>
              Press <code>Cmd+S</code> to save changes quickly
            </li>
          </ul>
        </div>
      </div>
    );
  }

  if (loading && !currentValue) {
    return (
      <div className={styles.loading}>
        <Spin size="large" />
      </div>
    );
  }

  const canFormat = detectedLanguage === "json" || detectedLanguage === "xml";
  const hasChanges =
    selectedValueName && editingContent[selectedValueName] !== undefined;

  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <div className={styles.titleSection}>
          <span className={styles.title}>{currentValue?.name}</span>
          {saving && <Spin size="small" style={{ marginLeft: 8 }} />}
        </div>
        <Space size={4}>
          {canFormat && (
            <Tooltip title="Format (JSON/XML)">
              <Button
                type="text"
                size="small"
                icon={<FormatPainterOutlined />}
                onClick={handleFormat}
              />
            </Tooltip>
          )}
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
      <div className={styles.statusBar} style={toolbarStyles.toolbar}>
        <span className={styles.hint}>
          Use <code>{"{" + (currentValue?.name || "name") + "}"}</code> to
          reference this value in rules
        </span>
        <span className={styles.language}>
          {detectedLanguage.toUpperCase()}
        </span>
      </div>
    </div>
  );
}
