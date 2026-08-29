# Design decisions

Parent: [knowledge tree](../README.md). Product intent: [mission](../mission.md).
Current implementation: [current architecture](../compilation/current-architecture.md).

These records explain durable choices. They do not define syntax, report live
status, or publish measurements. Each record states the intent, decision,
tradeoff, and refusal so an implementation change can be reviewed without
loading migration history or research logs.

1. [Contracts constrain objectives](contracts-before-objectives.md)
2. [Exact codec scores, bounded search](exact-codec-bounded-search.md)
3. [Typed proofs instead of port glue](typed-proofs-not-glue.md)
4. [Representation is private; ABI is explicit](representation-and-abi.md)
5. [Use a narrow hygienic target-JS representation](hygienic-target-js.md)
6. [Evidence before compression claims](evidence-before-claims.md)

Research and board notes can motivate a decision, but only a promoted record
here is durable architecture rationale.
