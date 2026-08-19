import {
  ArrowRight,
  BadgeInfo,
  Check,
  Copy,
  ExternalLink,
  GitBranch,
  Menu,
  Play,
  Search,
  TriangleAlert,
  X,
  createIcons,
} from "lucide";
import { localPath, withBase } from "./base.js";

export const icons = {
  ArrowRight,
  BadgeInfo,
  Check,
  Copy,
  ExternalLink,
  GitBranch,
  Menu,
  Play,
  Search,
  TriangleAlert,
  X,
};

export function renderIcons(root = document) {
  createIcons({ icons, attrs: { "stroke-width": 1.8 }, root });
}

const navigation = [
  ["Overview", "/"],
  ["Language", "/language.html"],
  ["Compare", "/compare.html"],
  ["Demos", "/demos.html"],
  ["Projects", "/lilastro.html"],
  ["Playground", "/playground.html"],
];

function navigationSection(pathname) {
  const path = localPath(pathname);
  if (path === "/" || path.endsWith("/index.html")) return "/";
  if (path.endsWith("/language.html") || path.endsWith("/docs.html")) return "/language.html";
  if (
    [
      "/compare.html",
      "/benchmarks.html",
      "/explorer.html",
      "/libraries.html",
      "/benchmark-detail.html",
      "/roadmap.html",
    ].some((href) => path.endsWith(href))
  ) {
    return "/compare.html";
  }
  if (path.endsWith("/demos.html") || path.endsWith("/marketplace.html")) return "/demos.html";
  if (["/lilastro.html", "/lastro.html", "/solidlil.html", "/delivery.html"].some((href) => path.endsWith(href))) {
    return "/lilastro.html";
  }
  if (path.endsWith("/playground.html")) return "/playground.html";
  return "";
}

function setupGlobalNavigation() {
  const nav = document.querySelector(".site-nav");
  if (!nav) return;
  const active = navigationSection(window.location.pathname);
  nav.innerHTML = navigation
    .map(([label, href]) => `<a${active === href ? ' class="active" aria-current="page"' : ""} href="${withBase(href)}">${label}</a>`)
    .join("");
  document.querySelector(".wordmark")?.setAttribute("aria-label", "LilScript home");
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

setupGlobalNavigation();
setupNavigation();
renderIcons();
