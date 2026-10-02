//! Native retained allocations, ownership and synchronous cycle collection. These are target
//! runtime recipes, not a semantic value representation or compiler allocator.
//! The emitter admits each byte before writing it and emits this support only
//! when a managed closure/cell or an explicit callback interface requires it.

pub(super) const CALLBACK_ABI_VERSION: u32 = 2;

/// Public portion of the callback ABI. A host receives a borrowed callable;
/// retaining its environment acquires an owner which must later be released.
/// The target emits signature-specific callable records and typed wrappers
/// around these operations in the same generated header. Calls/retains/releases
/// must remain on the originating thread; cross-thread or asynchronous use is
/// outside version 2. A synchronous host may retain between calls and reenter.
pub(super) const INTERFACE: &str = include_str!("runtime/memory.h");

/// One runtime owner for retained allocations. Generated trace functions expose
/// each owned child once; borrowed pointers are never graph edges.
/// The header is the first member of each generated typed allocation.
pub(super) const IMPLEMENTATION: &str = include_str!("runtime/memory.c");

/// Used only by an explicitly requested qualification interface. These are
/// observations of the real allocator, not a separate ownership simulator.
pub(super) const QUALIFICATION_INTERFACE: &str = include_str!("runtime/qualification.h");

pub(super) const QUALIFICATION_IMPLEMENTATION: &str = include_str!("runtime/qualification.c");
