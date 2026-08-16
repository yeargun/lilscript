import { sum } from "./math.ts";

export function add(left: number, right: number): number {
  return sum(left, right);
}

export function showAnswer(value: number): void {
  document.querySelector("#answer").textContent = `answer=${value}`;
}
