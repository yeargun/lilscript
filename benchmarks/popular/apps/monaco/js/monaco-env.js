self.MonacoEnvironment = {
  getWorker(_id, label) {
    const file =
      label === "json"
        ? "json.worker.js"
        : label === "css" || label === "scss" || label === "less"
          ? "css.worker.js"
          : label === "html" || label === "handlebars" || label === "razor"
            ? "html.worker.js"
            : label === "typescript" || label === "javascript"
              ? "ts.worker.js"
              : "editor.worker.js";
    return new Worker(file, { name: label });
  },
};
