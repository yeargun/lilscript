import eslint from "@eslint/js";

export default [
  {
    ignores: [
      "dist/**",
      "node_modules/**",
      "upstream/**",
      "artifacts/**",
      "packages/solidlil/reactive.generated.js",
    ],
  },
  eslint.configs.recommended,
  {
    files: ["**/*.js", "**/*.mjs", "**/*.jsx"],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
      globals: {
        console: "readonly",
        document: "readonly",
        Event: "readonly",
        globalThis: "readonly",
        HTMLHeadElement: "readonly",
        performance: "readonly",
        process: "readonly",
        queueMicrotask: "readonly",
        setTimeout: "readonly",
        window: "readonly",
      },
      parserOptions: {
        ecmaFeatures: { jsx: true },
      },
    },
    rules: {
      "no-console": ["error", { allow: ["log", "error"] }],
      "no-var": "error",
      "prefer-const": "error",
    },
  },
  {
    // These files intentionally mirror Solid's published browser facades,
    // including compatibility stubs and upstream mutable-local spelling.
    files: ["packages/solidlil/{index,store,web}.js"],
    languageOptions: {
      globals: {
        _$HY: "readonly",
      },
    },
    rules: {
      "no-case-declarations": "off",
      "no-prototype-builtins": "off",
      "no-unused-vars": "off",
      "prefer-const": "off",
    },
  },
  {
    files: ["tooling/closure.externs.js"],
    languageOptions: {
      globals: {
        Element: "readonly",
        Event: "readonly",
      },
    },
    rules: {
      "no-unused-vars": "off",
      "no-var": "off",
    },
  },
];
