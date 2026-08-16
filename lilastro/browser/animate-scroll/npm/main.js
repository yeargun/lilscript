import { animate, animateMini, scroll } from "motion";

const options = { duration: 0.2 };
const mini = animateMini("#mini", { transform: "translateX(100px)" }, options);
const js = animate("#js", { x: 100 }, options);
const waapi = animate("#waapi", { transform: "translateX(100px)" }, options);
const scrollOptions = { offset: ["start start", "end end"] };

let progressSamples = 0;
scroll((progress) => {
  progressSamples += 1;
  const status = document.querySelector("#status");
  if (status) status.textContent = `progress:${progress.toFixed(3)}:${progressSamples}`;
}, scrollOptions);
scroll(mini, scrollOptions);
scroll(js, scrollOptions);
scroll(waapi, scrollOptions);

window.__featureReady = true;
window.__scrollProgressSamples = () => progressSamples;
