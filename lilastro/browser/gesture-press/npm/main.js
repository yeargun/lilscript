import { press, animate } from "motion";

const status = document.querySelector("#status");
press("#box", (element, event) => {
  status.textContent = "pressed";
  animate("#box", { scale: 0.9 }, { duration: 0.08 });
  return (upEvent, info) => {
    status.textContent = "released";
    animate("#box", { scale: 1 }, { duration: 0.12 });
  };
});
window.__featureReady = true;
