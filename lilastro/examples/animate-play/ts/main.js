import { animate, animateMini, spring } from "motion";

const keep = [animate, animateMini, spring]
  .map((fn) => (typeof fn === "function" ? fn.name || "fn" : "x"))
  .join("|");

const gen = spring({
  keyframes: [0, 100],
  stiffness: 170,
  damping: 26,
  mass: 1,
});
let digest = 0;
for (let t = 0; t <= 256; t += 16) {
  digest += Math.round(gen.next(t).value * 1000);
}

if (typeof document !== "undefined") {
  const mini = document.createElement("div");
  const js = document.createElement("div");
  document.body.append(mini, js);
  animateMini(mini, { transform: "translateX(100px)" }, { duration: 0.2 });
  animate(js, { x: 100 }, { duration: 0.2 });
}

console.log(`animate-play:${keep}:${digest}`);
