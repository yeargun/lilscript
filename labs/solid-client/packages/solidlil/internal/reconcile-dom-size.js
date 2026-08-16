export function reconcileDomNodesForSize(parent, marker, current, next) {
  const host = marker.parentNode ?? parent;
  if (next.length === 0) {
    if (
      current.length > 0 &&
      host &&
      marker.parentNode === host &&
      host.childNodes.length === current.length + 1
    ) {
      host.textContent = "";
      host.appendChild(marker);
      return;
    }
    for (const node of current) node.remove();
    return;
  }

  if (current === next) {
    let reference = marker;
    for (let index = next.length - 1; index >= 0; index -= 1) {
      const node = next[index];
      if (node.nextSibling !== reference) parent.insertBefore(node, reference);
      reference = node;
    }
    return;
  }

  if (current.length === next.length) {
    let first = -1;
    let second = -1;
    let many = false;
    for (let index = 0; index < next.length; index += 1) {
      if (current[index] === next[index]) continue;
      if (first < 0) first = index;
      else if (second < 0) second = index;
      else {
        many = true;
        break;
      }
    }
    if (!many && first < 0) return;
    if (
      !many &&
      second >= 0 &&
      current[first] === next[second] &&
      current[second] === next[first]
    ) {
      const afterSecond = current[second].nextSibling;
      parent.insertBefore(next[first], current[first].nextSibling);
      parent.insertBefore(next[second], afterSecond);
      return;
    }
  } else if (next.length === current.length - 1) {
    let removed = -1;
    let offset = 0;
    let many = false;
    for (let index = 0; index < current.length; index += 1) {
      const nextIndex = index - offset;
      if (
        offset === 0 &&
        (nextIndex >= next.length || current[index] !== next[nextIndex])
      ) {
        removed = index;
        offset = 1;
        continue;
      }
      if (nextIndex >= next.length || current[index] !== next[nextIndex]) {
        many = true;
        break;
      }
    }
    if (!many && removed >= 0 && offset === 1) {
      current[removed].remove();
      return;
    }
  }

  const retained = new Set(next);
  const live = [];

  for (const node of current) {
    if (retained.has(node)) live.push(node);
    else node.remove();
  }

  if (live.length === 0) {
    for (const node of next) parent.insertBefore(node, marker);
    return;
  }

  let reference = marker;
  for (let index = next.length - 1; index >= 0; index -= 1) {
    const node = next[index];
    if (node.nextSibling !== reference) parent.insertBefore(node, reference);
    reference = node;
  }
}
