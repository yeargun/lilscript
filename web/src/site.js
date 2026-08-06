import {
  ArrowRight,
  BadgeInfo,
  Check,
  Copy,
  ExternalLink,
  Menu,
  Play,
  Search,
  TriangleAlert,
  X,
  createIcons,
} from "lucide";

export const icons = {
  ArrowRight,
  BadgeInfo,
  Check,
  Copy,
  ExternalLink,
  Menu,
  Play,
  Search,
  TriangleAlert,
  X,
};

export function renderIcons(root = document) {
  createIcons({ icons, attrs: { "stroke-width": 1.8 }, root });
}

function setupNavigation() {
  const toggle = document.querySelector(".nav-toggle");
  const nav = document.querySelector(".site-nav");
  if (!toggle || !nav) return;

  toggle.addEventListener("click", () => {
    const open = nav.classList.toggle("open");
    toggle.setAttribute("aria-expanded", String(open));
    toggle.innerHTML = `<i data-lucide="${open ? "x" : "menu"}" aria-hidden="true"></i>`;
    renderIcons(toggle);
  });
}

setupNavigation();
renderIcons();
