import { transform as esbuildTransform } from "esbuild";
import { minify as terserMinify } from "terser";
import { minify as viteOxcMinify } from "vite";

function requireCode(label, code) {
  if (typeof code !== "string" || code.length === 0) {
    throw new Error(`${label} did not produce JavaScript`);
  }
  return code;
}

function formatOxcErrors(errors) {
  return errors
    .map((error) =>
      [error.severity, error.message, error.codeframe]
        .filter(Boolean)
        .join(": "),
    )
    .join("\n");
}

/**
 * Run diagnostic minifier comparisons against one already-linked ESM source.
 * These outputs measure what external tools can recover from the compiler's
 * single artifact; they are never candidates for the LilScript size gate.
 */
export async function minifyJqueryBundle(source, filename) {
  if (typeof source !== "string" || source.length === 0) {
    throw new Error("jQuery bundle source must be a non-empty string");
  }
  if (typeof filename !== "string" || filename.length === 0) {
    throw new Error("jQuery bundle filename must be a non-empty string");
  }

  const [esbuildResult, terserResult, oxcResult] = await Promise.all([
    esbuildTransform(source, {
      sourcefile: filename,
      loader: "js",
      format: "esm",
      target: "esnext",
      minify: true,
      legalComments: "none",
    }),
    terserMinify(source, {
      module: true,
      compress: { passes: 3 },
      mangle: true,
      format: { comments: false },
    }),
    viteOxcMinify(filename, source, {
      module: true,
      compress: true,
      mangle: true,
      codegen: {
        removeWhitespace: true,
        legalComments: "none",
      },
      sourcemap: false,
    }),
  ]);

  if (oxcResult.errors.length > 0) {
    throw new Error(`Vite/Oxc minification failed:\n${formatOxcErrors(oxcResult.errors)}`);
  }

  return {
    esbuild: requireCode("esbuild", esbuildResult.code),
    terser: requireCode("Terser", terserResult.code),
    oxc: requireCode("Vite/Oxc", oxcResult.code),
  };
}
