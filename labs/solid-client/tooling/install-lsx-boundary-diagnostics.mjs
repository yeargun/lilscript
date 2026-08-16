export function installLsxBoundaryDiagnostics(window) {
  window.registerLsxBoundaryDiagnostics = (
    boundaryCleanups,
    initialBoundaryCleanups,
    suspenseContentCleanups,
    suspenseFallbackCleanups,
  ) => {
    window.__lsxBoundaryDiagnostics = () => ({
      boundaryCleanups: boundaryCleanups(),
      initialBoundaryCleanups: initialBoundaryCleanups(),
      suspenseContentCleanups: suspenseContentCleanups(),
      suspenseFallbackCleanups: suspenseFallbackCleanups(),
    });
  };
}
