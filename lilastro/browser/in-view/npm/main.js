import { inView, animate } from "motion";

const status = document.querySelector("#status");
inView("#box", (element) => {
  status.textContent = "in";
  animate(element, { opacity: 1 }, { duration: 0.2 });
  return () => {
    status.textContent = "out";
    animate(element, { opacity: 0.2 }, { duration: 0.2 });
  };
});
window.__featureReady = true;
