const DEFAULT_RANK_TOLERANCE = 1e-9;
const DEFAULT_EXACT_COMBINATION_LIMIT = 50_000;
const DEFAULT_EXCHANGE_PASSES = 32;

function finiteNumber(value, label) {
  if (!Number.isFinite(value)) throw new TypeError(`${label} must be finite`);
  return Number(value);
}

function normalizePositions(positions) {
  if (!Array.isArray(positions)) throw new TypeError("positions must be an array");
  return positions.map((position, index) => {
    const pair = Array.isArray(position) ? position : [position?.x, position?.y];
    if (pair.length !== 2) throw new TypeError(`positions[${index}] must be two-dimensional`);
    return [
      finiteNumber(pair[0], `positions[${index}][0]`),
      finiteNumber(pair[1], `positions[${index}][1]`),
    ];
  });
}

function positiveTolerance(value) {
  finiteNumber(value, "rankTolerance");
  if (value <= 0) throw new RangeError("rankTolerance must be positive");
  return value;
}

function normalizeFormationScale(points) {
  if (points.length <= 1) return { points: points.map((point) => point.slice()), scale: 0 };
  const center = points.reduce(
    (sum, point) => [sum[0] + point[0], sum[1] + point[1]],
    [0, 0],
  ).map((value) => value / points.length);
  const centered = points.map((point) => [point[0] - center[0], point[1] - center[1]]);
  const scale = Math.sqrt(
    centered.reduce((sum, point) => sum + point[0] ** 2 + point[1] ** 2, 0)
      / points.length,
  );
  if (scale === 0 || !Number.isFinite(scale)) return { points: centered, scale };
  return {
    points: centered.map((point) => [point[0] / scale, point[1] / scale]),
    scale,
  };
}

function normalizeCandidates(robotCount, candidates) {
  const source = candidates ?? Array.from({ length: robotCount }, (_, from) =>
    Array.from({ length: robotCount - from - 1 }, (__, offset) => ({
      from,
      to: from + offset + 1,
      weight: 1,
    })),
  ).flat();
  if (!Array.isArray(source)) throw new TypeError("candidates must be an array");

  const seen = new Set();
  const normalized = source.map((candidate, index) => {
    const tuple = Array.isArray(candidate)
      ? { from: candidate[0], to: candidate[1], weight: candidate[2] ?? 1 }
      : candidate;
    const from = tuple?.from;
    const to = tuple?.to;
    if (tuple?.noiseStdDev !== undefined
        && (!Number.isFinite(tuple.noiseStdDev) || tuple.noiseStdDev <= 0)) {
      throw new RangeError(`candidates[${index}].noiseStdDev must be positive`);
    }
    const weight = tuple?.weight ?? (
      tuple?.noiseStdDev === undefined ? 1 : 1 / (tuple.noiseStdDev * tuple.noiseStdDev)
    );
    if (!Number.isInteger(from) || from < 0 || from >= robotCount) {
      throw new RangeError(`candidates[${index}].from is outside the formation`);
    }
    if (!Number.isInteger(to) || to < 0 || to >= robotCount) {
      throw new RangeError(`candidates[${index}].to is outside the formation`);
    }
    if (from === to) throw new RangeError(`candidates[${index}] is a self-arrow`);
    finiteNumber(weight, `candidates[${index}].weight`);
    if (weight <= 0) throw new RangeError(`candidates[${index}].weight must be positive`);
    const pairKey = from < to ? `${from}:${to}` : `${to}:${from}`;
    if (seen.has(pairKey)) {
      throw new RangeError(`candidates[${index}] duplicates the robot pair ${pairKey}`);
    }
    seen.add(pairKey);
    return { from, to, weight, pairKey };
  });

  normalized.sort((left, right) =>
    left.pairKey.localeCompare(right.pairKey)
    || left.from - right.from
    || left.to - right.to,
  );
  return normalized;
}

function dot(left, right) {
  let sum = 0;
  for (let index = 0; index < left.length; index++) sum += left[index] * right[index];
  return sum;
}

function normSquared(vector) {
  return dot(vector, vector);
}

