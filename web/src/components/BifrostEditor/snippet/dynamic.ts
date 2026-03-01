import { languages, editor, Position } from 'monaco-editor';
import type { IRange } from 'monaco-editor';

export type ReferenceType = 'value' | 'requestScript' | 'responseScript';
export type NavigationType = 'page' | 'editor';

export interface ReferenceLocation {
  name: string;
  type: ReferenceType;
  navigationType: NavigationType;
  uri?: string;
}

export interface DynamicCompletionData {
  values: string[];
  requestScripts: string[];
  responseScripts: string[];
  referenceLocations: Map<string, ReferenceLocation>;
}

let dynamicData: DynamicCompletionData = {
  values: [],
  requestScripts: [],
  responseScripts: [],
  referenceLocations: new Map(),
};

export const updateDynamicData = (data: Partial<DynamicCompletionData>) => {
  dynamicData = { ...dynamicData, ...data };
};

export const getDynamicData = (): DynamicCompletionData => dynamicData;

const createValueSuggestions = (range: IRange): languages.CompletionItem[] => {
  return dynamicData.values.map((name) => ({
    label: `{${name}}`,
    kind: languages.CompletionItemKind.Variable,
    detail: `Value: ${name}`,
    documentation: `Reference to global value "${name}"`,
    insertText: `{${name}}`,
    range,
    sortText: `0_${name}`,
  }));
};

const createScriptSuggestions = (
  range: IRange,
  type: 'request' | 'response'
): languages.CompletionItem[] => {
  const scripts =
    type === 'request' ? dynamicData.requestScripts : dynamicData.responseScripts;
  const prefix = type === 'request' ? 'reqScript' : 'resScript';

  return scripts.map((name) => ({
    label: `${prefix}://${name}`,
    kind: languages.CompletionItemKind.Reference,
    detail: `${type === 'request' ? 'Request' : 'Response'} Script: ${name}`,
    documentation: `Execute ${type} script "${name}"`,
    insertText: `${prefix}://${name}`,
    range,
    sortText: `0_${name}`,
  }));
};

export const dynamicProvider: languages.CompletionItemProvider = {
  triggerCharacters: ['{', '/', ':'],
  provideCompletionItems: (
    model: editor.ITextModel,
    position: Position
  ): languages.ProviderResult<languages.CompletionList> => {
    const suggestions: languages.CompletionItem[] = [];

    if (model.isDisposed()) {
      return { suggestions };
    }

    const lineContent = model.getLineContent(position.lineNumber);
    const textBeforeCursor = lineContent.substring(0, position.column - 1);

    if (textBeforeCursor.trim().startsWith('@')) {
      return { suggestions };
    }

    const word = model.getWordUntilPosition(position);
    const range: IRange = {
      startLineNumber: position.lineNumber,
      endLineNumber: position.lineNumber,
      startColumn: word.startColumn,
      endColumn: word.endColumn,
    };

    const reqScriptMatch = textBeforeCursor.match(/reqScript:\/\/([^/\s]*)$/);
    const resScriptMatch = textBeforeCursor.match(/resScript:\/\/([^/\s]*)$/);

    if (reqScriptMatch) {
      const scriptRange: IRange = {
        ...range,
        startColumn: position.column - reqScriptMatch[1].length,
      };
      return {
        suggestions: dynamicData.requestScripts.map((name) => ({
          label: name,
          kind: languages.CompletionItemKind.Reference,
          detail: `Request Script: ${name}`,
          documentation: `Execute request script "${name}"`,
          insertText: name,
          range: scriptRange,
          sortText: `0_${name}`,
        })),
      };
    }

    if (resScriptMatch) {
      const scriptRange: IRange = {
        ...range,
        startColumn: position.column - resScriptMatch[1].length,
      };
      return {
        suggestions: dynamicData.responseScripts.map((name) => ({
          label: name,
          kind: languages.CompletionItemKind.Reference,
          detail: `Response Script: ${name}`,
          documentation: `Execute response script "${name}"`,
          insertText: name,
          range: scriptRange,
          sortText: `0_${name}`,
        })),
      };
    }

    const valueRefMatch = textBeforeCursor.match(/\{([^}\s]*)$/);
    if (valueRefMatch) {
      const valueRange: IRange = {
        ...range,
        startColumn: position.column - valueRefMatch[0].length,
      };
      return {
        suggestions: dynamicData.values.map((name) => ({
          label: `{${name}}`,
          kind: languages.CompletionItemKind.Variable,
          detail: `Value: ${name}`,
          documentation: `Reference to global value "${name}"`,
          insertText: `{${name}}`,
          range: valueRange,
          sortText: `0_${name}`,
        })),
      };
    }

    suggestions.push(...createValueSuggestions(range));
    suggestions.push(...createScriptSuggestions(range, 'request'));
    suggestions.push(...createScriptSuggestions(range, 'response'));

    return { suggestions };
  },
};

export default dynamicProvider;
