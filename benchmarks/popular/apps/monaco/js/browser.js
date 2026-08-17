globalThis.MonacoEnvironment = {
  getWorkerUrl() {
    return new URL("./build/editor.worker.js", import.meta.url).href;
  },
};

const { runDemo } = await import("./build/main.js");
runDemo(
  document.getElementById("editor"),
  document.getElementById("diff"),
  document.getElementById("log"),
);
