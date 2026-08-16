import { animateMini, stagger } from "motion";

const COUNT = 192;
const DURATION = 0.12;
const STAGGER = 0.001;
const SAMPLE_MS = Math.ceil((DURATION + COUNT * STAGGER + 0.08) * 1000);

function ensureBoxes() {
  const stage = document.getElementById("stage");
  stage.replaceChildren();
  const boxes = [];
  for (let i = 0; i < COUNT; i++) {
    const el = document.createElement("div");
    el.className = "box";
    el.style.top = `${(i % 8) * 24}px`;
    el.style.left = `${Math.floor(i / 8) * 24}px`;
    el.style.transform = "";
    el.style.opacity = "1";
    stage.appendChild(el);
    boxes.push(el);
  }
  return boxes;
}

function frameStats(frameTimes) {
  const sorted = [...frameTimes].sort((a, b) => a - b);
  const at = (q) =>
    sorted.length === 0
      ? 0
      : sorted[Math.min(sorted.length - 1, Math.floor(q * (sorted.length - 1)))];
  const mean =
    frameTimes.length === 0
      ? 0
      : frameTimes.reduce((s, v) => s + v, 0) / frameTimes.length;
  return {
    frames: frameTimes.length,
    frameMean: mean,
    frameP50: at(0.5),
    frameP95: at(0.95),
  };
}

window.__perfSampleDone = false;
window.__perfSample = null;

window.__runPerfSample = () => {
  window.__perfSampleDone = false;
  window.__perfSample = null;
  const boxes = ensureBoxes();
  const heapStart = performance.memory ? performance.memory.usedJSHeapSize : 0;
  const frameTimes = [];
  let last = performance.now();
  let probing = true;
  const probe = (now) => {
    frameTimes.push(now - last);
    last = now;
    if (probing) requestAnimationFrame(probe);
  };
  requestAnimationFrame(probe);

  const t0 = performance.now();
  animateMini(
    boxes,
    { transform: "translateX(160px)", opacity: 0.85 },
    { duration: DURATION, delay: stagger(STAGGER) },
  );
  const scheduleMs = performance.now() - t0;

  window.setTimeout(() => {
    probing = false;
    const heapEnd = performance.memory ? performance.memory.usedJSHeapSize : 0;
    window.__perfSample = {
      scheduleMs,
      animateMs: scheduleMs,
      heapDelta: heapEnd - heapStart,
      heapEnd,
      ...frameStats(frameTimes),
    };
    window.__perfSampleDone = true;
  }, SAMPLE_MS);
};

window.__perfReady = true;
