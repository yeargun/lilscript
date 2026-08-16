// Reconcile a keyed DOM range while preserving node identity and minimizing
// moves. The fast paths mirror Solid's proven two-ended list reconciler.
export function reconcileDomNodes(parent, marker, current, next, release) {
  const retained = new Set(next);
  const live = [];

  for (const node of current) {
    if (!node) continue;
    if (retained.has(node)) live.push(node);
    else {
      node.remove();
      release(node);
    }
  }

  if (live.length === 0) {
    for (const node of next) {
      if (node) parent.insertBefore(node, marker);
    }
    return;
  }

  let nextLength = next.length;
  let liveEnd = live.length;
  let nextEnd = nextLength;
  let liveStart = 0;
  let nextStart = 0;
  const after = live[liveEnd - 1].nextSibling;
  let positions;

  while (liveStart < liveEnd || nextStart < nextEnd) {
    if (live[liveStart] === next[nextStart]) {
      liveStart += 1;
      nextStart += 1;
      continue;
    }
    while (live[liveEnd - 1] === next[nextEnd - 1]) {
      liveEnd -= 1;
      nextEnd -= 1;
    }
    if (liveEnd === liveStart) {
      const reference =
        nextEnd < nextLength
          ? nextStart
            ? next[nextStart - 1].nextSibling
            : next[nextEnd - nextStart]
          : after;
      while (nextStart < nextEnd) {
        parent.insertBefore(next[nextStart], reference);
        nextStart += 1;
      }
    } else if (nextEnd === nextStart) {
      while (liveStart < liveEnd) {
        if (!positions || !positions.has(live[liveStart])) {
          live[liveStart].remove();
        }
        liveStart += 1;
      }
    } else if (
      live[liveStart] === next[nextEnd - 1] &&
      next[nextStart] === live[liveEnd - 1]
    ) {
      const reference = live[--liveEnd].nextSibling;
      parent.insertBefore(next[nextStart++], live[liveStart++].nextSibling);
      parent.insertBefore(next[--nextEnd], reference);
      live[liveEnd] = next[nextEnd];
    } else {
      if (!positions) {
        positions = new Map();
        let index = nextStart;
        while (index < nextEnd) {
          positions.set(next[index], index);
          index += 1;
        }
      }
      const index = positions.get(live[liveStart]);
      if (index != null) {
        if (nextStart < index && index < nextEnd) {
          let cursor = liveStart;
          let sequence = 1;
          let position;
          while (++cursor < liveEnd && cursor < nextEnd) {
            position = positions.get(live[cursor]);
            if (position == null || position !== index + sequence) break;
            sequence += 1;
          }
          if (sequence > index - nextStart) {
            const reference = live[liveStart];
            while (nextStart < index) {
              parent.insertBefore(next[nextStart], reference);
              nextStart += 1;
            }
          } else {
            parent.replaceChild(next[nextStart++], live[liveStart++]);
          }
        } else liveStart += 1;
      } else live[liveStart++].remove();
    }
  }
}
