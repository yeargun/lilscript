/*! @itslil/unified 11.0.6 | LilScript reimplementation of unified | MIT */

var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

// unified.esm.js
var unified_esm_exports = {};
__export(unified_esm_exports, {
  unified: () => me
});
module.exports = __toCommonJS(unified_esm_exports);
var re = Array.prototype.splice;
function ne(e2) {
  return e2 != null && !("object" != typeof e2 && "function" != typeof e2) && !!Error.prototype.isPrototypeOf(e2);
}
function ie(e2) {
  return "string" == typeof e2 || _(e2);
}
function _(e2) {
  return !!e2 && "object" == typeof e2 && "byteLength" in e2 && "byteOffset" in e2;
}
function c(e2) {
  throw new Error(e2);
}
function i(e2) {
  throw new TypeError(e2);
}
function z(e2) {
  if (e2) throw e2;
}
function b(e2) {
  if ("object" != typeof e2 || e2 == null) return false;
  var r2 = Object.getPrototypeOf(e2), t2 = r2 == null || r2 === Object.prototype || Object.getPrototypeOf(r2) == null;
  return !!t2 && !(Symbol.toStringTag in e2) && !(Symbol.iterator in e2);
}
function S(e2) {
  if (!e2) return false;
  if ("[object Object]" != q.call(e2) + "") return false;
  var t2 = !!a.call(e2, "constructor"), r2 = e2.constructor, n2 = r2 && r2.prototype && a.call(r2.prototype, "isPrototypeOf");
  if (r2 && !t2 && !n2) return false;
  n2 = "", t2 = false;
  for (r2 in e2) n2 = r2, t2 = true;
  return !t2 || !!a.call(e2, n2);
}
function T(e2, r2, t2) {
  if ("__proto__" == r2) {
    Object.defineProperty(e2, "__proto__", { enumerable: true, configurable: true, writable: true, value: t2 });
    return;
  }
  e2[r2] = t2;
}
function R(e2, r2) {
  return "__proto__" == r2 ? a.call(e2, r2) ? Object.getOwnPropertyDescriptor(e2, r2).value : void 0 : e2[r2];
}
function l(e2, r2) {
  (e2 == null || "object" != typeof e2 && "function" != typeof e2) && (e2 = {});
  if (r2 == null) return e2;
  for (var i2 in r2) {
    var n2 = R(e2, i2), t2 = R(r2, i2);
    e2 === t2 || (t2 && (S(t2) || Array.isArray(t2)) ? (Array.isArray(t2) ? n2 && Array.isArray(n2) || (n2 = []) : n2 && S(n2) || (n2 = {}), T(e2, i2, l(n2, t2))) : "undefined" != typeof t2 && T(e2, i2, t2));
  }
  return e2;
}
function oe(e2, r2) {
  var i2 = false;
  let n2 = function() {
    if (!i2) i2 = true, r2.apply(void 0, arguments);
  }, o2 = function(e3) {
    n2(g, e3);
  };
  return function() {
    var a2 = t.call(arguments), s2 = e2.length > a2.length;
    s2 && a2.push(n2);
    var r3;
    try {
      r3 = e2.apply(this, a2);
    } catch (e3) {
      if (s2 && i2) throw e3;
      n2(e3);
      return;
    }
    !s2 && (r3 && r3.then && "function" == typeof r3.then ? r3.then(o2, n2) : ne(r3) ? n2(r3) : o2(r3));
  };
}
function L(e2) {
  return e2 != null && "object" == typeof e2 && "href" in e2 && e2.href && "protocol" in e2 && e2.protocol && e2.auth === void 0;
}
function ae() {
  var e2 = globalThis.process;
  return e2 && "function" == typeof e2.cwd ? e2.cwd() + "" : "/";
}
function C(e2, r2) {
  var n2, i2, a2, o2, t2 = e2.length;
  if (0 == r2.length || r2.length > e2.length) {
    for (r2 = -1, n2 = false; ; ) {
      if (t2 <= 0) {
        t2 = 0;
        break;
      }
      --t2;
      if ("/" == e2.charAt(t2)) {
        if (n2) {
          t2++;
          break;
        }
      } else r2 < 0 && (r2 = t2 + 1, n2 = true);
    }
    return r2 < 0 ? "" : e2.slice(t2, r2);
  }
  if (r2 == e2) return "";
  for (n2 = -1, a2 = false, o2 = -1, i2 = r2.length - 1; ; ) {
    if (t2 <= 0) {
      r2 = 0;
      break;
    }
    --t2;
    if ("/" == e2.charAt(t2)) {
      if (a2) {
        r2 = t2 + 1;
        break;
      }
    } else o2 < 0 && (a2 = true, o2 = t2 + 1), i2 > -1 && (e2.charAt(t2) == r2.charAt(i2) ? (i2 = i2 - 1 | 0) < 0 && (n2 = t2) : (n2 = o2, i2 = -1));
  }
  r2 == n2 ? n2 = o2 : n2 < 0 && (n2 = e2.length);
  return e2.slice(r2, n2);
}
function se(e2) {
  if (0 == e2.length) return ".";
  for (var r2 = e2.length, t2 = false; ; ) {
    if (r2 <= 1) {
      r2 = -1;
      break;
    }
    --r2;
    if ("/" == e2.charAt(r2)) {
      if (t2) break;
    } else t2 = t2 || true;
  }
  return r2 < 0 ? "/" == e2.charAt(0) ? "/" : "." : 1 == r2 && "/" == e2.charAt(0) ? "//" : e2.slice(0, r2);
}
function ue(e2) {
  for (var a2, t2 = e2.length, n2 = -1, i2 = -1, r2 = 0, o2 = false; ; ) {
    if (t2 <= 0) {
      o2 = 0;
      break;
    }
    --t2;
    a2 = e2.charAt(t2);
    if ("/" == a2) {
      if (o2) {
        o2 = t2 + 1;
        break;
      }
    } else n2 < 0 && (n2 = t2 + 1, o2 = true), "." == a2 ? i2 < 0 ? i2 = t2 : 1 != r2 && (r2 = 1) : i2 > -1 && (r2 = -1);
  }
  return i2 < 0 || n2 < 0 || 0 == r2 || 1 == r2 && i2 == (n2 - 1 | 0) && i2 == (o2 + 1 | 0) ? "" : e2.slice(i2, n2);
}
function fe(e2, r2) {
  let t2 = "", o2 = 0, i2 = -1, a2 = 0, n2 = 0, s2;
  for (; n2 <= e2.length; ) {
    s2 = n2 < e2.length ? e2.charAt(n2) : "/";
    if ("/" == s2) {
      if (!(i2 == n2 - 1 || 1 == a2)) if (i2 != n2 - 1 && 2 == a2) {
        if (t2.length < 2 || 2 != o2 || "." != t2.charAt(t2.length - 1) || "." != t2.charAt(t2.length - 2)) {
          if (t2.length > 2) {
            i2 = t2.lastIndexOf("/");
            if (i2 != t2.length - 1) {
              i2 < 0 ? (t2 = "", o2 = 0) : (t2 = t2.slice(0, i2), o2 = t2.length - 1 - t2.lastIndexOf("/") | 0), i2 = n2, a2 = 0, n2++;
              continue;
            }
          } else if (t2.length > 0) {
            t2 = "", o2 = 0, i2 = n2, a2 = 0, n2++;
            continue;
          }
        }
        r2 && (t2 = t2.length > 0 ? t2 + "/.." : "..", o2 = 2);
      } else o2 = e2.slice(i2 + 1 | 0, n2), t2 = t2.length > 0 ? t2 + "/" + o2 : o2, o2 = (n2 - i2 | 0) - 1 | 0;
      i2 = n2;
      a2 = 0;
    } else a2 = "." == s2 && a2 > -1 ? a2 + 1 | 0 : -1;
    n2++;
  }
  return t2;
}
function ce(e2) {
  var t2 = "/" == e2.charAt(0), r2 = fe(e2, !t2);
  0 == r2.length && !t2 && (r2 = "."), r2.length > 0 && "/" == e2.charAt(e2.length - 1) && (r2 += "/");
  return t2 ? "/" + r2 : r2;
}
function F(e2) {
  if ("string" != typeof e2) throw new TypeError("Path must be a string. Received " + JSON.stringify(e2));
}
function h(e2, r2) {
  F(e2), F(r2);
  var t2 = e2 + "";
  e2 = r2 + "", t2.length > 0 || (t2 = ""), e2.length > 0 ? t2.length > 0 && (e2 = t2 + "/" + e2) : e2 = t2;
  return 0 == e2.length ? "." : ce(e2);
}
function m(e2, r2) {
  if (e2 && e2.includes("/")) throw new Error("`" + r2 + "` cannot be a path: did not expect `/`");
}
function w(e2, r2) {
  if (!e2) throw new Error("`" + r2 + "` cannot be empty");
}
function o(e2) {
  var r2 = e2.history;
  return 0 == r2.length ? void 0 : r2[r2.length - 1];
}
function u(e2, r2) {
  if (L(r2)) {
    if ("file:" != r2.protocol + "") {
      e2 = new TypeError("The URL must be of scheme file"), e2.code = "ERR_INVALID_URL_SCHEME";
      throw e2;
    }
    if ((r2.hostname + "").length > 0) {
      e2 = new TypeError('File URL host must be "localhost" or empty on darwin'), e2.code = "ERR_INVALID_FILE_URL_HOST";
      throw e2;
    }
    var t2 = r2.pathname + "";
    for (r2 = 0; r2 < t2.length; ) {
      if ("%" == t2.charAt(r2) && "2" == t2.charAt(r2 + 1) && ("F" == t2.charAt(r2 + 2) || "f" == t2.charAt(r2 + 2))) {
        e2 = new TypeError("File URL path must not include encoded / characters"), e2.code = "ERR_INVALID_FILE_URL_PATH";
        throw e2;
      }
      ++r2;
    }
    r2 = globalThis.decodeURIComponent(t2);
  }
  w(r2, "path");
  o(e2) === r2 || e2.history.push(r2);
}
function O(e2) {
  if (!e2) return "1:1";
  var r2 = e2.line, t2 = e2.column;
  e2 = "number" == typeof r2 && r2 ? r2 + "" : "1", r2 = "number" == typeof t2 && t2 ? t2 + "" : "1";
  return e2 + ":" + r2;
}
function le(e2) {
  return e2 ? "start" in e2 || "end" in e2 ? O(e2.start) + "-" + O(e2.end) : O(e2) : "1:1";
}
function V(e2, r2, t2) {
  "string" == typeof r2 && (t2 = r2, r2 = void 0);
  var n2 = {};
  r2 && ("line" in r2 && "column" in r2 || "start" in r2 && "end" in r2 ? n2.place = r2 : "type" in r2 ? (n2.ancestors = [r2], n2.place = r2.position) : n2 = Object.assign(n2, r2));
  if ("string" == typeof e2) var i2, o2 = e2 + "", a2 = false;
  else !n2.cause && e2 ? (o2 = e2.message, n2.cause = e2, a2 = true) : (o2 = "", a2 = false);
  !n2.ruleId && !n2.source && "string" == typeof t2 && (e2 = t2 + "", r2 = e2.indexOf(":"), r2 < 0 ? n2.ruleId = e2 : (n2.source = e2.slice(0, r2), n2.ruleId = e2.slice(r2 + 1 | 0)));
  i2 = n2.ancestors, !n2.place && i2 && i2.length > 0 && (e2 = i2[i2.length - 1], n2.place = e2.position), r2 = n2.place, t2 = r2 && "start" in r2 ? r2.start : r2, e2 = new Error(), e2.ancestors = void 0, i2 && (e2.ancestors = i2), e2.cause = void 0, n2.cause && (e2.cause = n2.cause), e2.column = void 0, t2 && (e2.column = t2.column), e2.fatal = void 0, e2.file = "", e2.message = o2, e2.line = void 0, t2 && (e2.line = t2.line), e2.name = le(r2), e2.place = void 0, r2 && (e2.place = r2), e2.reason = o2, e2.ruleId = void 0, n2.ruleId && (e2.ruleId = n2.ruleId), e2.source = void 0, n2.source && (e2.source = n2.source), e2.actual = void 0, e2.expected = void 0, e2.note = void 0, e2.url = void 0, e2.stack = a2 && "string" == typeof n2.cause.stack ? n2.cause.stack : "";
  return e2;
}
function f(e2, r2) {
  Object.defineProperty(y, e2, r2);
  let t2 = r2.get, n2 = r2.set;
  Object.defineProperty(t2, "name", { configurable: true, value: "get " + e2 }), Object.defineProperty(n2, "name", { configurable: true, value: "set " + e2 });
}
function p(e2, r2) {
  Object.defineProperty(r2, "name", { configurable: true, value: e2 }), Object.defineProperty(y, e2, { configurable: true, writable: true, value: r2 });
}
function he(e2) {
  return !(!e2 || "object" != typeof e2) && "message" in e2 && "messages" in e2;
}
function v(e2) {
  return he(e2) ? e2 : new s(e2);
}
function j(e2, r2) {
  "function" == typeof r2 || i("Cannot `" + e2 + "` without `parser`");
}
function P(e2, r2) {
  "function" == typeof r2 || i("Cannot `" + e2 + "` without `compiler`");
}
function A(e2, r2) {
  !r2 || c("Cannot call `" + e2 + "` on a frozen processor.\nCreate a new processor first, by calling it: use `processor()` instead of `processor`.");
}
function N(e2) {
  (!b(e2) || "string" != typeof e2.type) && i("Expected node, got `" + e2 + "`");
}
function U(e2, r2, t2) {
  t2 || c("`" + e2 + "` finished async. Use `" + r2 + "` instead");
}
function E(e2) {
  return e2.parser || e2.Parser;
}
function I(e2) {
  return e2.compiler || e2.Compiler;
}
function x(e2, r2, n2) {
  for (var a2, s2, i2 = e2.length, o2 = -1; ; ) {
    if (false) {
      o2 = -1;
      break;
    }
    o2++;
    if (o2 >= i2) {
      o2 = -1;
      break;
    }
    if (e2[o2][0] === r2) break;
  }
  if (o2 == -1) {
    n2 = t.call(n2, 0), n2.unshift(r2), e2.push(n2);
    return;
  }
  if (n2.length > 0) {
    for (i2 = n2[0], s2 = t.call(n2, 1), a2 = e2[o2][1], b(a2) && b(i2) && (i2 = l(a2, i2)), n2 = [], n2.push(r2), n2.push(i2), i2 = s2.length, r2 = 0; r2 < i2; ) n2.push(s2[r2]), ++r2;
    re.call(e2, o2, 1, n2);
  }
}
function D(e2, r2, t2) {
  if (t2 != null) {
    Array.isArray(t2) || i("Expected a list of plugins, not `" + t2 + "`");
    for (var o2 = t2.length, n2 = -1; ++n2 < o2; ) pe(e2, r2, t2[n2]);
  }
}
function H(e2, r2, t2) {
  !("plugins" in t2) && !("settings" in t2) && c("Expected usable value but received an empty preset, which is probably a mistake: presets typically come with `plugins` and sometimes with `settings`, but this has neither"), D(e2, r2, t2.plugins);
  if (e2 = t2.settings) {
    var n2 = l(r2.settings, e2);
    r2.settings = n2;
  }
}
function pe(e2, r2, n2) {
  if ("function" == typeof n2) {
    x(e2, n2, []);
    return;
  }
  if ("object" == typeof n2) {
    if (Array.isArray(n2)) {
      r2 = n2[0], x(e2, r2, t.call(n2, 1));
      return;
    }
    H(e2, r2, n2);
    return;
  }
  i("Expected usable value, not `" + n2 + "`");
}
function M(e2) {
  for (var r2 = new n(), i2 = e2.attachers, o2 = i2.length, t2 = -1; ++t2 < o2; ) r2.use.apply(r2, i2[t2]);
  t2 = r2.data, r2.data(l({}, e2.namespace));
  return r2;
}
function r(e2, r2, t2) {
  Object.defineProperty(r2, "name", { configurable: true, value: e2 }), Object.defineProperty(r2, "length", { configurable: true, value: t2 }), Object.defineProperty(k, e2, { configurable: true, writable: true, value: r2 });
}
function ve() {
  var e2 = (0, function() {
    return M(e2);
  });
  Object.setPrototypeOf(e2, k), e2.Compiler = void 0, e2.Parser = void 0, e2.attachers = [], e2.compiler = void 0, e2.freezeIndex = -1, e2.frozen = void 0, e2.namespace = {}, e2.parser = void 0;
  var r2 = {};
  r2.fns = [], r2.run = J, r2.use = Y, e2.transformers = r2;
  return e2;
}
var g = JSON.parse("null");
var a = Object.prototype.hasOwnProperty;
var q = Object.prototype.toString;
var t = Array.prototype.slice;
var J = (0, function() {
  var e2 = t.call(arguments), r2 = e2.pop();
  "function" == typeof r2 || i("Expected function as last argument, not " + r2);
  var n2, o2 = -1, a2 = this.fns;
  n2 = function() {
    ++o2;
    var y2;
    o2 < a2.length && (y2 = a2[o2]);
    var b2, s3 = [];
    arguments.length > 0 && (b2 = arguments[0], s3 = t.call(arguments, 1));
    if (b2) {
      r2(b2);
      return;
    }
    for (var O2 = e2.length, i2 = -1; ++i2 < O2; ) b2 = void 0, i2 < s3.length && (b2 = s3[i2]), b2 == null && (s3[i2] = e2[i2]);
    e2 = s3, "function" == typeof y2 ? oe(y2, n2).apply(void 0, s3) : (i2 = t.call(s3), i2.unshift(g), r2.apply(void 0, i2));
  };
  var s2 = t.call(e2);
  s2.unshift(g), n2.apply(void 0, s2);
});
var Y = (0, function(e2) {
  return "function" == typeof e2 || i("Expected `middelware` to be a function, not " + e2), this.fns.push(e2), this;
});
var d = "history path basename stem extname dirname".split(" ");
var e = {};
var be = (0, function(e2, r2, t2) {
  if (this === void 0) throw new TypeError("Class constructor VFileMessage cannot be invoked without 'new'");
  return V(e2, r2, t2);
});
e = be.prototype, Object.setPrototypeOf(be, Error), Object.setPrototypeOf(e, Error.prototype), Object.defineProperty(be, "name", { configurable: true, value: "VFileMessage" }), e.file = "", e.name = "", e.reason = "", e.message = "", e.stack = "", e.column = void 0, e.line = void 0, e.ancestors = void 0, e.cause = void 0, e.fatal = void 0, e.place = void 0, e.ruleId = void 0, e.source = void 0, Object.defineProperty(be, "prototype", { writable: false });
var s = class VFile extends Object {
  constructor(e2) {
    super();
    if (this === void 0) throw new TypeError("Class constructor VFile cannot be invoked without 'new'");
    e2 ? L(e2) ? e2 = { path: e2 } : ("string" == typeof e2 || _(e2)) && (e2 = { value: e2 }) : e2 = {};
    var r2 = ae();
    "cwd" in e2 && (r2 = "");
    this.cwd = r2, this.data = {}, this.history = [], this.messages = [];
    for (var t2, n2 = 0; n2 < d.length; ) r2 = d[n2] || "", r2 in e2 && e2[r2] != null && e2[r2] !== void 0 && (t2 = e2[r2], "history" == r2 && (t2 = t2.slice()), this[r2] = t2), ++n2;
    for (r2 in e2) d.includes(r2) || (this[r2] = e2[r2]);
  }
};
var y = s.prototype;
Object.defineProperty(s, "name", { configurable: true, value: "VFile" });
var me = (0, function(e2, r2, t2) {
  e2 = this.message(e2, r2, t2), e2.fatal = true;
  throw e2;
});
var we = (0, function(e2, r2, t2) {
  e2 = this.message(e2, r2, t2), e2.fatal = void 0;
  return e2;
});
var Oe = (0, function(e2, r2, t2) {
  e2 = V(e2, r2, t2);
  if (r2 = o(this)) {
    var n2 = r2 + ":" + e2.name;
    e2.name = n2, e2.file = r2;
  }
  e2.fatal = false;
  this.messages.push(e2);
  return e2;
});
var je = (0, function(e2) {
  var r2 = this.value;
  if (r2 === void 0) return "";
  if ("string" == typeof r2) return r2;
  var t2;
  e2 = e2 || t2, t2 = new TextDecoder(e2);
  return t2.decode(r2);
});
f("basename", { configurable: true, get: function() {
  var e2 = o(this);
  if ("string" == typeof e2) return C(e2 + "", "");
}, set: function(e2) {
  w(e2, "basename"), m(e2, "basename");
  var r2 = this.dirname || "";
  u(this, h(r2, e2));
} }), f("dirname", { configurable: true, get: function() {
  var e2 = o(this);
  if ("string" == typeof e2) return se(e2 + "");
}, set: function(e2) {
  var r2 = this.basename;
  if (!r2) throw new Error("Setting `dirname` requires `path` to be set too");
  e2 = e2 || "", u(this, h(e2, r2));
} }), f("extname", { configurable: true, get: function() {
  var e2 = o(this);
  if ("string" == typeof e2) return ue(e2 + "");
}, set: function(e2) {
  m(e2, "extname");
  var r2 = this.dirname;
  if (!r2) throw new Error("Setting `extname` requires `path` to be set too");
  if (e2) {
    if (46 != (+e2.codePointAt(0) | 0)) throw new Error("`extname` must start with `.`");
    if (e2.includes(".", 1)) throw new Error("`extname` cannot contain multiple dots");
  }
  e2 = e2 ? e2 + "" : "";
  u(this, h(r2, this.stem + "" + e2));
} }), f("path", { configurable: true, get: function() {
  return o(this);
}, set: function(e2) {
  u(this, e2);
} }), f("stem", { configurable: true, get: function() {
  var e2 = o(this);
  if ("string" == typeof e2) return e2 += "", C(e2, this.extname + "");
}, set: function(e2) {
  w(e2, "stem"), m(e2, "stem");
  var t2 = e2 + "";
  e2 = this.dirname, e2 = e2 ? e2 + "" : "";
  var r2 = this.extname;
  r2 = r2 ? r2 + "" : "", u(this, h(e2, t2 + r2));
} }), p("fail", me), p("info", we), p("message", Oe), p("toString", je), Object.defineProperty(s, "prototype", { writable: false });
var n;
var B = (0, function() {
  A("use", this.frozen);
  var r2 = this.attachers, n2 = this.namespace, e2;
  arguments.length > 0 && (e2 = arguments[0]);
  if (e2 == null) return this;
  if ("function" == typeof e2) return x(r2, e2, t.call(arguments, 1)), this;
  if ("object" == typeof e2) return Array.isArray(e2) ? D(r2, n2, e2) : H(r2, n2, e2), this;
  throw new TypeError("Expected usable value, not `" + e2 + "`");
});
var G = (0, function() {
  return M(this);
});
var K = (0, function(e2) {
  this.freeze(), e2 = v(e2);
  let r2 = E(this);
  j("parse", r2);
  return r2(String(e2), e2);
});
var Q = (0, function(e2, r2, t2) {
  N(e2);
  var o2 = this.freeze;
  this.freeze(), !t2 && "function" == typeof r2 && (t2 = r2, r2 = void 0);
  var i2 = this.transformers, n2 = function(n3, o3) {
    let a2 = v(r2);
    i2.run(e2, a2, function(r3, i3, a3) {
      var s2 = i3 ? i3 : e2;
      if (r3) {
        o3(r3);
        return;
      }
      if (n3) {
        n3(s2);
        return;
      }
      t2(void 0, s2, a3);
    });
  };
  return t2 ? (n2(void 0, t2), void 0) : new Promise(n2);
});
var W = (0, function(e2, r2) {
  var t2, n2 = false;
  this.run(e2, r2, function(e3, r3, i2) {
    z(e3), t2 = r3, n2 = true;
  }), U("runSync", "run", n2);
  return t2;
});
var X = (0, function(e2, r2) {
  this.freeze();
  let t2 = v(r2);
  r2 = I(this), P("stringify", r2), N(e2);
  return r2(e2, t2);
});
var Z = (0, function(e2, r2) {
  var t2 = this;
  t2.freeze(), j("process", E(t2)), P("process", I(t2));
  var n2 = function(n3, i2) {
    let o2 = v(e2), a2 = t2.parse(o2);
    t2.run(a2, o2, function(e3, o3, a3) {
      if (e3 || !o3 || !a3) {
        i2(e3);
        return;
      }
      var s2 = t2.stringify(o3, a3);
      ie(s2) ? a3.value = s2 : a3.result = s2;
      if (n3) {
        n3(a3);
        return;
      }
      r2(void 0, a3);
    });
  };
  return r2 ? (n2(void 0, r2), void 0) : new Promise(n2);
});
var $ = (0, function(e2) {
  this.freeze(), j("processSync", E(this)), P("processSync", I(this));
  var r2, t2 = false;
  this.process(e2, function(e3, n2) {
    t2 = true, z(e3), r2 = n2;
  }), U("processSync", "process", t2);
  return r2;
});
var ee = (0, function(e2, r2) {
  var n2 = this.namespace, i2 = arguments.length, t2;
  i2 > 0 && (t2 = e2);
  if ("string" == typeof t2) {
    if (2 == i2) return A("data", this.frozen), n2[t2] = r2, this;
    var o2;
    return a.call(n2, t2) && (o2 = n2[t2]) ? o2 : void 0;
  }
  return t2 ? (A("data", this.frozen), this.namespace = t2, this) : n2;
});
var te = (0, function() {
  if (this.frozen) return this;
  for (var r2 = this.attachers, i2 = this.transformers; true; ) {
    var e2 = +this.freezeIndex + 1;
    this.freezeIndex = e2;
    if (e2 >= r2.length) break;
    var n2 = r2[e2], o2 = n2[0];
    e2 = t.call(n2, 1);
    if (!(e2.length > 0 && e2[0] === false)) e2.length > 0 && true === e2[0] && re.call(e2, 0, 1, void 0), e2 = o2.apply(this, e2), "function" == typeof e2 && i2.use(e2);
  }
  this.frozen = true;
  this.freezeIndex = Number.POSITIVE_INFINITY;
  return this;
});
n = (0, function() {
  if (this === void 0) throw new TypeError("Class constructor Processor cannot be invoked without 'new'");
  return ve();
}), Object.defineProperty(n, "name", { configurable: true, value: "Processor" });
var k = n.prototype;
r("copy", G, 0), r("data", ee, 2), r("freeze", te, 0), r("parse", K, 1), r("process", Z, 2), r("processSync", $, 1), r("run", Q, 3), r("runSync", W, 2), r("stringify", X, 2), r("use", B, 1), Object.defineProperty(n, "prototype", { writable: false }), me = new n(), me.freeze();
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  unified
});
