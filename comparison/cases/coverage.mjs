export const MIN_UNIQUE_GENERATED_BEHAVIORS = 100;

const behaviorPattern = /^[a-z0-9]+(?:[/-][a-z0-9]+)*$/;

function compareCodeUnits(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

const unaryNumericPrefixCharacters = new Set("([{,:;=!?&|^~<>*/%+-");
const numericLiteralPlaceholder = "\uE000";
const unaryNumericPrefixWords = new Set([
  "await",
  "case",
  "delete",
  "new",
  "return",
  "throw",
  "typeof",
  "void",
  "yield",
]);

function normalizeUnaryNumericSigns(source) {
  return source.replace(/[+-](?=\uE000)/g, (sign, offset) => {
    let prefix = offset - 1;
    while (prefix >= 0 && /\s/.test(source[prefix])) {
      prefix -= 1;
    }
    if (prefix < 0 || unaryNumericPrefixCharacters.has(source[prefix])) {
      return "";
    }
    if (/[A-Za-z_$]/.test(source[prefix])) {
      let start = prefix;
      while (start > 0 && /[\w$]/.test(source[start - 1])) {
        start -= 1;
      }
      if (unaryNumericPrefixWords.has(source.slice(start, prefix + 1))) {
        return "";
      }
    }
    return sign;
  });
}

function literalNormalizedJavaScript(source) {
  const normalizedLiterals = source
    .replace(/`(?:\\[\s\S]|[^`\\$]|\$(?!\{))*`/g, "<string>")
    .replace(/"(?:\\[\s\S]|[^"\\])*"|'(?:\\[\s\S]|[^'\\])*'/g, "<string>")
    .replace(
      /(?<![\w$.])(?:0[xob][0-9a-f]+|(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)(?![\w$])/gi,
      numericLiteralPlaceholder,
    );
  return normalizeUnaryNumericSigns(normalizedLiterals)
    .replace(/\s+/g, " ")
    .trim();
}

export function assertNoBehaviorLabelSplits(entries, { label = "corpus" } = {}) {
  if (!Array.isArray(entries)) {
    throw new TypeError(`${label}: cases must be an array`);
  }
  const byShape = new Map();
  for (const entry of entries) {
    if (!entry || typeof entry.js !== "string") {
      throw new Error(`${label}: every audited case must have JavaScript source`);
    }
    const shape = literalNormalizedJavaScript(entry.js);
    const existing = byShape.get(shape);
    if (existing && existing.behavior !== entry.behavior) {
      throw new Error(
        `${label}: ${entry.name} (${entry.behavior}) and ${existing.name} ` +
          `(${existing.behavior}) differ only by literal parameters; ` +
          "they must share one behavior id",
      );
    }
    if (!existing) {
      byShape.set(shape, {
        behavior: entry.behavior,
        name: entry.name,
      });
    }
  }
  return {
    strategy:
      "JavaScript source normalized for quoted/static-template string, signed numeric, and whitespace parameters",
    caseInstancesAudited: entries.length,
    crossBehaviorShapeCollisions: 0,
  };
}

export function summarizeBehaviorCoverage(
  entries,
  {
    label = "corpus",
    minimumUniqueBehaviors = 0,
  } = {},
) {
  if (!Array.isArray(entries)) {
    throw new TypeError(`${label}: cases must be an array`);
  }
  if (
    !Number.isSafeInteger(minimumUniqueBehaviors) ||
    minimumUniqueBehaviors < 0
  ) {
    throw new TypeError(
      `${label}: minimumUniqueBehaviors must be a non-negative safe integer`,
    );
  }
  const variantsByBehavior = new Map();
  const names = new Set();
  for (const entry of entries) {
    if (!entry || typeof entry.name !== "string" || entry.name.length === 0) {
      throw new Error(`${label}: every case must have a non-empty name`);
    }
    if (names.has(entry.name)) {
      throw new Error(`${label}: duplicate case ${entry.name}`);
    }
    names.add(entry.name);
    if (
      typeof entry.behavior !== "string" ||
      !behaviorPattern.test(entry.behavior)
    ) {
      throw new Error(
        `${label}/${entry.name}: behavior must be a stable slash-separated id`,
      );
    }
    const variants = variantsByBehavior.get(entry.behavior) ?? [];
    variants.push(entry.name);
    variantsByBehavior.set(entry.behavior, variants);
  }

  const uniqueBehaviorTemplates = variantsByBehavior.size;
  if (uniqueBehaviorTemplates < minimumUniqueBehaviors) {
    throw new Error(
      `${label}: ${uniqueBehaviorTemplates} unique behavior templates; ` +
        `at least ${minimumUniqueBehaviors} are required. Parameter variants do not count.`,
    );
  }

  const behaviorFamilies = new Map();
  for (const [behavior, variants] of variantsByBehavior) {
    const family = behavior.split("/", 1)[0];
    const summary = behaviorFamilies.get(family) ?? {
      uniqueBehaviorTemplates: 0,
      caseInstances: 0,
    };
    summary.uniqueBehaviorTemplates += 1;
    summary.caseInstances += variants.length;
    behaviorFamilies.set(family, summary);
  }

  return {
    uniqueBehaviorTemplates,
    parameterVariants: entries.length - uniqueBehaviorTemplates,
    caseInstances: entries.length,
    behaviorFamilies: Object.fromEntries(
      [...behaviorFamilies].sort(([left], [right]) =>
        compareCodeUnits(left, right),
      ),
    ),
    variantsByBehavior: Object.fromEntries(
      [...variantsByBehavior]
        .sort(([left], [right]) => compareCodeUnits(left, right))
        .map(([behavior, variants]) => [
          behavior,
          variants.sort(compareCodeUnits),
        ]),
    ),
  };
}
