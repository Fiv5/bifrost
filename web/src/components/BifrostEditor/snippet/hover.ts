import { languages, editor, Position, Uri } from 'monaco-editor';
import type { IRange } from 'monaco-editor';
import { getDynamicData, type ReferenceLocation, type ReferenceType } from './dynamic';

export interface ReferenceMatch {
  name: string;
  type: ReferenceType;
  range: IRange;
}

const VALUE_REF_PATTERN = /\{([\w-]+)\}/g;
const REQ_SCRIPT_PATTERN = /reqScript:\/\/([\w\-.]+)/g;
const RES_SCRIPT_PATTERN = /resScript:\/\/([\w\-.]+)/g;

function findReferenceAtPosition(
  model: editor.ITextModel,
  position: Position
): ReferenceMatch | null {
  const lineContent = model.getLineContent(position.lineNumber);
  const column = position.column;

  const patterns: { pattern: RegExp; type: ReferenceType }[] = [
    { pattern: VALUE_REF_PATTERN, type: 'value' },
    { pattern: REQ_SCRIPT_PATTERN, type: 'requestScript' },
    { pattern: RES_SCRIPT_PATTERN, type: 'responseScript' },
  ];

  for (const { pattern, type } of patterns) {
    pattern.lastIndex = 0;
    let match;
    while ((match = pattern.exec(lineContent)) !== null) {
      const startCol = match.index + 1;
      const endCol = match.index + match[0].length + 1;

      if (column >= startCol && column <= endCol) {
        return {
          name: match[1],
          type,
          range: {
            startLineNumber: position.lineNumber,
            endLineNumber: position.lineNumber,
            startColumn: startCol,
            endColumn: endCol,
          },
        };
      }
    }
  }

  return null;
}

function getReferenceLocation(
  name: string,
  type: ReferenceType
): ReferenceLocation | undefined {
  const data = getDynamicData();

  const key = `${type}:${name}`;
  const location = data.referenceLocations.get(key);
  if (location) return location;

  switch (type) {
    case 'value':
      if (data.values.includes(name)) {
        return {
          name,
          type: 'value',
          navigationType: 'page',
          uri: `/values?name=${encodeURIComponent(name)}`,
        };
      }
      break;
    case 'requestScript':
      if (data.requestScripts.includes(name)) {
        return {
          name,
          type: 'requestScript',
          navigationType: 'page',
          uri: `/scripts?type=request&name=${encodeURIComponent(name)}`,
        };
      }
      break;
    case 'responseScript':
      if (data.responseScripts.includes(name)) {
        return {
          name,
          type: 'responseScript',
          navigationType: 'page',
          uri: `/scripts?type=response&name=${encodeURIComponent(name)}`,
        };
      }
      break;
  }

  return undefined;
}

function getTypeLabel(type: ReferenceType): string {
  switch (type) {
    case 'value':
      return 'Global Value';
    case 'requestScript':
      return 'Request Script';
    case 'responseScript':
      return 'Response Script';
  }
}

export type NavigateCallback = (location: ReferenceLocation) => void;

let onNavigate: NavigateCallback | null = null;

export const setNavigateCallback = (callback: NavigateCallback | null) => {
  onNavigate = callback;
};

export const hoverProvider: languages.HoverProvider = {
  provideHover: (
    model: editor.ITextModel,
    position: Position
  ): languages.ProviderResult<languages.Hover> => {
    if (model.isDisposed()) return null;

    const reference = findReferenceAtPosition(model, position);
    if (!reference) return null;

    const location = getReferenceLocation(reference.name, reference.type);
    if (!location) return null;

    const typeLabel = getTypeLabel(reference.type);
    const actionText =
      location.navigationType === "page"
        ? "Go to definition"
        : "Jump to definition";

    const contents = [
      {
        value: `**${typeLabel}**: \`${reference.name}\``,
      },
      {
        value: `[${actionText}](command:bifrost.navigateToReference?${encodeURIComponent(JSON.stringify(location))})`,
        isTrusted: true,
      },
    ];

    return {
      range: reference.range,
      contents,
    };
  },
};

export const definitionProvider: languages.DefinitionProvider = {
  provideDefinition: (
    model: editor.ITextModel,
    position: Position
  ): languages.ProviderResult<languages.Definition> => {
    if (model.isDisposed()) return null;

    const reference = findReferenceAtPosition(model, position);
    if (!reference) return null;

    const location = getReferenceLocation(reference.name, reference.type);
    if (!location) return null;

    if (onNavigate) {
      setTimeout(() => {
        onNavigate?.(location);
      }, 0);
    }

    return null;
  },
};

export const linkProvider: languages.LinkProvider = {
  provideLinks: (
    model: editor.ITextModel
  ): languages.ProviderResult<languages.ILinksList> => {
    if (model.isDisposed()) return { links: [] };

    const links: languages.ILink[] = [];
    const lineCount = model.getLineCount();

    for (let lineNumber = 1; lineNumber <= lineCount; lineNumber++) {
      const lineContent = model.getLineContent(lineNumber);

      const patterns: { pattern: RegExp; type: ReferenceType }[] = [
        { pattern: new RegExp(VALUE_REF_PATTERN.source, 'g'), type: 'value' },
        { pattern: new RegExp(REQ_SCRIPT_PATTERN.source, 'g'), type: 'requestScript' },
        { pattern: new RegExp(RES_SCRIPT_PATTERN.source, 'g'), type: 'responseScript' },
      ];

      for (const { pattern, type } of patterns) {
        let match;
        while ((match = pattern.exec(lineContent)) !== null) {
          const name = match[1];
          const location = getReferenceLocation(name, type);
          if (!location) continue;

          const startCol = match.index + 1;
          const endCol = match.index + match[0].length + 1;

          links.push({
            range: {
              startLineNumber: lineNumber,
              endLineNumber: lineNumber,
              startColumn: startCol,
              endColumn: endCol,
            },
            tooltip: `Go to ${getTypeLabel(type)}: ${name}`,
            url: Uri.parse(`command:bifrost.navigateToReference?${encodeURIComponent(JSON.stringify(location))}`),
          });
        }
      }
    }

    return { links };
  },
};
