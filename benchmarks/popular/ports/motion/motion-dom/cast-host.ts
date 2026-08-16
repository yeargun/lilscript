/**
 * Bridges values that cross the untyped `JsValue` boundary back to their
 * declared port type. LilScript dispatches class methods statically, so a
 * port object arriving as `JsValue` must be re-typed before its methods can
 * be called; `callMethod*` only works on host objects.
 */
export function identity(value: any): any {
  return value === undefined ? null : value
}
