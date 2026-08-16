(() => {
  const panel = document.createElement("pre");
  panel.setAttribute("data-demo-log", "");
  panel.style.cssText =
    "margin:0;padding:18px 20px;min-height:100vh;box-sizing:border-box;background:#151a18;color:#dce8e2;font:13px/1.55 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;white-space:pre-wrap;word-break:break-word;";
  const mount = () => {
    if (document.body && !panel.isConnected) document.body.prepend(panel);
  };
  const write = (kind, args) => {
    mount();
    const line = document.createElement("div");
    line.textContent = args
      .map((value) => {
        if (typeof value === "string") return value;
        try {
          return JSON.stringify(value);
        } catch {
          return String(value);
        }
      })
      .join(" ");
    if (kind === "error") line.style.color = "#f0a8a0";
    panel.append(line);
  };
  const origLog = console.log.bind(console);
  const origError = console.error.bind(console);
  console.log = (...args) => {
    origLog(...args);
    write("log", args);
  };
  console.error = (...args) => {
    origError(...args);
    write("error", args);
  };
  window.addEventListener("error", (event) => write("error", [event.message]));
  if (document.body) mount();
  else document.addEventListener("DOMContentLoaded", mount);
})();
