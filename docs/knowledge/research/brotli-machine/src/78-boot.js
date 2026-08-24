/* Start everything once the document is parsed. */
(function (BM) {
  "use strict";
  function boot() {
    const U = BM.ui;
    try {
      U.initSections();
      U.initRest();
      U.initMachine();
    } catch (e) {
      const banner = document.createElement("div");
      banner.style.cssText = "background:#3a1f1c;color:#f0c4bc;padding:14px 18px;font:13px/1.5 monospace;white-space:pre-wrap";
      banner.textContent = "The page failed to start:\n" + (e.stack || e.message);
      document.body.insertBefore(banner, document.body.firstChild);
      throw e;
    }
    /* highlight the section in view */
    const links = new Map(BM.ui.$$("nav.toc a").map((a) => [a.getAttribute("href").slice(1), a]));
    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        for (const a of links.values()) a.classList.remove("active");
        const link = links.get(entry.target.id);
        if (link) link.classList.add("active");
      }
    }, { rootMargin: "-10% 0px -80% 0px" });
    for (const id of links.keys()) {
      const node = document.getElementById(id);
      if (node) observer.observe(node);
    }
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", boot);
  else boot();
})(globalThis.BM || (globalThis.BM = {}));
