//! The property names a program does not own.
//!
//! Closure's ADVANCED mode renames every property that is not in an externs
//! file. LilScript derives most of that surface instead of asking for it --
//! host field reads and writes, host method calls, extern aggregate fields and
//! anything an export can observe are all visible in the IR. One part cannot be
//! derived: a member reached through `JS.invoke` or `JS.getProperty` on an
//! untyped `JsValue` is indistinguishable from a member the program invented,
//! because on an untyped value there is nothing to distinguish them by.
//! `value.toUpperCase()` and `node.measuredDepth` are the same shape.
//!
//! So the standard library and DOM surface is written down here. A name in this
//! list is never renamed, whatever the ownership policy says. The cost of a
//! name that belongs here and is missing is a wrong program; the cost of a name
//! that does not belong and is present is a few bytes. The list is therefore
//! generous on purpose, and it is the ECMAScript surface as the engine reports
//! it plus the DOM members a browser port touches.
//!
//! Audited against the protocols the engine reads *out of* an object the program
//! builds, which is where the gaps were. `internal_properties = "all"` on zodlil
//! emitted `Object.defineProperty(a,b,{value:c,c:!0,b:!0,a:!0})` --
//! `writable`, `enumerable` and `configurable` were absent, and so were `next`
//! and `done`, which is how `for...of` talks to a hand-rolled iterator. A method
//! name is easy to remember; a key the engine looks up on a plain object handed
//! to it is the part that gets forgotten. Property descriptors, the iteration
//! protocol, RegExp instance state and `exec` results, `Error.cause` and
//! `.stack`, and Intl option bags are all that shape.
//!
//! The SVG attribute surface is here for the same reason and was found the same
//! way: katexlil renders `viewBox="0 0 400000 522"` out of a property bag, and
//! with the bag's keys renamed it emitted `Sa="0 0 400000 522"`. A browser
//! reads `viewBox` and `preserveAspectRatio` off an element; they are no more
//! ours than `appendChild` is. Single-character SVG attributes (`d`, `x`, `y`,
//! `r`) are free to keep -- renaming a one-character key saves nothing.

