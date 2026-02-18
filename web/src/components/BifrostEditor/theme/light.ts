import { editor } from 'monaco-editor';

const name = 'bifrost-light';

const config: editor.IStandaloneThemeData = {
  base: 'vs',
  inherit: true,
  rules: [
    { token: '', foreground: '#373c41' },
    { token: 'comment', foreground: '#9e9a88', fontStyle: 'italic' },
    { token: 'emphasis', fontStyle: 'italic' },
    { token: 'strong', fontStyle: 'bold' },

    { token: 'regexp', foreground: '#e06c75' },
    { token: 'regexp.escape', foreground: '#408790' },
    { token: 'regexp.escape.control', foreground: '#408790' },

    { token: 'keyword', foreground: '#a628a4' },
    { token: 'identifier', foreground: '#e57b7b' },
    {
      token: 'identifier.var',
      foreground: '#e57b7b',
      fontStyle: 'italic bold',
    },
    { token: 'type.identifier', foreground: '#e06c75' },

    { token: 'reference.user', foreground: '#e06c75', fontStyle: 'bold' },
    { token: 'reference.env', foreground: '#1b8200', fontStyle: 'bold' },
    { token: 'scheme', foreground: '#1b8200', fontStyle: 'underline' },
  ],
  colors: {},
};

export default { name, config };
