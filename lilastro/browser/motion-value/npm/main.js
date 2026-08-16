import { animate, motionValue } from "motion";

const status = document.querySelector("#status");
const x = motionValue(0);
x.on("change", (v) => {
  status.textContent = String(Math.round(v));
});
animate(x, 200, { duration: 0.2 });
animate("#box", { x: 200 }, { duration: 0.2 });
window.__featureReady = true;
