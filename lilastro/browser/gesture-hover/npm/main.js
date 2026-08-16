import { hover, animate } from "motion";

const status = document.querySelector("#status");
hover("#box", (element, event) => {
  status.textContent = "hovered";
  animate("#box", { scale: 1.1 }, { duration: 0.1 });
  return () => {
    status.textContent = "left";
    animate("#box", { scale: 1 }, { duration: 0.1 });
  };
});
window.__featureReady = true;
