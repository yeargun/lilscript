/*! @itslil/unified vfile browser runtime | LilScript reimplementation of vfile@6.0.3 | MIT */

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

// vfile.esm.js
var vfile_esm_exports = {};
__export(vfile_esm_exports, {
  VFile: () => o,
  VFileMessage: () => m,
  createVFileMessage: () => H
});
module.exports = __toCommonJS(vfile_esm_exports);
var J = -1;
function z(b2) {
  return !b2 ? false : "object" != typeof b2 ? false : "byteLength" in b2 && "byteOffset" in b2;
}
function w(b2) {
  return b2 != null && "object" == typeof b2 && "href" in b2 && b2.href && "protocol" in b2 && b2.protocol && b2.auth === void 0;
}
function A() {
  var b2 = globalThis.process;
  return b2 && "function" == typeof b2.cwd ? b2.cwd() + "" : "/";
}
function x(b2, a2) {
  var e, r2, h, d2, c2 = b2.length;
  if (0 == a2.length || a2.length > b2.length) {
    for (a2 = J, e = false; ; ) {
      if (c2 <= 0) {
        c2 = 0;
        break;
      }
      c2--;
      if ("/" == b2.charAt(c2)) {
        if (e) {
          c2++;
          break;
        }
      } else a2 < 0 && (a2 = c2 + 1, e = true);
    }
    return a2 < 0 ? "" : b2.slice(c2, a2);
  }
  if (a2 == b2) return "";
  for (e = J, h = false, d2 = J, r2 = a2.length - 1; ; ) {
    if (c2 <= 0) {
      a2 = 0;
      break;
    }
    c2--;
    if ("/" == b2.charAt(c2)) {
      if (h) {
        a2 = c2 + 1;
        break;
      }
    } else d2 < 0 && (h = true, d2 = c2 + 1), r2 > J && (b2.charAt(c2) == a2.charAt(r2) ? (r2 = r2 - 1 | 0, r2 < 0 && (e = c2)) : (e = d2, r2 = J));
  }
  a2 == e ? e = d2 : e < 0 && (e = b2.length);
  return b2.slice(a2, e);
}
function B(b2) {
  if (0 == b2.length) return ".";
  for (var c2 = b2.length, a2 = false; ; ) {
    if (c2 <= 1) {
      c2 = J;
      break;
    }
    c2--;
    if ("/" == b2.charAt(c2)) {
      if (a2) break;
    } else a2 = a2 || true;
  }
  return c2 < 0 ? "/" == b2.charAt(0) ? "/" : "." : 1 == c2 && "/" == b2.charAt(0) ? "//" : b2.slice(0, c2);
}
function C(b2) {
  for (var h, c2 = b2.length, e = J, a2 = J, r2 = 0, d2 = false; ; ) {
    if (c2 <= 0) {
      d2 = 0;
      break;
    }
    c2--;
    h = b2.charAt(c2);
    if ("/" == h) {
      if (d2) {
        d2 = c2 + 1;
        break;
      }
    } else e < 0 && (e = c2 + 1, d2 = true), "." == h ? a2 < 0 ? a2 = c2 : 1 != r2 && (r2 = 1) : a2 > J && (r2 = J);
  }
  return a2 < 0 || e < 0 || 0 == r2 || 1 == r2 && a2 == (e - 1 | 0) && a2 == (d2 + 1 | 0) ? "" : b2.slice(a2, e);
}
function D(b2, a2) {
  for (var o2, c2 = "", d2 = 0, r2 = J, h = 0, e = 0; e <= b2.length; ) {
    o2 = e < b2.length ? b2.charAt(e) : "/";
    if ("/" == o2) {
      if (!(r2 == e - 1 || 1 == h)) if (r2 != e - 1 && 2 == h) {
        if (c2.length < 2 || 2 != d2 || "." != c2.charAt(c2.length - 1) || "." != c2.charAt(c2.length - 2)) {
          if (c2.length > 2) {
            r2 = c2.lastIndexOf("/");
            if (r2 != c2.length - 1) {
              r2 < 0 ? (c2 = "", d2 = 0) : (c2 = c2.slice(0, r2), d2 = c2.length - 1 - c2.lastIndexOf("/") | 0), r2 = e, h = 0, e++;
              continue;
            }
          } else if (c2.length > 0) {
            c2 = "", d2 = 0, r2 = e, h = 0, e++;
            continue;
          }
        }
        a2 && (c2 = c2.length > 0 ? c2 + "/.." : "..", d2 = 2);
      } else {
        d2 = b2.slice(r2 + 1 | 0, e), c2 = c2.length > 0 ? c2 + "/" + d2 : d2, d2 = (e - r2 | 0) - 1 | 0;
      }
      r2 = e;
      h = 0;
    } else {
      h = "." == o2 && h > J ? h + 1 | 0 : J;
    }
    e++;
  }
  return c2;
}
function E(b2) {
  var c2 = "/" == b2.charAt(0), a2 = D(b2, !c2);
  0 == a2.length && !c2 && (a2 = "."), a2.length > 0 && "/" == b2.charAt(b2.length - 1) && (a2 = a2 + "/");
  return c2 ? "/" + a2 : a2;
}
function y(b2) {
  if ("string" != typeof b2) throw new TypeError("Path must be a string. Received " + JSON.stringify(b2));
}
function p(b2, a2) {
  y(b2), y(a2);
  var c2 = b2 + "";
  b2 = a2 + "", c2.length > 0 || (c2 = ""), b2.length > 0 ? c2.length > 0 && (b2 = c2 + "/" + b2) : b2 = c2;
  return 0 == b2.length ? "." : E(b2);
}
function s(b2, a2) {
  if (b2 && b2.includes("/")) throw new Error("`" + a2 + "` cannot be a path: did not expect `/`");
}
function t(b2, a2) {
  if (!b2) throw new Error("`" + a2 + "` cannot be empty");
}
function T(b2) {
  var a2 = b2.history;
  return 0 == a2.length ? void 0 : a2[a2.length - 1];
}
function k(b2, a2) {
  if (w(a2)) {
    if ("file:" != a2.protocol + "") {
      b2 = new TypeError("The URL must be of scheme file"), b2.code = "ERR_INVALID_URL_SCHEME";
      throw b2;
    }
    if ((a2.hostname + "").length > 0) {
      b2 = new TypeError('File URL host must be "localhost" or empty on darwin'), b2.code = "ERR_INVALID_FILE_URL_HOST";
      throw b2;
    }
    var c2 = a2.pathname + "";
    for (a2 = 0; a2 < c2.length; ) {
      if ("%" == c2.charAt(a2) && "2" == c2.charAt(a2 + 1) && ("F" == c2.charAt(a2 + 2) || "f" == c2.charAt(a2 + 2))) {
        b2 = new TypeError("File URL path must not include encoded / characters"), b2.code = "ERR_INVALID_FILE_URL_PATH";
        throw b2;
      }
      a2++;
    }
    a2 = globalThis.decodeURIComponent(c2);
  }
  t(a2, "path");
  T(b2) === a2 || b2.history.push(a2);
}
function _(b2) {
  if (!b2) return "1:1";
  var a2 = b2.line, c2 = b2.column;
  b2 = "number" == typeof a2 && a2 ? a2 + "" : "1", a2 = "number" == typeof c2 && c2 ? c2 + "" : "1";
  return b2 + ":" + a2;
}
function F(b2) {
  return !b2 ? "1:1" : "start" in b2 || "end" in b2 ? _(b2.start) + "-" + _(b2.end) : _(b2);
}
function v(b2, a2, c2) {
  "string" == typeof a2 && (c2 = a2, a2 = void 0);
  var e = {};
  a2 && ("line" in a2 && "column" in a2 ? e.place = a2 : "start" in a2 && "end" in a2 ? e.place = a2 : "type" in a2 ? (e.ancestors = [a2], e.place = a2.position) : e = Object.assign(e, a2));
  if ("string" == typeof b2) var r2, d2 = b2 + "", h = false;
  else {
    !e.cause && b2 ? (d2 = b2.message, e.cause = b2, h = true) : (d2 = "", h = false);
  }
  !e.ruleId && !e.source && "string" == typeof c2 && (b2 = c2 + "", a2 = b2.indexOf(":"), a2 < 0 ? e.ruleId = b2 : (e.source = b2.slice(0, a2), e.ruleId = b2.slice(a2 + 1 | 0)));
  r2 = e.ancestors, !e.place && r2 && r2.length > 0 && (b2 = r2[r2.length - 1], e.place = b2.position), a2 = e.place, c2 = a2 && "start" in a2 ? a2.start : a2, b2 = new Error(), Object.setPrototypeOf(b2, i), b2.ancestors = void 0, r2 && (b2.ancestors = r2), b2.cause = void 0, e.cause && (b2.cause = e.cause), b2.column = void 0, c2 && (b2.column = c2.column), b2.fatal = void 0, b2.file = "", b2.message = d2, b2.line = void 0, c2 && (b2.line = c2.line), b2.name = F(a2), b2.place = void 0, a2 && (b2.place = a2), b2.reason = d2, b2.ruleId = void 0, e.ruleId && (b2.ruleId = e.ruleId), b2.source = void 0, e.source && (b2.source = e.source), b2.actual = void 0, b2.expected = void 0, b2.note = void 0, b2.url = void 0, b2.stack = h && "string" == typeof e.cause.stack ? e.cause.stack : "";
  return b2;
}
function H(b2, a2) {
  return v(b2, a2, void 0);
}
function l(b2, a2) {
  Object.defineProperty(n, b2, a2);
  let c2 = a2.get, e = a2.set;
  Object.defineProperty(c2, "name", { configurable: true, value: "get " + b2 }), Object.defineProperty(e, "name", { configurable: true, value: "set " + b2 });
}
function q(b2, a2) {
  Object.defineProperty(a2, "name", { configurable: true, value: b2 }), Object.defineProperty(n, b2, { configurable: true, writable: true, value: a2 });
}
JSON.parse("null");
Object.prototype.hasOwnProperty, Object.prototype.toString, Array.prototype.slice;
var r = "history path basename stem extname dirname".split(" ");
var i = {};
var m = (0, function(b2, a2, c2) {
  if (this === void 0) throw new TypeError("Class constructor VFileMessage cannot be invoked without 'new'");
  return v(b2, a2, c2);
});
i = m.prototype, Object.setPrototypeOf(m, Error), Object.setPrototypeOf(i, Error.prototype), Object.defineProperty(m, "name", { configurable: true, value: "VFileMessage" }), i.file = "", i.name = "", i.reason = "", i.message = "", i.stack = "", i.column = void 0, i.line = void 0, i.ancestors = void 0, i.cause = void 0, i.fatal = void 0, i.place = void 0, i.ruleId = void 0, i.source = void 0, Object.defineProperty(m, "prototype", { writable: false });
var n;
var o = class VFile extends Object {
  constructor(b2) {
    super();
    if (this === void 0) throw new TypeError("Class constructor VFile cannot be invoked without 'new'");
    !b2 ? b2 = {} : w(b2) ? b2 = { path: b2 } : ("string" == typeof b2 || z(b2)) && (b2 = { value: b2 });
    var a2 = A();
    "cwd" in b2 && (a2 = "");
    this.cwd = a2, this.data = {}, this.history = [], this.messages = [];
    for (var e, c2 = 0; c2 < r.length; c2++) a2 = r[c2] || "", a2 in b2 && b2[a2] != null && b2[a2] !== void 0 && (e = b2[a2], "history" == a2 && (e = e.slice()), this[a2] = e);
    for (a2 in b2) r.includes(a2) || (this[a2] = b2[a2]);
  }
};
n = o.prototype, Object.defineProperty(o, "name", { configurable: true, value: "VFile" });
var a = (0, function(b2, a2, c2) {
  b2 = this.message(b2, a2, c2), b2.fatal = true;
  throw b2;
});
var b = (0, function(b2, a2, c2) {
  b2 = this.message(b2, a2, c2), b2.fatal = void 0;
  return b2;
});
var c = (0, function(b2, a2, c2) {
  b2 = v(b2, a2, c2);
  if (a2 = T(this)) {
    var e = a2 + ":" + b2.name;
    b2.name = e, b2.file = a2;
  }
  b2.fatal = false;
  this.messages.push(b2);
  return b2;
});
var d = (0, function(b2) {
  var a2 = this.value;
  if (a2 === void 0) return "";
  if ("string" == typeof a2) return a2;
  var c2;
  b2 = b2 || c2, c2 = new TextDecoder(b2);
  return c2.decode(a2);
});
l("basename", { configurable: true, get: function() {
  var b2 = T(this);
  if ("string" == typeof b2) return x(b2 + "", "");
}, set: function(b2) {
  t(b2, "basename"), s(b2, "basename");
  var a2 = this.dirname || "";
  k(this, p(a2, b2));
} }), l("dirname", { configurable: true, get: function() {
  var b2 = T(this);
  if ("string" == typeof b2) return B(b2 + "");
}, set: function(b2) {
  var a2 = this.basename;
  if (!a2) throw new Error("Setting `dirname` requires `path` to be set too");
  b2 = b2 || "", k(this, p(b2, a2));
} }), l("extname", { configurable: true, get: function() {
  var b2 = T(this);
  if ("string" == typeof b2) return C(b2 + "");
}, set: function(b2) {
  s(b2, "extname");
  var a2 = this.dirname;
  if (!a2) throw new Error("Setting `extname` requires `path` to be set too");
  if (b2) {
    if (46 != (+b2.codePointAt(0) | 0)) throw new Error("`extname` must start with `.`");
    if (b2.includes(".", 1)) throw new Error("`extname` cannot contain multiple dots");
  }
  b2 = b2 ? b2 + "" : "";
  k(this, p(a2, this.stem + "" + b2));
} }), l("path", { configurable: true, get: function() {
  return T(this);
}, set: function(b2) {
  k(this, b2);
} }), l("stem", { configurable: true, get: function() {
  var b2 = T(this);
  if ("string" == typeof b2) return b2 = b2 + "", x(b2, this.extname + "");
}, set: function(b2) {
  t(b2, "stem"), s(b2, "stem");
  var c2 = b2 + "";
  b2 = this.dirname, b2 = b2 ? b2 + "" : "";
  var a2 = this.extname;
  a2 = a2 ? a2 + "" : "", k(this, p(b2, c2 + a2));
} }), q("fail", a), q("info", b), q("message", c), q("toString", d), Object.defineProperty(o, "prototype", { writable: false });
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  VFile,
  VFileMessage,
  createVFileMessage
});
