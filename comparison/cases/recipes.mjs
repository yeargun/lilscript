// The competitor recipes of the micro corpus, declared once. The paired-case
// gate (run.mjs) and the generic corpus ratchet (scripts/ratchet.mjs, which
// applies them to comparison/apps as well) both read them from here, so a bar
// is always built with the recipe its report names.
export const javascriptTarget = "es2022";

export const baselineToolNames = [
  "terser",
  "terser-properties",
  "oxc",
  "esbuild-script",
  "esbuild-iife",
];

export const baselineOptions = {
  terser: {
    ecma: 2022,
    compress: {
      ecma: 2022,
      passes: 3,
      drop_console: false,
      toplevel: true,
    },
    mangle: { toplevel: true },
    format: { ecma: 2022, comments: false },
  },
  "terser-properties": {
    ecma: 2022,
    compress: {
      ecma: 2022,
      passes: 3,
      drop_console: false,
      toplevel: true,
    },
    mangle: {
      toplevel: true,
      properties: {
        builtins: false,
        keep_quoted: true,
        reserved: ["__proto__", "constructor", "prototype"],
      },
    },
    format: { ecma: 2022, comments: false },
  },
  oxc: {
    module: false,
    compress: { target: javascriptTarget },
    mangle: { toplevel: true },
    codegen: { target: javascriptTarget, legalComments: "none" },
  },
  "esbuild-script": {
    minify: true,
    target: javascriptTarget,
    legalComments: "none",
  },
  "esbuild-iife": {
    minify: true,
    target: javascriptTarget,
    legalComments: "none",
    format: "iife",
  },
};
