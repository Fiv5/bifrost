import { editor } from 'monaco-editor';

const name = 'bifrost-dark';

const config: editor.IStandaloneThemeData = {
  base: 'vs-dark',
  inherit: true,
  rules: [
    { token: '', foreground: '#e4e4e4' },
    { token: 'comment', foreground: '#75715e', fontStyle: 'italic' },
    { token: 'emphasis', fontStyle: 'italic' },
    { token: 'strong', fontStyle: 'bold' },

    { token: 'string', foreground: '#89CA78' },
    { token: 'number', foreground: '#d19a66' },

    { token: 'regexp', foreground: '#e06c75' },
    { token: 'regexp.escape', foreground: '#56b6c2' },
    { token: 'regexp.escape.control', foreground: '#56b6c2' },

    { token: 'delimiter.parenthesis', foreground: '#56b6c2' },
    { token: 'delimiter.square', foreground: '#56b6c2' },
    { token: 'delimiter.bracket', foreground: '#2BBAC5' },
    { token: 'delimiter.html', foreground: '#808080' },
    { token: 'delimiter.css', foreground: '#808080' },
    { token: 'delimiter.bracket.css', foreground: '#808080' },

    { token: 'keyword', foreground: '#D55FDE' },
    { token: 'identifier', foreground: '#E5C07B' },
    {
      token: 'identifier.var',
      foreground: '#E5C07B',
      fontStyle: 'italic bold',
    },
    { token: 'type.identifier', foreground: '#e06c75' },

    { token: 'metatag.html', foreground: '#e06c75' },
    { token: 'metatag.content.html', foreground: '#d19a66' },
    { token: 'tag', foreground: '#e06c75' },
    { token: 'attribute.name', foreground: '#d19a66' },
    { token: 'attribute.value', foreground: '#89CA78' },

    { token: 'attribute.value.hex.css', foreground: '#89CA78' },
    { token: 'attribute.value.number.css', foreground: '#D55FDE' },
    { token: 'attribute.value.unit.css', foreground: '#e06c75' },

    { token: 'reference.user', foreground: '#e06c75', fontStyle: 'bold' },
    { token: 'reference.env', foreground: '#89CA78', fontStyle: 'bold' },
    { token: 'scheme', foreground: '#89CA78', fontStyle: 'underline' },
  ],
  colors: {
    'editor.foreground': '#F3F3F3',
    'editor.background': '#171717',
    'editor.lineHighlightBackground': '#303232',
    'editor.selectionBackground': '#7f8c8d',
  },
};

export default { name, config };
