import Editor from '@monaco-editor/react';

interface RuleEditorProps {
  value: string;
  onChange?: (value: string) => void;
  readOnly?: boolean;
  height?: string | number;
}

export default function RuleEditor({
  value,
  onChange,
  readOnly = false,
  height = '400px',
}: RuleEditorProps) {
  return (
    <Editor
      height={height}
      defaultLanguage="plaintext"
      value={value}
      onChange={(v) => onChange?.(v || '')}
      options={{
        readOnly,
        minimap: { enabled: false },
        fontSize: 13,
        lineNumbers: 'on',
        scrollBeyondLastLine: false,
        wordWrap: 'on',
        tabSize: 2,
        automaticLayout: true,
      }}
      theme="vs-dark"
    />
  );
}
