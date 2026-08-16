import { animate } from "motion";

animate(
  "#box",
  { rotate: [0, 90], scale: [1, 1.15, 1] },
  {
    type: "spring",
    stiffness: 180,
    damping: 12,
    repeat: Infinity,
    repeatType: "mirror",
  },
);
