# LilScript Language Support

VS Code language support for standalone LilScript `.lil` source files.

Included features:

- TextMate syntax highlighting;
- comments, brackets, auto-closing pairs, folding, and indentation;
- declaration and control-flow snippets;
- compiler-backed, module-aware diagnostics;
- keyword, snippet, and document completion;
- language and standard-method hover documentation;
- document symbols for structs, classes, typed host interfaces and globals,
  fields, methods, functions, externs, and top-level bindings;
- semantic highlighting, scope-aware references and rename;
- canonical formatting and import organization;
- compiler-aware lint diagnostics and machine-applicable quick fixes.

Build the server before installing the extension:

```sh
cargo build --release --bin lilscript-lsp
cd vscode-extension
npm install
npm run package
code --install-extension lilscript-vscode-0.1.0.vsix
```

The extension checks the repository's `target/release` and `target/debug`
directories before resolving `lilscript-lsp` from `PATH`. Set
`lilscript.server.path` when the server is installed elsewhere.

The server reloads project policy from the nearest `lilscript.toml`. Set
`[format].enabled = false` or `[lint].enabled = false` when those services
should be disabled for a workspace.
