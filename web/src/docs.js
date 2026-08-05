import { renderIcons } from "./site.js";

const search = document.querySelector("#docs-search");
const links = [...document.querySelectorAll("#docs-nav a")];
const sections = [...document.querySelectorAll("#docs-content > section")];

for (const block of document.querySelectorAll(".docs-content pre")) {
  const button = document.createElement("button");
  button.className = "copy-button icon-button";
  button.type = "button";
  button.title = "Copy code";
  button.setAttribute("aria-label", "Copy code");
  button.innerHTML = '<i data-lucide="copy" aria-hidden="true"></i>';
  button.addEventListener("click", async () => {
    await navigator.clipboard.writeText(block.textContent);
    button.innerHTML = '<i data-lucide="check" aria-hidden="true"></i>';
    button.classList.add("copied");
    renderIcons(button);
    window.setTimeout(() => {
      button.innerHTML = '<i data-lucide="copy" aria-hidden="true"></i>';
      button.classList.remove("copied");
      renderIcons(button);
    }, 1400);
  });
  block.append(button);
}

search.addEventListener("input", () => {
  const query = search.value.trim().toLowerCase();
  for (const link of links) {
    const section = document.querySelector(link.hash);
    const searchable = `${link.textContent} ${section?.dataset.title ?? ""}`.toLowerCase();
    link.hidden = query !== "" && !searchable.includes(query);
  }
});

const visibleSections = new IntersectionObserver(
  (entries) => {
    const visible = entries
      .filter((entry) => entry.isIntersecting)
      .sort((left, right) => left.boundingClientRect.top - right.boundingClientRect.top);
    if (visible.length === 0) return;
    for (const link of links) link.classList.toggle("current", link.hash === `#${visible[0].target.id}`);
  },
  { rootMargin: "-18% 0px -70%", threshold: 0 },
);

for (const section of sections) visibleSections.observe(section);
renderIcons();
