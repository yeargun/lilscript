import { renderIcons } from "./site.js";

const source = document.querySelector("#source");
const runButton = document.querySelector("#run");
const status = document.querySelector("#status");
const consoleView = document.querySelector("#console");
const jsView = document.querySelector("#javascript");
const consoleTab = document.querySelector("#console-tab");
const jsTab = document.querySelector("#js-tab");
const runner = document.querySelector("#runner");

source.value = `class Vector {
  float x;
  float y;

  init(float x, float y) {
    this.x = x;
    this.y = y;
  }

  float lengthSquared() {
    return this.x * this.x + this.y * this.y;
  }
}

int[] values = [1, 2, 3, 4];
auto doubled = values.map((int value) => value * 2);
int sum = doubled.reduce((int total, int value) => total + value, 0);
Vector vector = new Vector(3.0, 4.0);

if (vector.lengthSquared() == 25.0) {
  print(\`sum=\${sum}\`);
}`;

let executionToken = "";

function selectTab(tab) {
  const consoleActive = tab === "console";
  consoleTab.classList.toggle("active", consoleActive);
  jsTab.classList.toggle("active", !consoleActive);
  consoleTab.setAttribute("aria-selected", String(consoleActive));
  jsTab.setAttribute("aria-selected", String(!consoleActive));
  consoleView.classList.toggle("active", consoleActive);
  jsView.classList.toggle("active", !consoleActive);
}

function setStatus(message, isError = false) {
  status.textContent = message;
  status.classList.toggle("error", isError);
}

function showError(message) {
  consoleView.textContent = message;
  consoleView.classList.add("error");
  selectTab("console");
  setStatus("Error", true);
}

async function compileAndRun() {
  runButton.disabled = true;
  setStatus("Compiling");
  consoleView.classList.remove("error");
  consoleView.textContent = "";
  try {
    const response = await fetch("/api/compile", {
      method: "POST",
      headers: { "Content-Type": "text/plain; charset=utf-8" },
      body: source.value,
    });
    if (!response.ok) throw new Error(`Compiler server returned ${response.status}`);
    const result = await response.json();
    if (!result.ok) {
      showError(result.error);
      return;
    }
    jsView.textContent = result.js;
    executionToken = crypto.randomUUID();
    const token = JSON.stringify(executionToken);
    const program = JSON.stringify(result.js);
    runner.srcdoc = `<script>
      const token=${token};
      const send=(type,value)=>parent.postMessage({lilscript:true,token,type,value},"*");
      console.log=(...values)=>send("log",values.map(String).join(" "));
      try{(0,eval)(${program});send("done","")}catch(error){send("error",error&&error.stack?error.stack:String(error))}
    <\/script>`;
    setStatus("Running");
    selectTab("console");
  } catch (error) {
    showError(String(error));
  } finally {
    runButton.disabled = false;
  }
}

window.addEventListener("message", (event) => {
  const message = event.data;
  if (!message || !message.lilscript || message.token !== executionToken) return;
  if (message.type === "log") {
    consoleView.textContent += `${message.value}\n`;
  } else if (message.type === "error") {
    showError(message.value);
  } else if (message.type === "done") {
    setStatus("Complete");
  }
});

runButton.addEventListener("click", compileAndRun);
consoleTab.addEventListener("click", () => selectTab("console"));
jsTab.addEventListener("click", () => selectTab("javascript"));
source.addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
    event.preventDefault();
    compileAndRun();
  }
  if (event.key === "Tab") {
    event.preventDefault();
    source.setRangeText("  ", source.selectionStart, source.selectionEnd, "end");
  }
});

renderIcons();
compileAndRun();