/// Sorted for binary search. Keep it sorted.
const NOT_OURS: &[&str] = &[
    "BYTES_PER_ELEMENT", "Collator", "Comment", "CustomEvent", "DOMParser", "DateTimeFormat",
    "DisplayNames", "Document", "DocumentFragment", "E", "EPSILON", "Element", "Event",
    "EventTarget", "HTMLElement", "LN10", "LN2", "LOG10E", "LOG2E", "ListFormat", "Locale",
    "MAX_SAFE_INTEGER", "MAX_VALUE", "MIN_SAFE_INTEGER", "MIN_VALUE", "MathMLElement",
    "NEGATIVE_INFINITY", "NaN", "Node", "NumberFormat", "PI", "POSITIVE_INFINITY",
    "PluralRules", "RelativeTimeFormat", "SQRT1_2", "SQRT2", "SVGElement", "Segmenter", "Text",
    "UTC", "XMLHttpRequest", "XMLSerializer", "__defineGetter__", "__defineSetter__",
    "__lookupGetter__", "__lookupSetter__", "__proto__", "abs", "acos", "acosh", "add",
    "addEventListener", "after", "alignmentBaseline", "all", "allSettled", "alt", "anchor",
    "animation", "any", "append", "appendChild", "apply", "arguments", "asIntN", "asUintN",
    "asin", "asinh", "assign", "asyncDispose", "asyncIterator", "at", "atan", "atan2", "atanh",
    "attributes", "background", "backgroundColor", "backgroundImage", "baselineShift", "before",
    "big", "bind", "blink", "body", "bold", "border", "borderBottom", "borderBottomWidth",
    "borderCollapse", "borderColor", "borderLeft", "borderLeftWidth", "borderRight",
    "borderRightWidth", "borderSpacing", "borderStyle", "borderTop", "borderTopWidth",
    "borderWidth", "bottom", "boxSizing", "buffer", "byteLength", "byteOffset", "call",
    "caller", "cancelAnimationFrame", "captureStackTrace", "catch", "cause", "cbrt", "ceil",
    "charAt", "charCodeAt", "checked", "childNodes", "children", "classList", "className",
    "clear", "clientHeight", "clientLeft", "clientTop", "clientWidth", "clipPath", "clipRule",
    "cloneNode", "clz32", "codePointAt", "colSpan", "color", "colorInterpolation",
    "colorInterpolationFilters", "columnNumber", "compatMode", "compile", "concat",
    "configurable", "console", "construct", "constructor", "contains", "copyWithin", "cos",
    "cosh", "create", "createComment", "createDocumentFragment", "createElement",
    "createElementNS", "createRange", "createTextNode", "cssText", "currency", "cx", "cy", "d",
    "day", "debug", "defaultView", "defineProperties", "defineProperty", "delete",
    "deleteProperty", "description", "detached", "difference", "dir", "disabled",
    "dispatchEvent", "display", "dispose", "document", "documentElement", "dominantBaseline",
    "done", "dotAll", "dx", "dy", "endsWith", "entries", "enumerable", "error", "errors",
    "every", "exec", "exp", "expm1", "fileName", "fill", "fillOpacity", "fillRule", "filter",
    "filterUnits", "finally", "find", "findIndex", "findLast", "findLastIndex", "firstChild",
    "firstElementChild", "fixed", "flags", "flat", "flatMap", "float", "floodColor",
    "floodOpacity", "floor", "font", "fontFamily", "fontSize", "fontStyle", "fontVariant",
    "fontWeight", "fontcolor", "fontsize", "for", "forEach", "form", "freeze", "from",
    "fromCharCode", "fromCodePoint", "fromEntries", "fround", "get", "getAttribute",
    "getAttributeNS", "getBigInt64", "getBigUint64", "getBoundingClientRect",
    "getCanonicalLocales", "getClientRects", "getComputedStyle", "getDate", "getDay",
    "getElementById", "getElementsByClassName", "getElementsByTagName", "getFloat32",
    "getFloat64", "getFullYear", "getHours", "getInt16", "getInt32", "getInt8", "getItem",
    "getMilliseconds", "getMinutes", "getMonth", "getOrInsert", "getOwnPropertyDescriptor",
    "getOwnPropertyDescriptors", "getOwnPropertyNames", "getOwnPropertySymbols",
    "getPropertyValue", "getPrototypeOf", "getSeconds", "getTime", "getTimezoneOffset",
    "getUTCDate", "getUTCDay", "getUTCFullYear", "getUTCHours", "getUTCMilliseconds",
    "getUTCMinutes", "getUTCMonth", "getUTCSeconds", "getUint16", "getUint32", "getUint8",
    "getYear", "global", "gradientTransform", "gradientUnits", "groups", "grow", "growable",
    "has", "hasAttribute", "hasIndices", "hasInstance", "hasOwn", "hasOwnProperty", "hash",
    "head", "height", "hidden", "host", "hostname", "hour", "href", "hypot", "id", "ignoreCase",
    "imageRendering", "imul", "in2", "includes", "index", "indexOf", "indices", "info",
    "innerHTML", "innerText", "input", "insertAdjacentElement", "insertAdjacentHTML",
    "insertBefore", "intersection", "is", "isArray", "isConcatSpreadable", "isDisjointFrom",
    "isExtensible", "isFinite", "isFrozen", "isInteger", "isNaN", "isPrototypeOf",
    "isSafeInteger", "isSealed", "isSubsetOf", "isSupersetOf", "isView", "isWellFormed",
    "italics", "iterator", "join", "keyFor", "keys", "lang", "lastChild", "lastElementChild",
    "lastIndex", "lastIndexOf", "lastMatch", "lastParen", "left", "leftContext", "length",
    "lengthAdjust", "letterSpacing", "lightingColor", "lineHeight", "lineNumber", "link",
    "localName", "localStorage", "locale", "localeCompare", "location", "log", "log10", "log1p",
    "log2", "map", "margin", "marginBottom", "marginLeft", "marginRight", "marginTop",
    "markerEnd", "markerHeight", "markerMid", "markerStart", "markerWidth", "mask",
    "maskContentUnits", "maskUnits", "match", "matchAll", "max", "maxByteLength", "maxHeight",
    "maxWidth", "message", "min", "minHeight", "minWidth", "minute", "month", "multiline",
    "name", "namespaceURI", "navigator", "next", "nextElementSibling", "nextSibling",
    "nodeName", "nodeType", "nodeValue", "normalize", "now", "numeric", "of", "offset",
    "offsetHeight", "offsetLeft", "offsetParent", "offsetTop", "offsetWidth", "opacity", "open",
    "orient", "outerHTML", "overflow", "ownKeys", "ownerDocument", "padEnd", "padStart",
    "padding", "paddingBottom", "paddingLeft", "paddingRight", "paddingTop", "parentElement",
    "parentNode", "parse", "parseFloat", "parseFromString", "parseInt", "pathLength",
    "pathname", "patternContentUnits", "patternTransform", "patternUnits", "performance",
    "placeholder", "points", "pop", "position", "pow", "prepareStackTrace", "prepend",
    "preserveAspectRatio", "preventDefault", "preventExtensions", "previousElementSibling",
    "previousSibling", "primitiveUnits", "propertyIsEnumerable", "protocol", "prototype",
    "push", "querySelector", "querySelectorAll", "r", "race", "random", "raw", "readyState",
    "reason", "reduce", "reduceRight", "refX", "refY", "reject", "rel", "remove",
    "removeAttribute", "removeChild", "removeEventListener", "removeItem", "removeProperty",
    "repeat", "replace", "replaceAll", "replaceChild", "replaceWith", "requestAnimationFrame",
    "requiredExtensions", "resizable", "resize", "resolve", "responseText", "responseType",
    "result", "return", "reverse", "revocable", "right", "rightContext", "rotate", "round",
    "rowSpan", "rx", "ry", "scrollHeight", "scrollLeft", "scrollTop", "scrollWidth", "seal",
    "search", "second", "selected", "send", "serializeToString", "sessionStorage", "set",
    "setAttribute", "setAttributeNS", "setBigInt64", "setBigUint64", "setDate", "setFloat32",
    "setFloat64", "setFullYear", "setHours", "setInt16", "setInt32", "setInt8", "setItem",
    "setMilliseconds", "setMinutes", "setMonth", "setProperty", "setPrototypeOf",
    "setRequestHeader", "setSeconds", "setTime", "setUTCDate", "setUTCFullYear", "setUTCHours",
    "setUTCMilliseconds", "setUTCMinutes", "setUTCMonth", "setUTCSeconds", "setUint16",
    "setUint32", "setUint8", "setYear", "shapeRendering", "shift", "sign", "sin", "sinh",
    "size", "slice", "small", "some", "sort", "source", "species", "splice", "split",
    "spreadMethod", "sqrt", "src", "stack", "stackTraceLimit", "startsWith", "status",
    "statusText", "stdDeviation", "sticky", "stopColor", "stopOpacity", "stopPropagation",
    "strike", "stringify", "stroke", "strokeDasharray", "strokeDashoffset", "strokeLinecap",
    "strokeLinejoin", "strokeMiterlimit", "strokeOpacity", "strokeWidth", "style", "sub",
    "subarray", "substr", "substring", "sup", "supportedValuesOf", "symmetricDifference",
    "systemLanguage", "tabIndex", "tagName", "tan", "tanh", "target", "test", "textAlign",
    "textAnchor", "textContent", "textDecoration", "textIndent", "textLength", "textRendering",
    "textTransform", "then", "throw", "timeZone", "title", "toDateString", "toExponential",
    "toFixed", "toGMTString", "toISOString", "toJSON", "toLocaleDateString",
    "toLocaleLowerCase", "toLocaleString", "toLocaleTimeString", "toLocaleUpperCase",
    "toLowerCase", "toPrecision", "toPrimitive", "toReversed", "toSorted", "toSpliced",
    "toString", "toStringTag", "toTimeString", "toUTCString", "toUpperCase", "toWellFormed",
    "toggleAttribute", "top", "trace", "transfer", "transform", "transformOrigin", "transition",
    "trim", "trimEnd", "trimLeft", "trimRight", "trimStart", "trunc", "type", "unicode",
    "unicodeBidi", "unicodeSets", "union", "unit", "unscopables", "unshift", "userAgent",
    "value", "valueOf", "values", "vectorEffect", "verticalAlign", "viewBox", "visibility",
    "warn", "whiteSpace", "width", "window", "with", "withResolvers", "wordSpacing", "writable",
    "writingMode", "x", "x1", "x2", "xmlSpace", "xmlns", "xmlnsXlink", "y", "y1", "y2", "year",
    "zIndex",
];

/// Whether `name` belongs to the standard library or the DOM, and so must keep
/// its spelling however the program is configured.
pub(crate) fn is_platform_property(name: &str) -> bool {
    NOT_OURS.binary_search(&name).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_sorted_so_the_search_is_valid() {
        assert!(NOT_OURS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn platform_members_reached_through_an_untyped_value_are_not_ours() {
        for name in [
            "toUpperCase", "charCodeAt", "hasOwnProperty", "push", "slice", "length",
            "setAttribute", "appendChild", "style", "className", "toFixed", "test",
        ] {
            assert!(is_platform_property(name), "{name} must never be renamed");
        }
    }

    #[test]
    fn names_a_program_invents_are_ours() {
        for name in [
            "measuredDepth", "nodeKind", "childList", "maxFontSize", "rawMessage",
        ] {
            assert!(!is_platform_property(name), "{name} is not a platform member");
        }
    }
}