function orthogonalize(vector, basis) {
  const result = vector.slice();
  // A second pass substantially improves rank decisions for nearly singular formations.
  for (let pass = 0; pass < 2; pass++) {
    for (const direction of basis) {
      const projection = dot(result, direction);
      for (let index = 0; index < result.length; index++) {
        result[index] -= projection * direction[index];
      }
    }
  }
  return result;
}

function appendOrthonormal(vector, basis, tolerance) {
  const residual = orthogonalize(vector, basis);
  const lengthSquared = normSquared(residual);
  if (lengthSquared <= tolerance * tolerance) return false;
  const inverseLength = 1 / Math.sqrt(lengthSquared);
  basis.push(residual.map((value) => value * inverseLength));
  return true;
}

function observableBasis(points, tolerance) {
  const robotCount = points.length;
  const width = robotCount * 2;
  if (robotCount <= 1) return { basis: [], spread: 0 };

  let meanX = 0;
  let meanY = 0;
  for (const [x, y] of points) {
    meanX += x;
    meanY += y;
  }
  meanX /= robotCount;
  meanY /= robotCount;

  const gauge = [];
  const translationX = Array(width).fill(0);
  const translationY = Array(width).fill(0);
  const scale = Array(width).fill(0);
  for (let robot = 0; robot < robotCount; robot++) {
    translationX[robot * 2] = 1;
    translationY[robot * 2 + 1] = 1;
    scale[robot * 2] = points[robot][0] - meanX;
    scale[robot * 2 + 1] = points[robot][1] - meanY;
  }
  appendOrthonormal(translationX, gauge, tolerance);
  appendOrthonormal(translationY, gauge, tolerance);
  const spread = Math.sqrt(normSquared(scale));
  if (!appendOrthonormal(scale, gauge, tolerance)) return { basis: [], spread };

  const basis = [];
  for (let coordinate = 0; coordinate < width; coordinate++) {
    const unit = Array(width).fill(0);
    unit[coordinate] = 1;
    const residual = orthogonalize(unit, [...gauge, ...basis]);
    appendOrthonormal(residual, basis, tolerance);
  }
  return { basis, spread };
}

function projectRow(row, basis) {
  return basis.map((direction) => dot(row, direction));
}

function buildCandidateRows(points, originalPoints, candidates, basis, tolerance) {
  const width = points.length * 2;
  const usable = [];
  const unusable = [];
  for (const candidate of candidates) {
    const dx = points[candidate.to][0] - points[candidate.from][0];
    const dy = points[candidate.to][1] - points[candidate.from][1];
    const distance = Math.hypot(dx, dy);
    if (distance <= tolerance) {
      unusable.push({ ...candidate, reason: "coincident endpoints" });
      continue;
    }
    const bearing = [dx / distance, dy / distance];
    const actualDistance = Math.hypot(
      originalPoints[candidate.to][0] - originalPoints[candidate.from][0],
      originalPoints[candidate.to][1] - originalPoints[candidate.from][1],
    );
    const normalScale = Math.sqrt(candidate.weight) / distance;
    const nx = -bearing[1] * normalScale;
    const ny = bearing[0] * normalScale;
    const row = Array(width).fill(0);
    row[candidate.from * 2] = -nx;
    row[candidate.from * 2 + 1] = -ny;
    row[candidate.to * 2] = nx;
    row[candidate.to * 2 + 1] = ny;
    usable.push({
      ...candidate,
      distance: actualDistance,
      bearing,
      row: projectRow(row, basis),
    });
  }
  return { usable, unusable };
}

