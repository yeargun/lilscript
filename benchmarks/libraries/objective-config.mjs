export function configForObjective(source, objective) {
  if (!new Set(["raw", "gzip", "brotli"]).has(objective)) {
    throw new Error(`unsupported JavaScript cost-model objective: ${objective}`);
  }
  if (/^cost_model\s*=/m.test(source)) {
    return source.replace(/^cost_model\s*=.*$/m, `cost_model = "${objective}"`);
  }
  if (!/^\[javascript\]\s*$/m.test(source)) {
    throw new Error("config has no [javascript] table");
  }
  return source.replace(
    /^\[javascript\]\s*$/m,
    `[javascript]\ncost_model = "${objective}"`,
  );
}
