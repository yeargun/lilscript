import { animate, hover, press } from "motion";

hover(".card", (el) => {
  animate(el, { y: -10, scale: 1.06 }, { type: "spring", stiffness: 400, damping: 22 });
  return () => animate(el, { y: 0, scale: 1 }, { type: "spring", stiffness: 350, damping: 24 });
});

press("#b, #c", (el) => {
  animate(el, { scale: 0.92, rotate: -2 }, { type: "spring", stiffness: 500, damping: 28 });
  return () => animate(el, { scale: 1, rotate: 0 }, { type: "spring", stiffness: 420, damping: 24 });
});
