import { animate } from "motion";

const ASSETS = [
  { src: "https://images.unsplash.com/photo-1644186171024-1bacda31db33?q=80&w=500&auto=format&fit=crop", title: "tail light" },
  { src: "https://images.unsplash.com/photo-1573750328968-179cc634660a?q=80&w=500&auto=format&fit=crop", title: "camera" },
  { src: "https://images.unsplash.com/photo-1611518757417-b5fa0838b12e?q=80&w=500&auto=format&fit=crop", title: "silhouette" },
  { src: "https://images.unsplash.com/photo-1625690303837-654c9666d2d0?q=80&w=500&auto=format&fit=crop", title: "tripod" },
  { src: "https://images.unsplash.com/photo-1541869440787-abe7669a951a?q=80&w=500&auto=format&fit=crop", title: "tree" },
  { src: "https://images.unsplash.com/photo-1611518050831-f945a6f6a1b3?q=80&w=500&auto=format&fit=crop", title: "floral" },
  { src: "https://images.unsplash.com/photo-1602043469092-aa55df5a8aae?q=80&w=500&auto=format&fit=crop", title: "ornament" },
  { src: "https://images.unsplash.com/photo-1603993138561-9151852b44e1?q=80&w=500&auto=format&fit=crop", title: "glass ball" },
  { src: "https://images.unsplash.com/photo-1611518016416-b544a3ee13ce?q=80&w=500&auto=format&fit=crop", title: "red dress" },
  { src: "https://images.unsplash.com/photo-1603993097397-89c963e325c7?q=80&w=500&auto=format&fit=crop", title: "jacket" },
];

const WIDTH = 300;
const IDLE_WIDTH = 70;
const EASE = [1, -0.03, 0.413, 0.965];

const track = document.getElementById("track");
const frame = document.getElementById("frame");
const dots = document.getElementById("dots");

let activeIndex = 3;
let isAnimating = false;

function layoutFor(index) {
  return ASSETS.map((_, i) => {
    const isActive = i === index;
    const dir = i < index ? 1 : i > index ? -1 : 0;
    return {
      rotateY: dir * 60,
      rotateZ: dir * 90,
      width: isActive ? WIDTH : IDLE_WIDTH,
    };
  });
}

track.innerHTML = ASSETS.map((item, i) => `
  <div class="slot" style="z-index:${ASSETS.length - Math.abs(activeIndex - i)}">
    <div class="slide" data-index="${i}">
      <div class="photo" style="background-image:url('${item.src}')" title="${item.title}"></div>
    </div>
  </div>
`).join("");

dots.innerHTML = ASSETS.map((_, i) => `<div class="dot${i === activeIndex ? " active" : ""}" data-index="${i}"></div>`).join("");

const slides = [...track.querySelectorAll(".slide")];
const slots = [...track.querySelectorAll(".slot")];
const dotEls = [...dots.querySelectorAll(".dot")];

function syncChrome() {
  dotEls.forEach((el, i) => el.classList.toggle("active", i === activeIndex));
  slots.forEach((el, i) => {
    el.style.zIndex = String(ASSETS.length - Math.abs(activeIndex - i));
  });
}

function applyPose(pose) {
  pose.forEach((p, i) => {
    slides[i].style.width = `${p.width}px`;
    slides[i].style.transform = `rotateY(${p.rotateY}deg) rotateZ(${p.rotateZ}deg)`;
  });
  track.style.transform = `translateX(${-(IDLE_WIDTH * activeIndex)}px)`;
}

function goTo(index) {
  if (isAnimating) return;
  const next = Math.max(0, Math.min(ASSETS.length - 1, index));
  if (next === activeIndex) return;
  const prevIndex = activeIndex;
  const from = layoutFor(prevIndex);
  activeIndex = next;
  isAnimating = true;
  syncChrome();
  const to = layoutFor(activeIndex);

  animate(
    track,
    { x: [-(IDLE_WIDTH * prevIndex), -(IDLE_WIDTH * activeIndex)] },
    { type: "spring", bounce: 0.2, duration: 0.8 },
  );

  slides.forEach((slide, i) => {
    animate(
      slide,
      {
        rotateY: [from[i].rotateY, to[i].rotateY],
        rotateZ: [from[i].rotateZ, to[i].rotateZ],
        width: [from[i].width, to[i].width],
      },
      { type: "tween", duration: 0.8, ease: EASE },
    );
  });

  animate(frame, { scale: [1, 1.07, 1] }, { duration: 0.8, ease: "easeInOut" }).then(() => {
    isAnimating = false;
  });
}

applyPose(layoutFor(activeIndex));

document.getElementById("prev").onclick = () => goTo(activeIndex - 1);
document.getElementById("next").onclick = () => goTo(activeIndex + 1);
slides.forEach((slide, i) => {
  slide.querySelector(".photo").onclick = () => goTo(i);
});
dotEls.forEach((dot, i) => {
  dot.onclick = () => goTo(i);
});