function symmetricEigen(matrix, vectorsRequested = false) {
  const size = matrix.length;
  const values = matrix.map((row) => row.slice());
  const vectors = Array.from({ length: size }, (_, row) =>
    Array.from({ length: size }, (__, column) => Number(row === column)),
  );
  if (size === 0) return { values: [], vectors };

  const scale = Math.max(0, ...values.flat().map(Math.abs));
  if (scale === 0) {
    return { values: Array(size).fill(0), vectors: vectorsRequested ? vectors : [] };
  }
  const convergence = Number.EPSILON * scale * size;
  const sweepLimit = Math.max(24, size * size * 20);
  for (let sweep = 0; sweep < sweepLimit; sweep++) {
    let pivotRow = 0;
    let pivotColumn = 0;
    let largest = 0;
    for (let row = 0; row < size; row++) {
      for (let column = row + 1; column < size; column++) {
        const magnitude = Math.abs(values[row][column]);
        if (magnitude > largest) {
          largest = magnitude;
          pivotRow = row;
          pivotColumn = column;
        }
      }
    }
    if (largest <= convergence) break;

    const app = values[pivotRow][pivotRow];
    const aqq = values[pivotColumn][pivotColumn];
    const apq = values[pivotRow][pivotColumn];
    const angle = 0.5 * Math.atan2(2 * apq, aqq - app);
    const cosine = Math.cos(angle);
    const sine = Math.sin(angle);
    for (let index = 0; index < size; index++) {
      if (index === pivotRow || index === pivotColumn) continue;
      const aip = values[index][pivotRow];
      const aiq = values[index][pivotColumn];
      values[index][pivotRow] = values[pivotRow][index] = cosine * aip - sine * aiq;
      values[index][pivotColumn] = values[pivotColumn][index] = sine * aip + cosine * aiq;
    }
    values[pivotRow][pivotRow] = cosine * cosine * app
      - 2 * sine * cosine * apq
      + sine * sine * aqq;
    values[pivotColumn][pivotColumn] = sine * sine * app
      + 2 * sine * cosine * apq
      + cosine * cosine * aqq;
    values[pivotRow][pivotColumn] = values[pivotColumn][pivotRow] = 0;

    if (vectorsRequested) {
      for (let row = 0; row < size; row++) {
        const vip = vectors[row][pivotRow];
        const viq = vectors[row][pivotColumn];
        vectors[row][pivotRow] = cosine * vip - sine * viq;
        vectors[row][pivotColumn] = sine * vip + cosine * viq;
      }
    }
  }

  const order = Array.from({ length: size }, (_, index) => index)
    .sort((left, right) => values[left][left] - values[right][right] || left - right);
  return {
    values: order.map((index) => Math.max(0, values[index][index])),
    vectors: vectorsRequested
      ? Array.from({ length: size }, (_, row) => order.map((column) => vectors[row][column]))
      : [],
  };
}

function informationMatrix(rows, width) {
  const matrix = Array.from({ length: width }, () => Array(width).fill(0));
  for (const row of rows) {
    for (let left = 0; left < width; left++) {
      for (let right = left; right < width; right++) {
        matrix[left][right] += row[left] * row[right];
      }
    }
  }
  for (let left = 0; left < width; left++) {
    for (let right = left + 1; right < width; right++) matrix[right][left] = matrix[left][right];
  }
  return matrix;
}

function selectionMetrics(edges, requiredRank, rankTolerance) {
  if (requiredRank === 0) {
    return {
      rank: 0,
      lambdaMin: Infinity,
      lambdaMax: 0,
      conditionNumber: 1,
      traceCovariance: 0,
      logDeterminant: 0,
    };
  }
  const eigenvalues = symmetricEigen(informationMatrix(edges.map((edge) => edge.row), requiredRank)).values;
  const lambdaMax = eigenvalues.at(-1) ?? 0;
  const cutoff = lambdaMax * rankTolerance * rankTolerance;
  const positive = eigenvalues.filter((value) => value > cutoff);
  const rank = positive.length;
  if (rank !== requiredRank) {
    return {
      rank,
      lambdaMin: 0,
      lambdaMax,
      conditionNumber: Infinity,
      traceCovariance: Infinity,
      logDeterminant: -Infinity,
    };
  }
  const lambdaMin = positive[0];
  return {
    rank,
    lambdaMin,
    lambdaMax,
    conditionNumber: lambdaMax / lambdaMin,
    traceCovariance: positive.reduce((sum, value) => sum + 1 / value, 0),
    logDeterminant: positive.reduce((sum, value) => sum + Math.log(value), 0),
  };
}

function compareNumbers(left, right) {
  if (Object.is(left, right)) return 0;
  if (!Number.isFinite(left) || !Number.isFinite(right)) return left > right ? 1 : -1;
  const tolerance = 1e-12 * Math.max(Number.MIN_VALUE, Math.abs(left), Math.abs(right));
  if (left > right + tolerance) return 1;
  if (left < right - tolerance) return -1;
  return 0;
}

