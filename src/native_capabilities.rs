//! Versioned native language/ABI inventory. Target checking is authoritative
//! for individual recipes; an implemented family does not promise every host
//! representation. N2 entries are commitments, not JavaScript-only exclusions.
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
pub const ASYNC: &str = "native tasks and async functions are not implemented yet (N2)";
pub const GENERATORS: &str = "native generators are not implemented yet (N2)";

pub const CAPABILITIES: &[Capability] = &[
    Capability { id: "scalars", status: Status::Implemented, boundary: "int32 wrapping arithmetic, strict binary64, bool, UTF-16 strings, null and scalar unions", remaining_owner: None },
    Capability { id: "shared-optimization", status: Status::Implemented, boundary: "shared folding, liveness, inlining, scalar replacement and final-use ownership transfers under the requested tactic permissions", remaining_owner: None },
    Capability { id: "functions", status: Status::Partial, boundary: "direct and indirect calls, captured declarations, generic tagged values, per-argument absence/defaults and managed/callable references under the shared nonescaping contract; polymorphic callable containers and nested signature bridges; full qualification remains", remaining_owner: Some("N2") },
    Capability { id: "value-structs", status: Status::Partial, boundary: "managed C products, generic tagged fields, nullable/union snapshots, collection payloads and logical field writeback; nested callable transport implemented; native specialization and full qualification remain", remaining_owner: Some("N2") },
    Capability { id: "collections", status: Status::Partial, boundary: "shared-identity concrete/generic arrays, owned higher-order callbacks, sparse copies, class objects, maps, sets, symbols, buffers and typed arrays; recipe checking determines supported element/call combinations", remaining_owner: Some("N2") },
    Capability { id: "records-and-shapes", status: Status::Partial, boundary: "traced string-keyed records, Object keys/values/hasOwn/assign, scalar/array/record JSON output including floats, owned JSON parsing with typed representation views, declared shape storage/spreads/optional fields/tag tests; callable transport implemented; immutable graph qualification remains", remaining_owner: Some("N2") },
    Capability { id: "exceptions", status: Status::Partial, boundary: "owned status propagation through calls and callbacks, catch/rethrow, finally completion overrides and checked runtime failures and allocation-witness views, explicit provider pending/take/raise; remaining runtime recipes and complete corpus qualification remain", remaining_owner: Some("N2") },
    Capability { id: "suspension", status: Status::Pending, boundary: "generator and async state machines, task ownership and microtask queue", remaining_owner: Some("N2") },
    Capability { id: "regex", status: Status::Partial, boundary: "pinned QuickJS libregexp, typed construction/test/search/replacement, metadata and shared lastIndex, configurable stack/work limits; JsValue match objects remain a JavaScript host boundary", remaining_owner: Some("N2") },
    Capability { id: "strings", status: Status::Partial, boundary: "ABI v2 reference-counted UTF-16 with owned slices and host copy/return operations; Unicode 17 default case conversion including contextual sigma; broad runtime qualification remains", remaining_owner: Some("N2") },
    Capability { id: "host-abi", status: Status::Partial, boundary: "explicit host_ function providers; synchronous same-thread callbacks, borrowed inputs and owned returned handles; C library exports and portable process/file/clock API remain", remaining_owner: Some("N2") },
    Capability { id: "memory", status: Status::Partial, boundary: "reference counts, managed product/string/closure/object ownership, final-use transfers and synchronous candidate trial-deletion collection; full native corpus qualification remains", remaining_owner: Some("N2") },
    Capability { id: "toolchain", status: Status::Partial, boundary: "one strict C11 driver and input/artifact receipts; performance, sanitizer matrix and cross-target qualification remain", remaining_owner: Some("N2") },
    Capability { id: "javascript-host", status: Status::JavaScriptOnly, boundary: "unknown, host-specific JsValue operations, JS namespace (except checked representation views), extern JS classes, object literals, JavaScript module namespaces/dynamic import and ambient JS APIs", remaining_owner: None },
];
pub fn receipt() -> serde_json::Value {
    serde_json::json!({"schema":1,"callback_abi":3,"string_abi":2,"capabilities":CAPABILITIES,
        "check":"--check --target native uses the same target admission as compilation"})
}
