import { animate, stagger } from "motion";

animate(
  ".box",
  { opacity: [0, 1], y: [50, 0] },
  { duration: 0.35, delay: stagger(0.08) },
);
