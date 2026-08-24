function requireRecord(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value;
}

function requireSafeInteger(value, label, minimum) {
  if (!Number.isSafeInteger(value) || value < minimum) {
    throw new Error(
      `${label} must be a safe integer greater than or equal to ${minimum}`,
    );
  }
}

export function parseSelectionExplanation(stderr, id) {
  let report;
  try {
    report = JSON.parse(stderr.trim());
  } catch (error) {
    throw new Error(
      `${id}: invalid --explain json output: ${error.message}\n${stderr}`,
    );
  }
  requireRecord(report, `${id}: --explain report`);
  const selection = requireRecord(
    report.javascript_selection,
    `${id}: javascript_selection`,
  );
  if (typeof selection.codec !== "string" || selection.codec.length === 0) {
    throw new Error(
      `${id}: javascript_selection.codec must be a non-empty string`,
    );
  }
  requireSafeInteger(
    selection.transfer_bytes,
    `${id}: javascript_selection.transfer_bytes`,
    0,
  );
  requireSafeInteger(
    selection.candidates_evaluated,
    `${id}: javascript_selection.candidates_evaluated`,
    1,
  );
  requireSafeInteger(
    selection.compiler_time_micros,
    `${id}: javascript_selection.compiler_time_micros`,
    0,
  );
  return selection;
}

export function assertSampledEffortFrontier(points, { label, objective }) {
  if (!Array.isArray(points) || points.length < 2) {
    throw new Error(`${label}: sampled effort frontier must contain at least two points`);
  }
  for (const [index, point] of points.entries()) {
    requireRecord(point, `${label}: sampled effort point ${index}`);
    requireSafeInteger(point.level, `${label}: sampled effort point ${index}.level`, 0);
    requireSafeInteger(
      point.selectedBytes,
      `${label}: sampled effort point ${index}.selectedBytes`,
      0,
    );
    if (index !== 0 && point.level <= points[index - 1].level) {
      throw new Error(`${label}: sampled effort levels must be strictly increasing`);
    }
  }

  let bestLower = points[0];
  for (const point of points.slice(1)) {
    if (point.selectedBytes > bestLower.selectedBytes) {
      throw new Error(
        `${label}: sampled level ${point.level} regressed ${objective} from ` +
          `best lower sampled level ${bestLower.level} ` +
          `(${bestLower.selectedBytes} bytes) to ${point.selectedBytes} bytes`,
      );
    }
    if (point.selectedBytes < bestLower.selectedBytes) {
      bestLower = point;
    }
  }
  return bestLower;
}

function assignments(text, key) {
  const pattern = new RegExp(
    `^\\s*${key}\\s*=\\s*([^#\\r\\n]+?)\\s*(?:#.*)?$`,
    "gm",
  );
  return [...text.matchAll(pattern)].map((match) => match[1].trim());
}

function oneAssignment(text, key, label) {
  const values = assignments(text, key);
  if (values.length !== 1) {
    throw new Error(`${label}: expected exactly one ${key} assignment`);
  }
  return values[0];
}

function quotedValue(value) {
  const match = value.match(/^(?:"([^"]*)"|'([^']*)')$/);
  return match ? (match[1] ?? match[2]) : null;
}

export function assertEffortConfig(text, { label, objective, level }) {
  if (assignments(text, "optimizations").length !== 0) {
    throw new Error(
      `${label}: javascript.optimizations must be absent so optimization_level controls the effective candidate cap`,
    );
  }
  const configuredLevel = Number(
    oneAssignment(text, "optimization_level", label),
  );
  if (!Number.isSafeInteger(configuredLevel) || configuredLevel !== level) {
    throw new Error(
      `${label}: optimization_level must be exactly ${level}, found ${configuredLevel}`,
    );
  }
  const search = quotedValue(oneAssignment(text, "candidate_search", label));
  if (search !== "always") {
    throw new Error(`${label}: candidate_search must be "always"`);
  }
  const limit = Number(oneAssignment(text, "candidate_limit", label));
  if (limit !== 1536) {
    throw new Error(`${label}: candidate_limit must be exactly 1536`);
  }
  const costModel = quotedValue(oneAssignment(text, "cost_model", label));
  if (costModel !== objective) {
    throw new Error(
      `${label}: cost_model must be ${JSON.stringify(objective)}, ` +
        `found ${JSON.stringify(costModel)}`,
    );
  }
}
