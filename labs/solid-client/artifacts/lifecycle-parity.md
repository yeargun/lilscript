# SolidLil lifecycle parity

Solid and SolidLil execute identical ownership/disposal workloads; retained-heap eligibility is measured separately with repeated isolated samples.

- 5,000 root/signal/memo/effect/cleanup cycles
- 500 keyed and indexed collection cycles
- 100 resources resolved after root disposal
- stale disposer after slot reuse: pass
- SolidLil owner/effect high-water: 8/16
- all slots released and pending queue empty: pass
