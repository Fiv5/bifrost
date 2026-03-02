import { languages, editor, Position } from 'monaco-editor';
import { getProtocolDoc } from './protocol-docs';

export const signatureHelpProvider: languages.SignatureHelpProvider = {
  signatureHelpTriggerCharacters: ['/'],
  signatureHelpRetriggerCharacters: [],

  provideSignatureHelp(
    model: editor.ITextModel,
    position: Position
  ): languages.ProviderResult<languages.SignatureHelpResult> {
    if (model.isDisposed()) return null;

    const lineContent = model.getLineContent(position.lineNumber);
    const textBeforeCursor = lineContent.substring(0, position.column - 1);

    const match = textBeforeCursor.match(/(\w+):\/\/$/);
    if (!match) return null;

    const protocol = match[1];
    const result = getProtocolDoc(protocol);
    if (!result) return null;

    return {
      value: {
        signatures: [
          {
            label: `${protocol}://<${result.doc.valueType}>`,
            documentation: {
              value: `${result.doc.description}\n\n**Examples:**\n${result.doc.examples.map((e: string) => `- \`${e}\``).join('\n')}`,
            },
            parameters: [
              {
                label: result.doc.valueType,
                documentation: result.doc.valueDescription,
              },
            ],
          },
        ],
        activeSignature: 0,
        activeParameter: 0,
      },
      dispose: () => {},
    };
  },
};
