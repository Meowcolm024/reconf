import React, { useState, useEffect, useRef } from 'react';
import Editor from '@monaco-editor/react';

// ReConf language configuration for Monaco
const reconfLanguage = {
  id: 'reconf',
  extensions: ['.reconf'],
  aliases: ['ReConf', 'reconf'],
  mimetypes: ['text/x-reconf'],
};

// ReConf theme (based on VS Dark)
const reconfTheme = {
  base: 'vs-dark',
  inherit: true,
  rules: [
    { token: 'comment', foreground: '6A9955', fontStyle: 'italic' },
    { token: 'keyword', foreground: '569CD6' },
    { token: 'type', foreground: '4EC9B0' },
    { token: 'string', foreground: 'CE9178' },
    { token: 'number', foreground: 'B5CEA8' },
    { token: 'operator', foreground: 'D4D4D4' },
  ],
  colors: {
    'editor.background': '#1E1E1E',
  },
};

// Example ReConf code
const exampleCode = `type Port = { x : Int | x > 1024 && x < 65535 };

type Config = {
  port : Port,
  host : String?,
  message : String,
};

let config = {
  port = 8080,
  message = "list length: {length [1, 2, 3]}",
} : Config;

config`;

// Mock WASM functions (to be replaced with actual WASM imports)
const checkReconf = (code) => {

  return { ok: true };
};

const evalReconf = (code, format) => {

  
  const outputs = {
    json: '{\n  "host": null,\n  "message": "list length: 3",\n  "port": 8080\n}',
    reconf: '{\n  host = none,\n  message = "list length: 3",\n  port = 8080\n}'
  };
  
  return {
    success: true,
    output: outputs[format],
    diagnostics: []
  };
};

export default function EditorComponent() {
  const [code, setCode] = useState(exampleCode);
  const [errors, setErrors] = useState([]);
  const [jsonOutput, setJsonOutput] = useState('');
  const [reconfOutput, setReconfOutput] = useState('');
  const [activeTab, setActiveTab] = useState('errors');
  const editorRef = useRef(null);

  // Handle editor changes
  const handleEditorChange = (value) => {
    setCode(value || '');
  };

  // Evaluate code with debounce
  useEffect(() => {
    const timer = setTimeout(() => {
      evaluateCode(code);
    }, 500);
    
    return () => clearTimeout(timer);
  }, [code]);

  // Evaluate the code and update outputs
  const evaluateCode = (currentCode) => {
    // Check for errors
    const checkResult = checkReconf(currentCode);
    if (checkResult.err) {
      const diagnostics = JSON.parse(checkResult.err);
      setErrors(diagnostics);
      setJsonOutput('');
      setReconfOutput('');
      return;
    }
    
    // Evaluate to JSON
    const jsonResult = evalReconf(currentCode, 'json');
    if (jsonResult.success) {
      setJsonOutput(jsonResult.output);
      setErrors([]);
    } else {
      setErrors(jsonResult.diagnostics);
      setJsonOutput('');
    }
    
    // Evaluate to ReConf
    const reconfResult = evalReconf(currentCode, 'reconf');
    if (reconfResult.success) {
      setReconfOutput(reconfResult.output);
    } else {
      setReconfOutput('');
    }
  };

  // Handle editor mount
  const handleEditorDidMount = (editor, monaco) => {
    editorRef.current = editor;
    
    // Register ReConf language
    monaco.languages.register(reconfLanguage);
    
    // Define ReConf language syntax highlighting
    monaco.languages.setMonarchTokensProvider('reconf', {
      keywords: [
        'import', 'export', 'native', 'type', 'let', 'in', 'some', 'none',
        'if', 'then', 'else', 'true', 'false'
      ],
       operators: '@operators',
      typeIdentifiers: /[A-Z][a-zA-Z0-9_-]*/,
      
       symbol: /->|=>|==|!=|<=|>=|&&|\|\||\+\+|\+|-|\*|\/|%|<|>|!|:|,|\?|\{|\}|\\[|\\]||\(|\)|\./,
       tokenizer: {
        root: [
          [/#.*$/, 'comment'],
          [/@typeIdentifiers/, 'type'],
          [/(if|then|else)/, 'keyword.control'],
          [/(import|export|native|type|let|in|some|none|true|false)\b/, 'keyword'],
          [/[0-9]+\.[0-9]+/, 'number.float'],
          [/[0-9]+/, 'number.integer'],
          [/"([^"\\]|\\.)*"/, 'string'],
          [/@operators/, 'operator'],
          [/[a-z_][a-zA-Z0-9_-]*/, 'variable'],
        ]
      }
    });
    
    // Define ReConf theme
    monaco.editor.defineTheme('reconf-theme', reconfTheme);
    monaco.editor.setTheme('reconf-theme');
    
    // Set up markers for errors
    updateMarkers(editor, errors);
  };

  // Update editor markers based on errors
  const updateMarkers = (editor, diagnostics) => {
    if (!editor) return;
    
    const model = editor.getModel();
    if (!model) return;
    
    monaco.editor.setModelMarkers(
      model,
      'reconf',
      diagnostics.map(diag => ({
        startLineNumber: diag.line,
        startColumn: diag.column,
        endLineNumber: diag.line,
        endColumn: diag.column + diag.length,
        message: diag.message,
        severity: monaco.MarkerSeverity.Error,
      }))
    );
  };

  // Update markers when errors change
  useEffect(() => {
    if (editorRef.current) {
      updateMarkers(editorRef.current, errors);
    }
  }, [errors]);

  return (
    <div className="container">
      <header>
        <h1>ReConf Playground</h1>
        <p>A web-based editor for ReConf configuration language</p>
      </header>
      
      <div className="editor-container">
        <Editor
          height="50vh"
          defaultLanguage="reconf"
          defaultValue={exampleCode}
          onChange={handleEditorChange}
          onMount={handleEditorDidMount}
                     options={{
            minimap: { enabled: false },
            wordWrap: 'on',
            fontSize: 14,
            automaticLayout: true,
          }}
        />
      </div>
      
      <div className="output-container">
        <div className="tabs">
          <button
            className={`tab-button ${activeTab === 'errors' ? 'active' : ''}`}
            onClick={() => setActiveTab('errors')}
          >
            Errors
          </button>
          <button
            className={`tab-button ${activeTab === 'json' ? 'active' : ''}`}
            onClick={() => setActiveTab('json')}
          >
            JSON
          </button>
          <button
            className={`tab-button ${activeTab === 'reconf' ? 'active' : ''}`}
            onClick={() => setActiveTab('reconf')}
          >
            ReConf
          </button>
        </div>
        
        <div className={`tab-content ${activeTab === 'errors' ? 'active' : ''}`}>
          <pre className="output">
            {errors.length === 0 ? 'No errors.' : errors.map((error, index) => (
              <div key={index} style={{ color: '#f44747' }}>
                Line {error.line}, Column {error.column}: [{error.code}] {error.message}
              </div>
            ))}
          </pre>
        </div>
        
        <div className={`tab-content ${activeTab === 'json' ? 'active' : ''}`}>
          <pre className="output">{jsonOutput || 'JSON output will appear here.'}</pre>
        </div>
        
        <div className={`tab-content ${activeTab === 'reconf' ? 'active' : ''}`}>
          <pre className="output">{reconfOutput || 'ReConf output will appear here.'}</pre>
        </div>
      </div>
    </div>
  );
}