function edgeKey(edges) {
  return edges.map((edge) => edge.pairKey).sort().join("|");
}

function compareSelections(leftEdges, leftMetrics, rightEdges, rightMetrics) {
  let order = compareNumbers(leftMetrics.lambdaMin, rightMetrics.lambdaMin);
  if (order !== 0) return order;
  order = compareNumbers(rightMetrics.traceCovariance, leftMetrics.traceCovariance);
  if (order !== 0) return order;
  order = compareNumbers(leftMetrics.logDeterminant, rightMetrics.logDeterminant);
  if (order !== 0) return order;
  return edgeKey(rightEdges).localeCompare(edgeKey(leftEdges));
}

function pivotedIndependentSet(edges, requiredRank, rankTolerance) {
  const selected = [];
  const available = edges.slice();
  const rowBasis = [];
  const maximumNorm = Math.sqrt(Math.max(0, ...edges.map((edge) => normSquared(edge.row))));
  const thresholdSquared = (maximumNorm * rankTolerance) ** 2;
  while (selected.length < requiredRank) {
    let best = null;
    for (const edge of available) {
      const residual = orthogonalize(edge.row, rowBasis);
      const residualSquared = normSquared(residual);
      if (best === null
          || residualSquared > best.residualSquared + thresholdSquared * 1e-3
          || (Math.abs(residualSquared - best.residualSquared) <= thresholdSquared * 1e-3
            && edge.pairKey < best.edge.pairKey)) {
        best = { edge, residual, residualSquared };
      }
    }
    if (best === null || best.residualSquared <= thresholdSquared) break;
    const inverseLength = 1 / Math.sqrt(best.residualSquared);
    rowBasis.push(best.residual.map((value) => value * inverseLength));
    selected.push(best.edge);
    available.splice(available.indexOf(best.edge), 1);
  }
  return selected;
}

function boundedCombinationCount(total, selected, limit) {
  selected = Math.min(selected, total - selected);
  let count = 1;
  for (let index = 1; index <= selected; index++) {
    count = count * (total - selected + index) / index;
    if (count > limit) return limit + 1;
  }
  return Math.round(count);
}

function exactRobustSelection(edges, requiredRank, rankTolerance) {
  let bestEdges = null;
  let bestMetrics = null;
  let combinationsEvaluated = 0;
  const chosen = [];

  function visit(from) {
    if (chosen.length === requiredRank) {
      combinationsEvaluated++;
      const metrics = selectionMetrics(chosen, requiredRank, rankTolerance);
      if (metrics.rank === requiredRank && (
        bestEdges === null || compareSelections(chosen, metrics, bestEdges, bestMetrics) > 0
      )) {
        bestEdges = chosen.slice();
        bestMetrics = metrics;
      }
      return;
    }
    const needed = requiredRank - chosen.length;
    for (let index = from; index <= edges.length - needed; index++) {
      chosen.push(edges[index]);
      visit(index + 1);
      chosen.pop();
    }
  }
  visit(0);
  return { edges: bestEdges, metrics: bestMetrics, combinationsEvaluated };
}

function exchangeRobustSelection(initial, allEdges, requiredRank, rankTolerance, maxPasses) {
  let selected = initial.slice();
  let metrics = selectionMetrics(selected, requiredRank, rankTolerance);
  let passes = 0;
  let combinationsEvaluated = 1;
  let converged = false;
  while (passes < maxPasses) {
    const selectedSet = new Set(selected);
    const outside = allEdges.filter((edge) => !selectedSet.has(edge));
    let bestEdges = selected;
    let bestMetrics = metrics;
    for (let remove = 0; remove < selected.length; remove++) {
      for (const replacement of outside) {
        const candidate = selected.slice();
        candidate[remove] = replacement;
        const candidateMetrics = selectionMetrics(candidate, requiredRank, rankTolerance);
        combinationsEvaluated++;
        if (candidateMetrics.rank === requiredRank
            && compareSelections(candidate, candidateMetrics, bestEdges, bestMetrics) > 0) {
          bestEdges = candidate;
          bestMetrics = candidateMetrics;
        }
      }
    }
    if (bestEdges === selected) {
      converged = true;
      break;
    }
    selected = bestEdges;
    metrics = bestMetrics;
    passes++;
  }
  return { edges: selected, metrics, passes, combinationsEvaluated, converged };
}

