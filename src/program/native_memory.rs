//! Native-only, acyclic retained environments and cells. These are target
//! runtime recipes, not a semantic value representation or compiler allocator.
//! The emitter admits each byte before writing it and emits this support only
//! when a managed closure/cell or an explicit callback interface requires it.

pub(super) const CALLBACK_ABI_VERSION: u32 = 1;

/// Public portion of the callback ABI. A host receives a borrowed callable;
/// retaining its environment acquires an owner which must later be released.
/// The target emits signature-specific callable records and typed wrappers
/// around these operations in the same generated header. Calls/retains/releases
/// must remain on the originating thread; cross-thread or asynchronous use is
/// outside version 1. A synchronous host may retain between calls and reenter.
pub(super) const INTERFACE: &str = r#"#include <stddef.h>
#include <stdint.h>
void ls_native_retain(void *environment);
void ls_native_release(void *environment);
"#;

/// One runtime owner for all retained allocations. Destruction is bounded to
/// environment -> plain native value cells in the first slice. Captured managed
/// payloads are rejected by NativePlan; reference counting is not a cycle proof.
/// The header is the first member of each generated typed allocation.
pub(super) const IMPLEMENTATION: &str = r#"#include <stdlib.h>
typedef struct ls_native_object {
    size_t references;
    void (*destroy)(struct ls_native_object *);
} ls_native_object;

static uint64_t ls_native_identity_counter;
#ifdef LS_NATIVE_QUALIFICATION
static size_t ls_native_live_objects;
#endif

static void ls_native_resource_failure(void) {
    fputs("LilScript native runtime resource exhaustion\n", stderr);
    abort();
}

static void *ls_native_allocate(size_t bytes,
                               void (*destroy)(ls_native_object *)) {
    if (bytes < sizeof(ls_native_object)) ls_native_resource_failure();
    ls_native_object *object = malloc(bytes);
    if (!object) ls_native_resource_failure();
    object->references = 1;
    object->destroy = destroy;
#ifdef LS_NATIVE_QUALIFICATION
    if (ls_native_live_objects == SIZE_MAX) ls_native_resource_failure();
    ++ls_native_live_objects;
#endif
    return object;
}

void ls_native_retain(void *environment) {
    ls_native_object *object = environment;
    if (!object) return;
    if (object->references == SIZE_MAX) ls_native_resource_failure();
    ++object->references;
}

void ls_native_release(void *environment) {
    ls_native_object *object = environment;
    if (!object) return;
    if (--object->references != 0) return;
    if (object->destroy) object->destroy(object);
#ifdef LS_NATIVE_QUALIFICATION
    --ls_native_live_objects;
#endif
    free(object);
}

static uint64_t ls_native_fresh_identity(void) {
    if (ls_native_identity_counter == UINT64_MAX) ls_native_resource_failure();
    return ++ls_native_identity_counter;
}
"#;

/// Used only by an explicitly requested qualification interface. These are
/// observations of the real allocator, not a separate ownership simulator.
pub(super) const QUALIFICATION_INTERFACE: &str = r#"#ifdef LS_NATIVE_QUALIFICATION
size_t ls_native_owned_objects(void);
#endif
"#;

pub(super) const QUALIFICATION_IMPLEMENTATION: &str = r#"#ifdef LS_NATIVE_QUALIFICATION
size_t ls_native_owned_objects(void) {
    return ls_native_live_objects;
}
#endif
"#;
