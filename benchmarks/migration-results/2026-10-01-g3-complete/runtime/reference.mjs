// Independent source behavior for the repeated receiver workload.
export class Box {
  constructor(n) { this.amount = n; }
  total() {
    return this.amount+this.amount+this.amount+this.amount+this.amount+this.amount+
      this.amount+this.amount+this.amount+this.amount+this.amount+this.amount;
  }
}
