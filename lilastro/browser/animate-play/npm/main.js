import { animate, animateMini } from "motion";

const options = { duration: 0.2 };

const mini = animateMini("#mini", { transform: "translateX(100px)" }, options);
const js = animate("#js", { x: 100 }, options);
const waapi = animate("#waapi", { transform: "translateX(100px)" }, options);

window.__finishedCount = 0;
Promise.all([mini.finished, js.finished, waapi.finished]).then(() => {
  window.__finishedCount = 3;
});
