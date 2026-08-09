import "./site.js";

const tabs = [...document.querySelectorAll("[data-strategy]")];
const panels = [...document.querySelectorAll("[data-strategy-panel]")];

function selectStrategy(strategy, focus = false) {
  for (const tab of tabs) {
    const selected = tab.dataset.strategy === strategy;
    tab.classList.toggle("active", selected);
    tab.setAttribute("aria-selected", String(selected));
    tab.tabIndex = selected ? 0 : -1;
    if (selected && focus) tab.focus();
  }
  for (const panel of panels) panel.hidden = panel.dataset.strategyPanel !== strategy;
}

tabs.forEach((tab, index) => {
  tab.addEventListener("click", () => selectStrategy(tab.dataset.strategy));
  tab.addEventListener("keydown", (event) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const next = event.key === "Home" ? 0
      : event.key === "End" ? tabs.length - 1
        : (index + (event.key === "ArrowRight" ? 1 : -1) + tabs.length) % tabs.length;
    selectStrategy(tabs[next].dataset.strategy, true);
  });
});

selectStrategy("static");