function publicArrow(edge) {
  return {
    from: edge.from,
    to: edge.to,
    weight: edge.weight,
    distance: edge.distance,
    bearing: edge.bearing.slice(),
  };
}

function baseResult(robotCount, requiredRank, candidates, unusable) {
  return {
    model: "directed-global-bearing-2d",
    robotCount,
    requiredRank,
    theoreticalMinimum: requiredRank,
    candidateCount: candidates.length,
    unusableCandidateCount: unusable.length,
  };
}

/**
 * Select a minimum set of directed global-bearing measurements that recovers a
 * 2D formation up to translation and positive scale. Among minimum sets, the
 * worst observable Fisher-information eigenvalue is maximized.
 */
export function selectBearingArrows(positions, options = {}) {
  const points = normalizePositions(positions);
  const robotCount = points.length;
  const requiredRank = Math.max(0, robotCount * 2 - 3);
  const rankTolerance = positiveTolerance(options.rankTolerance ?? DEFAULT_RANK_TOLERANCE);
  const exactCombinationLimit = options.exactCombinationLimit
    ?? DEFAULT_EXACT_COMBINATION_LIMIT;
  const maxExchangePasses = options.maxExchangePasses ?? DEFAULT_EXCHANGE_PASSES;
  if (!Number.isSafeInteger(exactCombinationLimit) || exactCombinationLimit < 0) {
    throw new RangeError("exactCombinationLimit must be a non-negative safe integer");
  }
  if (!Number.isSafeInteger(maxExchangePasses) || maxExchangePasses < 0) {
    throw new RangeError("maxExchangePasses must be a non-negative safe integer");
  }

  const candidates = normalizeCandidates(robotCount, options.candidates);
  if (requiredRank === 0) {
    return {
      ...baseResult(robotCount, requiredRank, candidates, []),
      recoverable: true,
      minimumProven: true,
      arrows: [],
      arrowCount: 0,
      selectionMethod: "trivial",
      combinationsEvaluated: 1,
      robustness: selectionMetrics([], 0, rankTolerance),
    };
  }

  const normalizedFormation = normalizeFormationScale(points);
  const { basis, spread } = observableBasis(normalizedFormation.points, rankTolerance);
  if (basis.length !== requiredRank) {
    return {
      ...baseResult(robotCount, requiredRank, candidates, candidates),
      recoverable: false,
      minimumProven: false,
      arrows: [],
      arrowCount: 0,
      maximumRank: 0,
      reason: spread <= rankTolerance
        ? "the formation has no nonzero scale"
        : "the translation-and-scale gauge could not be separated numerically",
    };
  }

  const { usable, unusable } = buildCandidateRows(
    normalizedFormation.points,
    points,
    candidates,
    basis,
    rankTolerance,
  );
  const independent = pivotedIndependentSet(usable, requiredRank, rankTolerance);
  if (independent.length !== requiredRank) {
    const maximumMetrics = selectionMetrics(independent, requiredRank, rankTolerance);
    return {
      ...baseResult(robotCount, requiredRank, candidates, unusable),
      recoverable: false,
      minimumProven: false,
      arrows: independent.map(publicArrow),
      arrowCount: independent.length,
      maximumRank: maximumMetrics.rank,
      reason: "candidate bearings do not span every observable shape deformation",
    };
  }

  const combinationCount = boundedCombinationCount(
    usable.length,
    requiredRank,
    exactCombinationLimit,
  );
  let selection;
  let selectionMethod;
  let optimality;
  if (combinationCount <= exactCombinationLimit) {
    selection = exactRobustSelection(usable, requiredRank, rankTolerance);
    selectionMethod = "exact-e-optimal";
    optimality = "global among all minimum-cardinality subsets";
  } else {
    selection = exchangeRobustSelection(
      independent,
      usable,
      requiredRank,
      rankTolerance,
      maxExchangePasses,
    );
    selectionMethod = "rank-pivot-plus-exchange";
    optimality = selection.converged
      ? "one-arrow-exchange local optimum"
      : "bounded one-arrow-exchange search";
  }

  if (!selection.edges || !selection.metrics || selection.metrics.rank !== requiredRank) {
    return {
      ...baseResult(robotCount, requiredRank, candidates, unusable),
      recoverable: false,
      minimumProven: false,
      arrows: independent.map(publicArrow),
      arrowCount: independent.length,
      maximumRank: selection.metrics?.rank ?? 0,
      reason: "numerical scoring could not certify the independently constructed basis",
    };
  }

  const arrows = selection.edges.slice().sort((left, right) =>
    left.pairKey.localeCompare(right.pairKey),
  );
  return {
    ...baseResult(robotCount, requiredRank, candidates, unusable),
    recoverable: true,
    minimumProven: true,
    arrows: arrows.map(publicArrow),
    arrowCount: arrows.length,
    selectionMethod,
    robustnessOptimality: optimality,
    combinationsEvaluated: selection.combinationsEvaluated,
    exchangePasses: selection.passes ?? 0,
    exchangeConverged: selection.converged ?? true,
    robustness: selection.metrics,
  };
}

