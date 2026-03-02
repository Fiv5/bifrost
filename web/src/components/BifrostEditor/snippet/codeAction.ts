import { languages, editor } from 'monaco-editor';

export interface CodeFix {
  title: string;
  range?: [number, number];
  new_text: string;
}

export interface ParseErrorWithFixes {
  line: number;
  start_column: number;
  end_column: number;
  message: string;
  fixes?: CodeFix[];
}

const fixesCache = new Map<string, CodeFix[]>();

export function setFixesForMarker(
  modelUri: string,
  line: number,
  startColumn: number,
  endColumn: number,
  fixes: CodeFix[]
): void {
  const key = `${modelUri}:${line}:${startColumn}:${endColumn}`;
  fixesCache.set(key, fixes);
}

export function clearFixesCache(modelUri?: string): void {
  if (modelUri) {
    const keysToDelete: string[] = [];
    for (const key of fixesCache.keys()) {
      if (key.startsWith(modelUri)) {
        keysToDelete.push(key);
      }
    }
    keysToDelete.forEach((key) => fixesCache.delete(key));
  } else {
    fixesCache.clear();
  }
}

export const codeActionProvider: languages.CodeActionProvider = {
  provideCodeActions(
    model: editor.ITextModel,
    _range,
    context
  ): languages.ProviderResult<languages.CodeActionList> {
    if (model.isDisposed()) {
      return { actions: [], dispose: () => {} };
    }

    const actions: languages.CodeAction[] = [];
    const modelUri = model.uri.toString();

    for (const marker of context.markers) {
      const key = `${modelUri}:${marker.startLineNumber}:${marker.startColumn}:${marker.endColumn}`;
      const fixes = fixesCache.get(key);

      if (fixes && fixes.length > 0) {
        for (const fix of fixes) {
          const fixRange = fix.range
            ? {
                startLineNumber: marker.startLineNumber,
                startColumn: fix.range[0],
                endLineNumber: marker.endLineNumber,
                endColumn: fix.range[1],
              }
            : {
                startLineNumber: marker.startLineNumber,
                startColumn: marker.startColumn,
                endLineNumber: marker.endLineNumber,
                endColumn: marker.endColumn,
              };

          actions.push({
            title: fix.title,
            kind: 'quickfix',
            diagnostics: [marker],
            isPreferred: actions.length === 0,
            edit: {
              edits: [
                {
                  resource: model.uri,
                  textEdit: {
                    range: fixRange,
                    text: fix.new_text,
                  },
                  versionId: model.getVersionId(),
                },
              ],
            },
          });
        }
      }
    }

    return { actions, dispose: () => {} };
  },
};
