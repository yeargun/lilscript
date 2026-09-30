# Q2 bounded reach storage: screening result

Source `f2133d6f2def444681269ab626155336b4817e90`; [binary identities](identity.json). This is a limited development screen, not a performance qualification. The [CPU report](compile-cost.json) records one alternating pair after warm-up for Marked under each objective at frozen effort 13, compared with the qualified objective-set pin.

Result lists reserve their proven node bounds once, and capture references are summarized per binding. All four independent traversal, ownership and refusal tests pass. Full library, generic and port qualification were not run for this intermediate pin; a further audit found eight remaining tree-only consumers whose temporary result reservations should end when their buffers are dropped.

| Workload | Candidate / previous CPU | Previous peak accounted bytes | Candidate peak accounted bytes |
|---|---:|---:|---:|
| markedlil-raw | 1.085 | 24,381,860 | 28,341,053 |
| markedlil-gzip | 1.053 | 24,439,749 | 28,398,942 |
| markedlil-brotli | 1.021 | 25,935,776 | 28,529,963 |

All three complete outputs, search receipts, input identities and exact-codec work counts match. Both implementations release all accounted storage after handoff. The screening medians improve on the first experiment's overhead but remain slower than the qualified baseline. One pair cannot support a stable percentage improvement claim. Accounted storage now includes previously unadmitted backing and cannot be interpreted as a process-memory comparison.

No size baseline changes. The complete consumer audit follows separately. Logs: `/tmp/lilscript-q2-reach-bounded-{focused,release,cpu}.log`.