/** Analyze a caller-supplied arrow set under the same bearing-noise model. */
export function analyzeBearingArrows(positions, arrows, options = {}) {
  const points = normalizePositions(positions);
  const rankTolerance = positiveTolerance(options.rankTolerance ?? DEFAULT_RANK_TOLERANCE);
  const requiredRank = Math.max(0, points.length * 2 - 3);
  const candidates = normalizeCandidates(points.length, arrows);
  if (requiredRank === 0) {
    return {
      ...selectionMetrics([], 0, rankTolerance),
      recoverable: true,
      requiredRank,
      arrowCount: arrows.length,
      unusableArrowCount: 0,
    };
  }
  const normalizedFormation = normalizeFormationScale(points);
  const { basis } = observableBasis(normalizedFormation.points, rankTolerance);
  if (basis.length !== requiredRank) {
    return {
      ...selectionMetrics([], requiredRank, rankTolerance),
      recoverable: false,
      requiredRank,
      arrowCount: arrows.length,
      unusableArrowCount: arrows.length,
    };
  }
  const { usable, unusable } = buildCandidateRows(
    normalizedFormation.points,
    points,
    candidates,
    basis,
    rankTolerance,
  );
  const metrics = selectionMetrics(usable, requiredRank, rankTolerance);
  return {
    ...metrics,
    recoverable: metrics.rank === requiredRank,
    requiredRank,
    arrowCount: arrows.length,
    unusableArrowCount: unusable.length,
  };
}

function centeredCoordinateBasis(robotCount, tolerance) {
  const width = robotCount * 2;
  const gauge = [];
  const translationX = Array(width).fill(0);
  const translationY = Array(width).fill(0);
  for (let robot = 0; robot < robotCount; robot++) {
    translationX[robot * 2] = 1;
    translationY[robot * 2 + 1] = 1;
  }
  appendOrthonormal(translationX, gauge, tolerance);
  appendOrthonormal(translationY, gauge, tolerance);
  const basis = [];
  for (let coordinate = 0; coordinate < width; coordinate++) {
    const unit = Array(width).fill(0);
    unit[coordinate] = 1;
    const residual = orthogonalize(unit, [...gauge, ...basis]);
    appendOrthonormal(residual, basis, tolerance);
  }
  return basis;
}

function normalizeMeasurements(robotCount, measurements) {
  if (!Array.isArray(measurements)) throw new TypeError("measurements must be an array");
  return measurements.map((measurement, index) => {
    const { from, to } = measurement;
    if (!Number.isInteger(from) || from < 0 || from >= robotCount
        || !Number.isInteger(to) || to < 0 || to >= robotCount || from === to) {
      throw new RangeError(`measurements[${index}] has invalid endpoints`);
    }
    const bearing = measurement.bearing;
    if (!Array.isArray(bearing) || bearing.length !== 2) {
      throw new TypeError(`measurements[${index}].bearing must be two-dimensional`);
    }
    const gx = finiteNumber(bearing[0], `measurements[${index}].bearing[0]`);
    const gy = finiteNumber(bearing[1], `measurements[${index}].bearing[1]`);
    const length = Math.hypot(gx, gy);
    if (length === 0) throw new RangeError(`measurements[${index}].bearing must be nonzero`);
    const weight = measurement.weight ?? 1;
    finiteNumber(weight, `measurements[${index}].weight`);
    if (weight <= 0) throw new RangeError(`measurements[${index}].weight must be positive`);
    const distance = measurement.distance ?? 1;
    finiteNumber(distance, `measurements[${index}].distance`);
    if (distance <= 0) throw new RangeError(`measurements[${index}].distance must be positive`);
    return { from, to, bearing: [gx / length, gy / length], weight, distance };
  });
}

