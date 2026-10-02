//! Versioned native language/ABI inventory. Target checking is authoritative
//! for individual recipes; an implemented family does not promise every host
//! representation. Qualification and explicit JavaScript host boundaries are
//! documented in docs/native-support.md and the N2 acceptance record.
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status { Implemented, Partial, Pending, JavaScriptOnly }
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Capability {
    pub id: &'static str,
    pub status: Status,
    pub boundary: &'static str,
    pub remaining_owner: Option<&'static str>,
}

pub const CAPABILITIES: &[Capability] = &[
    Capability { id: "scalars", status: Status::Implemented, boundary: "int32 wrapping arithmetic, strict binary64, bool, UTF-16 strings, null and scalar unions", remaining_owner: None },
    Capability { id: "shared-optimization", status: Status::Implemented, boundary: "shared folding, liveness, inlining, scalar replacement, final-use transfers and safe exception-check omission; configurable borrowed closures and closed generic function storage", remaining_owner: None },
    Capability { id: "functions", status: Status::Implemented, boundary: "direct and indirect calls, captured declarations, generic tagged values, per-argument absence/defaults and managed/callable references under the shared nonescaping contract; polymorphic callable containers and nested signature bridges", remaining_owner: None },
    Capability { id: "value-structs", status: Status::Implemented, boundary: "managed C products, generic tagged fields, nullable/union snapshots, collection payloads and logical field writeback; nested callable transport implemented", remaining_owner: None },
    Capability { id: "collections", status: Status::Implemented, boundary: "shared-identity concrete/generic arrays, owned higher-order callbacks, sparse copies, class objects, maps, sets, symbols, buffers and typed arrays; recipe checking determines supported element/call combinations", remaining_owner: None },
    Capability { id: "records-and-shapes", status: Status::Implemented, boundary: "traced string-keyed records, Object keys/values/hasOwn/assign, scalar/array/record JSON output including floats, owned JSON parsing with typed representation views, declared shape storage/spreads/optional fields/tag tests; callable transport and configurable immutable static graphs implemented", remaining_owner: None },
    Capability { id: "exceptions", status: Status::Implemented, boundary: "owned status propagation through calls and callbacks, catch/rethrow, finally completion overrides and checked runtime failures and allocation-witness views, explicit provider pending/take/raise", remaining_owner: None },
    Capability { id: "suspension", status: Status::Implemented, boundary: "owned generator and async region frames, call-time defaults, lazy generator bodies, delegation and iterator completion; task adoption, typed continuations and owned FIFO microtasks implemented", remaining_owner: None },
    Capability { id: "regex", status: Status::Implemented, boundary: "pinned QuickJS libregexp, typed construction/test/search/replacement, metadata and shared lastIndex, configurable stack/work limits; JsValue match objects remain a JavaScript host boundary", remaining_owner: None },
    Capability { id: "strings", status: Status::Implemented, boundary: "ABI v2 reference-counted UTF-16 with owned slices and host copy/return operations; Unicode 17 default case conversion including contextual sigma", remaining_owner: None },
    Capability { id: "host-abi", status: Status::Implemented, boundary: "explicit host_ function providers; owned C library exports, namespaced headers, initialization/drain/shutdown, same-thread callbacks, task/generator/container handles and typed argv/env/streams/files/clock/exit providers", remaining_owner: None },
    Capability { id: "memory", status: Status::Implemented, boundary: "reference counts, managed product/string/closure/object ownership, final-use transfers and synchronous candidate trial-deletion collection", remaining_owner: None },
    Capability { id: "toolchain", status: Status::Implemented, boundary: "one strict C11 driver, executable/shared-library/object artifacts and input/output receipts; explicit speed/size/balanced, sanitizers, cross triples and WASI stack profiles", remaining_owner: None },
    Capability { id: "javascript-host", status: Status::JavaScriptOnly, boundary: "unknown, host-specific JsValue operations, JS namespace (except checked representation views), extern JS classes, object literals, JavaScript module namespaces/dynamic import and ambient JS APIs", remaining_owner: None },
];
pub fn receipt() -> serde_json::Value {
    serde_json::json!({"schema":1,"callback_abi":3,"string_abi":2,"capabilities":CAPABILITIES,
        "check":"--check --target native uses the same target admission as compilation"})
}
