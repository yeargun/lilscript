import { animate, stagger } from "motion";

const grid = document.getElementById("grid");
grid.innerHTML = Array.from({ length: 25 }, () => `<div class="dot"></div>`).join("");

animate(
  ".dot",
  { opacity: [0, 1], scale: [0.4, 1], rotate: [20, 0] },
  {
    delay: stagger(0.035, { from: "center" }),
    type: "spring",
    stiffness: 320,
    damping: 18,
  },
);
