import { animate } from "motion";

animate([
  ["#a", { opacity: 1, scale: [0.5, 1.2, 1] }, { duration: 0.45 }],
  ["#b", { opacity: 1, y: [-40, 0], scale: 1 }, { duration: 0.4 }],
  ["#c", { opacity: 1, x: [40, 0], rotate: [30, 0] }, { duration: 0.45 }],
  ["#a, #b, #c", { scale: 0.9 }, { duration: 0.25 }],
  ["#a, #b, #c", { scale: 1, opacity: 0.35 }, { duration: 0.35 }],
]);
