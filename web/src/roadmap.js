import catalog from "./benchmark-catalog.json";
import "./site.js";

const counts = Object.fromEntries([...new Set(catalog.projects.map((project) => project.category))].map((category) => [category, catalog.projects.filter((project) => project.category === category).length]));
document.querySelector("[data-roadmap-summary]").textContent = `Current catalog: ${catalog.metadata.projectCount} projects and ${catalog.metadata.artifactCount} checked artifact lanes, including ${counts["real-app"]} application scenarios, ${counts["complete-library"]} complete selected npm APIs, ${counts["popular-library"]} popular-package audits, and ${counts["generated-pair"]} mechanically paired compiler cases. Generated with Vite ${catalog.metadata.versions.vite} and Closure Compiler ${catalog.metadata.versions.closure}.`;
