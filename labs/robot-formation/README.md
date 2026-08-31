# Minimum robust bearing arrows

`bearing-rigidity.mjs` selects directed measurements for a known 2D robot
formation. An arrow `i -> j` measures the global-frame unit bearing

`g_ij = (p_j - p_i) / ||p_j - p_i||`.

The bearings determine shape up to translation and positive scale. Rotation is
observable because bearings use a shared global frame. If arrows instead measure
distance, full relative displacement, or body-frame bearings, this is a different
problem and this API deliberately does not pretend otherwise.

For `n >= 2`, the observable shape space has rank `2n - 3`. Each 2D bearing adds
at most one independent perpendicular constraint, so every recoverable set needs
at least `2n - 3` arrows. The selector constructs a basis of exactly that size;
therefore its cardinality is globally minimal. Collinear, coincident, disconnected,
and otherwise rank-deficient inputs return a structured non-recoverable result.

Among minimum sets, selection maximizes the smallest eigenvalue of the reduced,
unit-RMS-shape weighted bearing Fisher-information matrix. This dimensionless,
scale-invariant E-optimal criterion minimizes
worst-direction amplification of small independent bearing errors. The result also
reports condition number, covariance trace, and log determinant. Candidate weights
are inverse bearing-noise variances.

Small candidate spaces are enumerated exactly. Larger spaces start from a
rank-revealing QR pivot set and apply deterministic one-arrow exchanges until no
exchange improves robustness or the configured pass cap is reached. The result
labels which guarantee was achieved.

`reconstructShape` uses the selected arrows' nominal `distance` values to weight
angular residuals consistently. Callers that omit distance get unit-distance line
residual weighting. Reconstruction rejects underdetermined systems and projected
solutions that reverse any directed measurement; it does not silently return an
arbitrary null-space vector.

```js
import {
  analyzeBearingArrows,
  reconstructShape,
  selectBearingArrows,
} from "./bearing-rigidity.mjs";

const positions = [[0, 0], [3, 0], [4, 2], [1, 4], [-1, 2]];
const design = selectBearingArrows(positions);

console.log(design.arrowCount);           // 2*n - 3
console.log(design.minimumProven);         // true
console.log(design.robustness.lambdaMin);  // worst-direction information

const check = analyzeBearingArrows(positions, design.arrows);
const recovered = reconstructShape(positions.length, design.arrows);
```

Run the test suite with:

```sh
node --test labs/robot-formation/bearing-rigidity.test.mjs
```
