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
    "addEventListener", "after", "all", "allSettled", "alt", "anchor", "animation", "any",
    "append", "appendChild", "apply", "arguments", "asIntN", "asUintN", "asin", "asinh",
    "assign", "asyncDispose", "asyncIterator", "at", "atan", "atan2", "atanh", "attributes",
    "background", "backgroundColor", "backgroundImage", "before", "big", "bind", "blink",
    "body", "bold", "border", "borderBottom", "borderBottomWidth", "borderCollapse",
    "borderColor", "borderLeft", "borderLeftWidth", "borderRight", "borderRightWidth",
    "borderSpacing", "borderStyle", "borderTop", "borderTopWidth", "borderWidth", "bottom",
    "boxSizing", "buffer", "byteLength", "byteOffset", "call", "caller", "cancelAnimationFrame",
    "captureStackTrace", "catch", "cbrt", "ceil", "charAt", "charCodeAt", "checked",
    "childNodes", "children", "classList", "className", "clear", "clientHeight", "clientLeft",
    "clientTop", "clientWidth", "cloneNode", "clz32", "codePointAt", "colSpan", "color",
    "compatMode", "compile", "concat", "console", "construct", "constructor", "contains",
    "copyWithin", "cos", "cosh", "create", "createComment", "createDocumentFragment",
    "createElement", "createElementNS", "createRange", "createTextNode", "cssText", "debug",
    "defaultView", "defineProperties", "defineProperty", "delete", "deleteProperty",
    "description", "dir", "disabled", "dispatchEvent", "display", "dispose", "document",
    "documentElement", "dotAll", "endsWith", "entries", "error", "every", "exec", "exp",
    "expm1", "fill", "filter", "finally", "find", "findIndex", "findLast", "findLastIndex",
    "firstChild", "firstElementChild", "fixed", "flags", "flat", "flatMap", "float", "floor",
    "font", "fontFamily", "fontSize", "fontStyle", "fontVariant", "fontWeight", "fontcolor",
    "fontsize", "for", "forEach", "form", "freeze", "from", "fromCharCode", "fromCodePoint",
    "fromEntries", "fround", "get", "getAttribute", "getAttributeNS", "getBigInt64",
    "getBigUint64", "getBoundingClientRect", "getCanonicalLocales", "getClientRects",
    "getComputedStyle", "getDate", "getDay", "getElementById", "getElementsByClassName",
    "getElementsByTagName", "getFloat32", "getFloat64", "getFullYear", "getHours", "getInt16",
    "getInt32", "getInt8", "getItem", "getMilliseconds", "getMinutes", "getMonth",
    "getOwnPropertyDescriptor", "getOwnPropertyDescriptors", "getOwnPropertyNames",
    "getOwnPropertySymbols", "getPropertyValue", "getPrototypeOf", "getSeconds", "getTime",
    "getTimezoneOffset", "getUTCDate", "getUTCDay", "getUTCFullYear", "getUTCHours",
    "getUTCMilliseconds", "getUTCMinutes", "getUTCMonth", "getUTCSeconds", "getUint16",
    "getUint32", "getUint8", "getYear", "grow", "growable", "has", "hasAttribute", "hasIndices",
    "hasInstance", "hasOwn", "hasOwnProperty", "hash", "head", "height", "hidden", "host",
    "hostname", "href", "hypot", "id", "ignoreCase", "imul", "includes", "indexOf", "info",
    "innerHTML", "innerText", "input", "insertAdjacentElement", "insertAdjacentHTML",
    "insertBefore", "is", "isArray", "isConcatSpreadable", "isExtensible", "isFinite",
    "isFrozen", "isInteger", "isNaN", "isPrototypeOf", "isSafeInteger", "isSealed", "isView",
    "isWellFormed", "italics", "iterator", "join", "keyFor", "keys", "lang", "lastChild",
    "lastElementChild", "lastIndexOf", "lastMatch", "lastParen", "left", "leftContext",
    "length", "letterSpacing", "lineHeight", "link", "localName", "localStorage",
    "localeCompare", "location", "log", "log10", "log1p", "log2", "map", "margin",
    "marginBottom", "marginLeft", "marginRight", "marginTop", "match", "matchAll", "max",
    "maxByteLength", "maxHeight", "maxWidth", "message", "min", "minHeight", "minWidth",
    "multiline", "name", "namespaceURI", "navigator", "nextElementSibling", "nextSibling",
    "nodeName", "nodeType", "nodeValue", "normalize", "now", "of", "offsetHeight", "offsetLeft",
    "offsetParent", "offsetTop", "offsetWidth", "opacity", "open", "outerHTML", "overflow",
    "ownKeys", "ownerDocument", "padEnd", "padStart", "padding", "paddingBottom", "paddingLeft",
    "paddingRight", "paddingTop", "parentElement", "parentNode", "parse", "parseFloat",
    "parseFromString", "parseInt", "pathname", "performance", "placeholder", "pop", "position",
    "pow", "prepareStackTrace", "prepend", "preventDefault", "preventExtensions",
    "previousElementSibling", "previousSibling", "propertyIsEnumerable", "protocol",
    "prototype", "push", "querySelector", "querySelectorAll", "race", "random", "raw",
    "readyState", "reduce", "reduceRight", "reject", "rel", "remove", "removeAttribute",
    "removeChild", "removeEventListener", "removeItem", "removeProperty", "repeat", "replace",
    "replaceAll", "replaceChild", "replaceWith", "requestAnimationFrame", "resizable", "resize",
    "resolve", "responseText", "responseType", "reverse", "revocable", "right", "rightContext",
    "round", "rowSpan", "scrollHeight", "scrollLeft", "scrollTop", "scrollWidth", "seal",
    "search", "selected", "send", "serializeToString", "sessionStorage", "set", "setAttribute",
    "setAttributeNS", "setBigInt64", "setBigUint64", "setDate", "setFloat32", "setFloat64",
    "setFullYear", "setHours", "setInt16", "setInt32", "setInt8", "setItem", "setMilliseconds",
    "setMinutes", "setMonth", "setProperty", "setPrototypeOf", "setRequestHeader", "setSeconds",
    "setTime", "setUTCDate", "setUTCFullYear", "setUTCHours", "setUTCMilliseconds",
    "setUTCMinutes", "setUTCMonth", "setUTCSeconds", "setUint16", "setUint32", "setUint8",
    "setYear", "shift", "sign", "sin", "sinh", "size", "slice", "small", "some", "sort",
    "source", "species", "splice", "split", "sqrt", "src", "stackTraceLimit", "startsWith",
    "status", "statusText", "sticky", "stopPropagation", "strike", "stringify", "style", "sub",
    "subarray", "substr", "substring", "sup", "supportedValuesOf", "tabIndex", "tagName", "tan",
    "tanh", "target", "test", "textAlign", "textContent", "textDecoration", "textIndent",
    "textTransform", "then", "title", "toDateString", "toExponential", "toFixed", "toGMTString",
    "toISOString", "toJSON", "toLocaleDateString", "toLocaleLowerCase", "toLocaleString",
    "toLocaleTimeString", "toLocaleUpperCase", "toLowerCase", "toPrecision", "toPrimitive",
    "toReversed", "toSorted", "toSpliced", "toString", "toStringTag", "toTimeString",
    "toUTCString", "toUpperCase", "toWellFormed", "toggleAttribute", "top", "trace",
    "transform", "transformOrigin", "transition", "trim", "trimEnd", "trimLeft", "trimRight",
    "trimStart", "trunc", "type", "unicode", "unicodeSets", "unscopables", "unshift",
    "userAgent", "value", "valueOf", "values", "verticalAlign", "visibility", "warn",
    "whiteSpace", "width", "window", "with", "wordSpacing", "zIndex",
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
