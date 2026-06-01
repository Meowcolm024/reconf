// Mock WASM functions for development
// These will be replaced with actual WASM imports later

// Mock diagnostic data
const mockDiagnostics = [
  {
    code: "E_REFINE_004",
    message: "Refinement predicate evaluated to false",
    line: 3,
    column: 12,
    length: 4,
    label: "Value does not satisfy refinement"
  }
];

// Mock evaluation results
const mockEvalResults = {
  json: {
    success: true,
    output: '{\n  "port": 8080,\n  "host": "localhost",\n  "enabled": true\n}'
  },
  reconf: {
    success: true,
    output: '{\n  port = 8080,\n  host = "localhost",\n  enabled = true\n}'
  }
};

// Mock WASM functions
function checkReconf(code) {
  // Simulate finding errors in the example code
  if (code.includes("port = 80")) {
    return { err: JSON.stringify([
      {
        code: "E_REFINE_004",
        message: "Refinement predicate evaluated to false: x > 1024",
        line: 3,
        column: 10,
        length: 2,
        label: "Value 80 does not satisfy refinement"
      }
    ]) };
  }
  return { ok: true };
}

function evalReconf(code, format) {
  if (code.includes("port = 80")) {
    return {
      success: false,
      output: null,
      diagnostics: [
        {
          code: "E_REFINE_004",
          message: "Refinement predicate evaluated to false: x > 1024",
          line: 3,
          column: 10,
          length: 2,
          label: "Value 80 does not satisfy refinement"
        }
      ]
    };
  }
  
  return {
    success: true,
    output: mockEvalResults[format].output,
    diagnostics: []
  };
}

// DOM elements
const editorElement = document.getElementById('editor');
const errorsOutput = document.getElementById('errors-output');
const jsonOutput = document.getElementById('json-output');
const reconfOutput = document.getElementById('reconf-output');
const tabButtons = document.querySelectorAll('.tab-button');
const tabContents = document.querySelectorAll('.tab-content');

// Example code
const exampleCode = `type Port = { x: Int | x > 1024 };

let config = {
  port = 8080,
  host = "localhost",
  enabled = true
} : {
  port: Port,
  host: String,
  enabled: Bool
};

config`;

// Initialize Monaco Editor (placeholder - will be implemented later)
let editor;
function initEditor() {
  // This is a placeholder for Monaco Editor initialization
  // In a real implementation, we would load Monaco and configure it
  editorElement.textContent = exampleCode;
  editor = {
    getValue: () => editorElement.textContent,
    setValue: (value) => { editorElement.textContent = value; },
    onDidChangeModelContent: (callback) => {
      // Mock editor change event
      const observer = new MutationObserver(callback);
      observer.observe(editorElement, { childList: true, subtree: true });
    },
    getModel: () => ({}),
    deltaDecorations: () => {}
  };
  
  // Set up tab switching
  tabButtons.forEach(button => {
    button.addEventListener('click', () => {
      const tab = button.getAttribute('data-tab');
      
      // Update active tab button
      tabButtons.forEach(btn => btn.classList.remove('active'));
      button.classList.add('active');
      
      // Update active tab content
      tabContents.forEach(content => content.classList.remove('active'));
      document.getElementById(tab).classList.add('active');
    });
  });
  
  // Set initial editor content
  editor.setValue(exampleCode);
  
  // Run initial evaluation
  evaluateCode();
  
  // Set up editor change handler
  editor.onDidChangeModelContent(() => {
    evaluateCode();
  });
}

// Evaluate the code and update outputs
function evaluateCode() {
  const code = editor.getValue();
  
  // Check for errors
  const checkResult = checkReconf(code);
  if (checkResult.err) {
    const diagnostics = JSON.parse(checkResult.err);
    displayErrors(diagnostics);
    displayJson("");
    displayReconf("");
    return;
  }
  
  // Evaluate to JSON
  const jsonResult = evalReconf(code, 'json');
  if (jsonResult.success) {
    displayJson(jsonResult.output);
    displayErrors([]);
  } else {
    displayErrors(jsonResultiagnostics);
    displayJson("");
  }
  
  // Evaluate to ReConf
  const reconfResult = evalReconf(code, 'reconf');
  if (reconfResult.success) {
    displayReconf(reconfResult.output);
  } else {
    displayReconf("");
  }
}

// Display errors in the UI
function displayErrors(diagnostics) {
  if (diagnostics.length === 0) {
    errorsOutput.textContent = "No errors.";
    errorsOutput.style.color = "#d4d4d4";
    return;
  }
  
  errorsOutput.style.color = "#f44747";
  errorsOutput.textContent = diagnostics.map(diag => {
    return `Line ${diag.line}, Column ${diag.column}: [${diag.code}] ${diag.message}`;
  }).join('\n\n');
}

// Display JSON output
function displayJson(output) {
  jsonOutput.textContent = output || "JSON output will appear here.";
}

// Display ReConf output
function displayReconf(output) {
  reconfOutput.textContent = output || "ReConf output will appear here.";
}

// Initialize the application
function init() {
  initEditor();
}

// Start the application
init();