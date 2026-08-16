// Keep Playwright resolution owned by the repository's browser benchmark
// workspace. Other labs import this module instead of installing duplicate
// browser drivers or reaching into this workspace's node_modules directory.
export { chromium } from "playwright";
