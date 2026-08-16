export function beginAggregate(first) {
  return { _total: first, _high: first, _low: first, _count: 1, _fingerprint: first ^ 97 };
}

export function updateAggregate(stats, score, index) {
  return {
    _total: stats._total + score | 0,
    _high: score > stats._high ? score : stats._high,
    _low: score < stats._low ? score : stats._low,
    _count: stats._count + 1 | 0,
    _fingerprint: (stats._fingerprint * 33 | 0) ^ score + (index * 7 | 0),
  };
}

function checksumAggregate(stats) {
  return stats._fingerprint ^ (stats._high * 17 | 0) + (stats._low * 11 | 0) + (stats._count * 5 | 0);
}

export function finishAggregate(stats) {
  return stats._total + checksumAggregate(stats) | 0;
}
