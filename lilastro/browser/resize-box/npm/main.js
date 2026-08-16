import { resize } from "motion";

const status = document.querySelector("#status");
resize("#box", (element, info) => {
  status.textContent = `${Math.round(info.width)}x${Math.round(info.height)}`;
});
window.__featureReady = true;
