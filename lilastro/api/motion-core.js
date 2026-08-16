// Public API slice implemented by benchmarks/popular/ports/motion/entry.lil.
// Keep this list explicit so the open-world comparison cannot gain or lose
// bindings through a package-level wildcard export.
export {
  clamp,
  distance,
  distance2D,
  getOriginIndex,
  mix,
  mixNumber,
  spring,
  stagger,
  wrap,
} from "motion";