/**
 * Recover a centered, unit-RMS shape from noisy directed global bearings.
 * Translation and scale are intentionally fixed by this output normalization.
 */
export function reconstructShape(robotCount, measurements, options = {}) {
  if (!Number.isSafeInteger(robotCount) || robotCount < 2) {
    throw new RangeError("robotCount must be an integer of at least two");
  }
  const rankTolerance = positiveTolerance(options.rankTolerance ?? DEFAULT_RANK_TOLERANCE);
  const normalized = normalizeMeasurements(robotCount, measurements);
  const requiredRank = robotCount * 2 - 3;
  if (normalized.length < requiredRank) {
    throw new Error(`at least ${requiredRank} bearing measurements are required`);
  }
  const basis = centeredCoordinateBasis(robotCount, rankTolerance);
  const width = robotCount * 2;
  const rows = normalized.map((measurement) => {
    const row = Array(width).fill(0);
    const scale = Math.sqrt(measurement.weight) / measurement.distance;
    const nx = -measurement.bearing[1] * scale;
    const ny = measurement.bearing[0] * scale;
    row[measurement.from * 2] = -nx;
    row[measurement.from * 2 + 1] = -ny;
    row[measurement.to * 2] = nx;
    row[measurement.to * 2 + 1] = ny;
    return projectRow(row, basis);
  });
  const decomposition = symmetricEigen(informationMatrix(rows, basis.length), true);
  if (decomposition.values.length < 2) throw new Error("formation has no recoverable shape mode");
  const lambdaMax = decomposition.values.at(-1) ?? 0;
  if (decomposition.values[1] <= lambdaMax * rankTolerance * rankTolerance) {
    throw new Error("bearing measurements do not define a unique shape mode");
  }
  const reducedShape = decomposition.vectors.map((row) => row[0]);
  const flattened = Array(width).fill(0);
  for (let coordinate = 0; coordinate < width; coordinate++) {
    for (let axis = 0; axis < basis.length; axis++) {
      flattened[coordinate] += basis[axis][coordinate] * reducedShape[axis];
    }
  }

  let orientation = 0;
  for (const measurement of normalized) {
    const dx = flattened[measurement.to * 2] - flattened[measurement.from * 2];
    const dy = flattened[measurement.to * 2 + 1] - flattened[measurement.from * 2 + 1];
    orientation += measurement.weight
      * (dx * measurement.bearing[0] + dy * measurement.bearing[1]);
  }
  if (orientation < 0) {
    for (let index = 0; index < flattened.length; index++) flattened[index] *= -1;
  }
  for (const measurement of normalized) {
    const dx = flattened[measurement.to * 2] - flattened[measurement.from * 2];
    const dy = flattened[measurement.to * 2 + 1] - flattened[measurement.from * 2 + 1];
    const projection = dx * measurement.bearing[0] + dy * measurement.bearing[1];
    if (projection <= rankTolerance * Math.hypot(dx, dy)) {
      throw new Error("directed bearings are inconsistent with one positive-scale shape");
    }
  }
  const rms = Math.sqrt(normSquared(flattened) / robotCount);
  if (rms <= rankTolerance) throw new Error("bearing system collapsed to a zero-scale shape");
  const points = Array.from({ length: robotCount }, (_, robot) => [
    flattened[robot * 2] / rms,
    flattened[robot * 2 + 1] / rms,
  ]);
  return {
    points,
    residualEigenvalue: decomposition.values[0],
    spectralGap: decomposition.values[1] - decomposition.values[0],
    measurementCount: normalized.length,
  };
}
