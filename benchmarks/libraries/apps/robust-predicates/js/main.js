import {
  incircle,
  incirclefast,
  insphere,
  inspherefast,
  orient2d,
  orient2dfast,
  orient3d,
  orient3dfast,
} from "robust-predicates";

let passed = 0;
if (orient2d(0, 0, 1, 0, 0, 1) === -1) passed += 1;
if (orient2dfast(0, 0, 1, 0, 0, 1) === -1) passed += 1;
if (orient3d(0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1) === -1) passed += 1;
if (orient3dfast(0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1) === -1) passed += 1;
if (incircle(0, 0, 1, 0, 0, 1, 0.5, 0.5) === 0.5) passed += 1;
if (incirclefast(0, 0, 1, 0, 0, 1, 0.5, 0.5) === 0.5) passed += 1;
if (insphere(0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0.5, 0.5, 0.5) === 0.75) passed += 1;
if (inspherefast(0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0.5, 0.5, 0.5) === 0.75) passed += 1;
console.log(`robust-predicates:${passed}`);
