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
  ["Demos", "/demos.html"],
  ["Language", "/docs.html"],
  ["Projects", "/lilastro.html"],
  ["Evidence", "/benchmarks.html"],
  ["Playground", "/playground.html"],
  ["About", "/about.html"],
];

function navigationSection(pathname) {
  if (pathname === "/" || pathname.endsWith("/index.html")) return "/";
  if (pathname.endsWith("/demos.html") || pathname.endsWith("/marketplace.html")) return "/demos.html";
  if (pathname.endsWith("/docs.html")) return "/docs.html";
  if (["/lilastro.html", "/lastro.html", "/solidlil.html", "/delivery.html"].some((path) => pathname.endsWith(path))) {
    return "/lilastro.html";
  }
  if (["/benchmarks.html", "/explorer.html", "/libraries.html", "/benchmark-detail.html", "/roadmap.html"].some((path) => pathname.endsWith(path))) {
    return "/benchmarks.html";
  }
  if (pathname.endsWith("/playground.html")) return "/playground.html";
  if (pathname.endsWith("/about.html")) return "/about.html";
  return "";
}

function setupGlobalNavigation() {
  const nav = document.querySelector(".site-nav");
  if (!nav) return;
  const active = navigationSection(window.location.pathname);
  nav.innerHTML = navigation
    .map(([label, href]) => `<a${active === href ? ' class="active" aria-current="page"' : ""} href="${href}">${label}</a>`)
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
