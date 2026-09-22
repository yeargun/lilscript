/*! @itslil/remark 15.0.2 | LilScript reimplementation of remark | MIT */

var remark = (() => {
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

  // remark.esm.js
  var remark_esm_exports = {};
  __export(remark_esm_exports, {
    remark: () => na
  });
  var Di = "Cannot serialize items with `";
  var gt = (r2) => !!r2 ? r2 : [];
  var or = (r2) => "listOrdered" == r2 || "listUnordered" == r2;
  var D = (r2, b2, e2) => {
    r2[b2 + ""] = e2;
  };
  var oe = (r2) => "&#x" + E(r2.toString(16)).toUpperCase() + ";";
  var de = (r2) => r2.replace(Ut, " ").replace(Ht, "").toLowerCase().toUpperCase();
  var Pe = (r2) => A(r2) || H(r2) || P(Ot, r2) ? 1 : P(zt, r2) ? 2 : void 0;
  var it = (r2) => V(r2) ? {} : r2;
  var J = (r2) => E(r2.type);
  var Ur = (r2) => {
    if (r2) throw r2;
  };
  var C = (r2) => r2 === -2 || r2 === -1 || 32 === r2;
  var H = (r2) => null !== r2 && (+r2 < 0 || 32 === r2);
  var Xe = (r2) => null !== r2 && (+r2 < 32 || 127 === r2);
  var T = (r2) => null !== r2 && +r2 < -2;
  var P = (r2, b2) => null !== b2 && +b2 > -1 && r2.test(pe(b2));
  var L = (r2, b2) => Object.assign(r2, b2);
  var Je = (r2, b2) => {
    !b2 || je("Cannot call `" + r2 + "` on a frozen processor.\nCreate a new processor first, by calling it: use `processor()` instead of `processor`.");
  };
  var Xr = (r2, b2, e2) => {
    e2 || je("`" + r2 + "` finished async. Use `" + b2 + "` instead");
  };
  var N = (r2, b2, e2) => function(t2) {
    r2.call(this, b2(t2), t2), !e2 || e2.call(this, t2);
  };
  var B = (r2, b2) => function(e2) {
    !b2 || b2.call(this, e2), r2.call(this, e2);
  };
  var ht = (r2) => {
    var b2 = O(r2);
    if (r2.data.inReference) {
      var e2 = r2.data.referenceType || "shortcut", t2 = E(b2.type) + "Reference";
      b2.type = t2, b2.referenceType = e2, delete b2.url, delete b2.title;
    } else {
      delete b2.identifier, delete b2.label;
    }
    r2.data.referenceType = void 0;
  };
  var $e = (r2) => r2.compiler || r2.Compiler;
  var We = (r2) => r2.parser || r2.Parser;
  var G = (r2) => {
    var b2 = r2.options;
    return V(b2) ? {} : b2;
  };
  var lr = (r2) => {
    var b2 = G(r2).quote;
    r2 = b2 ? E(b2) : '"', '"' != r2 && "'" != r2 && W("Cannot serialize title with `" + r2 + "` for `options.quote`, expected `\"`, or `'`");
    return r2;
  };
  var qt = (r2) => {
    var b2 = G(r2).rule;
    r2 = b2 ? E(b2) : "*", "*" != r2 && "-" != r2 && "_" != r2 && W("Cannot serialize rules with `" + r2 + "` for `options.rule`, expected `*`, `-`, or `_`");
    return r2;
  };
  var cr = (r2) => {
    var b2 = G(r2).bullet;
    r2 = b2 ? E(b2) : "*", "*" != r2 && "+" != r2 && "-" != r2 && W(Di + r2 + "` for `options.bullet`, expected `*`, `+`, or `-`");
    return r2;
  };
  var ct = (r2) => {
    for (var b2, f2, n2, w2, i2, c2, o2, a2 = {}, t2 = new wr(r2), e2 = -1, x2 = false; ++e2 < t2.length; ) {
      while (e2 + "" in Object(a2)) e2 = +a2[e2 + ""] | 0;
      var u2 = t2.get;
      c2 = t2.get(e2);
      if (e2 > 0 && "chunkFlow" == c2[1].type && "listItemPrefix" == t2.get(e2 - 1 | 0)[1].type) {
        b2 = c2[1]._tokenizer.events, f2 = 0 < q(b2) && "lineEndingBlank" == b2[0][1].type ? 2 : 0;
        if (f2 < q(b2) && "content" == b2[f2][1].type) for (n2 = f2; ++n2 < q(b2); ) {
          if ("content" == b2[n2][1].type) break;
          "chunkText" == b2[n2][1].type && (b2[n2][1]._isInFirstContentOfListItem = true, n2++);
        }
      }
      if ("enter" == c2[0]) !c2[1].contentType || (L(a2, ((r3, b3) => {
        var c3 = r3.get(b3), t3 = c3[1], i3 = c3[2], n3 = b3 - 1 | 0, a3 = [];
        b3 = t3._tokenizer, b3 || (c3 = i3.parser, b3 = c3[E(t3.contentType)](t3.start), !t3._contentTypeTextTrailing || (b3._contentTypeTextTrailing = true));
        var f3 = b3.events, o3 = [], x3 = {};
        c3 = [], c3.push(0);
        var w3, u3, e3 = t3;
        while (e3) {
          while (true) {
            n3++;
            if (r3.get(n3)[1] == e3) break;
          }
          a3.push(n3);
          e3._tokenizer || (w3 = i3.sliceStream(e3), e3.next || w3.push(null), !u3 || b3.defineSkip(e3.start), !e3._isInFirstContentOfListItem || (b3._gfmTasklistFirstContentOfListItem = true), b3.write(w3), !e3._isInFirstContentOfListItem || (b3._gfmTasklistFirstContentOfListItem = void 0)), w3 = e3.next, u3 = e3, e3 = w3;
        }
        for (i3 = q(f3), e3 = -1; ++e3 < i3; ) {
          "exit" == f3[e3][0] && "enter" == f3[e3 - 1][0] ? (n3 = f3[e3][1].type, n3 = n3 == f3[e3 - 1][1].type) : n3 = false;
          n3 ? (n3 = f3[e3][1].start.line, n3 = n3 != f3[e3][1].end.line) : n3 = false, n3 && (c3.push(e3 + 1 | 0), t3._tokenizer = void 0, t3.previous = void 0, t3 = t3.next);
        }
        b3.events = [];
        t3 ? (t3._tokenizer = void 0, t3.previous = void 0) : c3.pop(), b3 = q(c3);
        for (; --b3 >= 0; ) t3 = void 0, (b3 + 1 | 0) < q(c3) && (t3 = c3[b3 + 1]), e3 = c3[b3], n3 = f3.slice(e3, t3), e3 = +a3.pop() | 0, t3 = [e3], i3 = (e3 + q(n3) | 0) - 1 | 0, t3.push(i3), o3.push(t3), r3.splice(e3, 2, n3);
        o3.reverse();
        for (n3 = q(o3), r3 = -1, b3 = 0; ++r3 < n3; ) t3 = o3[r3], e3 = +t3[0] | 0, c3 = +t3[1] | 0, c3 = b3 + c3 | 0, x3[(b3 + e3 | 0) + ""] = c3, b3 = (c3 - e3 | 0) - 1 | 0;
        return x3;
      })(t2, e2)), e2 = +a2[e2 + ""] | 0, x2 = true);
      else if (c2[1]._container) {
        for (w2 = e2, b2 = 0; --w2 >= 0; ) {
          o2 = t2.get(w2), i2 = E(o2[1].type);
          if ("lineEnding" == i2 || "lineEndingBlank" == i2) "enter" == o2[0] && (b2 > 0 && (t2.get(b2)[1].type = "lineEndingBlank"), o2[1].type = "lineEnding", b2 = w2);
          else if (!("linePrefix" == i2 || "listItemIndent" == i2)) break;
        }
        b2 > 0 && (n2 = c2[1], n2.end = z(t2.get(b2)[1].start), f2 = t2.slice(b2, e2), f2.unshift(c2), n2 = (e2 - b2 | 0) + 1 | 0, t2.splice(b2, n2, f2));
      }
    }
    Y(r2, 0, 1 / 0, t2.slice(0, void 0));
    return !x2;
  };
  var lt = (r2, b2, e2, t2) => {
    var c2 = b2[3], n2 = r2.sliceStream(e2);
    t2 && n2.push(null), e2.previous = b2[4], !b2[4] || (b2[4].next = e2), b2[4] = e2, c2.defineSkip(e2.start), c2.write(n2), t2 = r2.parser.lazy;
    if (t2[e2.start.line]) {
      e2 = q(c2.events);
      for (var f2 = b2[5]; --e2 >= 0; ) {
        t2 = c2.events[e2][1];
        if (+t2.start.offset < f2 && (!t2.end || +t2.end.offset > f2)) return;
      }
      for (c2 = q(r2.events), t2 = void 0, e2 = c2, f2 = false; --e2 >= 0; ) {
        n2 = r2.events[e2];
        if ("exit" == n2[0] && "chunkFlow" == n2[1].type) {
          if (f2) {
            t2 = n2[1].end;
            break;
          }
          f2 = true;
        }
      }
      ke(r2, b2[2], b2[0], b2[1]);
      for (b2 = c2; b2 < q(r2.events); b2++) n2 = r2.events[b2][1], n2.end = z(t2);
      n2 = r2.events, t2 = e2 + 1 | 0, e2 = r2.events, Y(n2, t2, 0, e2.slice(c2)), r2.events.length = b2;
    }
  };
  var ot = (r2) => "number" == typeof r2 ? r2 | 0 : r2 | 0;
  var Re = (r2) => !!r2 && "number" == typeof r2 ? E(r2) : "1";
  var Qe = (r2) => {
    if (!r2) return "1:1";
    var b2 = r2.line, e2 = r2.column;
    r2 = "number" == typeof b2 && b2 ? b2 + "" : "1", b2 = "number" == typeof e2 && e2 ? e2 + "" : "1";
    return r2 + ":" + b2;
  };
  var vt = (r2) => {
    if (V(r2)) return r2 = Se(void 0) + "-", r2 + Se(void 0);
    var b2 = Se(r2.start) + "-";
    return b2 + Se(r2.end);
  };
  var Se = (r2) => {
    if (V(r2)) return r2 = Re(void 0) + ":", r2 + Re(void 0);
    var b2 = Re(r2.line) + ":";
    return b2 + Re(r2.column);
  };
  var q = (r2) => V(r2) ? 0 : r2.length;
  var F = (r2, b2) => V(r2) ? false : !!Object.prototype.hasOwnProperty.call(r2, b2);
  var M = (r2, b2) => V(r2) ? false : !!r2.includes(b2);
  var ur = (r2, b2) => {
    if (false !== G(b2).fences) return false;
    b2 = r2.value;
    if (!b2) return false;
    if (r2.lang) return false;
    r2 = E(b2);
    return !cn.test(r2) ? false : sn.test(r2) ? false : true;
  };
  var Et = (r2, b2) => {
    if (G(b2).resourceLink) return false;
    var e2 = r2.url;
    if (!e2) return false;
    if (r2.title) return false;
    b2 = r2.children;
    if (!b2 || 1 != q(b2)) return false;
    if ("text" != E(b2[0].type)) return false;
    b2 = ar(r2, void 0), r2 = E(e2);
    return b2 != r2 && "mailto:" + b2 != r2 ? false : !yn.test(r2) ? false : kn.test(r2) ? false : true;
  };
  var wt = (r2, b2) => {
    var e2 = b2(r2);
    if (false === e2) return true;
    if ("skip" === e2) return false;
    if (!V(r2) && "object" == typeof r2 && "children" in r2) {
      e2 = r2.children;
      var t2 = q(e2);
      for (r2 = 0; r2 < t2; r2++) if (wt(e2[r2], b2)) return true;
    }
    return false;
  };
  var St = (r2, b2) => {
    var t2 = false;
    wt(r2, function(r3) {
      if ("value" in r3) {
        var b3 = vn;
        b3 = b3.test(E(r3.value));
      } else {
        b3 = false;
      }
      if (b3) return t2 = true, false;
      if ("break" == E(r3.type)) return t2 = true, false;
    });
    var e2 = r2.depth;
    e2 = !e2 || +e2 < 3;
    return !e2 ? false : 0 == ar(r2, void 0).length ? false : G(b2).setext || t2 ? true : false;
  };
  var Gr = (r2) => {
    if (!r2) return false;
    if ("[object Object]" != Lt.call(r2) + "") return false;
    var e2 = !!he.call(r2, "constructor"), b2 = r2.constructor, t2 = b2 && b2.prototype && he.call(b2.prototype, "isPrototypeOf");
    if (b2 && !e2 && !t2) return false;
    t2 = "", e2 = false;
    for (b2 in r2) t2 = b2, e2 = true;
    return !e2 ? true : !!he.call(r2, t2);
  };
  var Ue = (r2) => {
    if ("object" != typeof r2 || r2 == null) return false;
    var b2 = Object.getPrototypeOf(r2), e2 = b2 == null || b2 === Object.prototype || Object.getPrototypeOf(b2) == null;
    return !e2 ? false : Symbol.toStringTag in r2 ? false : Symbol.iterator in r2 ? false : true;
  };
  var Ie = (r2, b2) => {
    (r2 == null || "object" != typeof r2 && "function" != typeof r2) && (r2 = {});
    if (b2 == null) return r2;
    for (var n2 in b2) {
      var t2 = Yr(r2, n2), e2 = Yr(b2, n2);
      r2 === e2 || (e2 && (Gr(e2) || Array.isArray(e2)) ? (Array.isArray(e2) ? t2 && Array.isArray(t2) || (t2 = []) : t2 && Gr(t2) || (t2 = {}), Qr(r2, n2, Ie(t2, e2))) : "undefined" != typeof e2 && Qr(r2, n2, e2));
    }
    return r2;
  };
  var jr = (r2) => !r2 ? false : "object" != typeof r2 ? false : "byteLength" in r2 && "byteOffset" in r2;
  var Fe = (r2) => !r2 || "object" != typeof r2 ? "" : F(r2, "position") || F(r2, "type") ? vt(r2.position) : F(r2, "start") || F(r2, "end") ? vt(r2) : F(r2, "line") || F(r2, "column") ? Se(r2) : "";
  var E = (r2) => "string" == typeof r2 ? r2 : r2 === void 0 ? "undefined" : r2 == null ? "null" : String(r2) + "";
  var De = (r2, b2) => "string" == typeof b2 ? r2[b2] : A(b2) ? r2.null : r2[b2 + ""];
  var nt = (r2, b2, e2) => {
    if ("string" == typeof b2) {
      r2[b2] = e2;
      return;
    }
    if (A(b2)) {
      r2.null = e2;
      return;
    }
    r2[b2 + ""] = e2;
  };
  var ut = (r2, b2) => {
    var f2 = b2.start, E2 = b2.end, e2 = f2._index | 0, t2 = f2._bufferIndex | 0, n2 = E2._index | 0, c2 = E2._bufferIndex | 0;
    e2 == n2 ? b2 = [r2[e2].slice(t2, c2)] : (b2 = r2.slice(e2, n2), t2 > -1 && (e2 = b2[0], "string" == typeof e2 ? b2[0] = e2.slice(t2) : b2.shift()), c2 > 0 && (e2 = r2[n2].slice(0, c2), b2.push(e2)));
    return b2;
  };
  var $r = (r2, b2, e2) => {
    "string" == typeof b2 && (e2 = b2, b2 = void 0);
    var t2 = {};
    if (b2) if ("line" in b2 && "column" in b2) t2.place = b2;
    else if ("start" in b2 && "end" in b2) t2.place = b2;
    else if ("type" in b2) t2.ancestors = [b2], t2.place = b2.position;
    else for (var c2, f2 = Object.keys(b2), n2 = 0; n2 < f2.length; n2++) c2 = f2[n2] + "", t2[c2] = b2[c2];
    "string" == typeof r2 ? (b2 = r2 + "", c2 = false) : !t2.cause && r2 ? (b2 = r2.message, t2.cause = r2, c2 = true) : (b2 = "", c2 = false);
    !t2.ruleId && !t2.source && "string" == typeof e2 && (r2 = e2 + "", e2 = r2.indexOf(":"), e2 < 0 ? t2.ruleId = r2 : (t2.source = r2.slice(0, e2), t2.ruleId = r2.slice(e2 + 1 | 0))), n2 = t2.ancestors, !t2.place && n2 && n2.length > 0 && (t2.place = n2[n2.length - 1].position), e2 = t2.place, f2 = e2 && "start" in e2 ? e2.start : e2, r2 = new Error(), Object.setPrototypeOf(r2, j), r2.ancestors = void 0, n2 && (r2.ancestors = n2), r2.cause = void 0, t2.cause && (r2.cause = t2.cause), r2.column = void 0, f2 && (r2.column = f2.column), r2.fatal = void 0, r2.file = "", r2.message = b2, r2.line = void 0, f2 && (r2.line = f2.line), r2.name = !e2 ? "1:1" : "start" in e2 || "end" in e2 ? Qe(e2.start) + "-" + Qe(e2.end) : Qe(e2), r2.place = void 0, e2 && (r2.place = e2), r2.reason = b2, r2.ruleId = void 0, t2.ruleId && (r2.ruleId = t2.ruleId), r2.source = void 0, t2.source && (r2.source = t2.source), r2.actual = void 0, r2.expected = void 0, r2.note = void 0, r2.url = void 0, r2.stack = c2 && "string" == typeof t2.cause.stack ? t2.cause.stack : "";
    return r2;
  };
  var hi = (r2) => {
    var b2 = r2.label, e2 = r2.identifier;
    if (b2 || !e2) return "string" == typeof b2 ? b2 : "";
    r2 = E(e2), b2 = function(r3, b3, e3) {
      var c2 = E(r3), t2 = b3;
      if (t2) return t2;
      t2 = E(e3);
      if (35 == (t2.charCodeAt(0) | 0)) {
        var n2 = t2.charCodeAt(1) | 0;
        return 120 == n2 || 88 == n2 ? ir(t2.slice(2), 16) : ir(t2.slice(1), 10);
      }
      t2 = nr(t2);
      return t2 || c2;
    };
    return E(r2.replace(Rr, b2));
  };
  var Hn = (r2, b2, e2) => {
    if (e2) {
      var n2 = e2.line;
      n2 = n2 ? +n2 : 1;
      var c2 = e2.column;
      c2 = c2 ? +c2 : 1;
      var f2 = e2.offset;
      e2 = f2 ? +f2 : 0;
    } else {
      n2 = 1, c2 = 1, e2 = 0;
    }
    var t2 = [-1, 0, n2, c2, e2], w2 = {};
    n2 = [];
    var E2 = [], o2 = [];
    e2 = {};
    var a2, i2;
    c2 = function() {
      var r3 = t2[2] + "";
      if (r3 in Object(w2) && t2[3] < 2) {
        t2[3] = +De(w2, r3);
        var b3 = t2[4];
        t2[4] = b3 + +De(w2, r3) - 1;
      }
    };
    var x2 = function(r3, b3) {
      r3.resolveAll && !M(n2, r3) && n2.push(r3);
      if (r3.resolve) {
        var t3 = e2.events, c3 = r3.resolve, f3 = r3.resolve(t3.slice(b3), e2);
        c3 = q(t3), Y(t3, b3, c3 - b3 | 0, f3);
      }
      r3.resolveTo && (t3 = r3.resolveTo(e2.events, e2), e2.events = t3);
    };
    f2 = function(r3, b3) {
      return function(n3, f3, E3) {
        var p2, u3, l2, w3, i3 = [], x3 = 0, S2 = 0, T2 = function(b4) {
          r3(p2, u3, S2);
          return f3;
        }, y2 = function(r4) {
          u3(), x3++;
          return x3 < i3.length ? l2(i3[x3]) : E3;
        };
        l2 = function(r4) {
          return function(n4) {
            var f4 = t2, i4 = [f4[0], f4[1], f4[2], f4[3], f4[4]], w4 = e2.previous, x4 = e2.currentConstruct;
            S2 = q(e2.events);
            var E4 = o2;
            E4 = Array.from(E4), u3 = function() {
              t2 = i4, e2.previous = w4, e2.currentConstruct = x4, e2.events.length = S2, o2 = E4, c2();
            }, p2 = r4, r4.partial || (e2.currentConstruct = r4);
            if (r4.name && (f4 = e2.parser.constructs.disable.null, f4.includes(r4.name))) return y2(n4);
            var l3 = b3 ? L(Object.create(e2), b3) : e2;
            return r4.tokenize.call(l3, a2, T2, y2)(n4);
          };
        }, w3 = function(r4) {
          i3 = r4, x3 = 0;
          return 0 == i3.length ? E3 : l2(i3[0]);
        };
        return Array.isArray(n3) ? w3(n3) : "tokenize" in Object(n3) ? w3([n3]) : function(r4) {
          var b4, e3;
          A(r4) || (b4 = De(n3, r4), e3 = n3.null);
          var t3 = [];
          Array.isArray(b4) ? t3 = b4 : !b4 || (t3 = [b4]), b4 = [], Array.isArray(e3) ? b4 = e3 : !e3 || (b4 = [e3]);
          return w3([...t3, ...b4])(r4);
        };
      };
    };
    var u2 = function(r3, b3, e3) {
      b3();
    }, I2 = f2(function(r3, b3, e3) {
      x2(r3, e3);
    }, void 0), s2 = f2(u2, void 0);
    a2 = { attempt: I2, check: s2, consume: function(r3) {
      if (T(r3)) {
        var b3 = t2[2];
        t2[2] = b3 + 1, t2[3] = 1, t2[4] = t2[4] + (r3 === -3 ? 2 : 1), c2();
      } else {
        r3 === -1 || (t2[3] = t2[3] + 1, t2[4] = t2[4] + 1);
      }
      if (t2[0] < 0) t2[1] = t2[1] + 1;
      else {
        t2[0] = t2[0] + 1, b3 = E2[t2[1]].length, t2[0] == b3 && (t2[0] = -1, t2[1] = t2[1] + 1);
      }
      e2.previous = r3;
    }, enter: function(r3, b3) {
      b3 = b3 || {}, b3.type = r3, b3.start = rr(t2), e2.events.push(["enter", b3, e2]), o2.push(b3);
      return b3;
    }, exit: function(r3) {
      let b3 = o2.pop();
      b3.end = rr(t2), e2.events.push(["exit", b3, e2]);
      return b3;
    }, interrupt: f2(u2, { interrupt: true }) }, f2 = null, e2.code = f2, e2.containerState = {}, e2.defineSkip = function(r3) {
      nt(w2, r3.line, r3.column), c2();
    }, e2.events = [], e2.now = function() {
      return rr(t2);
    }, e2.parser = r2, e2.previous = f2, e2.sliceSerialize = function(r3, b3) {
      return Un(ut(E2, r3), b3);
    }, e2.sliceStream = function(r3) {
      return ut(E2, r3);
    }, e2.write = function(r3) {
      for (E2 = Z(E2, r3); ; ) {
        var c3 = t2[1];
        if (c3 >= q(E2)) break;
        c3 = E2[t2[1]];
        if ("string" == typeof c3) {
          var w3 = t2[1];
          t2[0] < 0 && (t2[0] = 0);
          for (; ; ) {
            r3 = t2[1] == w3 && t2[0] < c3.length;
            if (!r3) break;
            i2 = i2(c3.charCodeAt(t2[0]) | 0);
          }
        } else i2 = i2(c3);
      }
      var f3 = E2;
      if (!A(f3[q(E2) - 1])) return [];
      x2(b2, 0), f3 = er(n2, e2.events, e2), e2.events = f3;
      return e2.events;
    }, i2 = b2.tokenize.call(e2, a2), !b2.resolveAll || n2.push(b2);
    return e2;
  };
  var st = (r2, b2, e2, t2, n2, c2, f2, E2, i2) => {
    var q2 = 1 / 0;
    0 != i2 && (q2 = i2);
    var o2, x2, w2, a2 = 0;
    i2 = function(b3) {
      return 60 === b3 || 62 === b3 || 92 === b3 ? (r2.consume(b3), o2) : o2(b3);
    }, o2 = function(b3) {
      if (62 === b3) return r2.exit("chunkString"), r2.exit(E2), x2(b3);
      if (A(b3) || 60 === b3 || T(b3)) return e2(b3);
      r2.consume(b3);
      return 92 === b3 ? i2 : o2;
    }, x2 = function(e3) {
      if (62 === e3) return r2.enter(c2), r2.consume(e3), r2.exit(c2), r2.exit(n2), r2.exit(t2), b2;
      r2.enter(E2);
      var f3 = r2.enter;
      r2.enter("chunkString", { contentType: "string" });
      return o2(e3);
    };
    var u2 = function(b3) {
      return 40 === b3 || 41 === b3 || 92 === b3 ? (r2.consume(b3), w2) : w2(b3);
    };
    w2 = function(n3) {
      if (0 == a2 && (A(n3) || 41 === n3 || H(n3))) return r2.exit("chunkString"), r2.exit(E2), r2.exit(f2), r2.exit(t2), b2(n3);
      if (a2 < q2 && 40 === n3) return r2.consume(n3), a2++, w2;
      if (41 === n3) return r2.consume(n3), a2--, w2;
      if (A(n3) || 32 === n3 || 40 === n3 || Xe(n3)) return e2(n3);
      r2.consume(n3);
      return 92 === n3 ? u2 : w2;
    };
    return function(b3) {
      if (60 === b3) return r2.enter(t2), r2.enter(n2), r2.enter(c2), r2.consume(b3), r2.exit(c2), x2;
      if (A(b3) || 32 === b3 || 41 === b3 || Xe(b3)) return e2(b3);
      r2.enter(t2), r2.enter(f2), r2.enter(E2);
      var i3 = r2.enter;
      r2.enter("chunkString", { contentType: "string" });
      return w2(b3);
    };
  };
  var ft = (r2, b2, e2, t2, n2, c2) => {
    var E2, i2, w2, f2 = 0;
    let o2 = function(b3) {
      return b3 == f2 || 92 === b3 ? (r2.consume(b3), E2) : E2(b3);
    };
    E2 = function(b3) {
      if (b3 == f2 || A(b3) || T(b3)) return r2.exit("chunkString"), i2(b3);
      r2.consume(b3);
      return 92 === b3 ? o2 : E2;
    }, i2 = function(b3) {
      if (b3 == f2) return r2.exit(c2), w2(f2);
      if (A(b3)) return e2(b3);
      if (T(b3)) return r2.enter("lineEnding"), r2.consume(b3), r2.exit("lineEnding"), R(r2, i2, "linePrefix", 0);
      r2.enter("chunkString", { contentType: "string" });
      return E2(b3);
    }, w2 = function(e3) {
      if (e3 == f2) return r2.enter(n2), r2.consume(e3), r2.exit(n2), r2.exit(t2), b2;
      r2.enter(c2);
      return i2(e3);
    };
    return function(b3) {
      return 34 === b3 || 39 === b3 || 40 === b3 ? (r2.enter(t2), r2.enter(n2), r2.consume(b3), r2.exit(n2), f2 = 40 === b3 ? 41 : b3 | 0, w2) : e2(b3);
    };
  };
  var we = (r2, b2) => {
    var t2 = false, e2 = function(n2) {
      return T(n2) ? (r2.enter("lineEnding"), r2.consume(n2), r2.exit("lineEnding"), t2 = true, e2) : C(n2) ? R(r2, e2, t2 ? "linePrefix" : "lineSuffix", 0)(n2) : b2(n2);
    };
    return e2;
  };
  var R = (r2, b2, e2, t2) => {
    var c2 = 1 / 0;
    0 != t2 && (c2 = t2 - 1);
    var n2 = 0, f2 = function(t3) {
      if (C(t3) && n2 < c2) return n2 = n2 + 1, r2.consume(t3), f2;
      r2.exit(e2);
      return b2(t3);
    };
    return function(t3) {
      return C(t3) ? (r2.enter(e2), f2(t3)) : b2(t3);
    };
  };
  var Tt = (r2, b2) => {
    b2 = b2 || {};
    var n2 = [], c2 = L({}, _), f2 = xt(Mr), i2 = {}, w2 = [], e2 = { handlers: c2, indexStack: n2, join: f2, options: i2, stack: w2, unsafe: xt(I), associationId: function(r3) {
      return hi(r3);
    }, containerPhrasing: function(r3, b3) {
      return ((r4, b4, e3) => {
        var l2 = b4.indexStack, A2 = r4.children || [], c3 = [], i3 = E(e3.before);
        l2.push(-1);
        for (var w3, t3, n3, f3, a2, o2 = b4.createTracker(e3), p2 = void 0, S2 = q(A2), x2 = -1, u2 = p2; ++x2 < S2; ) {
          w3 = A2[x2], l2[q(l2) - 1] = x2, (x2 + 1 | 0) < S2 ? (t3 = b4.handle.handlers, n3 = t3[E(A2[x2 + 1].type)], n3 && n3.peek && (n3 = t3 = n3.peek), n3 ? (f3 = L({}, o2.current()), f3.before = "", f3.after = "", t3 = [], a2 = A2[x2 + 1], t3.push(a2), t3.push(r4), t3.push(b4), t3.push(f3), f3 = E(n3.apply(void 0, t3)), f3 = f3.length > 0 ? f3.charAt(0) : "") : f3 = "") : f3 = E(e3.after), q(c3) > 0 && ("\r" == i3 || "\n" == i3) && "html" == E(w3.type) && (o2 = E(c3[q(c3) - 1]), i3 = q(c3) - 1, c3[i3] = o2.replace(Sn, " "), o2 = b4.createTracker(e3), i3 = o2.move, o2.move(c3.join("")), i3 = " ");
          var m2 = o2.current, y2 = o2.current();
          n3 = L({}, y2), n3.after = f3, n3.before = i3, t3 = [];
          var v2 = t3.push;
          t3.push(w3);
          var h2 = t3.push;
          t3.push(r4);
          var g2 = t3.push;
          t3.push(b4);
          var I2 = t3.push;
          t3.push(n3), w3 = b4.handle;
          var s2 = w3.apply, T2 = w3.apply(b4, t3);
          n3 = E(T2), u2 && E(u2) == n3.slice(0, 1) && (n3 = oe(E(u2).charCodeAt(0) | 0) + n3.slice(1)), w3 = b4.attentionEncodeSurroundingInfo, b4.attentionEncodeSurroundingInfo = void 0, u2 = void 0, w3 && (q(c3) > 0 && w3.before && (t3 = E(c3[q(c3) - 1]), a2 = t3.length > 0 ? t3.slice(t3.length - 1) : "", i3 == a2 && (a2 = q(c3) - 1, t3 = t3.slice(0, t3.length - 1), c3[a2] = t3 + oe(i3.charCodeAt(0) | 0))), !w3.after || (u2 = f3)), o2.move(n3), c3.push(n3), i3 = n3.length > 0 ? n3.slice(n3.length - 1) : "";
        }
        l2.pop();
        return E(c3.join(""));
      })(r3, this, b3);
    }, containerFlow: function(r3, b3) {
      return ((r4, b4, e3) => {
        var c3 = b4.indexStack, w3 = r4.children || [], f3 = b4.createTracker(e3), o2 = [], x2 = q(w3);
        c3.push(-1);
        for (var i3, t3, a2, n3 = -1; ++n3 < x2; ) i3 = w3[n3], c3[q(c3) - 1] = n3, t3 = L({}, f3.current()), t3.before = "\n", t3.after = "\n", e3 = [i3, r4, b4, t3], a2 = f3.move, t3 = b4.handle, a2 = f3.move(t3.apply(b4, e3)), o2.push(a2), "list" != E(i3.type) && (b4.bulletLastUsed = void 0), n3 < (x2 - 1 | 0) && (e3 = f3.move(((r5, b5, e4, t4) => {
          for (var c4 = q(t4.join); --c4 >= 0; ) {
            var f4 = t4.join[c4], n4 = f4.apply(void 0, [r5, b5, e4, t4]);
            if (true === n4 || 1 === n4) break;
            if ("number" == typeof n4) return "\n".repeat(1 + n4 | 0);
            if (false === n4) return "\n\n<!---->\n\n";
          }
          return "\n\n";
        })(i3, w3[n3 + 1], r4, b4)), o2.push(e3));
        c3.pop();
        return E(o2.join(""));
      })(r3, this, b3);
    }, createTracker: function(r3) {
      return ((r4) => {
        r4 = r4 || {};
        var t3 = r4.now || {}, b3 = 0;
        r4.lineShift && (b3 = +r4.lineShift), r4 = t3.line || 1;
        var c3 = t3.column || 1, n3 = +r4, e3 = +c3;
        return { move: function(r5) {
          var t4 = r5 ? E(r5) : "";
          r5 = t4.split(Tn);
          var c4 = r5.length, f3 = c4 - 1, i3 = r5[f3] || "";
          n3 += f3, e3 = 1 == c4 ? e3 + i3.length : 1 + i3.length + b3;
          return t4;
        }, current: function() {
          return { now: { line: n3, column: e3 }, lineShift: b3 };
        }, shift: function(r5) {
          let e4 = b3;
          b3 = e4 + +r5;
        } };
      })(r3);
    }, compilePattern: function(r3) {
      return ((r4) => {
        if (!r4._compiled) {
          var e3, b3 = r4.atBreak ? "[\\r\\n][\\t ]*" : "";
          e3 = r4.before, e3 && (b3 = b3 + "(?:" + E(e3) + ")"), b3 = b3.length > 0 ? "(" + b3 + ")" : "", e3 = E(r4.character), wn.test(e3) && (b3 = b3 + "\\"), b3 += e3, e3 = r4.after, e3 && (b3 = b3 + "(?:" + E(e3) + ")"), r4._compiled = new RegExp(b3, "g");
        }
        return r4._compiled;
      })(r3);
    } };
    e2.enter = function(r3) {
      e2.stack.push(r3);
      return function() {
        e2.stack.pop();
      };
    }, e2.indentLines = function(r3, b3) {
      return ((r4, b4) => {
        var n3 = E(r4);
        r4 = [];
        for (var t3, e3 = 0, c3 = 0; true; c3++) {
          t3 = En.exec(n3);
          if (t3 == null) break;
          e3 = n3.slice(e3, t3.index | 0), e3 = b4(e3, c3, 0 == e3.length), r4.push(e3), e3 = t3[0], r4.push(e3), e3 = (t3.index | 0) + E(t3[0]).length;
        }
        t3 = n3.slice(e3);
        b4 = b4(t3, c3, 0 == t3.length), r4.push(b4);
        return E(r4.join(""));
      })(r3, b3);
    }, e2.safe = function(r3, b3) {
      return ((r4, b4, e3) => {
        var i3 = e3.before ? E(e3.before) : "";
        b4 = b4 ? E(b4) : "";
        var x2 = e3.after ? E(e3.after) : "", w3 = i3 + b4 + x2, o2 = [];
        b4 = [];
        for (var t3, S2, l2, f3, A2, c3, a2, u2 = {}, p2 = r4.unsafe, y2 = q(p2), n3 = -1; ++n3 < y2; ) {
          t3 = p2[n3];
          if (!!kt(r4.stack, t3)) {
            S2 = r4.compilePattern(t3);
            while (true) {
              l2 = S2.exec(w3);
              if (l2 == null) break;
              f3 = F(t3, "before") || t3.atBreak, A2 = F(t3, "after"), c3 = l2.index | 0, f3 && (c3 = c3 + E(l2[1]).length), M(o2, c3) ? (a2 = u2[c3 + ""], a2.before && !f3 && (a2.before = false), a2.after && !A2 && (a2.after = false)) : (o2.push(c3), u2[c3 + ""] = { before: f3, after: A2 });
            }
          }
        }
        o2.sort(An);
        for (r4 = i3.length > 0 ? i3.length : 0, i3 = w3.length, x2.length > 0 && (i3 = w3.length - x2.length), x2 = q(o2), c3 = -1, n3 = r4; ++c3 < x2; ) {
          r4 = +o2[c3] | 0;
          if (!(r4 < n3 || r4 >= i3)) {
            (r4 + 1 | 0) < i3 && (c3 + 1 | 0) < x2 && o2[c3 + 1] === (r4 + 1 | 0) ? (f3 = u2[r4 + ""], t3 = u2[(r4 + 1 | 0) + ""], t3 = f3.after && !t3.before && !t3.after) : t3 = false, !t3 && c3 > 0 && o2[c3 - 1] === (r4 - 1 | 0) && (a2 = u2[r4 + ""], f3 = u2[(r4 - 1 | 0) + ""], a2.before && !f3.before && !f3.after && (t3 = true));
            if (!t3) n3 != r4 && (n3 = At(w3.slice(n3, r4), "\\"), b4.push(n3)), n3 = w3.charAt(r4), t3 = e3.encode, t3 = t3 && M(t3, n3), qn.test(n3) && !t3 ? b4.push("\\") : (n3 = oe(w3.charCodeAt(r4) | 0), b4.push(n3), r4++), n3 = r4;
          }
        }
        r4 = e3.after ? E(e3.after) : "";
        r4 = At(w3.slice(n3, i3), r4), b4.push(r4);
        return E(b4.join(""));
      })(this, r3, b3);
    }, bt(e2, b2);
    if (e2.options.tightDefinitions) {
      b2 = e2.join;
      var t2 = function(r3, b3) {
        if ("definition" == E(r3.type)) {
          var e3 = E(r3.type);
          e3 = e3 == E(b3.type);
        } else {
          e3 = false;
        }
        if (e3) return 0;
      };
      b2.push(t2);
    }
    b2 = function(r3) {
      throw new ve("Cannot handle value `" + E(r3) + "`, expected node");
    };
    t2 = function(r3) {
      throw new ve("Cannot handle unknown node `" + E(r3.type) + "`");
    }, e2.handle = ((r3) => {
      V(r3) && (r3 = {});
      var b3 = (0, function() {
        var r4;
        arguments.length > 0 && (r4 = arguments[0]);
        var e4 = b3.invalid, t3 = b3.handlers;
        if (r4 && F(r4, "type")) {
          var n3 = E(r4.type);
          e4 = F(t3, n3) ? t3[n3] : b3.unknown;
        }
        if (e4) return e4.apply(this, arguments);
      }), e3 = r3.handlers;
      V(e3) && (e3 = {}), b3.handlers = e3, b3.invalid = r3.invalid, b3.unknown = r3.unknown;
      return b3;
    })({ invalid: b2, unknown: t2, handlers: e2.handlers }), b2 = [r2, void 0, e2, { before: "\n", after: "\n", now: { line: 1, column: 1 }, lineShift: 0 }], t2 = e2.handle, r2 = E(t2.apply(e2, b2)), r2.length > 0 && (b2 = r2.charCodeAt(r2.length - 1) | 0, 10 != b2 && 13 != b2 && (r2 = r2 + "\n"));
    return r2;
  };
  var At = (r2, b2) => {
    var t2 = new RegExp("\\\\(?=[!-/:-@[-`{-~])", "g"), c2 = [], n2 = [], e2 = r2 + b2;
    while (true) {
      b2 = t2.exec(e2);
      if (b2 == null) break;
      var f2 = b2.index;
      c2.push(f2);
    }
    for (f2 = q(c2), e2 = 0, b2 = -1; ++b2 < f2; ) t2 = +c2[b2] | 0, e2 != t2 && (e2 = r2.slice(e2, t2), n2.push(e2)), n2.push("\\"), e2 = t2;
    r2 = r2.slice(t2), n2.push(r2);
    return E(n2.join(""));
  };
  var ar = (r2, b2) => {
    var t2 = it(b2);
    b2 = t2.includeImageAlt;
    var e2 = t2.includeHtml;
    "boolean" == typeof b2 || (b2 = true);
    "boolean" == typeof e2 || (e2 = true);
    return E(Jt(r2, b2, e2));
  };
  var ze = (r2, b2, e2) => {
    var t2 = Pe(r2);
    r2 = Pe(b2);
    return t2 === void 0 ? r2 === void 0 ? "_" == e2 ? { inside: true, outside: true } : { inside: false, outside: false } : 1 === r2 ? { inside: true, outside: true } : { inside: false, outside: true } : 1 === t2 ? r2 === void 0 ? { inside: false, outside: false } : 1 === r2 ? { inside: true, outside: true } : { inside: false, outside: false } : r2 === void 0 ? { inside: false, outside: false } : 1 === r2 ? { inside: true, outside: false } : { inside: false, outside: false };
  };
  var A = (r2) => r2 === null;
  var V = (r2) => r2 == null;
  var Zr = (r2) => r2 != null && "object" == typeof r2 && "href" in r2 && r2.href && "protocol" in r2 && r2.protocol && r2.auth === void 0;
  var Kr = (r2) => {
    (!Ue(r2) || "string" != typeof r2.type) && le("Expected node, got `" + r2 + "`");
  };
  var Wr = (r2) => {
    if ("string" != typeof r2) throw new TypeError("Path must be a string. Received " + JSON.stringify(r2));
  };
  var W = (r2) => {
    throw new ve(r2);
  };
  var le = (r2) => {
    throw new TypeError(r2);
  };
  var je = (r2) => {
    throw new Error(r2);
  };
  var at = (r2) => {
    throw new Error(r2);
  };
  var Hr = (r2) => {
    if (!r2) throw new Error("we either bailed on an error or have a tree");
  };
  var He = (r2, b2) => {
    if (r2 && r2.includes("/")) throw new Error("`" + b2 + "` cannot be a path: did not expect `/`");
  };
  var Ge = (r2, b2) => {
    if (!r2) throw new Error("`" + b2 + "` cannot be empty");
  };
  var ne = (r2, b2) => {
    Object.defineProperty(Ee, r2, b2);
  };
  var re = (r2, b2) => {
    Object.defineProperty(xr, r2, { configurable: true, writable: true, value: b2 });
  };
  var Qr = (r2, b2, e2) => {
    if ("__proto__" == b2) {
      Object.defineProperty(r2, "__proto__", { enumerable: true, configurable: true, writable: true, value: e2 });
      return;
    }
    r2[b2] = e2;
  };
  var ee = (r2, b2, e2) => {
    Object.defineProperty(r2, "name", { configurable: true, value: b2 }), Object.defineProperty(r2, "length", { configurable: true, value: e2 });
  };
  var ye = (r2) => {
    let b2 = Object.getOwnPropertyDescriptor(Ee, r2), e2 = b2.get, t2 = b2.set;
    Object.defineProperty(e2, "name", { configurable: true, value: "get " + r2 }), Object.defineProperty(t2, "name", { configurable: true, value: "set " + r2 });
  };
  var Yr = (r2, b2) => "__proto__" == b2 ? !he.call(r2, b2) ? void 0 : Object.getOwnPropertyDescriptor(r2, b2).value : r2[b2];
  var ce = (r2) => {
    var b2 = r2.history;
    return 0 == b2.length ? void 0 : b2[b2.length - 1];
  };
  var Le = (r2, b2) => {
    Wr(r2), Wr(b2);
    var e2 = r2 + "";
    r2 = b2 + "", e2.length > 0 || (e2 = ""), r2.length > 0 ? e2.length > 0 && (r2 = e2 + "/" + r2) : r2 = e2;
    return 0 == r2.length ? "." : zn(r2);
  };
  var ie = (r2, b2) => {
    var t2 = r2.left, n2 = r2.right, e2 = t2.length, c2 = n2.length;
    if (!(b2 == e2 || b2 > e2 && 0 == c2 || b2 < 0 && 0 == e2)) b2 < e2 ? (r2 = ue.POSITIVE_INFINITY, r2 = t2.splice(b2, r2), r2.reverse(), xe(n2, r2)) : (r2 = e2 + c2 - b2, b2 = ue.POSITIVE_INFINITY, r2 = n2.splice(r2, b2), r2.reverse(), xe(t2, r2));
  };
  var tr = (r2, b2) => {
    var e2 = b2[3];
    e2 && e2.write([null]), b2[4] = void 0, b2[3] = void 0, r2.containerState._closeFlow = void 0;
  };
  var ke = (r2, b2, e2, t2) => {
    var n2 = e2.length;
    while (n2 > t2) {
      n2--;
      var c2 = e2[n2];
      r2.containerState = c2.state, c2.construct.exit.call(r2, b2);
    }
    while (e2.length > t2) e2.pop();
  };
  var Jr = (r2, b2) => {
    var t2, n2, f2, c2, e2 = r2.length;
    if (0 == b2.length || b2.length > r2.length) {
      for (b2 = -1, t2 = false; ; ) {
        if (e2 <= 0) {
          e2 = 0;
          break;
        }
        e2--;
        if ("/" == r2.charAt(e2)) {
          if (t2) {
            e2++;
            break;
          }
        } else b2 < 0 && (b2 = e2 + 1, t2 = true);
      }
      return b2 < 0 ? "" : r2.slice(e2, b2);
    }
    if (b2 == r2) return "";
    for (t2 = -1, f2 = false, c2 = -1, n2 = b2.length - 1; ; ) {
      if (e2 <= 0) {
        b2 = 0;
        break;
      }
      e2--;
      if ("/" == r2.charAt(e2)) {
        if (f2) {
          b2 = e2 + 1;
          break;
        }
      } else c2 < 0 && (f2 = true, c2 = e2 + 1), n2 > -1 && (r2.charAt(e2) == b2.charAt(n2) ? (n2 = n2 - 1 | 0, n2 < 0 && (t2 = e2)) : (t2 = c2, n2 = -1));
    }
    b2 == t2 ? t2 = c2 : t2 < 0 && (t2 = r2.length);
    return r2.slice(b2, t2);
  };
  var zn = (r2) => {
    var e2 = "/" == r2.charAt(0), b2 = ((r3, b3) => {
      var E2, e3 = "", c2 = 0, n2 = -1, f2 = 0, t2 = 0;
      while (t2 <= r3.length) {
        E2 = t2 < r3.length ? r3.charAt(t2) : "/";
        if ("/" == E2) {
          if (!(n2 == t2 - 1 || 1 == f2)) if (n2 != t2 - 1 && 2 == f2) {
            if (e3.length < 2 || 2 != c2 || "." != e3.charAt(e3.length - 1) || "." != e3.charAt(e3.length - 2)) {
              if (e3.length > 2) {
                n2 = e3.lastIndexOf("/");
                if (n2 != e3.length - 1) {
                  n2 < 0 ? (e3 = "", c2 = 0) : (e3 = e3.slice(0, n2), c2 = e3.length - 1 - e3.lastIndexOf("/") | 0), n2 = t2, f2 = 0, t2++;
                  continue;
                }
              } else if (e3.length > 0) {
                e3 = "", c2 = 0, n2 = t2, f2 = 0, t2++;
                continue;
              }
            }
            b3 && (e3 = e3.length > 0 ? e3 + "/.." : "..", c2 = 2);
          } else {
            c2 = r3.slice(n2 + 1 | 0, t2), e3 = e3.length > 0 ? e3 + "/" + c2 : c2, c2 = (t2 - n2 | 0) - 1 | 0;
          }
          n2 = t2;
          f2 = 0;
        } else {
          f2 = "." == E2 && f2 > -1 ? f2 + 1 | 0 : -1;
        }
        t2++;
      }
      return e3;
    })(r2, !e2);
    0 == b2.length && !e2 && (b2 = "."), b2.length > 0 && "/" == r2.charAt(r2.length - 1) && (b2 = b2 + "/");
    return e2 ? "/" + b2 : b2;
  };
  var be = (r2, b2) => {
    if (Zr(b2)) {
      if ("file:" != b2.protocol + "") {
        r2 = new TypeError("The URL must be of scheme file"), r2.code = "ERR_INVALID_URL_SCHEME";
        throw r2;
      }
      if ((b2.hostname + "").length > 0) {
        r2 = new TypeError('File URL host must be "localhost" or empty on darwin'), r2.code = "ERR_INVALID_FILE_URL_HOST";
        throw r2;
      }
      var e2 = b2.pathname + "";
      b2 = 0;
      while (b2 < e2.length) {
        if ("%" == e2.charAt(b2) && "2" == e2.charAt(b2 + 1) && ("F" == e2.charAt(b2 + 2) || "f" == e2.charAt(b2 + 2))) {
          r2 = new TypeError("File URL path must not include encoded / characters"), r2.code = "ERR_INVALID_FILE_URL_PATH";
          throw r2;
        }
        b2++;
      }
      b2 = globalThis.decodeURIComponent(e2);
    }
    Ge(b2, "path");
    ce(r2) === b2 || Array.prototype.push.call(r2.history, b2);
  };
  var xt = (r2) => {
    for (var e2 = [], t2 = q(r2), b2 = 0; b2 < t2; b2++) Array.prototype.push.call(e2, r2[b2]);
    return e2;
  };
  var Ke = (r2, b2, e2) => {
    for (var c2, f2, n2 = r2.length, t2 = -1; ; ) {
      if (false) {
        t2 = -1;
        break;
      }
      t2++;
      if (t2 >= n2) {
        t2 = -1;
        break;
      }
      if (r2[t2][0] === b2) break;
    }
    if (t2 == -1) {
      for (t2 = [], t2.push(b2), n2 = e2.length, b2 = 0; b2 < n2; b2++) Array.prototype.push.call(t2, e2[b2]);
      Array.prototype.push.call(r2, t2);
      return;
    }
    if (e2.length > 0) {
      for (n2 = e2[0], f2 = se.call(e2, 1), c2 = r2[t2][1], Ue(c2) && Ue(n2) && (n2 = Ie(c2, n2)), e2 = [], e2.push(b2), e2.push(n2), n2 = f2.length, b2 = 0; b2 < n2; b2++) Array.prototype.push.call(e2, f2[b2]);
      Array.prototype.splice.call(r2, t2, 1, e2);
    }
  };
  var It = (r2, b2) => {
    if ("function" == typeof r2.data) return r2.data(b2);
  };
  var Ye = (r2, b2) => {
    "function" == typeof b2 || le("Cannot `" + r2 + "` without `parser`");
  };
  var Ze = (r2, b2) => {
    "function" == typeof b2 || le("Cannot `" + r2 + "` without `compiler`");
  };
  var rt = (r2, b2, e2) => {
    !("plugins" in e2) && !("settings" in e2) && je("Expected usable value but received an empty preset, which is probably a mistake: presets typically come with `plugins` and sometimes with `settings`, but this has neither"), et(r2, b2, e2.plugins);
    if (r2 = e2.settings) {
      var t2 = Ie(b2.settings, r2);
      b2.settings = t2;
    }
  };
  var pt = (r2, b2, e2) => {
    for (var c2 = q(r2), n2 = "", t2 = 0; t2 < c2; t2++) n2 += E(Me(r2[t2], b2, e2));
    return n2;
  };
  var Y = (r2, b2, e2, t2) => {
    var n2 = r2.length;
    b2 = +b2, e2 = +e2, b2 < 0 ? n2 = 0 - b2 > n2 ? 0 : n2 + b2 : b2 > n2 || (n2 = b2), e2 < 0 && (e2 = 0);
    var c2 = q(t2);
    if (c2 < 1e4) b2 = Array.from(t2), b2.unshift(n2, e2), e2 = r2.splice, e2.apply(r2, b2);
    else {
      e2 > 0 && r2.splice(n2, e2), b2 = 0;
      while (b2 < c2) e2 = b2 + 1e4 | 0, b2 = t2.slice(b2, e2), b2.unshift(n2, 0), r2.splice.apply(r2, b2), n2 += 1e4, b2 = e2;
    }
  };
  var xe = (r2, b2) => {
    var t2 = q(b2);
    if (t2 < 1e4) {
      var e2 = Array.from(b2);
      b2 = r2.push, b2.apply(r2, e2);
    } else {
      e2 = 0;
      while (e2 < t2) {
        var n2 = e2 + 1e4 | 0, c2 = b2.slice(e2, n2);
        e2 = r2.push, e2.apply(r2, c2), e2 = n2;
      }
    }
  };
  var dt = (r2, b2) => {
    for (var e2, n2 = q(b2), t2 = -1; ++t2 < n2; ) e2 = b2[t2], Array.isArray(e2) ? dt(r2, e2) : ((r3, b3) => {
      for (var e3 in b3) if (!!F(b3, e3)) if ("canContainEols" == e3) {
        var t3 = b3[e3];
        if (t3) {
          var n3 = r3[e3], c2 = q(t3);
          for (e3 = 0; e3 < c2; e3++) {
            var f2 = t3[e3];
            n3.push(f2);
          }
        }
      } else if ("transforms" == e3) {
        if (t3 = b3[e3]) for (n3 = r3[e3], c2 = q(t3), e3 = 0; e3 < c2; e3++) f2 = t3[e3], n3.push(f2);
      } else ("enter" == e3 || "exit" == e3) && (t3 = b3[e3], !t3 || L(r3[e3], t3));
    })(r2, e2);
  };
  var et = (r2, b2, e2) => {
    if (e2 != null) {
      Array.isArray(e2) || le("Expected a list of plugins, not `" + e2 + "`");
      for (var n2 = e2.length, t2 = -1; ++t2 < n2; ) ((r3, b3, e3) => {
        if ("function" == typeof e3) {
          Ke(r3, e3, []);
          return;
        }
        if ("object" == typeof e3) {
          if (Array.isArray(e3)) {
            b3 = e3[0], Ke(r3, b3, se.call(e3, 1));
            return;
          }
          rt(r3, b3, e3);
          return;
        }
        le("Expected usable value, not `" + e3 + "`");
      })(r2, b2, e2[t2]);
    }
  };
  var tt = (r2) => {
    for (var b2 = sr(), t2 = r2.attachers, n2 = t2.length, e2 = -1; ++e2 < n2; ) b2.use.apply(b2, t2[e2]);
    e2 = b2.data, b2.data(Ie({}, r2.namespace));
    return b2;
  };
  var er = (r2, b2, e2) => {
    for (var t2, n2 = [], f2 = q(r2), c2 = -1; ++c2 < f2; ) t2 = r2[c2].resolveAll, "function" == typeof t2 && !M(n2, t2) && (b2 = t2(b2, e2), n2.push(t2));
    return b2;
  };
  var Un = (r2, b2) => {
    for (var e2, t2, n2 = [], i2 = q(r2), c2 = -1, f2 = false; ++c2 < i2; ) {
      e2 = r2[c2];
      if ("string" == typeof e2) t2 = E(e2);
      else if (e2 === -5) t2 = "\r";
      else if (e2 === -4) t2 = "\n";
      else if (e2 === -3) t2 = "\r\n";
      else if (e2 === -2) t2 = b2 ? " " : "	";
      else if (e2 === -1) {
        if (!b2 && f2) continue;
        t2 = " ";
      } else {
        t2 = pe(e2);
      }
      f2 = e2 === -2;
      n2.push(t2);
    }
    return n2.join("");
  };
  var yt = (r2, b2, e2) => {
    if ("string" == typeof b2) {
      var t2 = [b2];
      b2 = t2;
    }
    if (V(b2) || 0 == q(b2)) return e2;
    for (t2 = q(b2), e2 = -1; ++e2 < t2; ) if (M(r2, b2[e2])) return true;
    return false;
  };
  var bt = (r2, b2) => {
    if (b2) {
      var t2 = b2.extensions;
      if (t2) for (var n2 = q(t2), e2 = -1; ++e2 < n2; ) bt(r2, t2[e2]);
      for (e2 in b2) F(b2, e2) && ("extensions" == e2 || ("unsafe" == e2 ? mt(r2.unsafe, b2[e2]) : "join" == e2 ? mt(r2.join, b2[e2]) : "handlers" == e2 ? ((r3, b3) => {
        !b3 || L(r3, b3);
      })(r2.handlers, b2[e2]) : r2.options[e2] = b2[e2]));
    }
  };
  var mt = (r2, b2) => {
    if (b2) for (var t2, n2 = q(b2), e2 = 0; e2 < n2; e2++) t2 = b2[e2], r2.push(t2);
  };
  var Z = (r2, b2) => q(r2) > 0 ? (Y(r2, r2.length, 0, b2), r2) : b2;
  var kt = (r2, b2) => {
    r2 = yt(r2, b2.inConstruct, true) && !yt(r2, b2.notInConstruct, false);
    return r2;
  };
  var Q = (r2, b2) => ({ name: r2, tokenize: b2 });
  var U = (r2) => ({ tokenize: r2 });
  var ae = (r2) => {
    let b2 = r2.line, e2 = r2.column;
    return { line: b2, column: e2, offset: r2.offset };
  };
  var z = (r2) => ({ _bufferIndex: r2._bufferIndex, _index: r2._index, line: r2.line, column: r2.column, offset: r2.offset });
  var rr = (r2) => ({ _bufferIndex: r2[0], _index: r2[1], line: r2[2], column: r2[3], offset: r2[4] });
  var pe = (r2) => E(String.fromCharCode(r2));
  var ir = (r2, b2) => {
    r2 = +Number.parseInt(r2, b2) | 0;
    return r2 < 9 || 11 == r2 || r2 > 13 && r2 < 32 || r2 > 126 && r2 < 160 || r2 > 55295 && r2 < 57344 || r2 > 64975 && r2 < 65008 || 65535 == (r2 & 65535) || 65534 == (r2 & 65535) || r2 > 1114111 ? "\uFFFD" : E(String.fromCodePoint(r2));
  };
  var nr = (r2) => F(Er, r2) ? Er[r2] : false;
  var Ce = (r2) => (!r2 || "object" != typeof r2 ? false : "message" in r2 && "messages" in r2) ? r2 : new ge(r2);
  var O = (r2) => {
    let b2 = r2.stack;
    return b2[q(b2) - 1];
  };
  var Oe = JSON.parse("null");
  var he = Object.prototype.hasOwnProperty;
  var Lt = Object.prototype.toString;
  var se = Array.prototype.slice;
  var Ct = (0, function() {
    var r2 = se.call(arguments), b2 = Array.prototype.pop.call(r2);
    "function" == typeof b2 || le("Expected function as last argument, not " + b2);
    var e2, t2 = -1, c2 = this.fns;
    e2 = function(n3) {
      t2++;
      var o2;
      t2 < c2.length && (o2 = c2[t2]);
      var f3, i2 = [], a2 = arguments.length;
      if (a2 > 0) {
        f3 = n3;
        for (var w2 = 1; w2 < a2; w2++) i2.push(arguments[w2]);
      }
      if (f3) {
        b2(f3);
        return;
      }
      var E3;
      for (w2 = r2.length, E3 = -1; ++E3 < w2; ) f3 = void 0, E3 < i2.length && (f3 = i2[E3]), f3 == null && (f3 = r2[E3], i2[E3 + ""] = f3);
      r2 = i2;
      if ("function" == typeof o2) (/* @__PURE__ */ ((r3, b3) => {
        var t3 = false;
        let e3 = function() {
          if (!t3) t3 = true, b3.apply(void 0, arguments);
        }, n4 = function(r4) {
          e3.apply(void 0, [Oe, r4]);
        };
        return function() {
          var c3 = se.call(arguments), f4 = r3.length > c3.length;
          f4 && Array.prototype.push.call(c3, e3);
          var b4;
          try {
            b4 = r3.apply(this, c3);
          } catch (r4) {
            if (f4 && t3) throw r4;
            e3.apply(void 0, [r4]);
            return;
          }
          if (!f4) {
            var E4;
            b4 && b4.then && "function" == typeof b4.then ? b4.then(n4, e3) : (b4 == null ? false : "object" != typeof b4 && "function" != typeof b4 ? false : !!Error.prototype.isPrototypeOf(b4)) ? (E4 = [b4], e3.apply(void 0, E4)) : n4(b4);
          }
        };
      })(o2, e2)).apply(void 0, i2);
      else {
        var x2;
        for (E3 = [], E3.push(Oe), x2 = i2.length, f3 = 0; f3 < x2; f3++) Array.prototype.push.call(E3, i2[f3]);
        b2.apply(void 0, E3);
      }
    };
    var f2 = [Oe];
    for (var E2 = r2.length, n2 = 0; n2 < E2; n2++) f2.push(r2[n2]);
    e2.apply(void 0, f2);
  });
  var Dt = (0, function(r2) {
    return "function" == typeof r2 || le("Expected `middelware` to be a function, not " + r2), Array.prototype.push.call(this.fns, r2), this;
  });
  var _e = "history path basename stem extname dirname".split(" ");
  var j = {};
  var n = (0, function(r2, b2, e2) {
    if (this === void 0) throw new TypeError("Class constructor VFileMessage cannot be invoked without 'new'");
    return $r(r2, b2, e2);
  });
  j = n.prototype, Object.setPrototypeOf(n, Error), Object.setPrototypeOf(j, Error.prototype);
  var e = true;
  Object.defineProperty(n, "name", { configurable: e, value: "VFileMessage" }), j.file = "", j.name = "", j.reason = "", j.message = "", j.stack = "", j.column = void 0, j.line = void 0, j.ancestors = void 0, j.cause = void 0, j.fatal = void 0, j.place = void 0, j.ruleId = void 0, j.source = void 0, Object.defineProperty(n, "prototype", { writable: false });
  var ge = class VFile extends Object {
    constructor(r2) {
      super();
      if (this === void 0) throw new TypeError("Class constructor VFile cannot be invoked without 'new'");
      !r2 ? r2 = {} : Zr(r2) ? r2 = { path: r2 } : ("string" == typeof r2 || jr(r2)) && (r2 = { value: r2 });
      var b2 = (() => {
        var r3 = globalThis.process;
        return r3 && "function" == typeof r3.cwd ? r3.cwd() + "" : "/";
      })();
      "cwd" in r2 && (b2 = "");
      this.cwd = b2, this.data = {}, this.history = [], this.messages = [];
      for (var e2, t2 = 0; t2 < _e.length; t2++) b2 = _e[t2] || "", b2 in r2 && r2[b2] != null && r2[b2] !== void 0 && (e2 = r2[b2], "history" == b2 && (e2 = e2.slice()), this[b2] = e2);
      for (b2 in r2) _e.includes(b2) || (this[b2] = r2[b2]);
    }
  };
  var Ee = ge.prototype;
  Object.defineProperty(ge, "name", { configurable: e, value: "VFile" });
  var r = (0, function(r2, b2, e2) {
    r2 = this.message(r2, b2, e2), r2.fatal = true;
    throw r2;
  });
  var t = (0, function(r2, b2, e2) {
    r2 = this.message(r2, b2, e2), r2.fatal = void 0;
    return r2;
  });
  var i = (0, function(r2, b2, e2) {
    r2 = $r(r2, b2, e2);
    if (b2 = ce(this)) {
      var t2 = b2 + ":" + r2.name;
      r2.name = t2, r2.file = b2;
    }
    r2.fatal = false;
    Array.prototype.push.call(this.messages, r2);
    return r2;
  });
  var a = (0, function(r2) {
    var b2 = this.value;
    if (b2 === void 0) return "";
    if ("string" == typeof b2) return b2;
    var e2 = r2 ? new TextDecoder(r2) : new TextDecoder();
    return e2.decode(b2);
  });
  Object.defineProperty(r, "name", { configurable: e, value: "fail" }), Object.defineProperty(t, "name", { configurable: e, value: "info" }), Object.defineProperty(i, "name", { configurable: e, value: "message" }), Object.defineProperty(a, "name", { configurable: e, value: "toString" }), ne("basename", { configurable: e, get: function() {
    var r2 = ce(this);
    if ("string" == typeof r2) return Jr(r2 + "", "");
  }, set: function(r2) {
    Ge(r2, "basename"), He(r2, "basename");
    var b2 = this.dirname || "";
    be(this, Le(b2, r2));
  } }), ne("dirname", { configurable: e, get: function() {
    var r2 = ce(this);
    if ("string" == typeof r2) return ((r3) => {
      if (0 == r3.length) return ".";
      for (var b2 = r3.length, e2 = false; ; ) {
        if (b2 <= 1) {
          b2 = -1;
          break;
        }
        b2--;
        if ("/" == r3.charAt(b2)) {
          if (e2) break;
        } else e2 = e2 || true;
      }
      return b2 < 0 ? "/" == r3.charAt(0) ? "/" : "." : 1 == b2 && "/" == r3.charAt(0) ? "//" : r3.slice(0, b2);
    })(r2 + "");
  }, set: function(r2) {
    var b2 = this.basename;
    if (!b2) throw new Error("Setting `dirname` requires `path` to be set too");
    r2 = r2 || "", be(this, Le(r2, b2));
  } }), ne("extname", { configurable: e, get: function() {
    var r2 = ce(this);
    if ("string" == typeof r2) return ((r3) => {
      for (var f2, e2 = r3.length, t2 = -1, b2 = -1, n2 = 0, c2 = false; ; ) {
        if (e2 <= 0) {
          c2 = 0;
          break;
        }
        e2--;
        f2 = r3.charAt(e2);
        if ("/" == f2) {
          if (c2) {
            c2 = e2 + 1;
            break;
          }
        } else t2 < 0 && (t2 = e2 + 1, c2 = true), "." == f2 ? b2 < 0 ? b2 = e2 : 1 != n2 && (n2 = 1) : b2 > -1 && (n2 = -1);
      }
      return b2 < 0 || t2 < 0 || 0 == n2 || 1 == n2 && b2 == (t2 - 1 | 0) && b2 == (c2 + 1 | 0) ? "" : r3.slice(b2, t2);
    })(r2 + "");
  }, set: function(r2) {
    He(r2, "extname");
    var b2 = this.dirname;
    if (!b2) throw new Error("Setting `extname` requires `path` to be set too");
    if (r2) {
      if (46 != (+r2.codePointAt(0) | 0)) throw new Error("`extname` must start with `.`");
      if (r2.includes(".", 1)) throw new Error("`extname` cannot contain multiple dots");
    }
    r2 = r2 ? r2 + "" : "";
    be(this, Le(b2, this.stem + "" + r2));
  } }), ne("path", { configurable: e, get: function() {
    return ce(this);
  }, set: function(r2) {
    be(this, r2);
  } }), ne("stem", { configurable: e, get: function() {
    var r2 = ce(this);
    if ("string" == typeof r2) return r2 = r2 + "", Jr(r2, this.extname + "");
  }, set: function(r2) {
    Ge(r2, "stem"), He(r2, "stem");
    var e2 = r2 + "";
    r2 = this.dirname, r2 = r2 ? r2 + "" : "";
    var b2 = this.extname;
    b2 = b2 ? b2 + "" : "", be(this, Le(r2, e2 + b2));
  } }), ne("fail", { configurable: e, writable: e, value: r }), ne("info", { configurable: e, writable: e, value: t }), ne("message", { configurable: e, writable: e, value: i }), ne("toString", { configurable: e, writable: e, value: a }), ye("basename"), ye("dirname"), ye("extname"), ye("path"), ye("stem"), Object.defineProperty(ge, "prototype", { writable: false });
  var sr;
  var fr = (0, function() {
    Je("use", this.frozen);
    var b2 = this.attachers, e2 = this.namespace, r2;
    arguments.length > 0 && (r2 = arguments[0]);
    if (r2 == null) return this;
    if ("function" == typeof r2) return Ke(b2, r2, se.call(arguments, 1)), this;
    if ("object" == typeof r2) return Array.isArray(r2) ? et(b2, e2, r2) : rt(b2, e2, r2), this;
    throw new TypeError("Expected usable value, not `" + r2 + "`");
  });
  var vr = (0, function() {
    return tt(this);
  });
  var pr = (0, function(r2) {
    this.freeze(), r2 = Ce(r2);
    let b2 = We(this);
    Ye("parse", b2);
    return b2(String(r2), r2);
  });
  var dr = (0, function(r2, b2, e2) {
    Kr(r2);
    var c2 = this.freeze;
    this.freeze(), !e2 && "function" == typeof b2 && (e2 = b2, b2 = void 0);
    var n2 = this.transformers, t2 = function(t3, c3) {
      let f2 = Ce(b2);
      n2.run(r2, f2, function(b3, n3, f3) {
        var E2 = !n3 ? r2 : n3;
        if (b3) {
          c3(b3);
          return;
        }
        if (t3) {
          t3(E2);
          return;
        }
        e2(void 0, E2, f3);
      });
    };
    if (e2) {
      t2(void 0, e2);
      return;
    }
    return new Promise(t2);
  });
  var hr = (0, function(r2, b2) {
    var e2, t2 = false;
    this.run(r2, b2, function(r3, b3, n2) {
      Ur(r3), e2 = b3, t2 = true;
    }), Xr("runSync", "run", t2), Hr(e2);
    return e2;
  });
  var gr = (0, function(r2, b2) {
    this.freeze();
    let e2 = Ce(b2);
    b2 = $e(this), Ze("stringify", b2), Kr(r2);
    return b2(r2, e2);
  });
  var mr = (0, function(r2, b2) {
    var e2 = this;
    e2.freeze(), Ye("process", We(e2)), Ze("process", $e(e2));
    var t2 = function(t3, n2) {
      let c2 = Ce(r2), f2 = e2.parse(c2);
      e2.run(f2, c2, function(r3, c3, f3) {
        if (r3 || !c3 || !f3) {
          n2(r3);
          return;
        }
        var E2 = e2.stringify(c3, f3);
        ("string" == typeof E2 ? true : jr(E2)) ? f3.value = E2 : f3.result = E2;
        if (t3) {
          t3(f3);
          return;
        }
        b2(void 0, f3);
      });
    };
    if (b2) {
      t2(void 0, b2);
      return;
    }
    return new Promise(t2);
  });
  var br = (0, function(r2) {
    this.freeze(), Ye("processSync", We(this)), Ze("processSync", $e(this));
    var b2, e2 = false;
    this.process(r2, function(r3, t2) {
      e2 = true, Ur(r3), b2 = t2;
    }), Xr("processSync", "process", e2), Hr(b2);
    return b2;
  });
  var yr = (0, function(r2, b2) {
    var t2 = this.namespace, n2 = arguments.length, e2;
    n2 > 0 && (e2 = r2);
    if ("string" == typeof e2) {
      if (2 == n2) return Je("data", this.frozen), t2[e2] = b2, this;
      var c2;
      return he.call(t2, e2) && (c2 = t2[e2]) ? c2 : void 0;
    }
    return e2 ? (Je("data", this.frozen), this.namespace = e2, this) : t2;
  });
  var kr = (0, function() {
    if (this.frozen) return this;
    var b2 = this.attachers, t2 = this.transformers;
    while (true) {
      var r2 = +this.freezeIndex + 1;
      this.freezeIndex = r2;
      if (r2 >= b2.length) break;
      var e2 = b2[r2], n2 = e2[0];
      r2 = se.call(e2, 1);
      if (!(r2.length > 0 && r2[0] === false)) r2.length > 0 && true === r2[0] && Array.prototype.splice.call(r2, 0, 1, void 0), r2 = n2.apply(this, r2), "function" == typeof r2 && t2.use(r2);
    }
    this.frozen = true;
    this.freezeIndex = Number.POSITIVE_INFINITY;
    return this;
  });
  var fe = (0, function() {
    if (this === void 0) throw new TypeError("Class constructor Processor cannot be invoked without 'new'");
    return (() => {
      var r2;
      r2 = (0, function() {
        return tt(r2);
      }), Object.setPrototypeOf(r2, xr), r2.Compiler = void 0, r2.Parser = void 0, r2.attachers = [], r2.compiler = void 0, r2.freezeIndex = -1, r2.frozen = void 0, r2.namespace = {}, r2.parser = void 0;
      var b2 = { fns: [], run: Ct, use: Dt };
      r2.transformers = b2;
      return r2;
    })();
  });
  Object.defineProperty(fe, "name", { configurable: e, value: "Processor" });
  var xr = fe.prototype;
  (function() {
    ee(vr, "copy", 0), ee(yr, "data", 2), ee(kr, "freeze", 0), ee(pr, "parse", 1), ee(mr, "process", 2), ee(br, "processSync", 1), ee(dr, "run", 3), ee(hr, "runSync", 2), ee(gr, "stringify", 2), ee(fr, "use", 1), re("copy", vr), re("data", yr), re("freeze", kr), re("parse", pr), re("process", mr), re("processSync", br), re("run", dr), re("runSync", hr), re("stringify", gr), re("use", fr), Object.defineProperty(fe, "prototype", { writable: false });
  })(), sr = function() {
    return new fe();
  };
  var l = new fe();
  l.freeze();
  var u = Object;
  var ue = Number;
  var qe = Math;
  var ve = Error;
  var $ = /[A-Za-z]/;
  var K = /[\dA-Za-z]/;
  var Pt = /[#-'*+\--9=?A-Z^-~]/;
  var Ne = /\d/;
  var Rt = /[\dA-Fa-f]/;
  var Ft = new RegExp("[!-/:-@[-`{-~]", "");
  var zt = new RegExp("\\p{P}|\\p{S}", "u");
  var Ot = /\s/;
  var _t = U((0, function(r2) {
    var b2, e2, t2, n2 = function(b3) {
      if (A(b3)) {
        r2.exit("chunkText"), r2.exit("paragraph"), r2.consume(b3);
        return;
      }
      if (T(b3)) return r2.consume(b3), r2.exit("chunkText"), e2;
      r2.consume(b3);
      return n2;
    };
    e2 = function(e3) {
      var t3 = { contentType: "text", previous: b2 };
      t3 = r2.enter("chunkText", t3), b2 && (b2.next = t3), b2 = t3;
      return n2(e3);
    };
    let E2 = r2.attempt;
    t2 = r2.attempt(this.parser.constructs.contentInitial, function(b3) {
      if (A(b3)) {
        r2.consume(b3);
        return;
      }
      r2.enter("lineEnding");
      r2.consume(b3), r2.exit("lineEnding");
      return R(r2, t2, "linePrefix", 0);
    }, function(b3) {
      r2.enter("paragraph");
      return e2(b3);
    });
    return t2;
  }));
  r = function(r2, b2, e2) {
    var n2 = !M(this.parser.constructs.disable.null, "codeIndented") ? 4 : 0, t2 = r2.attempt;
    return R(r2, r2.attempt(this.parser.constructs.document, b2, e2), "linePrefix", n2);
  };
  var Be = { tokenize: r };
  var Nt = U((0, function(r2) {
    var e2 = this;
    let b2 = [[], 0, null, null, null, 0, null, null];
    b2[2] = r2;
    var f2 = function(t3) {
      if (A(t3)) {
        lt(e2, b2, r2.exit("chunkFlow"), true), ke(e2, r2, b2[0], 0), r2.consume(t3);
        return;
      }
      if (T(t3)) return r2.consume(t3), lt(e2, b2, r2.exit("chunkFlow"), false), b2[1] = 0, e2.interrupt = void 0, b2[6];
      r2.consume(t3);
      return f2;
    };
    let t2 = function(t3) {
      if (A(t3)) {
        !b2[3] || tr(e2, b2), ke(e2, r2, b2[0], 0), r2.consume(t3);
        return;
      }
      if (!b2[3]) {
        var n3 = e2.parser, E3 = n3.flow;
        b2[3] = n3.flow(e2.now());
      }
      var c3 = { _tokenizer: b2[3], contentType: "flow", previous: b2[4] };
      r2.enter("chunkFlow", c3);
      return f2(t3);
    }, i2 = function(r3) {
      let t3 = b2[1] + 1 | 0;
      b2[1] = t3;
      let n3 = b2[0], c3 = e2.currentConstruct;
      n3.push({ construct: c3, state: e2.containerState });
      return b2[7](r3);
    }, n2 = function(b3) {
      e2.containerState = {};
      return r2.attempt(Be, i2, t2)(b3);
    }, w2 = function(r3) {
      let n3 = e2.parser.lazy, c3 = e2.now().line, f3 = b2[1];
      nt(n3, c3, f3 != q(b2[0])), b2[5] = +e2.now().offset;
      return t2(r3);
    }, o2 = function(t3) {
      !b2[3] || tr(e2, b2), ke(e2, r2, b2[0], b2[1]);
      return n2(t3);
    }, c2 = function(c3) {
      if (b2[1] == b2[0].length) {
        if (!b2[3]) return n2(c3);
        var f3 = b2[3].currentConstruct;
        if (f3 && f3.concrete) return t2(c3);
        e2.interrupt = f3 && !b2[3]._gfmTableDynamicInterruptHack;
      }
      e2.containerState = {};
      return r2.check(Be, o2, w2)(c3);
    }, a2 = function(t3) {
      var a3 = b2[1] + 1 | 0;
      b2[1] = a3;
      if (e2.containerState._closeFlow) {
        e2.containerState._closeFlow = void 0, !b2[3] || tr(e2, b2);
        for (var f3, E3, i3, n3, o3 = q(e2.events), w3 = o3; --w3 >= 0; ) {
          E3 = e2.events[w3];
          if ("exit" == E3[0] && "chunkFlow" == E3[1].type) {
            f3 = E3[1].end;
            break;
          }
        }
        ke(e2, r2, b2[0], b2[1]);
        for (n3 = o3; n3 < q(e2.events); n3++) i3 = e2.events[n3][1], i3.end = z(f3);
        f3 = e2.events, w3++, i3 = e2.events, Y(f3, w3, 0, i3.slice(o3)), e2.events.length = n3;
        return c2(t3);
      }
      return b2[6](t3);
    }, E2 = function(t3) {
      if (b2[1] < b2[0].length) {
        var n3 = b2[0][b2[1]];
        e2.containerState = n3.state;
        return r2.attempt(n3.construct.continuation, a2, c2)(t3);
      }
      return c2(t3);
    };
    b2[6] = E2, b2[7] = n2;
    return E2;
  }));
  var me = U(function(r2, b2, e2) {
    let t2 = function(r3) {
      return A(r3) || T(r3) ? b2(r3) : e2(r3);
    };
    return function(b3) {
      return C(b3) ? R(r2, t2, "linePrefix", 0)(b3) : t2(b3);
    };
  });
  me.partial = e;
  var wr = class {
    constructor(r2) {
      r2 ? this.left = Array.from(r2) : this.left = [], this.right = [];
    }
  };
  r = wr.prototype;
  r.get = function(r2) {
    var e2 = +r2, t2 = q(this.left), b2 = t2 + q(this.right) | 0;
    (e2 < 0 || e2 >= +b2) && at("Cannot access index `" + E(r2) + "` in a splice buffer of size `" + b2 + "`"), b2 = this.left, t2 = q(b2);
    if (e2 < +t2) return b2[+r2];
    e2 = this.right, b2 = q(e2);
    return e2[((b2 - r2 | 0) + t2 | 0) - 1];
  }, r.slice = function(r2, b2) {
    var t2 = +ue.POSITIVE_INFINITY;
    V(b2) || (t2 = +b2);
    var n2 = +r2, c2 = this.left;
    r2 = this.right, b2 = c2.length;
    if (t2 < b2) return c2.slice(n2, t2);
    if (n2 > b2) {
      var e2 = r2.length;
      t2 = e2 - t2 + b2, e2 = e2 - n2 + b2, e2 = r2.slice(t2, e2);
      return e2.reverse();
    }
    n2 = c2.slice(n2);
    e2 = r2.length - t2 + b2, e2 = r2.slice(e2), e2.reverse();
    return n2.concat(e2);
  }, r.splice = function(r2, b2, e2) {
    b2 = b2 ? +b2 : 0, ie(this, +qe.trunc(+r2)), r2 = this.right, b2 = r2.length - b2;
    var t2 = ue.POSITIVE_INFINITY;
    b2 = r2.splice(b2, t2), !e2 || xe(this.left, e2);
    return b2.reverse();
  }, r.shift = function() {
    ie(this, 0);
    let r2 = this.right;
    return r2.pop();
  }, r.pop = function() {
    ie(this, +ue.POSITIVE_INFINITY);
    let r2 = this.left;
    return r2.pop();
  };
  r.push = function(r2) {
    ie(this, +ue.POSITIVE_INFINITY);
    let b2 = this.left;
    b2.push(r2);
  }, r.pushMany = function(r2) {
    ie(this, +ue.POSITIVE_INFINITY), xe(this.left, r2);
  }, r.unshift = function(r2) {
    ie(this, 0);
    let b2 = this.right;
    b2.push(r2);
  }, r.unshiftMany = function(r2) {
    ie(this, 0), r2 = Array.from(r2), r2.reverse(), xe(this.right, r2);
  }, r.setCursor = function(r2) {
    ie(this, +r2);
  }, u.defineProperty(r, "length", { enumerable: e, configurable: e, get: function() {
    let r2 = q(this.left);
    return r2 + q(this.right) | 0;
  } }), r = function(r2, b2) {
    ct(r2);
    return r2;
  };
  var Sr = U((0, function(r2, b2, e2) {
    var t2 = this;
    let n2 = function(n3) {
      if (A(n3) || T(n3)) return e2(n3);
      var i2, f2 = t2.events, w2 = q(f2), c2 = f2[w2 - 1];
      if (!M(t2.parser.constructs.disable.null, "codeIndented") && c2 && "linePrefix" == c2[1].type && E(c2[2].sliceSerialize.call(c2[2], c2[1], true)).length >= 4) return b2(n3);
      i2 = r2.interrupt;
      return r2.interrupt(t2.parser.constructs.flow, e2, b2)(n3);
    };
    return function(b3) {
      r2.exit("chunkContent"), r2.enter("lineEnding"), r2.consume(b3), r2.exit("lineEnding");
      return R(r2, n2, "linePrefix", 0);
    };
  }));
  Sr.partial = e;
  var Bt = { tokenize: function(r2, b2, e2) {
    var t2, n2, f2, c2 = function(b3) {
      if (A(b3)) return n2(b3);
      if (T(b3)) return r2.check(Sr, f2, n2)(b3);
      r2.consume(b3);
      return c2;
    };
    n2 = function(e3) {
      r2.exit("chunkContent"), r2.exit("content");
      return b2(e3);
    }, f2 = function(b3) {
      r2.consume(b3), r2.exit("chunkContent");
      let e3 = { contentType: "content", previous: t2 };
      e3 = r2.enter("chunkContent", e3), t2.next = e3, t2 = e3;
      return c2;
    };
    return function(b3) {
      r2.enter("content"), t2 = r2.enter("chunkContent", { contentType: "content" });
      return c2(b3);
    };
  }, resolve: r };
  var Vt = U((0, function(r2) {
    var t2 = this;
    let n2 = t2.parser.constructs;
    var e2;
    let b2 = function(b3) {
      if (A(b3)) {
        r2.consume(b3);
        return;
      }
      r2.enter("lineEnding");
      r2.consume(b3), r2.exit("lineEnding"), t2.currentConstruct = void 0;
      return e2;
    }, c2 = r2.attempt, E2 = n2.flow;
    c2 = R(r2, r2.attempt(E2, b2, r2.attempt(Bt, b2, void 0)), "linePrefix", 0), b2 = r2.attempt(n2.flowInitial, b2, c2), e2 = r2.attempt(me, function(b3) {
      if (A(b3)) {
        r2.consume(b3);
        return;
      }
      r2.enter("lineEndingBlank");
      r2.consume(b3), r2.exit("lineEndingBlank"), t2.currentConstruct = void 0;
      return e2;
    }, b2);
    return e2;
  }));
  var ca;
  var c = { resolveAll: function(r2, b2) {
    var e2, f2, c2 = q(r2), n2 = -1, t2 = f2;
    while (true) {
      e2 = n2 + 1 | 0;
      if (e2 > c2) break;
      t2 === void 0 ? (e2 < c2 && r2[e2] && "data" == r2[e2][1].type && (n2 = e2 + 1 | 0, t2 = e2, e2 = n2), n2 = e2) : ((e2 >= c2 || !r2[e2] || "data" != r2[e2][1].type) && (t2 = t2 | 0, e2 != (t2 + 2 | 0) && (c2 = r2[t2][1], c2.end = r2[e2 - 1][1].end, n2 = t2 + 2 | 0, r2.splice(n2, (e2 - t2 | 0) - 2 | 0), c2 = q(r2), e2 = n2), t2 = void 0), n2 = e2);
    }
    return "function" == typeof ca ? ca(r2, b2) : r2;
  } };
  var sa;
  r = { resolveAll: function(r2, b2) {
    var e2, f2, c2 = q(r2), n2 = -1, t2 = f2;
    while (true) {
      e2 = n2 + 1 | 0;
      if (e2 > c2) break;
      t2 === void 0 ? (e2 < c2 && r2[e2] && "data" == r2[e2][1].type && (n2 = e2 + 1 | 0, t2 = e2, e2 = n2), n2 = e2) : ((e2 >= c2 || !r2[e2] || "data" != r2[e2][1].type) && (t2 = t2 | 0, e2 != (t2 + 2 | 0) && (c2 = r2[t2][1], c2.end = r2[e2 - 1][1].end, n2 = t2 + 2 | 0, r2.splice(n2, (e2 - t2 | 0) - 2 | 0), c2 = q(r2), e2 = n2), t2 = void 0), n2 = e2);
    }
    return "function" == typeof sa ? sa(r2, b2) : r2;
  }, tokenize: function(r2) {
    var b2 = this;
    let t2 = b2.parser.constructs.string;
    var e2;
    let n2 = function(r3) {
      if (A(r3)) return true;
      var e3 = De(t2, r3);
      if (!e3) return false;
      for (var c3 = q(e3), n3 = -1; ++n3 < c3; ) {
        r3 = e3[n3].previous;
        if ("function" != typeof r3 || r3.call(b2, b2.previous)) return true;
      }
      return false;
    };
    var c2 = function(b3) {
      if (n2(b3)) return r2.exit("data"), e2(b3);
      r2.consume(b3);
      return c2;
    };
    let f2 = function(b3) {
      if (A(b3)) {
        r2.consume(b3);
        return;
      }
      r2.enter("data");
      r2.consume(b3);
      return c2;
    }, E2 = function(r3) {
      return n2(r3) ? e2(r3) : f2(r3);
    };
    e2 = r2.attempt(t2, E2, f2);
    return E2;
  } };
  var Mt = r;
  var oa = function(r2, b2) {
    return ((r3, b3) => {
      for (var t2, e2, n2, w2, i2, c2, o2, E2 = q(r3), f2 = 0; ++f2 <= E2; ) {
        t2 = f2 == E2, e2 = !t2 && "lineEnding" == r3[f2][1].type;
        if ((t2 || e2) && "data" == r3[f2 - 1][1].type) {
          for (n2 = r3[f2 - 1][1], o2 = b3.sliceStream(n2), w2 = q(o2), e2 = -1, t2 = 0, c2 = false; --w2 >= 0; ) {
            i2 = o2[w2];
            if ("string" == typeof i2) {
              i2 += "", e2 = i2.length;
              for (; e2 > 0 && 32 == (i2.charCodeAt(e2 - 1) | 0); t2++) e2--;
              if (e2 > 0) break;
              e2 = -1;
            } else if (i2 === -2) t2++, c2 = true;
            else if (i2 !== -1) {
              w2++;
              break;
            }
          }
          b3._contentTypeTextTrailing && f2 == E2 && (t2 = 0);
          if (t2 > 0) {
            c2 = f2 == E2 || c2 || t2 < 2, i2 = !c2 ? "hardBreakTrailing" : "lineSuffix", c2 = {}, c2._bufferIndex = 0 != w2 ? e2 : (+n2.start._bufferIndex | 0) + e2 | 0, c2._index = (+n2.start._index | 0) + w2 | 0, c2.line = n2.end.line, c2.column = (+n2.end.column | 0) - t2 | 0, c2.offset = (+n2.end.offset | 0) - t2 | 0, t2 = { type: i2, start: c2, end: z(n2.end) }, n2.end = z(t2.start), e2 = n2.start.offset;
            if (e2 == n2.end.offset) L(n2, t2);
            else {
              e2 = [], e2[0] = "enter", e2[1] = t2, e2[2] = b3, E2 = [], E2[0] = "exit", E2[1] = t2, E2[2] = b3, r3.splice(f2, 0, e2, E2);
              var a2 = f2 + 2 | 0;
              E2 = q(r3), f2 = a2;
            }
          }
          f2++;
        }
      }
      return r3;
    })(r2, b2);
  };
  r = { resolveAll: function(r2, b2) {
    var e2, f2, c2 = q(r2), n2 = -1, t2 = f2;
    while (true) {
      e2 = n2 + 1 | 0;
      if (e2 > c2) break;
      t2 === void 0 ? (e2 < c2 && r2[e2] && "data" == r2[e2][1].type && (n2 = e2 + 1 | 0, t2 = e2, e2 = n2), n2 = e2) : ((e2 >= c2 || !r2[e2] || "data" != r2[e2][1].type) && (t2 = t2 | 0, e2 != (t2 + 2 | 0) && (c2 = r2[t2][1], c2.end = r2[e2 - 1][1].end, n2 = t2 + 2 | 0, r2.splice(n2, (e2 - t2 | 0) - 2 | 0), c2 = q(r2), e2 = n2), t2 = void 0), n2 = e2);
    }
    return "function" == typeof oa ? oa(r2, b2) : r2;
  }, tokenize: function(r2) {
    var b2 = this;
    let t2 = b2.parser.constructs.text;
    var e2;
    let n2 = function(r3) {
      if (A(r3)) return true;
      var e3 = De(t2, r3);
      if (!e3) return false;
      for (var c3 = q(e3), n3 = -1; ++n3 < c3; ) {
        r3 = e3[n3].previous;
        if ("function" != typeof r3 || r3.call(b2, b2.previous)) return true;
      }
      return false;
    };
    var c2 = function(b3) {
      if (n2(b3)) return r2.exit("data"), e2(b3);
      r2.consume(b3);
      return c2;
    };
    let f2 = function(b3) {
      if (A(b3)) {
        r2.consume(b3);
        return;
      }
      r2.enter("data");
      r2.consume(b3);
      return c2;
    }, E2 = function(r3) {
      return n2(r3) ? e2(r3) : f2(r3);
    };
    e2 = r2.attempt(t2, E2, f2);
    return E2;
  } };
  var jt = r;
  i = { name: "attention", tokenize: function(r2, b2, e2) {
    let c2 = this.parser.constructs.attentionMarkers.null;
    e2 = this.previous;
    let t2 = Pe(e2);
    var n2 = 0, f2 = function(E2) {
      if (E2 == n2) return r2.consume(E2), f2;
      var i2, w2, o2 = r2.exit("attentionSequence"), a2 = Pe(E2);
      i2 = !a2 || 2 == a2 && t2 || M(c2, E2), w2 = !t2 || 2 == t2 && a2 || M(c2, e2), 42 == n2 ? (o2._open = i2, o2._close = w2) : (o2._open = i2 && (!!t2 || !w2), o2._close = w2 && (!!a2 || !i2));
      return b2(E2);
    };
    return function(b3) {
      n2 = b3 | 0, r2.enter("attentionSequence");
      return f2(b3);
    };
  }, resolveAll: function(r2, b2) {
    var f2, n2, w2, i2, u2, E2, o2, a2, x2, l2, e2 = q(r2), t2 = -1;
    while (true) {
      f2 = t2 + 1 | 0;
      if (f2 >= e2) break;
      var A2 = r2[f2], S2 = A2[1];
      n2 = S2;
      if ("enter" == r2[f2][0] && "attentionSequence" == n2.type && n2._close) for (w2 = f2; --w2 >= 0; ) {
        var p2 = r2[w2], c2 = p2[1];
        if ("exit" == r2[w2][0] && "attentionSequence" == c2.type && c2._open) {
          t2 = b2.sliceSerialize(c2);
          if ((t2.charCodeAt(0) | 0) == (b2.sliceSerialize(n2).charCodeAt(0) | 0)) {
            G = c2.start.offset | 0, i2 = (c2.end.offset | 0) - G | 0, ne = n2.start.offset | 0, t2 = (n2.end.offset | 0) - ne | 0, E2 = (c2._close || n2._open) && 0 != t2 % 3 && 0 == (i2 + t2 | 0) % 3;
            if (!E2) {
              e2 = i2 > 1 && t2 > 1 ? 2 : 1, E2 = z(c2.end), o2 = z(n2.start), t2 = 0 - e2;
              var y2 = E2.column + t2;
              E2.column = y2;
              var T2 = E2.offset + t2;
              E2.offset = T2;
              var m2 = E2._bufferIndex + t2;
              E2._bufferIndex = m2;
              var v2 = o2.column + e2;
              o2.column = v2;
              var L2 = o2.offset + e2;
              o2.offset = L2;
              var h2 = o2._bufferIndex + e2;
              o2._bufferIndex = h2, e2 > 1 ? (e2 = "strongSequence", t2 = "strongText", i2 = "strong") : (e2 = "emphasisSequence", t2 = "emphasisText", i2 = "emphasis"), a2 = { type: e2, start: E2, end: z(c2.end) }, x2 = { type: e2, start: z(n2.start), end: o2 }, e2 = z(c2.end), t2 = { type: t2, start: e2, end: z(n2.start) }, e2 = z(a2.start), i2 = { type: i2, start: e2, end: z(x2.end) }, c2.end = z(a2.start), n2.start = z(x2.end), e2 = [], 0 != c2.end.offset - c2.start.offset && (e2 = Z(e2, [["enter", c2, b2], ["exit", c2, b2]])), e2 = Z(e2, [["enter", i2, b2], ["enter", a2, b2], ["exit", a2, b2], ["enter", t2, b2]]), u2 = b2.parser.constructs.insideSpan.null, l2 = w2 + 1 | 0, e2 = Z(Z(e2, er(u2, r2.slice(l2, f2), b2)), [["exit", t2, b2], ["enter", x2, b2], ["exit", x2, b2], ["exit", i2, b2]]), 0 != n2.end.offset - n2.start.offset ? (e2 = Z(e2, [["enter", n2, b2], ["exit", n2, b2]]), n2 = 2) : n2 = 0, Y(r2, w2 - 1 | 0, (f2 - w2 | 0) + 3 | 0, e2), t2 = ((w2 + q(e2) | 0) - n2 | 0) - 2 | 0, e2 = q(r2), f2 = t2;
              break;
            }
          }
        }
      }
      t2 = f2;
    }
    for (b2 = q(r2), f2 = -1; ++f2 < b2; ) "attentionSequence" == r2[f2][1].type && (r2[f2][1].type = "data");
    return r2;
  } };
  var m = Q("autolink", function(r2, b2, e2) {
    var c2, f2, t2 = 0, E2 = function(b3) {
      if ((45 === b3 || P(K, b3)) && t2 < 63) {
        t2++;
        var n3 = c2;
        45 === b3 && (n3 = E2), r2.consume(b3);
        return n3;
      }
      return e2(b3);
    };
    c2 = function(e3) {
      return 46 === e3 ? (r2.consume(e3), t2 = 0, f2) : 62 === e3 ? (r2.exit("autolinkProtocol").type = "autolinkEmail", r2.enter("autolinkMarker"), r2.consume(e3), r2.exit("autolinkMarker"), r2.exit("autolink"), b2) : E2(e3);
    }, f2 = function(r3) {
      return P(K, r3) ? c2(r3) : e2(r3);
    };
    var n2 = function(b3) {
      return 64 === b3 ? (r2.consume(b3), f2) : P(Pt, b3) ? (r2.consume(b3), n2) : e2(b3);
    }, i2 = function(t3) {
      if (62 === t3) return r2.exit("autolinkProtocol"), r2.enter("autolinkMarker"), r2.consume(t3), r2.exit("autolinkMarker"), r2.exit("autolink"), b2;
      if (A(t3) || 32 === t3 || 60 === t3 || Xe(t3)) return e2(t3);
      r2.consume(t3);
      return i2;
    }, w2 = function(b3) {
      if (58 === b3) return r2.consume(b3), t2 = 0, i2;
      if ((43 === b3 || 45 === b3 || 46 === b3 || P(K, b3)) && t2 < 32) return t2 = t2 + 1 | 0, r2.consume(b3), w2;
      t2 = 0;
      return n2(b3);
    };
    let o2 = function(r3) {
      return 43 === r3 || 45 === r3 || 46 === r3 || P(K, r3) ? (t2 = 1, w2(r3)) : n2(r3);
    }, a2 = function(b3) {
      return P($, b3) ? (r2.consume(b3), o2) : 64 === b3 ? e2(b3) : n2(b3);
    };
    return function(b3) {
      r2.enter("autolink"), r2.enter("autolinkMarker"), r2.consume(b3), r2.exit("autolinkMarker"), r2.enter("autolinkProtocol");
      return a2;
    };
  });
  var Ae = Q("blockQuote", (0, function(r2, b2, e2) {
    var t2 = this;
    let n2 = function(e3) {
      if (C(e3)) return r2.enter("blockQuotePrefixWhitespace"), r2.consume(e3), r2.exit("blockQuotePrefixWhitespace"), r2.exit("blockQuotePrefix"), b2;
      r2.exit("blockQuotePrefix");
      return b2(e3);
    };
    return function(b3) {
      if (62 === b3) {
        var c2 = t2.containerState;
        !c2.open && (r2.enter("blockQuote", { _container: true }), c2.open = true), r2.enter("blockQuotePrefix"), r2.enter("blockQuoteMarker"), r2.consume(b3), r2.exit("blockQuoteMarker");
        return n2;
      }
      return e2(b3);
    };
  }));
  t = function(r2) {
    r2.exit("blockQuote");
  }, r = { tokenize: function(r2, b2, e2) {
    var n2 = this;
    let t2 = function(t3) {
      return r2.attempt(Ae, b2, e2)(t3);
    };
    return function(b3) {
      if (C(b3)) {
        var e3 = M(n2.parser.constructs.disable.null, "codeIndented") ? 0 : 4;
        return R(r2, t2, "linePrefix", e3)(b3);
      }
      return t2(b3);
    };
  } }, Ae.continuation = r, Ae.exit = t;
  var s = Q("characterEscape", function(r2, b2, e2) {
    let t2 = function(t3) {
      return P(Ft, t3) ? (r2.enter("characterEscapeValue"), r2.consume(t3), r2.exit("characterEscapeValue"), r2.exit("characterEscape"), b2) : e2(t3);
    };
    return function(b3) {
      r2.enter("characterEscape"), r2.enter("escapeMarker"), r2.consume(b3), r2.exit("escapeMarker");
      return t2;
    };
  });
  var Er = { AElig: "\xC6", AMP: "&", Aacute: "\xC1", Abreve: "\u0102", Acirc: "\xC2", Acy: "\u0410", Afr: "\u{1D504}", Agrave: "\xC0", Alpha: "\u0391", Amacr: "\u0100", And: "\u2A53", Aogon: "\u0104", Aopf: "\u{1D538}", ApplyFunction: "\u2061", Aring: "\xC5", Ascr: "\u{1D49C}", Assign: "\u2254", Atilde: "\xC3", Auml: "\xC4", Backslash: "\u2216", Barv: "\u2AE7", Barwed: "\u2306", Bcy: "\u0411", Because: "\u2235", Bernoullis: "\u212C", Beta: "\u0392", Bfr: "\u{1D505}", Bopf: "\u{1D539}", Breve: "\u02D8", Bscr: "\u212C", Bumpeq: "\u224E", CHcy: "\u0427", COPY: "\xA9", Cacute: "\u0106", Cap: "\u22D2", CapitalDifferentialD: "\u2145", Cayleys: "\u212D", Ccaron: "\u010C", Ccedil: "\xC7", Ccirc: "\u0108", Cconint: "\u2230", Cdot: "\u010A", Cedilla: "\xB8", CenterDot: "\xB7", Cfr: "\u212D", Chi: "\u03A7", CircleDot: "\u2299", CircleMinus: "\u2296", CirclePlus: "\u2295", CircleTimes: "\u2297", ClockwiseContourIntegral: "\u2232", CloseCurlyDoubleQuote: "\u201D", CloseCurlyQuote: "\u2019", Colon: "\u2237", Colone: "\u2A74", Congruent: "\u2261", Conint: "\u222F", ContourIntegral: "\u222E", Copf: "\u2102", Coproduct: "\u2210", CounterClockwiseContourIntegral: "\u2233", Cross: "\u2A2F", Cscr: "\u{1D49E}", Cup: "\u22D3", CupCap: "\u224D", DD: "\u2145", DDotrahd: "\u2911", DJcy: "\u0402", DScy: "\u0405", DZcy: "\u040F", Dagger: "\u2021", Darr: "\u21A1", Dashv: "\u2AE4", Dcaron: "\u010E", Dcy: "\u0414", Del: "\u2207", Delta: "\u0394", Dfr: "\u{1D507}", DiacriticalAcute: "\xB4", DiacriticalDot: "\u02D9", DiacriticalDoubleAcute: "\u02DD", DiacriticalGrave: "`", DiacriticalTilde: "\u02DC", Diamond: "\u22C4", DifferentialD: "\u2146", Dopf: "\u{1D53B}", Dot: "\xA8", DotDot: "\u20DC", DotEqual: "\u2250", DoubleContourIntegral: "\u222F", DoubleDot: "\xA8", DoubleDownArrow: "\u21D3", DoubleLeftArrow: "\u21D0", DoubleLeftRightArrow: "\u21D4", DoubleLeftTee: "\u2AE4", DoubleLongLeftArrow: "\u27F8", DoubleLongLeftRightArrow: "\u27FA", DoubleLongRightArrow: "\u27F9", DoubleRightArrow: "\u21D2", DoubleRightTee: "\u22A8", DoubleUpArrow: "\u21D1", DoubleUpDownArrow: "\u21D5", DoubleVerticalBar: "\u2225", DownArrow: "\u2193", DownArrowBar: "\u2913", DownArrowUpArrow: "\u21F5", DownBreve: "\u0311", DownLeftRightVector: "\u2950", DownLeftTeeVector: "\u295E", DownLeftVector: "\u21BD", DownLeftVectorBar: "\u2956", DownRightTeeVector: "\u295F", DownRightVector: "\u21C1", DownRightVectorBar: "\u2957", DownTee: "\u22A4", DownTeeArrow: "\u21A7", Downarrow: "\u21D3", Dscr: "\u{1D49F}", Dstrok: "\u0110", ENG: "\u014A", ETH: "\xD0", Eacute: "\xC9", Ecaron: "\u011A", Ecirc: "\xCA", Ecy: "\u042D", Edot: "\u0116", Efr: "\u{1D508}", Egrave: "\xC8", Element: "\u2208", Emacr: "\u0112", EmptySmallSquare: "\u25FB", EmptyVerySmallSquare: "\u25AB", Eogon: "\u0118", Eopf: "\u{1D53C}", Epsilon: "\u0395", Equal: "\u2A75", EqualTilde: "\u2242", Equilibrium: "\u21CC", Escr: "\u2130", Esim: "\u2A73", Eta: "\u0397", Euml: "\xCB", Exists: "\u2203", ExponentialE: "\u2147", Fcy: "\u0424", Ffr: "\u{1D509}", FilledSmallSquare: "\u25FC", FilledVerySmallSquare: "\u25AA", Fopf: "\u{1D53D}", ForAll: "\u2200", Fouriertrf: "\u2131", Fscr: "\u2131", GJcy: "\u0403", GT: ">", Gamma: "\u0393", Gammad: "\u03DC", Gbreve: "\u011E", Gcedil: "\u0122", Gcirc: "\u011C", Gcy: "\u0413", Gdot: "\u0120", Gfr: "\u{1D50A}", Gg: "\u22D9", Gopf: "\u{1D53E}", GreaterEqual: "\u2265", GreaterEqualLess: "\u22DB", GreaterFullEqual: "\u2267", GreaterGreater: "\u2AA2", GreaterLess: "\u2277", GreaterSlantEqual: "\u2A7E", GreaterTilde: "\u2273", Gscr: "\u{1D4A2}", Gt: "\u226B", HARDcy: "\u042A", Hacek: "\u02C7", Hat: "^", Hcirc: "\u0124", Hfr: "\u210C", HilbertSpace: "\u210B", Hopf: "\u210D", HorizontalLine: "\u2500", Hscr: "\u210B", Hstrok: "\u0126", HumpDownHump: "\u224E", HumpEqual: "\u224F", IEcy: "\u0415", IJlig: "\u0132", IOcy: "\u0401", Iacute: "\xCD", Icirc: "\xCE", Icy: "\u0418", Idot: "\u0130", Ifr: "\u2111", Igrave: "\xCC", Im: "\u2111", Imacr: "\u012A", ImaginaryI: "\u2148", Implies: "\u21D2", Int: "\u222C", Integral: "\u222B", Intersection: "\u22C2", InvisibleComma: "\u2063", InvisibleTimes: "\u2062", Iogon: "\u012E", Iopf: "\u{1D540}", Iota: "\u0399", Iscr: "\u2110", Itilde: "\u0128", Iukcy: "\u0406", Iuml: "\xCF", Jcirc: "\u0134", Jcy: "\u0419", Jfr: "\u{1D50D}", Jopf: "\u{1D541}", Jscr: "\u{1D4A5}", Jsercy: "\u0408", Jukcy: "\u0404", KHcy: "\u0425", KJcy: "\u040C", Kappa: "\u039A", Kcedil: "\u0136", Kcy: "\u041A", Kfr: "\u{1D50E}", Kopf: "\u{1D542}", Kscr: "\u{1D4A6}", LJcy: "\u0409", LT: "<", Lacute: "\u0139", Lambda: "\u039B", Lang: "\u27EA", Laplacetrf: "\u2112", Larr: "\u219E", Lcaron: "\u013D", Lcedil: "\u013B", Lcy: "\u041B", LeftAngleBracket: "\u27E8", LeftArrow: "\u2190", LeftArrowBar: "\u21E4", LeftArrowRightArrow: "\u21C6", LeftCeiling: "\u2308", LeftDoubleBracket: "\u27E6", LeftDownTeeVector: "\u2961", LeftDownVector: "\u21C3", LeftDownVectorBar: "\u2959", LeftFloor: "\u230A", LeftRightArrow: "\u2194", LeftRightVector: "\u294E", LeftTee: "\u22A3", LeftTeeArrow: "\u21A4", LeftTeeVector: "\u295A", LeftTriangle: "\u22B2", LeftTriangleBar: "\u29CF", LeftTriangleEqual: "\u22B4", LeftUpDownVector: "\u2951", LeftUpTeeVector: "\u2960", LeftUpVector: "\u21BF", LeftUpVectorBar: "\u2958", LeftVector: "\u21BC", LeftVectorBar: "\u2952", Leftarrow: "\u21D0", Leftrightarrow: "\u21D4", LessEqualGreater: "\u22DA", LessFullEqual: "\u2266", LessGreater: "\u2276", LessLess: "\u2AA1", LessSlantEqual: "\u2A7D", LessTilde: "\u2272", Lfr: "\u{1D50F}", Ll: "\u22D8", Lleftarrow: "\u21DA", Lmidot: "\u013F", LongLeftArrow: "\u27F5", LongLeftRightArrow: "\u27F7", LongRightArrow: "\u27F6", Longleftarrow: "\u27F8", Longleftrightarrow: "\u27FA", Longrightarrow: "\u27F9", Lopf: "\u{1D543}", LowerLeftArrow: "\u2199", LowerRightArrow: "\u2198", Lscr: "\u2112", Lsh: "\u21B0", Lstrok: "\u0141", Lt: "\u226A", Map: "\u2905", Mcy: "\u041C", MediumSpace: "\u205F", Mellintrf: "\u2133", Mfr: "\u{1D510}", MinusPlus: "\u2213", Mopf: "\u{1D544}", Mscr: "\u2133", Mu: "\u039C", NJcy: "\u040A", Nacute: "\u0143", Ncaron: "\u0147", Ncedil: "\u0145", Ncy: "\u041D", NegativeMediumSpace: "\u200B", NegativeThickSpace: "\u200B", NegativeThinSpace: "\u200B", NegativeVeryThinSpace: "\u200B", NestedGreaterGreater: "\u226B", NestedLessLess: "\u226A", NewLine: "\n", Nfr: "\u{1D511}", NoBreak: "\u2060", NonBreakingSpace: "\xA0", Nopf: "\u2115", Not: "\u2AEC", NotCongruent: "\u2262", NotCupCap: "\u226D", NotDoubleVerticalBar: "\u2226", NotElement: "\u2209", NotEqual: "\u2260", NotEqualTilde: "\u2242\u0338", NotExists: "\u2204", NotGreater: "\u226F", NotGreaterEqual: "\u2271", NotGreaterFullEqual: "\u2267\u0338", NotGreaterGreater: "\u226B\u0338", NotGreaterLess: "\u2279", NotGreaterSlantEqual: "\u2A7E\u0338", NotGreaterTilde: "\u2275", NotHumpDownHump: "\u224E\u0338", NotHumpEqual: "\u224F\u0338", NotLeftTriangle: "\u22EA", NotLeftTriangleBar: "\u29CF\u0338", NotLeftTriangleEqual: "\u22EC", NotLess: "\u226E", NotLessEqual: "\u2270", NotLessGreater: "\u2278", NotLessLess: "\u226A\u0338", NotLessSlantEqual: "\u2A7D\u0338", NotLessTilde: "\u2274", NotNestedGreaterGreater: "\u2AA2\u0338", NotNestedLessLess: "\u2AA1\u0338", NotPrecedes: "\u2280", NotPrecedesEqual: "\u2AAF\u0338", NotPrecedesSlantEqual: "\u22E0", NotReverseElement: "\u220C", NotRightTriangle: "\u22EB", NotRightTriangleBar: "\u29D0\u0338", NotRightTriangleEqual: "\u22ED", NotSquareSubset: "\u228F\u0338", NotSquareSubsetEqual: "\u22E2", NotSquareSuperset: "\u2290\u0338", NotSquareSupersetEqual: "\u22E3", NotSubset: "\u2282\u20D2", NotSubsetEqual: "\u2288", NotSucceeds: "\u2281", NotSucceedsEqual: "\u2AB0\u0338", NotSucceedsSlantEqual: "\u22E1", NotSucceedsTilde: "\u227F\u0338", NotSuperset: "\u2283\u20D2", NotSupersetEqual: "\u2289", NotTilde: "\u2241", NotTildeEqual: "\u2244", NotTildeFullEqual: "\u2247", NotTildeTilde: "\u2249", NotVerticalBar: "\u2224", Nscr: "\u{1D4A9}", Ntilde: "\xD1", Nu: "\u039D", OElig: "\u0152", Oacute: "\xD3", Ocirc: "\xD4", Ocy: "\u041E", Odblac: "\u0150", Ofr: "\u{1D512}", Ograve: "\xD2", Omacr: "\u014C", Omega: "\u03A9", Omicron: "\u039F", Oopf: "\u{1D546}", OpenCurlyDoubleQuote: "\u201C", OpenCurlyQuote: "\u2018", Or: "\u2A54", Oscr: "\u{1D4AA}", Oslash: "\xD8", Otilde: "\xD5", Otimes: "\u2A37", Ouml: "\xD6", OverBar: "\u203E", OverBrace: "\u23DE", OverBracket: "\u23B4", OverParenthesis: "\u23DC", PartialD: "\u2202", Pcy: "\u041F", Pfr: "\u{1D513}", Phi: "\u03A6", Pi: "\u03A0", PlusMinus: "\xB1", Poincareplane: "\u210C", Popf: "\u2119", Pr: "\u2ABB", Precedes: "\u227A", PrecedesEqual: "\u2AAF", PrecedesSlantEqual: "\u227C", PrecedesTilde: "\u227E", Prime: "\u2033", Product: "\u220F", Proportion: "\u2237", Proportional: "\u221D", Pscr: "\u{1D4AB}", Psi: "\u03A8", QUOT: '"', Qfr: "\u{1D514}", Qopf: "\u211A", Qscr: "\u{1D4AC}", RBarr: "\u2910", REG: "\xAE", Racute: "\u0154", Rang: "\u27EB", Rarr: "\u21A0", Rarrtl: "\u2916", Rcaron: "\u0158", Rcedil: "\u0156", Rcy: "\u0420", Re: "\u211C", ReverseElement: "\u220B", ReverseEquilibrium: "\u21CB", ReverseUpEquilibrium: "\u296F", Rfr: "\u211C", Rho: "\u03A1", RightAngleBracket: "\u27E9", RightArrow: "\u2192", RightArrowBar: "\u21E5", RightArrowLeftArrow: "\u21C4", RightCeiling: "\u2309", RightDoubleBracket: "\u27E7", RightDownTeeVector: "\u295D", RightDownVector: "\u21C2", RightDownVectorBar: "\u2955", RightFloor: "\u230B", RightTee: "\u22A2", RightTeeArrow: "\u21A6", RightTeeVector: "\u295B", RightTriangle: "\u22B3", RightTriangleBar: "\u29D0", RightTriangleEqual: "\u22B5", RightUpDownVector: "\u294F", RightUpTeeVector: "\u295C", RightUpVector: "\u21BE", RightUpVectorBar: "\u2954", RightVector: "\u21C0", RightVectorBar: "\u2953", Rightarrow: "\u21D2", Ropf: "\u211D", RoundImplies: "\u2970", Rrightarrow: "\u21DB", Rscr: "\u211B", Rsh: "\u21B1", RuleDelayed: "\u29F4", SHCHcy: "\u0429", SHcy: "\u0428", SOFTcy: "\u042C", Sacute: "\u015A", Sc: "\u2ABC", Scaron: "\u0160", Scedil: "\u015E", Scirc: "\u015C", Scy: "\u0421", Sfr: "\u{1D516}", ShortDownArrow: "\u2193", ShortLeftArrow: "\u2190", ShortRightArrow: "\u2192", ShortUpArrow: "\u2191", Sigma: "\u03A3", SmallCircle: "\u2218", Sopf: "\u{1D54A}", Sqrt: "\u221A", Square: "\u25A1", SquareIntersection: "\u2293", SquareSubset: "\u228F", SquareSubsetEqual: "\u2291", SquareSuperset: "\u2290", SquareSupersetEqual: "\u2292", SquareUnion: "\u2294", Sscr: "\u{1D4AE}", Star: "\u22C6", Sub: "\u22D0", Subset: "\u22D0", SubsetEqual: "\u2286", Succeeds: "\u227B", SucceedsEqual: "\u2AB0", SucceedsSlantEqual: "\u227D", SucceedsTilde: "\u227F", SuchThat: "\u220B", Sum: "\u2211", Sup: "\u22D1", Superset: "\u2283", SupersetEqual: "\u2287", Supset: "\u22D1", THORN: "\xDE", TRADE: "\u2122", TSHcy: "\u040B", TScy: "\u0426", Tab: "	", Tau: "\u03A4", Tcaron: "\u0164", Tcedil: "\u0162", Tcy: "\u0422", Tfr: "\u{1D517}", Therefore: "\u2234", Theta: "\u0398", ThickSpace: "\u205F\u200A", ThinSpace: "\u2009", Tilde: "\u223C", TildeEqual: "\u2243", TildeFullEqual: "\u2245", TildeTilde: "\u2248", Topf: "\u{1D54B}", TripleDot: "\u20DB", Tscr: "\u{1D4AF}", Tstrok: "\u0166", Uacute: "\xDA", Uarr: "\u219F", Uarrocir: "\u2949", Ubrcy: "\u040E", Ubreve: "\u016C", Ucirc: "\xDB", Ucy: "\u0423", Udblac: "\u0170", Ufr: "\u{1D518}", Ugrave: "\xD9", Umacr: "\u016A", UnderBar: "_", UnderBrace: "\u23DF", UnderBracket: "\u23B5", UnderParenthesis: "\u23DD", Union: "\u22C3", UnionPlus: "\u228E", Uogon: "\u0172", Uopf: "\u{1D54C}", UpArrow: "\u2191", UpArrowBar: "\u2912", UpArrowDownArrow: "\u21C5", UpDownArrow: "\u2195", UpEquilibrium: "\u296E", UpTee: "\u22A5", UpTeeArrow: "\u21A5", Uparrow: "\u21D1", Updownarrow: "\u21D5", UpperLeftArrow: "\u2196", UpperRightArrow: "\u2197", Upsi: "\u03D2", Upsilon: "\u03A5", Uring: "\u016E", Uscr: "\u{1D4B0}", Utilde: "\u0168", Uuml: "\xDC", VDash: "\u22AB", Vbar: "\u2AEB", Vcy: "\u0412", Vdash: "\u22A9", Vdashl: "\u2AE6", Vee: "\u22C1", Verbar: "\u2016", Vert: "\u2016", VerticalBar: "\u2223", VerticalLine: "|", VerticalSeparator: "\u2758", VerticalTilde: "\u2240", VeryThinSpace: "\u200A", Vfr: "\u{1D519}", Vopf: "\u{1D54D}", Vscr: "\u{1D4B1}", Vvdash: "\u22AA", Wcirc: "\u0174", Wedge: "\u22C0", Wfr: "\u{1D51A}", Wopf: "\u{1D54E}", Wscr: "\u{1D4B2}", Xfr: "\u{1D51B}", Xi: "\u039E", Xopf: "\u{1D54F}", Xscr: "\u{1D4B3}", YAcy: "\u042F", YIcy: "\u0407", YUcy: "\u042E", Yacute: "\xDD", Ycirc: "\u0176", Ycy: "\u042B", Yfr: "\u{1D51C}", Yopf: "\u{1D550}", Yscr: "\u{1D4B4}", Yuml: "\u0178", ZHcy: "\u0416", Zacute: "\u0179", Zcaron: "\u017D", Zcy: "\u0417", Zdot: "\u017B", ZeroWidthSpace: "\u200B", Zeta: "\u0396", Zfr: "\u2128", Zopf: "\u2124", Zscr: "\u{1D4B5}", aacute: "\xE1", abreve: "\u0103", ac: "\u223E", acE: "\u223E\u0333", acd: "\u223F", acirc: "\xE2", acute: "\xB4", acy: "\u0430", aelig: "\xE6", af: "\u2061", afr: "\u{1D51E}", agrave: "\xE0", alefsym: "\u2135", aleph: "\u2135", alpha: "\u03B1", amacr: "\u0101", amalg: "\u2A3F", amp: "&", and: "\u2227", andand: "\u2A55", andd: "\u2A5C", andslope: "\u2A58", andv: "\u2A5A", ang: "\u2220", ange: "\u29A4", angle: "\u2220", angmsd: "\u2221", angmsdaa: "\u29A8", angmsdab: "\u29A9", angmsdac: "\u29AA", angmsdad: "\u29AB", angmsdae: "\u29AC", angmsdaf: "\u29AD", angmsdag: "\u29AE", angmsdah: "\u29AF", angrt: "\u221F", angrtvb: "\u22BE", angrtvbd: "\u299D", angsph: "\u2222", angst: "\xC5", angzarr: "\u237C", aogon: "\u0105", aopf: "\u{1D552}", ap: "\u2248", apE: "\u2A70", apacir: "\u2A6F", ape: "\u224A", apid: "\u224B", apos: "'", approx: "\u2248", approxeq: "\u224A", aring: "\xE5", ascr: "\u{1D4B6}", ast: "*", asymp: "\u2248", asympeq: "\u224D", atilde: "\xE3", auml: "\xE4", awconint: "\u2233", awint: "\u2A11", bNot: "\u2AED", backcong: "\u224C", backepsilon: "\u03F6", backprime: "\u2035", backsim: "\u223D", backsimeq: "\u22CD", barvee: "\u22BD", barwed: "\u2305", barwedge: "\u2305", bbrk: "\u23B5", bbrktbrk: "\u23B6", bcong: "\u224C", bcy: "\u0431", bdquo: "\u201E", becaus: "\u2235", because: "\u2235", bemptyv: "\u29B0", bepsi: "\u03F6", bernou: "\u212C", beta: "\u03B2", beth: "\u2136", between: "\u226C", bfr: "\u{1D51F}", bigcap: "\u22C2", bigcirc: "\u25EF", bigcup: "\u22C3", bigodot: "\u2A00", bigoplus: "\u2A01", bigotimes: "\u2A02", bigsqcup: "\u2A06", bigstar: "\u2605", bigtriangledown: "\u25BD", bigtriangleup: "\u25B3", biguplus: "\u2A04", bigvee: "\u22C1", bigwedge: "\u22C0", bkarow: "\u290D", blacklozenge: "\u29EB", blacksquare: "\u25AA", blacktriangle: "\u25B4", blacktriangledown: "\u25BE", blacktriangleleft: "\u25C2", blacktriangleright: "\u25B8", blank: "\u2423", blk12: "\u2592", blk14: "\u2591", blk34: "\u2593", block: "\u2588", bne: "=\u20E5", bnequiv: "\u2261\u20E5", bnot: "\u2310", bopf: "\u{1D553}", bot: "\u22A5", bottom: "\u22A5", bowtie: "\u22C8", boxDL: "\u2557", boxDR: "\u2554", boxDl: "\u2556", boxDr: "\u2553", boxH: "\u2550", boxHD: "\u2566", boxHU: "\u2569", boxHd: "\u2564", boxHu: "\u2567", boxUL: "\u255D", boxUR: "\u255A", boxUl: "\u255C", boxUr: "\u2559", boxV: "\u2551", boxVH: "\u256C", boxVL: "\u2563", boxVR: "\u2560", boxVh: "\u256B", boxVl: "\u2562", boxVr: "\u255F", boxbox: "\u29C9", boxdL: "\u2555", boxdR: "\u2552", boxdl: "\u2510", boxdr: "\u250C", boxh: "\u2500", boxhD: "\u2565", boxhU: "\u2568", boxhd: "\u252C", boxhu: "\u2534", boxminus: "\u229F", boxplus: "\u229E", boxtimes: "\u22A0", boxuL: "\u255B", boxuR: "\u2558", boxul: "\u2518", boxur: "\u2514", boxv: "\u2502", boxvH: "\u256A", boxvL: "\u2561", boxvR: "\u255E", boxvh: "\u253C", boxvl: "\u2524", boxvr: "\u251C", bprime: "\u2035", breve: "\u02D8", brvbar: "\xA6", bscr: "\u{1D4B7}", bsemi: "\u204F", bsim: "\u223D", bsime: "\u22CD", bsol: "\\", bsolb: "\u29C5", bsolhsub: "\u27C8", bull: "\u2022", bullet: "\u2022", bump: "\u224E", bumpE: "\u2AAE", bumpe: "\u224F", bumpeq: "\u224F", cacute: "\u0107", cap: "\u2229", capand: "\u2A44", capbrcup: "\u2A49", capcap: "\u2A4B", capcup: "\u2A47", capdot: "\u2A40", caps: "\u2229\uFE00", caret: "\u2041", caron: "\u02C7", ccaps: "\u2A4D", ccaron: "\u010D", ccedil: "\xE7", ccirc: "\u0109", ccups: "\u2A4C", ccupssm: "\u2A50", cdot: "\u010B", cedil: "\xB8", cemptyv: "\u29B2", cent: "\xA2", centerdot: "\xB7", cfr: "\u{1D520}", chcy: "\u0447", check: "\u2713", checkmark: "\u2713", chi: "\u03C7", cir: "\u25CB", cirE: "\u29C3", circ: "\u02C6", circeq: "\u2257", circlearrowleft: "\u21BA", circlearrowright: "\u21BB", circledR: "\xAE", circledS: "\u24C8", circledast: "\u229B", circledcirc: "\u229A", circleddash: "\u229D", cire: "\u2257", cirfnint: "\u2A10", cirmid: "\u2AEF", cirscir: "\u29C2", clubs: "\u2663", clubsuit: "\u2663", colon: ":", colone: "\u2254", coloneq: "\u2254", comma: ",", commat: "@", comp: "\u2201", compfn: "\u2218", complement: "\u2201", complexes: "\u2102", cong: "\u2245", congdot: "\u2A6D", conint: "\u222E", copf: "\u{1D554}", coprod: "\u2210", copy: "\xA9", copysr: "\u2117", crarr: "\u21B5", cross: "\u2717", cscr: "\u{1D4B8}", csub: "\u2ACF", csube: "\u2AD1", csup: "\u2AD0", csupe: "\u2AD2", ctdot: "\u22EF", cudarrl: "\u2938", cudarrr: "\u2935", cuepr: "\u22DE", cuesc: "\u22DF", cularr: "\u21B6", cularrp: "\u293D", cup: "\u222A", cupbrcap: "\u2A48", cupcap: "\u2A46", cupcup: "\u2A4A", cupdot: "\u228D", cupor: "\u2A45", cups: "\u222A\uFE00", curarr: "\u21B7", curarrm: "\u293C", curlyeqprec: "\u22DE", curlyeqsucc: "\u22DF", curlyvee: "\u22CE", curlywedge: "\u22CF", curren: "\xA4", curvearrowleft: "\u21B6", curvearrowright: "\u21B7", cuvee: "\u22CE", cuwed: "\u22CF", cwconint: "\u2232", cwint: "\u2231", cylcty: "\u232D", dArr: "\u21D3", dHar: "\u2965", dagger: "\u2020", daleth: "\u2138", darr: "\u2193", dash: "\u2010", dashv: "\u22A3", dbkarow: "\u290F", dblac: "\u02DD", dcaron: "\u010F", dcy: "\u0434", dd: "\u2146", ddagger: "\u2021", ddarr: "\u21CA", ddotseq: "\u2A77", deg: "\xB0", delta: "\u03B4", demptyv: "\u29B1", dfisht: "\u297F", dfr: "\u{1D521}", dharl: "\u21C3", dharr: "\u21C2", diam: "\u22C4", diamond: "\u22C4", diamondsuit: "\u2666", diams: "\u2666", die: "\xA8", digamma: "\u03DD", disin: "\u22F2", div: "\xF7", divide: "\xF7", divideontimes: "\u22C7", divonx: "\u22C7", djcy: "\u0452", dlcorn: "\u231E", dlcrop: "\u230D", dollar: "$", dopf: "\u{1D555}", dot: "\u02D9", doteq: "\u2250", doteqdot: "\u2251", dotminus: "\u2238", dotplus: "\u2214", dotsquare: "\u22A1", doublebarwedge: "\u2306", downarrow: "\u2193", downdownarrows: "\u21CA", downharpoonleft: "\u21C3", downharpoonright: "\u21C2", drbkarow: "\u2910", drcorn: "\u231F", drcrop: "\u230C", dscr: "\u{1D4B9}", dscy: "\u0455", dsol: "\u29F6", dstrok: "\u0111", dtdot: "\u22F1", dtri: "\u25BF", dtrif: "\u25BE", duarr: "\u21F5", duhar: "\u296F", dwangle: "\u29A6", dzcy: "\u045F", dzigrarr: "\u27FF", eDDot: "\u2A77", eDot: "\u2251", eacute: "\xE9", easter: "\u2A6E", ecaron: "\u011B", ecir: "\u2256", ecirc: "\xEA", ecolon: "\u2255", ecy: "\u044D", edot: "\u0117", ee: "\u2147", efDot: "\u2252", efr: "\u{1D522}", eg: "\u2A9A", egrave: "\xE8", egs: "\u2A96", egsdot: "\u2A98", el: "\u2A99", elinters: "\u23E7", ell: "\u2113", els: "\u2A95", elsdot: "\u2A97", emacr: "\u0113", empty: "\u2205", emptyset: "\u2205", emptyv: "\u2205", emsp13: "\u2004", emsp14: "\u2005", emsp: "\u2003", eng: "\u014B", ensp: "\u2002", eogon: "\u0119", eopf: "\u{1D556}", epar: "\u22D5", eparsl: "\u29E3", eplus: "\u2A71", epsi: "\u03B5", epsilon: "\u03B5", epsiv: "\u03F5", eqcirc: "\u2256", eqcolon: "\u2255", eqsim: "\u2242", eqslantgtr: "\u2A96", eqslantless: "\u2A95", equals: "=", equest: "\u225F", equiv: "\u2261", equivDD: "\u2A78", eqvparsl: "\u29E5", erDot: "\u2253", erarr: "\u2971", escr: "\u212F", esdot: "\u2250", esim: "\u2242", eta: "\u03B7", eth: "\xF0", euml: "\xEB", euro: "\u20AC", excl: "!", exist: "\u2203", expectation: "\u2130", exponentiale: "\u2147", fallingdotseq: "\u2252", fcy: "\u0444", female: "\u2640", ffilig: "\uFB03", fflig: "\uFB00", ffllig: "\uFB04", ffr: "\u{1D523}", filig: "\uFB01", fjlig: "fj", flat: "\u266D", fllig: "\uFB02", fltns: "\u25B1", fnof: "\u0192", fopf: "\u{1D557}", forall: "\u2200", fork: "\u22D4", forkv: "\u2AD9", fpartint: "\u2A0D", frac12: "\xBD", frac13: "\u2153", frac14: "\xBC", frac15: "\u2155", frac16: "\u2159", frac18: "\u215B", frac23: "\u2154", frac25: "\u2156", frac34: "\xBE", frac35: "\u2157", frac38: "\u215C", frac45: "\u2158", frac56: "\u215A", frac58: "\u215D", frac78: "\u215E", frasl: "\u2044", frown: "\u2322", fscr: "\u{1D4BB}", gE: "\u2267", gEl: "\u2A8C", gacute: "\u01F5", gamma: "\u03B3", gammad: "\u03DD", gap: "\u2A86", gbreve: "\u011F", gcirc: "\u011D", gcy: "\u0433", gdot: "\u0121", ge: "\u2265", gel: "\u22DB", geq: "\u2265", geqq: "\u2267", geqslant: "\u2A7E", ges: "\u2A7E", gescc: "\u2AA9", gesdot: "\u2A80", gesdoto: "\u2A82", gesdotol: "\u2A84", gesl: "\u22DB\uFE00", gesles: "\u2A94", gfr: "\u{1D524}", gg: "\u226B", ggg: "\u22D9", gimel: "\u2137", gjcy: "\u0453", gl: "\u2277", glE: "\u2A92", gla: "\u2AA5", glj: "\u2AA4", gnE: "\u2269", gnap: "\u2A8A", gnapprox: "\u2A8A", gne: "\u2A88", gneq: "\u2A88", gneqq: "\u2269", gnsim: "\u22E7", gopf: "\u{1D558}", grave: "`", gscr: "\u210A", gsim: "\u2273", gsime: "\u2A8E", gsiml: "\u2A90", gt: ">", gtcc: "\u2AA7", gtcir: "\u2A7A", gtdot: "\u22D7", gtlPar: "\u2995", gtquest: "\u2A7C", gtrapprox: "\u2A86", gtrarr: "\u2978", gtrdot: "\u22D7", gtreqless: "\u22DB", gtreqqless: "\u2A8C", gtrless: "\u2277", gtrsim: "\u2273", gvertneqq: "\u2269\uFE00", gvnE: "\u2269\uFE00", hArr: "\u21D4", hairsp: "\u200A", half: "\xBD", hamilt: "\u210B", hardcy: "\u044A", harr: "\u2194", harrcir: "\u2948", harrw: "\u21AD", hbar: "\u210F", hcirc: "\u0125", hearts: "\u2665", heartsuit: "\u2665", hellip: "\u2026", hercon: "\u22B9", hfr: "\u{1D525}", hksearow: "\u2925", hkswarow: "\u2926", hoarr: "\u21FF", homtht: "\u223B", hookleftarrow: "\u21A9", hookrightarrow: "\u21AA", hopf: "\u{1D559}", horbar: "\u2015", hscr: "\u{1D4BD}", hslash: "\u210F", hstrok: "\u0127", hybull: "\u2043", hyphen: "\u2010", iacute: "\xED", ic: "\u2063", icirc: "\xEE", icy: "\u0438", iecy: "\u0435", iexcl: "\xA1", iff: "\u21D4", ifr: "\u{1D526}", igrave: "\xEC", ii: "\u2148", iiiint: "\u2A0C", iiint: "\u222D", iinfin: "\u29DC", iiota: "\u2129", ijlig: "\u0133", imacr: "\u012B", image: "\u2111", imagline: "\u2110", imagpart: "\u2111", imath: "\u0131", imof: "\u22B7", imped: "\u01B5", in: "\u2208", incare: "\u2105", infin: "\u221E", infintie: "\u29DD", inodot: "\u0131", int: "\u222B", intcal: "\u22BA", integers: "\u2124", intercal: "\u22BA", intlarhk: "\u2A17", intprod: "\u2A3C", iocy: "\u0451", iogon: "\u012F", iopf: "\u{1D55A}", iota: "\u03B9", iprod: "\u2A3C", iquest: "\xBF", iscr: "\u{1D4BE}", isin: "\u2208", isinE: "\u22F9", isindot: "\u22F5", isins: "\u22F4", isinsv: "\u22F3", isinv: "\u2208", it: "\u2062", itilde: "\u0129", iukcy: "\u0456", iuml: "\xEF", jcirc: "\u0135", jcy: "\u0439", jfr: "\u{1D527}", jmath: "\u0237", jopf: "\u{1D55B}", jscr: "\u{1D4BF}", jsercy: "\u0458", jukcy: "\u0454", kappa: "\u03BA", kappav: "\u03F0", kcedil: "\u0137", kcy: "\u043A", kfr: "\u{1D528}", kgreen: "\u0138", khcy: "\u0445", kjcy: "\u045C", kopf: "\u{1D55C}", kscr: "\u{1D4C0}", lAarr: "\u21DA", lArr: "\u21D0", lAtail: "\u291B", lBarr: "\u290E", lE: "\u2266", lEg: "\u2A8B", lHar: "\u2962", lacute: "\u013A", laemptyv: "\u29B4", lagran: "\u2112", lambda: "\u03BB", lang: "\u27E8", langd: "\u2991", langle: "\u27E8", lap: "\u2A85", laquo: "\xAB", larr: "\u2190", larrb: "\u21E4", larrbfs: "\u291F", larrfs: "\u291D", larrhk: "\u21A9", larrlp: "\u21AB", larrpl: "\u2939", larrsim: "\u2973", larrtl: "\u21A2", lat: "\u2AAB", latail: "\u2919", late: "\u2AAD", lates: "\u2AAD\uFE00", lbarr: "\u290C", lbbrk: "\u2772", lbrace: "{", lbrack: "[", lbrke: "\u298B", lbrksld: "\u298F", lbrkslu: "\u298D", lcaron: "\u013E", lcedil: "\u013C", lceil: "\u2308", lcub: "{", lcy: "\u043B", ldca: "\u2936", ldquo: "\u201C", ldquor: "\u201E", ldrdhar: "\u2967", ldrushar: "\u294B", ldsh: "\u21B2", le: "\u2264", leftarrow: "\u2190", leftarrowtail: "\u21A2", leftharpoondown: "\u21BD", leftharpoonup: "\u21BC", leftleftarrows: "\u21C7", leftrightarrow: "\u2194", leftrightarrows: "\u21C6", leftrightharpoons: "\u21CB", leftrightsquigarrow: "\u21AD", leftthreetimes: "\u22CB", leg: "\u22DA", leq: "\u2264", leqq: "\u2266", leqslant: "\u2A7D", les: "\u2A7D", lescc: "\u2AA8", lesdot: "\u2A7F", lesdoto: "\u2A81", lesdotor: "\u2A83", lesg: "\u22DA\uFE00", lesges: "\u2A93", lessapprox: "\u2A85", lessdot: "\u22D6", lesseqgtr: "\u22DA", lesseqqgtr: "\u2A8B", lessgtr: "\u2276", lesssim: "\u2272", lfisht: "\u297C", lfloor: "\u230A", lfr: "\u{1D529}", lg: "\u2276", lgE: "\u2A91", lhard: "\u21BD", lharu: "\u21BC", lharul: "\u296A", lhblk: "\u2584", ljcy: "\u0459", ll: "\u226A", llarr: "\u21C7", llcorner: "\u231E", llhard: "\u296B", lltri: "\u25FA", lmidot: "\u0140", lmoust: "\u23B0", lmoustache: "\u23B0", lnE: "\u2268", lnap: "\u2A89", lnapprox: "\u2A89", lne: "\u2A87", lneq: "\u2A87", lneqq: "\u2268", lnsim: "\u22E6", loang: "\u27EC", loarr: "\u21FD", lobrk: "\u27E6", longleftarrow: "\u27F5", longleftrightarrow: "\u27F7", longmapsto: "\u27FC", longrightarrow: "\u27F6", looparrowleft: "\u21AB", looparrowright: "\u21AC", lopar: "\u2985", lopf: "\u{1D55D}", loplus: "\u2A2D", lotimes: "\u2A34", lowast: "\u2217", lowbar: "_", loz: "\u25CA", lozenge: "\u25CA", lozf: "\u29EB", lpar: "(", lparlt: "\u2993", lrarr: "\u21C6", lrcorner: "\u231F", lrhar: "\u21CB", lrhard: "\u296D", lrm: "\u200E", lrtri: "\u22BF", lsaquo: "\u2039", lscr: "\u{1D4C1}", lsh: "\u21B0", lsim: "\u2272", lsime: "\u2A8D", lsimg: "\u2A8F", lsqb: "[", lsquo: "\u2018", lsquor: "\u201A", lstrok: "\u0142", lt: "<", ltcc: "\u2AA6", ltcir: "\u2A79", ltdot: "\u22D6", lthree: "\u22CB", ltimes: "\u22C9", ltlarr: "\u2976", ltquest: "\u2A7B", ltrPar: "\u2996", ltri: "\u25C3", ltrie: "\u22B4", ltrif: "\u25C2", lurdshar: "\u294A", luruhar: "\u2966", lvertneqq: "\u2268\uFE00", lvnE: "\u2268\uFE00", mDDot: "\u223A", macr: "\xAF", male: "\u2642", malt: "\u2720", maltese: "\u2720", map: "\u21A6", mapsto: "\u21A6", mapstodown: "\u21A7", mapstoleft: "\u21A4", mapstoup: "\u21A5", marker: "\u25AE", mcomma: "\u2A29", mcy: "\u043C", mdash: "\u2014", measuredangle: "\u2221", mfr: "\u{1D52A}", mho: "\u2127", micro: "\xB5", mid: "\u2223", midast: "*", midcir: "\u2AF0", middot: "\xB7", minus: "\u2212", minusb: "\u229F", minusd: "\u2238", minusdu: "\u2A2A", mlcp: "\u2ADB", mldr: "\u2026", mnplus: "\u2213", models: "\u22A7", mopf: "\u{1D55E}", mp: "\u2213", mscr: "\u{1D4C2}", mstpos: "\u223E", mu: "\u03BC", multimap: "\u22B8", mumap: "\u22B8", nGg: "\u22D9\u0338", nGt: "\u226B\u20D2", nGtv: "\u226B\u0338", nLeftarrow: "\u21CD", nLeftrightarrow: "\u21CE", nLl: "\u22D8\u0338", nLt: "\u226A\u20D2", nLtv: "\u226A\u0338", nRightarrow: "\u21CF", nVDash: "\u22AF", nVdash: "\u22AE", nabla: "\u2207", nacute: "\u0144", nang: "\u2220\u20D2", nap: "\u2249", napE: "\u2A70\u0338", napid: "\u224B\u0338", napos: "\u0149", napprox: "\u2249", natur: "\u266E", natural: "\u266E", naturals: "\u2115", nbsp: "\xA0", nbump: "\u224E\u0338", nbumpe: "\u224F\u0338", ncap: "\u2A43", ncaron: "\u0148", ncedil: "\u0146", ncong: "\u2247", ncongdot: "\u2A6D\u0338", ncup: "\u2A42", ncy: "\u043D", ndash: "\u2013", ne: "\u2260", neArr: "\u21D7", nearhk: "\u2924", nearr: "\u2197", nearrow: "\u2197", nedot: "\u2250\u0338", nequiv: "\u2262", nesear: "\u2928", nesim: "\u2242\u0338", nexist: "\u2204", nexists: "\u2204", nfr: "\u{1D52B}", ngE: "\u2267\u0338", nge: "\u2271", ngeq: "\u2271", ngeqq: "\u2267\u0338", ngeqslant: "\u2A7E\u0338", nges: "\u2A7E\u0338", ngsim: "\u2275", ngt: "\u226F", ngtr: "\u226F", nhArr: "\u21CE", nharr: "\u21AE", nhpar: "\u2AF2", ni: "\u220B", nis: "\u22FC", nisd: "\u22FA", niv: "\u220B", njcy: "\u045A", nlArr: "\u21CD", nlE: "\u2266\u0338", nlarr: "\u219A", nldr: "\u2025", nle: "\u2270", nleftarrow: "\u219A", nleftrightarrow: "\u21AE", nleq: "\u2270", nleqq: "\u2266\u0338", nleqslant: "\u2A7D\u0338", nles: "\u2A7D\u0338", nless: "\u226E", nlsim: "\u2274", nlt: "\u226E", nltri: "\u22EA", nltrie: "\u22EC", nmid: "\u2224", nopf: "\u{1D55F}", not: "\xAC", notin: "\u2209", notinE: "\u22F9\u0338", notindot: "\u22F5\u0338", notinva: "\u2209", notinvb: "\u22F7", notinvc: "\u22F6", notni: "\u220C", notniva: "\u220C", notnivb: "\u22FE", notnivc: "\u22FD", npar: "\u2226", nparallel: "\u2226", nparsl: "\u2AFD\u20E5", npart: "\u2202\u0338", npolint: "\u2A14", npr: "\u2280", nprcue: "\u22E0", npre: "\u2AAF\u0338", nprec: "\u2280", npreceq: "\u2AAF\u0338", nrArr: "\u21CF", nrarr: "\u219B", nrarrc: "\u2933\u0338", nrarrw: "\u219D\u0338", nrightarrow: "\u219B", nrtri: "\u22EB", nrtrie: "\u22ED", nsc: "\u2281", nsccue: "\u22E1", nsce: "\u2AB0\u0338", nscr: "\u{1D4C3}", nshortmid: "\u2224", nshortparallel: "\u2226", nsim: "\u2241", nsime: "\u2244", nsimeq: "\u2244", nsmid: "\u2224", nspar: "\u2226", nsqsube: "\u22E2", nsqsupe: "\u22E3", nsub: "\u2284", nsubE: "\u2AC5\u0338", nsube: "\u2288", nsubset: "\u2282\u20D2", nsubseteq: "\u2288", nsubseteqq: "\u2AC5\u0338", nsucc: "\u2281", nsucceq: "\u2AB0\u0338", nsup: "\u2285", nsupE: "\u2AC6\u0338", nsupe: "\u2289", nsupset: "\u2283\u20D2", nsupseteq: "\u2289", nsupseteqq: "\u2AC6\u0338", ntgl: "\u2279", ntilde: "\xF1", ntlg: "\u2278", ntriangleleft: "\u22EA", ntrianglelefteq: "\u22EC", ntriangleright: "\u22EB", ntrianglerighteq: "\u22ED", nu: "\u03BD", num: "#", numero: "\u2116", numsp: "\u2007", nvDash: "\u22AD", nvHarr: "\u2904", nvap: "\u224D\u20D2", nvdash: "\u22AC", nvge: "\u2265\u20D2", nvgt: ">\u20D2", nvinfin: "\u29DE", nvlArr: "\u2902", nvle: "\u2264\u20D2", nvlt: "<\u20D2", nvltrie: "\u22B4\u20D2", nvrArr: "\u2903", nvrtrie: "\u22B5\u20D2", nvsim: "\u223C\u20D2", nwArr: "\u21D6", nwarhk: "\u2923", nwarr: "\u2196", nwarrow: "\u2196", nwnear: "\u2927", oS: "\u24C8", oacute: "\xF3", oast: "\u229B", ocir: "\u229A", ocirc: "\xF4", ocy: "\u043E", odash: "\u229D", odblac: "\u0151", odiv: "\u2A38", odot: "\u2299", odsold: "\u29BC", oelig: "\u0153", ofcir: "\u29BF", ofr: "\u{1D52C}", ogon: "\u02DB", ograve: "\xF2", ogt: "\u29C1", ohbar: "\u29B5", ohm: "\u03A9", oint: "\u222E", olarr: "\u21BA", olcir: "\u29BE", olcross: "\u29BB", oline: "\u203E", olt: "\u29C0", omacr: "\u014D", omega: "\u03C9", omicron: "\u03BF", omid: "\u29B6", ominus: "\u2296", oopf: "\u{1D560}", opar: "\u29B7", operp: "\u29B9", oplus: "\u2295", or: "\u2228", orarr: "\u21BB", ord: "\u2A5D", order: "\u2134", orderof: "\u2134", ordf: "\xAA", ordm: "\xBA", origof: "\u22B6", oror: "\u2A56", orslope: "\u2A57", orv: "\u2A5B", oscr: "\u2134", oslash: "\xF8", osol: "\u2298", otilde: "\xF5", otimes: "\u2297", otimesas: "\u2A36", ouml: "\xF6", ovbar: "\u233D", par: "\u2225", para: "\xB6", parallel: "\u2225", parsim: "\u2AF3", parsl: "\u2AFD", part: "\u2202", pcy: "\u043F", percnt: "%", period: ".", permil: "\u2030", perp: "\u22A5", pertenk: "\u2031", pfr: "\u{1D52D}", phi: "\u03C6", phiv: "\u03D5", phmmat: "\u2133", phone: "\u260E", pi: "\u03C0", pitchfork: "\u22D4", piv: "\u03D6", planck: "\u210F", planckh: "\u210E", plankv: "\u210F", plus: "+", plusacir: "\u2A23", plusb: "\u229E", pluscir: "\u2A22", plusdo: "\u2214", plusdu: "\u2A25", pluse: "\u2A72", plusmn: "\xB1", plussim: "\u2A26", plustwo: "\u2A27", pm: "\xB1", pointint: "\u2A15", popf: "\u{1D561}", pound: "\xA3", pr: "\u227A", prE: "\u2AB3", prap: "\u2AB7", prcue: "\u227C", pre: "\u2AAF", prec: "\u227A", precapprox: "\u2AB7", preccurlyeq: "\u227C", preceq: "\u2AAF", precnapprox: "\u2AB9", precneqq: "\u2AB5", precnsim: "\u22E8", precsim: "\u227E", prime: "\u2032", primes: "\u2119", prnE: "\u2AB5", prnap: "\u2AB9", prnsim: "\u22E8", prod: "\u220F", profalar: "\u232E", profline: "\u2312", profsurf: "\u2313", prop: "\u221D", propto: "\u221D", prsim: "\u227E", prurel: "\u22B0", pscr: "\u{1D4C5}", psi: "\u03C8", puncsp: "\u2008", qfr: "\u{1D52E}", qint: "\u2A0C", qopf: "\u{1D562}", qprime: "\u2057", qscr: "\u{1D4C6}", quaternions: "\u210D", quatint: "\u2A16", quest: "?", questeq: "\u225F", quot: '"', rAarr: "\u21DB", rArr: "\u21D2", rAtail: "\u291C", rBarr: "\u290F", rHar: "\u2964", race: "\u223D\u0331", racute: "\u0155", radic: "\u221A", raemptyv: "\u29B3", rang: "\u27E9", rangd: "\u2992", range: "\u29A5", rangle: "\u27E9", raquo: "\xBB", rarr: "\u2192", rarrap: "\u2975", rarrb: "\u21E5", rarrbfs: "\u2920", rarrc: "\u2933", rarrfs: "\u291E", rarrhk: "\u21AA", rarrlp: "\u21AC", rarrpl: "\u2945", rarrsim: "\u2974", rarrtl: "\u21A3", rarrw: "\u219D", ratail: "\u291A", ratio: "\u2236", rationals: "\u211A", rbarr: "\u290D", rbbrk: "\u2773", rbrace: "}", rbrack: "]", rbrke: "\u298C", rbrksld: "\u298E", rbrkslu: "\u2990", rcaron: "\u0159", rcedil: "\u0157", rceil: "\u2309", rcub: "}", rcy: "\u0440", rdca: "\u2937", rdldhar: "\u2969", rdquo: "\u201D", rdquor: "\u201D", rdsh: "\u21B3", real: "\u211C", realine: "\u211B", realpart: "\u211C", reals: "\u211D", rect: "\u25AD", reg: "\xAE", rfisht: "\u297D", rfloor: "\u230B", rfr: "\u{1D52F}", rhard: "\u21C1", rharu: "\u21C0", rharul: "\u296C", rho: "\u03C1", rhov: "\u03F1", rightarrow: "\u2192", rightarrowtail: "\u21A3", rightharpoondown: "\u21C1", rightharpoonup: "\u21C0", rightleftarrows: "\u21C4", rightleftharpoons: "\u21CC", rightrightarrows: "\u21C9", rightsquigarrow: "\u219D", rightthreetimes: "\u22CC", ring: "\u02DA", risingdotseq: "\u2253", rlarr: "\u21C4", rlhar: "\u21CC", rlm: "\u200F", rmoust: "\u23B1", rmoustache: "\u23B1", rnmid: "\u2AEE", roang: "\u27ED", roarr: "\u21FE", robrk: "\u27E7", ropar: "\u2986", ropf: "\u{1D563}", roplus: "\u2A2E", rotimes: "\u2A35", rpar: ")", rpargt: "\u2994", rppolint: "\u2A12", rrarr: "\u21C9", rsaquo: "\u203A", rscr: "\u{1D4C7}", rsh: "\u21B1", rsqb: "]", rsquo: "\u2019", rsquor: "\u2019", rthree: "\u22CC", rtimes: "\u22CA", rtri: "\u25B9", rtrie: "\u22B5", rtrif: "\u25B8", rtriltri: "\u29CE", ruluhar: "\u2968", rx: "\u211E", sacute: "\u015B", sbquo: "\u201A", sc: "\u227B", scE: "\u2AB4", scap: "\u2AB8", scaron: "\u0161", sccue: "\u227D", sce: "\u2AB0", scedil: "\u015F", scirc: "\u015D", scnE: "\u2AB6", scnap: "\u2ABA", scnsim: "\u22E9", scpolint: "\u2A13", scsim: "\u227F", scy: "\u0441", sdot: "\u22C5", sdotb: "\u22A1", sdote: "\u2A66", seArr: "\u21D8", searhk: "\u2925", searr: "\u2198", searrow: "\u2198", sect: "\xA7", semi: ";", seswar: "\u2929", setminus: "\u2216", setmn: "\u2216", sext: "\u2736", sfr: "\u{1D530}", sfrown: "\u2322", sharp: "\u266F", shchcy: "\u0449", shcy: "\u0448", shortmid: "\u2223", shortparallel: "\u2225", shy: "\xAD", sigma: "\u03C3", sigmaf: "\u03C2", sigmav: "\u03C2", sim: "\u223C", simdot: "\u2A6A", sime: "\u2243", simeq: "\u2243", simg: "\u2A9E", simgE: "\u2AA0", siml: "\u2A9D", simlE: "\u2A9F", simne: "\u2246", simplus: "\u2A24", simrarr: "\u2972", slarr: "\u2190", smallsetminus: "\u2216", smashp: "\u2A33", smeparsl: "\u29E4", smid: "\u2223", smile: "\u2323", smt: "\u2AAA", smte: "\u2AAC", smtes: "\u2AAC\uFE00", softcy: "\u044C", sol: "/", solb: "\u29C4", solbar: "\u233F", sopf: "\u{1D564}", spades: "\u2660", spadesuit: "\u2660", spar: "\u2225", sqcap: "\u2293", sqcaps: "\u2293\uFE00", sqcup: "\u2294", sqcups: "\u2294\uFE00", sqsub: "\u228F", sqsube: "\u2291", sqsubset: "\u228F", sqsubseteq: "\u2291", sqsup: "\u2290", sqsupe: "\u2292", sqsupset: "\u2290", sqsupseteq: "\u2292", squ: "\u25A1", square: "\u25A1", squarf: "\u25AA", squf: "\u25AA", srarr: "\u2192", sscr: "\u{1D4C8}", ssetmn: "\u2216", ssmile: "\u2323", sstarf: "\u22C6", star: "\u2606", starf: "\u2605", straightepsilon: "\u03F5", straightphi: "\u03D5", strns: "\xAF", sub: "\u2282", subE: "\u2AC5", subdot: "\u2ABD", sube: "\u2286", subedot: "\u2AC3", submult: "\u2AC1", subnE: "\u2ACB", subne: "\u228A", subplus: "\u2ABF", subrarr: "\u2979", subset: "\u2282", subseteq: "\u2286", subseteqq: "\u2AC5", subsetneq: "\u228A", subsetneqq: "\u2ACB", subsim: "\u2AC7", subsub: "\u2AD5", subsup: "\u2AD3", succ: "\u227B", succapprox: "\u2AB8", succcurlyeq: "\u227D", succeq: "\u2AB0", succnapprox: "\u2ABA", succneqq: "\u2AB6", succnsim: "\u22E9", succsim: "\u227F", sum: "\u2211", sung: "\u266A", sup1: "\xB9", sup2: "\xB2", sup3: "\xB3", sup: "\u2283", supE: "\u2AC6", supdot: "\u2ABE", supdsub: "\u2AD8", supe: "\u2287", supedot: "\u2AC4", suphsol: "\u27C9", suphsub: "\u2AD7", suplarr: "\u297B", supmult: "\u2AC2", supnE: "\u2ACC", supne: "\u228B", supplus: "\u2AC0", supset: "\u2283", supseteq: "\u2287", supseteqq: "\u2AC6", supsetneq: "\u228B", supsetneqq: "\u2ACC", supsim: "\u2AC8", supsub: "\u2AD4", supsup: "\u2AD6", swArr: "\u21D9", swarhk: "\u2926", swarr: "\u2199", swarrow: "\u2199", swnwar: "\u292A", szlig: "\xDF", target: "\u2316", tau: "\u03C4", tbrk: "\u23B4", tcaron: "\u0165", tcedil: "\u0163", tcy: "\u0442", tdot: "\u20DB", telrec: "\u2315", tfr: "\u{1D531}", there4: "\u2234", therefore: "\u2234", theta: "\u03B8", thetasym: "\u03D1", thetav: "\u03D1", thickapprox: "\u2248", thicksim: "\u223C", thinsp: "\u2009", thkap: "\u2248", thksim: "\u223C", thorn: "\xFE", tilde: "\u02DC", times: "\xD7", timesb: "\u22A0", timesbar: "\u2A31", timesd: "\u2A30", tint: "\u222D", toea: "\u2928", top: "\u22A4", topbot: "\u2336", topcir: "\u2AF1", topf: "\u{1D565}", topfork: "\u2ADA", tosa: "\u2929", tprime: "\u2034", trade: "\u2122", triangle: "\u25B5", triangledown: "\u25BF", triangleleft: "\u25C3", trianglelefteq: "\u22B4", triangleq: "\u225C", triangleright: "\u25B9", trianglerighteq: "\u22B5", tridot: "\u25EC", trie: "\u225C", triminus: "\u2A3A", triplus: "\u2A39", trisb: "\u29CD", tritime: "\u2A3B", trpezium: "\u23E2", tscr: "\u{1D4C9}", tscy: "\u0446", tshcy: "\u045B", tstrok: "\u0167", twixt: "\u226C", twoheadleftarrow: "\u219E", twoheadrightarrow: "\u21A0", uArr: "\u21D1", uHar: "\u2963", uacute: "\xFA", uarr: "\u2191", ubrcy: "\u045E", ubreve: "\u016D", ucirc: "\xFB", ucy: "\u0443", udarr: "\u21C5", udblac: "\u0171", udhar: "\u296E", ufisht: "\u297E", ufr: "\u{1D532}", ugrave: "\xF9", uharl: "\u21BF", uharr: "\u21BE", uhblk: "\u2580", ulcorn: "\u231C", ulcorner: "\u231C", ulcrop: "\u230F", ultri: "\u25F8", umacr: "\u016B", uml: "\xA8", uogon: "\u0173", uopf: "\u{1D566}", uparrow: "\u2191", updownarrow: "\u2195", upharpoonleft: "\u21BF", upharpoonright: "\u21BE", uplus: "\u228E", upsi: "\u03C5", upsih: "\u03D2", upsilon: "\u03C5", upuparrows: "\u21C8", urcorn: "\u231D", urcorner: "\u231D", urcrop: "\u230E", uring: "\u016F", urtri: "\u25F9", uscr: "\u{1D4CA}", utdot: "\u22F0", utilde: "\u0169", utri: "\u25B5", utrif: "\u25B4", uuarr: "\u21C8", uuml: "\xFC", uwangle: "\u29A7", vArr: "\u21D5", vBar: "\u2AE8", vBarv: "\u2AE9", vDash: "\u22A8", vangrt: "\u299C", varepsilon: "\u03F5", varkappa: "\u03F0", varnothing: "\u2205", varphi: "\u03D5", varpi: "\u03D6", varpropto: "\u221D", varr: "\u2195", varrho: "\u03F1", varsigma: "\u03C2", varsubsetneq: "\u228A\uFE00", varsubsetneqq: "\u2ACB\uFE00", varsupsetneq: "\u228B\uFE00", varsupsetneqq: "\u2ACC\uFE00", vartheta: "\u03D1", vartriangleleft: "\u22B2", vartriangleright: "\u22B3", vcy: "\u0432", vdash: "\u22A2", vee: "\u2228", veebar: "\u22BB", veeeq: "\u225A", vellip: "\u22EE", verbar: "|", vert: "|", vfr: "\u{1D533}", vltri: "\u22B2", vnsub: "\u2282\u20D2", vnsup: "\u2283\u20D2", vopf: "\u{1D567}", vprop: "\u221D", vrtri: "\u22B3", vscr: "\u{1D4CB}", vsubnE: "\u2ACB\uFE00", vsubne: "\u228A\uFE00", vsupnE: "\u2ACC\uFE00", vsupne: "\u228B\uFE00", vzigzag: "\u299A", wcirc: "\u0175", wedbar: "\u2A5F", wedge: "\u2227", wedgeq: "\u2259", weierp: "\u2118", wfr: "\u{1D534}", wopf: "\u{1D568}", wp: "\u2118", wr: "\u2240", wreath: "\u2240", wscr: "\u{1D4CC}", xcap: "\u22C2", xcirc: "\u25EF", xcup: "\u22C3", xdtri: "\u25BD", xfr: "\u{1D535}", xhArr: "\u27FA", xharr: "\u27F7", xi: "\u03BE", xlArr: "\u27F8", xlarr: "\u27F5", xmap: "\u27FC", xnis: "\u22FB", xodot: "\u2A00", xopf: "\u{1D569}", xoplus: "\u2A01", xotime: "\u2A02", xrArr: "\u27F9", xrarr: "\u27F6", xscr: "\u{1D4CD}", xsqcup: "\u2A06", xuplus: "\u2A04", xutri: "\u25B3", xvee: "\u22C1", xwedge: "\u22C0", yacute: "\xFD", yacy: "\u044F", ycirc: "\u0177", ycy: "\u044B", yen: "\xA5", yfr: "\u{1D536}", yicy: "\u0457", yopf: "\u{1D56A}", yscr: "\u{1D4CE}", yucy: "\u044E", yuml: "\xFF", zacute: "\u017A", zcaron: "\u017E", zcy: "\u0437", zdot: "\u017C", zeetrf: "\u2128", zeta: "\u03B6", zfr: "\u{1D537}", zhcy: "\u0436", zigrarr: "\u21DD", zopf: "\u{1D56B}", zscr: "\u{1D4CF}", zwj: "\u200D", zwnj: "\u200C" };
  var f = Q("characterReference", (0, function(r2, b2, e2) {
    var c2 = this;
    let t2 = [0, 0, 0];
    var n2 = function(f3) {
      if (59 === f3 && t2[0] > 0) {
        var i3 = r2.exit("characterReferenceValue");
        if (0 == t2[2] && !nr(E(c2.sliceSerialize(i3)))) return e2(f3);
        r2.enter("characterReferenceMarker"), r2.consume(f3), r2.exit("characterReferenceMarker"), r2.exit("characterReference");
        return b2;
      }
      if (((r3, b3) => 0 == r3 ? P(K, b3) : 1 == r3 ? P(Rt, b3) : P(Ne, b3))(t2[2], f3) && t2[0] < t2[1]) {
        var w2 = t2[0] + 1 | 0;
        t2[0] = w2, r2.consume(f3);
        return n2;
      }
      return e2(f3);
    };
    let f2 = function(b3) {
      if (88 === b3 || 120 === b3) return r2.enter("characterReferenceMarkerHexadecimal"), r2.consume(b3), r2.exit("characterReferenceMarkerHexadecimal"), r2.enter("characterReferenceValue"), t2[1] = 6, t2[2] = 1, n2;
      r2.enter("characterReferenceValue"), t2[1] = 7, t2[2] = 2;
      return n2(b3);
    }, i2 = function(b3) {
      if (35 === b3) return r2.enter("characterReferenceMarkerNumeric"), r2.consume(b3), r2.exit("characterReferenceMarkerNumeric"), f2;
      r2.enter("characterReferenceValue"), t2[1] = 31, t2[2] = 0;
      return n2(b3);
    };
    return function(b3) {
      r2.enter("characterReference"), r2.enter("characterReferenceMarker"), r2.consume(b3), r2.exit("characterReferenceMarker");
      return i2;
    };
  }));
  var Ve = U((0, function(r2, b2, e2) {
    var t2 = this;
    let n2 = function(r3) {
      return t2.parser.lazy[t2.now().line] ? e2(r3) : b2(r3);
    };
    return function(b3) {
      if (A(b3)) return e2(b3);
      r2.enter("lineEnding"), r2.consume(b3), r2.exit("lineEnding");
      return n2;
    };
  }));
  Ve.partial = e;
  var v = { name: "codeFenced", tokenize: function(r2, b2, e2) {
    var t2, x2, c2, i2, w2 = this, o2 = 0, f2 = 0, n2 = 0;
    let u2 = U(function(r3, b3, e3) {
      var t3 = 0;
      let c3 = function(t4) {
        return A(t4) || T(t4) ? (r3.exit("codeFencedFence"), b3(t4)) : e3(t4);
      };
      var E2 = function(b4) {
        return b4 == n2 ? (t3 = t3 + 1 | 0, r3.consume(b4), E2) : t3 >= f2 ? (r3.exit("codeFencedFenceSequence"), C(b4) ? R(r3, c3, "whitespace", 0)(b4) : c3(b4)) : e3(b4);
      };
      let i3 = function(b4) {
        return b4 == n2 ? (r3.enter("codeFencedFenceSequence"), E2(b4)) : e3(b4);
      }, o3 = function(b4) {
        r3.enter("codeFencedFence");
        if (C(b4)) {
          var e4 = M(w2.parser.constructs.disable.null, "codeIndented") ? 0 : 4;
          return R(r3, i3, "linePrefix", e4)(b4);
        }
        return i3(b4);
      };
      return function(b4) {
        r3.enter("lineEnding"), r3.consume(b4), r3.exit("lineEnding");
        return o3;
      };
    });
    u2.partial = true;
    let a2 = function(e3) {
      r2.exit("codeFenced");
      return b2(e3);
    };
    var l2 = function(b3) {
      if (A(b3) || T(b3)) return r2.exit("codeFlowValue"), c2(b3);
      r2.consume(b3);
      return l2;
    };
    c2 = function(b3) {
      if (A(b3) || T(b3)) return r2.check(Ve, i2, a2)(b3);
      r2.enter("codeFlowValue");
      return l2(b3);
    };
    let m2 = function(b3) {
      return o2 > 0 && C(b3) ? R(r2, c2, "linePrefix", o2 + 1)(b3) : c2(b3);
    }, v2 = function(b3) {
      r2.enter("lineEnding"), r2.consume(b3), r2.exit("lineEnding");
      return m2;
    };
    i2 = function(b3) {
      return r2.attempt(u2, a2, v2)(b3);
    };
    var S2 = function(b3) {
      if (A(b3) || T(b3)) return r2.exit("chunkString"), r2.exit("codeFencedFenceInfo"), t2(b3);
      if (C(b3)) return r2.exit("chunkString"), r2.exit("codeFencedFenceInfo"), R(r2, x2, "whitespace", 0)(b3);
      if (96 === b3 && b3 == n2) return e2(b3);
      r2.consume(b3);
      return S2;
    }, p2 = function(b3) {
      if (A(b3) || T(b3)) return r2.exit("chunkString"), r2.exit("codeFencedFenceMeta"), t2(b3);
      if (96 === b3 && b3 == n2) return e2(b3);
      r2.consume(b3);
      return p2;
    };
    x2 = function(b3) {
      if (A(b3) || T(b3)) return t2(b3);
      r2.enter("codeFencedFenceMeta"), r2.enter("chunkString", { contentType: "string" });
      return p2(b3);
    }, t2 = function(e3) {
      if (A(e3) || T(e3)) return r2.exit("codeFencedFence"), w2.interrupt ? b2(e3) : r2.check(Ve, i2, a2)(e3);
      r2.enter("codeFencedFenceInfo"), r2.enter("chunkString", { contentType: "string" });
      return S2(e3);
    };
    var y2 = function(b3) {
      if (b3 == n2) return f2 = f2 + 1 | 0, r2.consume(b3), y2;
      if (f2 < 3) return e2(b3);
      r2.exit("codeFencedFenceSequence");
      return C(b3) ? R(r2, t2, "whitespace", 0)(b3) : t2(b3);
    };
    return function(b3) {
      var c3, t3 = w2.events, e3 = t3[q(t3) - 1];
      o2 = c3 = e3 && "linePrefix" == e3[1].type ? E(e3[2].sliceSerialize.call(e3[2], e3[1], true)).length : 0, n2 = b3 | 0, r2.enter("codeFenced"), r2.enter("codeFencedFence"), r2.enter("codeFencedFenceSequence");
      return y2(b3);
    };
  }, concrete: e };
  var qr = U((0, function(r2, b2, e2) {
    var t2, n2 = this;
    let c2 = function(r3) {
      var f2 = n2.events, c3 = f2[q(f2) - 1];
      return c3 && "linePrefix" == c3[1].type && E(c3[2].sliceSerialize.call(c3[2], c3[1], true)).length >= 4 ? b2(r3) : T(r3) ? t2(r3) : e2(r3);
    };
    t2 = function(b3) {
      return n2.parser.lazy[n2.now().line] ? e2(b3) : T(b3) ? (r2.enter("lineEnding"), r2.consume(b3), r2.exit("lineEnding"), t2) : R(r2, c2, "linePrefix", 5)(b3);
    };
    return t2;
  }));
  qr.partial = e, r = Q("codeIndented", (0, function(r2, b2, e2) {
    var t2, f2 = this, n2 = function(b3) {
      if (A(b3) || T(b3)) return r2.exit("codeFlowValue"), t2(b3);
      r2.consume(b3);
      return n2;
    };
    let c2 = function(e3) {
      r2.exit("codeIndented");
      return b2(e3);
    };
    t2 = function(b3) {
      if (A(b3)) return c2(b3);
      if (T(b3)) return r2.attempt(qr, t2, c2)(b3);
      r2.enter("codeFlowValue");
      return n2(b3);
    };
    let i2 = function(r3) {
      var n3 = f2.events, b3 = n3[q(n3) - 1];
      return b3 && "linePrefix" == b3[1].type && E(b3[2].sliceSerialize.call(b3[2], b3[1], true)).length >= 4 ? t2(r3) : e2(r3);
    };
    return function(b3) {
      r2.enter("codeIndented");
      return R(r2, i2, "linePrefix", 5)(b3);
    };
  }));
  var b = { name: "codeText", tokenize: function(r2, b2, e2) {
    var f2, t2, E2 = 0, n2 = 0, c2 = function(b3) {
      if (A(b3) || 32 === b3 || 96 === b3 || T(b3)) return r2.exit("codeTextData"), t2(b3);
      r2.consume(b3);
      return c2;
    }, i2 = function(e3) {
      if (96 === e3) return r2.consume(e3), n2++, i2;
      if (n2 == E2) return r2.exit("codeTextSequence"), r2.exit("codeText"), b2(e3);
      f2.type = "codeTextData";
      return c2(e3);
    };
    t2 = function(b3) {
      if (A(b3)) return e2(b3);
      if (32 === b3) return r2.enter("space"), r2.consume(b3), r2.exit("space"), t2;
      if (96 === b3) return f2 = r2.enter("codeTextSequence"), n2 = 0, i2(b3);
      if (T(b3)) return r2.enter("lineEnding"), r2.consume(b3), r2.exit("lineEnding"), t2;
      r2.enter("codeTextData");
      return c2(b3);
    };
    var w2 = function(b3) {
      if (96 === b3) return r2.consume(b3), E2++, w2;
      r2.exit("codeTextSequence");
      return t2(b3);
    };
    return function(b3) {
      r2.enter("codeText"), r2.enter("codeTextSequence");
      return w2(b3);
    };
  }, previous: function(r2) {
    if (96 !== r2) return true;
    r2 = this.events;
    return "characterEscape" == r2[q(r2) - 1][1].type;
  }, resolve: function(r2, b2) {
    var e2, n2, c2, t2 = q(r2) - 4 | 0;
    if (("lineEnding" == r2[3][1].type || "space" == r2[3][1].type) && ("lineEnding" == r2[t2][1].type || "space" == r2[t2][1].type)) for (e2 = 3; ; ) {
      if ((e2 + 1 | 0) >= t2) {
        e2 = 3;
        break;
      }
      e2++;
      if ("codeTextData" == r2[e2][1].type) {
        r2[3][1].type = "codeTextPadding", r2[t2][1].type = "codeTextPadding", t2 = e2 = t2 - 2 | 0, e2 = 5;
        break;
      }
    }
    else {
      e2 = 3;
    }
    b2 = t2 + 1 | 0;
    e2--, t2 = -1;
    for (; e2 < b2; ) e2++, t2 < 0 ? e2 != b2 && "lineEnding" != r2[e2][1].type && (t2 = e2) : (e2 == b2 || "lineEnding" == r2[e2][1].type) && (r2[t2][1].type = "codeTextData", e2 != (t2 + 2 | 0) && (n2 = r2[t2][1], n2.end = r2[e2 - 1][1].end, n2 = t2 + 2 | 0, c2 = (e2 - t2 | 0) - 2 | 0, r2.splice(n2, c2), b2 = b2 - c2 | 0, e2 = n2), t2 = -1);
    return r2;
  } };
  var Ar = (0, function(r2, b2, e2, t2, n2, c2) {
    var q2 = this;
    let a2 = t2 + "", w2 = n2 + "", x2 = c2 + "";
    var f2, E2, i2 = 0, o2 = false;
    let u2 = function(b3) {
      return 91 === b3 || 92 === b3 || 93 === b3 ? (r2.consume(b3), i2++, f2) : f2(b3);
    };
    f2 = function(b3) {
      if (A(b3) || 91 === b3 || 93 === b3 || T(b3)) return r2.exit("chunkString"), E2(b3);
      var e3 = i2;
      i2++;
      if (e3 > 999) return r2.exit("chunkString"), E2(b3);
      r2.consume(b3), o2 = o2 || !C(b3);
      return 92 === b3 ? u2 : f2;
    }, E2 = function(t3) {
      if (i2 > 999 || A(t3) || 91 === t3 || 93 === t3 && !o2 || 94 === t3 && 0 == i2 && F(q2.parser.constructs, "_hiddenFootnoteSupport")) return e2(t3);
      if (93 === t3) return r2.exit(x2), r2.enter(w2), r2.consume(t3), r2.exit(w2), r2.exit(a2), b2;
      if (T(t3)) return r2.enter("lineEnding"), r2.consume(t3), r2.exit("lineEnding"), E2;
      r2.enter("chunkString", { contentType: "string" });
      return f2(t3);
    };
    return function(b3) {
      r2.enter(a2), r2.enter(w2), r2.consume(b3), r2.exit(w2), r2.enter(x2);
      return E2;
    };
  });
  var Ut = /[\t\n\r ]+/g;
  var Ht = /^ | $/g;
  var Tr = U(function(r2, b2, e2) {
    let t2 = function(r3) {
      return A(r3) || T(r3) ? b2(r3) : e2(r3);
    }, n2 = function(b3) {
      return C(b3) ? R(r2, t2, "whitespace", 0)(b3) : t2(b3);
    }, c2 = function(b3) {
      return ft(r2, n2, e2, "definitionTitle", "definitionTitleMarker", "definitionTitleString")(b3);
    };
    return function(b3) {
      return H(b3) ? we(r2, c2)(b3) : e2(b3);
    };
  });
  Tr.partial = e;
  var y = Q("definition", (0, function(r2, b2, e2) {
    var t2 = this, n2 = "";
    let c2 = function(c3) {
      return A(c3) || T(c3) ? (r2.exit("definition"), t2.parser.defined.push(n2), b2(c3)) : e2(c3);
    }, f2 = function(b3) {
      return C(b3) ? R(r2, c2, "whitespace", 0)(b3) : c2(b3);
    }, w2 = function(b3) {
      return r2.attempt(Tr, f2, f2)(b3);
    }, i2 = function(b3) {
      return st(r2, w2, e2, "definitionDestination", "definitionDestinationLiteral", "definitionDestinationLiteralMarker", "definitionDestinationRaw", "definitionDestinationString", 0)(b3);
    }, o2 = function(b3) {
      return H(b3) ? we(r2, i2)(b3) : i2(b3);
    }, a2 = function(b3) {
      var c3 = t2.events, f3 = t2.sliceSerialize;
      f3 = E(t2.sliceSerialize(c3[q(c3) - 1][1])), n2 = de(E(f3.slice(1, -1)));
      return 58 === b3 ? (r2.enter("definitionMarker"), r2.consume(b3), r2.exit("definitionMarker"), o2) : e2(b3);
    }, x2 = function(b3) {
      return Ar.apply(t2, [r2, a2, e2, "definitionLabel", "definitionLabelMarker", "definitionLabelString"])(b3);
    };
    return function(b3) {
      r2.enter("definition");
      return x2(b3);
    };
  }));
  var k = Q("hardBreakEscape", function(r2, b2, e2) {
    let t2 = function(t3) {
      return T(t3) ? (r2.exit("hardBreakEscape"), b2(t3)) : e2(t3);
    };
    return function(b3) {
      r2.enter("hardBreakEscape"), r2.consume(b3);
      return t2;
    };
  });
  var x = { name: "headingAtx", tokenize: function(r2, b2, e2) {
    var t2, n2 = 0, c2 = function(b3) {
      if (A(b3) || 35 === b3 || H(b3)) return r2.exit("atxHeadingText"), t2(b3);
      r2.consume(b3);
      return c2;
    }, f2 = function(b3) {
      if (35 === b3) return r2.consume(b3), f2;
      r2.exit("atxHeadingSequence");
      return t2(b3);
    };
    t2 = function(e3) {
      if (35 === e3) return r2.enter("atxHeadingSequence"), f2(e3);
      if (A(e3) || T(e3)) return r2.exit("atxHeading"), b2(e3);
      if (C(e3)) return R(r2, t2, "whitespace", 0)(e3);
      r2.enter("atxHeadingText");
      return c2(e3);
    };
    var E2 = function(b3) {
      return 35 === b3 && n2 < 6 ? (n2 = n2 + 1 | 0, r2.consume(b3), E2) : A(b3) || H(b3) ? (r2.exit("atxHeadingSequence"), t2(b3)) : e2(b3);
    };
    let i2 = function(b3) {
      r2.enter("atxHeadingSequence");
      return E2(b3);
    };
    return function(b3) {
      r2.enter("atxHeading");
      return i2(b3);
    };
  }, resolve: function(r2, b2) {
    var n2, c2, e2 = q(r2) - 2 | 0, t2 = "whitespace" == r2[3][1].type ? 5 : 3;
    (e2 - 2 | 0) > t2 && "whitespace" == r2[e2][1].type && (e2 = e2 - 2 | 0), "atxHeadingSequence" == r2[e2][1].type && (t2 == (e2 - 1 | 0) || (e2 - 4 | 0) > t2 && "whitespace" == r2[e2 - 2][1].type) && (e2 = t2 + 1 == e2 ? e2 - 2 | 0 : e2 - 4 | 0), e2 > t2 && (c2 = { type: "atxHeadingText", start: r2[t2][1].start, end: r2[e2][1].end }, n2 = { type: "chunkText", start: r2[t2][1].start, end: r2[e2][1].end, contentType: "text" }, Y(r2, t2, (e2 - t2 | 0) + 1 | 0, [["enter", c2, b2], ["enter", n2, b2], ["exit", n2, b2], ["exit", c2, b2]]));
    return r2;
  } };
  var Gt = "address article aside base basefont blockquote body caption center col colgroup dd details dialog dir div dl dt fieldset figcaption figure footer form frame frameset h1 h2 h3 h4 h5 h6 head header hr html iframe legend li link main menu menuitem nav noframes ol optgroup option p param search section summary table tbody td tfoot th thead title tr track ul".split(" ");
  var Ir = "pre script style textarea".split(" ");
  t = function(r2, b2) {
    for (var e2 = q(r2); --e2 >= 0; ) if ("enter" == r2[e2][0] && "htmlFlow" == r2[e2][1].type) break;
    if (e2 > 1 && "linePrefix" == r2[e2 - 2][1].type) {
      var t2 = r2[e2][1];
      b2 = e2 - 2 | 0, t2.start = r2[b2][1].start, t2 = r2[e2 + 1][1], t2.start = r2[b2][1].start, r2.splice(b2, 2);
    }
    return r2;
  };
  var Lr = U((0, function(r2, b2, e2) {
    var t2 = this;
    let n2 = function(r3) {
      return t2.parser.lazy[t2.now().line] ? e2(r3) : b2(r3);
    };
    return function(b3) {
      return T(b3) ? (r2.enter("lineEnding"), r2.consume(b3), r2.exit("lineEnding"), n2) : e2(b3);
    };
  }));
  Lr.partial = e;
  var Cr = U(function(r2, b2, e2) {
    return function(t2) {
      r2.enter("lineEnding"), r2.consume(t2), r2.exit("lineEnding");
      return r2.attempt(me, b2, e2);
    };
  });
  Cr.partial = e;
  var w = { name: "htmlFlow", tokenize: function(r2, b2, e2) {
    var o2, a2, m2, v2, L2, h2, i2, w2, x2, f2 = this, n2 = 0, l2 = false, E2 = "", q2 = 0, S2 = 0;
    let t2 = function(b3) {
      r2.consume(b3);
    };
    o2 = function(e3) {
      r2.exit("htmlFlow");
      return b2(e3);
    };
    var u2 = function(b3) {
      if (A(b3) || T(b3)) return r2.exit("htmlFlowData"), o2(b3);
      t2(b3);
      return u2;
    }, c2 = function(b3) {
      if (45 === b3 && 2 == n2) return t2(b3), v2;
      if (60 === b3 && 1 == n2) return t2(b3), L2;
      if (62 === b3 && 4 == n2) return t2(b3), u2;
      if (63 === b3 && 3 == n2) return t2(b3), i2;
      if (93 === b3 && 5 == n2) return t2(b3), h2;
      if (T(b3) && (6 == n2 || 7 == n2)) return r2.exit("htmlFlowData"), r2.check(Cr, o2, a2)(b3);
      if (A(b3) || T(b3)) return r2.exit("htmlFlowData"), a2(b3);
      t2(b3);
      return c2;
    };
    let D2 = function(b3) {
      r2.enter("lineEnding"), t2(b3), r2.exit("lineEnding");
      return m2;
    };
    a2 = function(b3) {
      return r2.check(Lr, D2, o2)(b3);
    }, m2 = function(b3) {
      if (A(b3) || T(b3)) return a2(b3);
      r2.enter("htmlFlowData");
      return c2(b3);
    }, v2 = function(r3) {
      return 45 === r3 ? (t2(r3), i2) : c2(r3);
    };
    var g2 = function(r3) {
      if (62 === r3) return M(Ir, E2.toLowerCase()) ? (t2(r3), u2) : c2(r3);
      if (P($, r3) && E2.length < 8) {
        t2(r3);
        var b3 = E2;
        E2 = b3 + pe(r3);
        return g2;
      }
      return c2(r3);
    };
    L2 = function(r3) {
      return 47 === r3 ? (t2(r3), E2 = "", g2) : c2(r3);
    }, h2 = function(r3) {
      return 93 === r3 ? (t2(r3), i2) : c2(r3);
    }, i2 = function(r3) {
      return 62 === r3 ? (t2(r3), u2) : 45 === r3 && 2 == n2 ? (t2(r3), i2) : c2(r3);
    };
    var I2 = function(r3) {
      return A(r3) || T(r3) ? c2(r3) : C(r3) ? (t2(r3), I2) : e2(r3);
    };
    let p2 = function(r3) {
      return 62 === r3 ? (t2(r3), I2) : e2(r3);
    }, Q2 = function(r3) {
      return 47 === r3 || 62 === r3 || C(r3) ? w2(r3) : e2(r3);
    };
    var s2 = function(r3) {
      if (r3 == S2) return t2(r3), S2 = 0, Q2;
      if (A(r3) || T(r3)) return e2(r3);
      t2(r3);
      return s2;
    }, Y2 = function(r3) {
      if (A(r3) || 34 === r3 || 39 === r3 || 47 === r3 || 60 === r3 || 61 === r3 || 62 === r3 || 96 === r3 || H(r3)) return x2(r3);
      t2(r3);
      return Y2;
    }, O2 = function(r3) {
      return A(r3) || 60 === r3 || 61 === r3 || 62 === r3 || 96 === r3 ? e2(r3) : 34 === r3 || 39 === r3 ? (t2(r3), S2 = r3 | 0, s2) : C(r3) ? (t2(r3), O2) : Y2(r3);
    };
    x2 = function(r3) {
      return 61 === r3 ? (t2(r3), O2) : C(r3) ? (t2(r3), x2) : w2(r3);
    };
    var d2 = function(r3) {
      return 45 === r3 || 46 === r3 || 58 === r3 || 95 === r3 || P(K, r3) ? (t2(r3), d2) : x2(r3);
    };
    w2 = function(r3) {
      return 47 === r3 ? (t2(r3), p2) : 58 === r3 || 95 === r3 || P($, r3) ? (t2(r3), d2) : C(r3) ? (t2(r3), w2) : p2(r3);
    };
    var R2 = function(r3) {
      return C(r3) ? (t2(r3), R2) : p2(r3);
    };
    let W2 = function(r3) {
      return 62 === r3 ? (t2(r3), f2.interrupt ? b2 : c2) : e2(r3);
    };
    var y2 = function(r3) {
      if (A(r3) || 47 === r3 || 62 === r3 || H(r3)) {
        var i3, o3 = 47 === r3, a3 = E2.toLowerCase();
        if (!o3 && !l2 && M(Ir, a3)) return n2 = 1, f2.interrupt ? b2(r3) : c2(r3);
        if (M(Gt, a3)) return n2 = 6, o3 ? (t2(r3), W2) : f2.interrupt ? b2(r3) : c2(r3);
        n2 = 7;
        if (f2.interrupt) {
          i3 = f2.parser.lazy;
          var x3 = !i3[f2.now().line];
        } else {
          x3 = false;
        }
        return x3 ? e2(r3) : l2 ? R2(r3) : w2(r3);
      }
      if (45 === r3 || P(K, r3)) {
        t2(r3);
        var q3 = E2;
        E2 = q3 + pe(r3);
        return y2;
      }
      return e2(r3);
    };
    let j2 = function(r3) {
      return P($, r3) ? (t2(r3), E2 = pe(r3), y2) : e2(r3);
    };
    var k2 = function(r3) {
      var n3 = q2, E3 = q2;
      q2 = E3 + 1 | 0;
      return !A(r3) && ot(r3) == ("CDATA[".charCodeAt(n3) | 0) ? (t2(r3), 6 == (n3 + 1 | 0) ? f2.interrupt ? b2 : c2 : k2) : e2(r3);
    };
    let N2 = function(r3) {
      return 45 === r3 ? (t2(r3), f2.interrupt ? b2 : i2) : e2(r3);
    }, B2 = function(r3) {
      return 45 === r3 ? (t2(r3), n2 = 2, N2) : 91 === r3 ? (t2(r3), n2 = 5, q2 = 0, k2) : P($, r3) ? (t2(r3), n2 = 4, f2.interrupt ? b2 : i2) : e2(r3);
    }, F2 = function(r3) {
      return 33 === r3 ? (t2(r3), B2) : 47 === r3 ? (t2(r3), l2 = true, j2) : 63 === r3 ? (t2(r3), n2 = 3, f2.interrupt ? b2 : i2) : P($, r3) ? (t2(r3), E2 = pe(r3), y2) : e2(r3);
    };
    return function(b3) {
      r2.enter("htmlFlow"), r2.enter("htmlFlowData"), t2(b3);
      return F2;
    };
  }, concrete: e, resolveTo: t };
  var S = Q("htmlText", (0, function(r2, b2, e2) {
    var n2, f2, v2, L2, x2, q2, k2 = this, u2 = 0, w2 = 0;
    let t2 = function(b3) {
      r2.consume(b3);
    }, h2 = function(b3) {
      r2.enter("htmlTextData");
      return n2(b3);
    }, D2 = function(b3) {
      if (C(b3)) {
        var e3 = M(k2.parser.constructs.disable.null, "codeIndented") ? 0 : 4;
        return R(r2, h2, "linePrefix", e3)(b3);
      }
      return h2(b3);
    }, c2 = function(b3) {
      r2.exit("htmlTextData"), r2.enter("lineEnding"), t2(b3), r2.exit("lineEnding");
      return D2;
    }, E2 = function(n3) {
      return 62 === n3 ? (t2(n3), r2.exit("htmlTextData"), r2.exit("htmlText"), b2) : e2(n3);
    }, Q2 = function(r3) {
      return 47 === r3 || 62 === r3 || H(r3) ? f2(r3) : e2(r3);
    };
    var g2 = function(r3) {
      if (A(r3) || 34 === r3 || 39 === r3 || 60 === r3 || 61 === r3 || 96 === r3) return e2(r3);
      if (47 === r3 || 62 === r3 || H(r3)) return f2(r3);
      t2(r3);
      return g2;
    }, l2 = function(r3) {
      if (r3 == u2) return t2(r3), u2 = 0, Q2;
      if (A(r3)) return e2(r3);
      if (T(r3)) return n2 = l2, c2(r3);
      t2(r3);
      return l2;
    }, S2 = function(r3) {
      if (A(r3) || 60 === r3 || 61 === r3 || 62 === r3 || 96 === r3) return e2(r3);
      if (34 === r3 || 39 === r3) return t2(r3), u2 = r3 | 0, l2;
      if (T(r3)) return n2 = S2, c2(r3);
      if (C(r3)) return t2(r3), S2;
      t2(r3);
      return g2;
    }, p2 = function(r3) {
      return 61 === r3 ? (t2(r3), S2) : T(r3) ? (n2 = p2, c2(r3)) : C(r3) ? (t2(r3), p2) : f2(r3);
    }, I2 = function(r3) {
      return 45 === r3 || 46 === r3 || 58 === r3 || 95 === r3 || P(K, r3) ? (t2(r3), I2) : p2(r3);
    };
    f2 = function(r3) {
      return 47 === r3 ? (t2(r3), E2) : 58 === r3 || 95 === r3 || P($, r3) ? (t2(r3), I2) : T(r3) ? (n2 = f2, c2(r3)) : C(r3) ? (t2(r3), f2) : E2(r3);
    };
    var s2 = function(r3) {
      return 45 === r3 || P(K, r3) ? (t2(r3), s2) : 47 === r3 || 62 === r3 || H(r3) ? f2(r3) : e2(r3);
    }, y2 = function(r3) {
      return T(r3) ? (n2 = y2, c2(r3)) : C(r3) ? (t2(r3), y2) : E2(r3);
    }, Y2 = function(r3) {
      return 45 === r3 || P(K, r3) ? (t2(r3), Y2) : y2(r3);
    };
    let W2 = function(r3) {
      return P($, r3) ? (t2(r3), Y2) : e2(r3);
    };
    var o2 = function(r3) {
      if (A(r3)) return e2(r3);
      if (63 === r3) return t2(r3), v2;
      if (T(r3)) return n2 = o2, c2(r3);
      t2(r3);
      return o2;
    };
    v2 = function(r3) {
      return 62 === r3 ? E2(r3) : o2(r3);
    };
    var m2 = function(r3) {
      if (A(r3) || 62 === r3) return E2(r3);
      if (T(r3)) return n2 = m2, c2(r3);
      t2(r3);
      return m2;
    }, i2 = function(r3) {
      if (A(r3)) return e2(r3);
      if (93 === r3) return t2(r3), L2;
      if (T(r3)) return n2 = i2, c2(r3);
      t2(r3);
      return i2;
    }, O2 = function(r3) {
      return 62 === r3 ? E2(r3) : 93 === r3 ? (t2(r3), O2) : i2(r3);
    };
    L2 = function(r3) {
      return 93 === r3 ? (t2(r3), O2) : i2(r3);
    };
    var d2 = function(r3) {
      var b3 = w2, n3 = w2;
      w2 = n3 + 1 | 0;
      return !A(r3) && ot(r3) == ("CDATA[".charCodeAt(b3) | 0) ? (t2(r3), 6 == (b3 + 1 | 0) ? i2 : d2) : e2(r3);
    }, a2 = function(r3) {
      if (A(r3)) return e2(r3);
      if (45 === r3) return t2(r3), x2;
      if (T(r3)) return n2 = a2, c2(r3);
      t2(r3);
      return a2;
    };
    x2 = function(r3) {
      return 45 === r3 ? (t2(r3), q2) : a2(r3);
    }, q2 = function(r3) {
      return 62 === r3 ? E2(r3) : 45 === r3 ? x2(r3) : a2(r3);
    };
    let j2 = function(r3) {
      return 45 === r3 ? (t2(r3), q2) : e2(r3);
    }, N2 = function(r3) {
      return 45 === r3 ? (t2(r3), j2) : 91 === r3 ? (t2(r3), w2 = 0, d2) : P($, r3) ? (t2(r3), m2) : e2(r3);
    }, B2 = function(r3) {
      return 33 === r3 ? (t2(r3), N2) : 47 === r3 ? (t2(r3), W2) : 63 === r3 ? (t2(r3), o2) : P($, r3) ? (t2(r3), s2) : e2(r3);
    };
    return function(b3) {
      r2.enter("htmlText"), r2.enter("htmlTextData"), t2(b3);
      return B2;
    };
  }));
  t = function(r2, b2) {
    b2 = [];
    for (var n2, e2, c2 = q(r2), t2 = -1; ++t2 < c2; ) n2 = r2[t2][1], b2.push(r2[t2]), e2 = E(n2.type), ("labelImage" == e2 || "labelLink" == e2 || "labelEnd" == e2) && (e2 = "labelImage" == e2 ? 4 : 2, n2.type = "data", t2 = t2 + e2 | 0);
    q(r2) != q(b2) && Y(r2, 0, q(r2), b2);
    return r2;
  }, a = function(r2, b2) {
    for (var n2, t2, f2, a2, u2, w2, x2, o2, l2, A2, e2 = q(r2), c2 = void 0, i2 = void 0; ; ) {
      if (false) {
        w2 = 0, e2 = c2;
        break;
      }
      e2--;
      if (e2 < 0) {
        w2 = 0, e2 = c2;
        break;
      }
      n2 = r2[e2][1];
      t2 = E(n2.type);
      if (c2) {
        if ("link" == t2 || "labelLink" == t2 && n2._inactive) {
          w2 = 0, e2 = c2;
          break;
        }
        "enter" == r2[e2][0] && "labelLink" == t2 && (n2._inactive = true);
      } else if (i2) {
        if ("enter" == r2[e2][0] && ("labelImage" == t2 || "labelLink" == t2) && !n2._balanced) {
          if ("labelLink" != t2) {
            w2 = 2;
            break;
          }
          c2 = e2;
        }
      } else "labelEnd" == t2 && (i2 = e2);
    }
    c2 = e2 | 0;
    i2 = i2 | 0, e2 = "labelLink" == r2[c2][1].type ? "link" : "image";
    var S2 = z(r2[c2][1].start);
    t2 = { type: e2, start: S2, end: z(r2[q(r2) - 1][1].end) };
    var p2 = z(r2[c2][1].start);
    n2 = { type: "label", start: p2, end: z(r2[i2][1].end) }, f2 = { type: "labelText" }, a2 = c2 + w2 | 0, f2.start = z(r2[a2 + 2][1].end), u2 = i2 - 2, f2.end = z(r2[u2][1].start), w2 = [["enter", t2, b2], ["enter", n2, b2]], e2 = c2 + 1 | 0, x2 = a2 + 3 | 0, e2 = Z(w2, r2.slice(e2, x2)), x2 = [["enter", f2, b2]], e2 = Z(e2, x2), o2 = b2.parser.constructs.insideSpan.null, l2 = a2 + 4 | 0, A2 = i2 - 3 | 0, o2 = Z(e2, er(o2, r2.slice(l2, A2), b2)), e2 = [["exit", f2, b2], r2[u2], r2[i2 - 1], ["exit", n2, b2]], n2 = Z(o2, e2), f2 = i2 + 1 | 0, o2 = q(r2), f2 = Z(n2, r2.slice(f2, o2)), n2 = [["exit", t2, b2]], t2 = Z(f2, n2), Y(r2, c2, q(r2), t2);
    return r2;
  };
  var o = (0, function(r2, b2, e2) {
    var t2 = this;
    let n2 = function(r3) {
      var n3 = t2.events, c3 = t2.sliceSerialize;
      c3 = E(t2.sliceSerialize(n3[q(n3) - 1][1]));
      var f2 = de(E(c3.slice(1, -1)));
      return M(t2.parser.defined, f2) ? b2(r3) : e2(r3);
    }, c2 = function(r3) {
      return e2(r3);
    };
    return function(b3) {
      return Ar.apply(t2, [r2, n2, c2, "reference", "referenceMarker", "referenceString"])(b3);
    };
  });
  var p = function(r2, b2, e2) {
    let t2 = function(t3) {
      return 93 === t3 ? (r2.enter("referenceMarker"), r2.consume(t3), r2.exit("referenceMarker"), r2.exit("reference"), b2) : e2(t3);
    };
    return function(b3) {
      r2.enter("reference"), r2.enter("referenceMarker"), r2.consume(b3), r2.exit("referenceMarker");
      return t2;
    };
  };
  var Qt = U(function(r2, b2, e2) {
    let t2 = function(t3) {
      return 41 === t3 ? (r2.enter("resourceMarker"), r2.consume(t3), r2.exit("resourceMarker"), r2.exit("resource"), b2) : e2(t3);
    }, c2 = function(b3) {
      return H(b3) ? we(r2, t2)(b3) : t2(b3);
    }, f2 = function(b3) {
      return 34 === b3 || 39 === b3 || 40 === b3 ? ft(r2, c2, e2, "resourceTitle", "resourceTitleMarker", "resourceTitleString")(b3) : t2(b3);
    }, E2 = function(r3) {
      return e2(r3);
    }, i2 = function(b3) {
      return H(b3) ? we(r2, f2)(b3) : t2(b3);
    }, n2 = function(b3) {
      return 41 === b3 ? t2(b3) : st(r2, i2, E2, "resourceDestination", "resourceDestinationLiteral", "resourceDestinationLiteralMarker", "resourceDestinationRaw", "resourceDestinationString", 32)(b3);
    }, w2 = function(b3) {
      return H(b3) ? we(r2, n2)(b3) : n2(b3);
    };
    return function(b3) {
      r2.enter("resource"), r2.enter("resourceMarker"), r2.consume(b3), r2.exit("resourceMarker");
      return w2;
    };
  });
  var Yt = U(o);
  var Zt = U(p);
  a = { name: "labelEnd", tokenize: function(r2, b2, e2) {
    for (var c2, f2 = this, n2 = q(f2.events); --n2 >= 0; ) {
      var t2 = f2.events[n2][1], i2 = E(t2.type);
      if (("labelImage" == i2 || "labelLink" == i2) && !t2._balanced) {
        c2 = t2;
        break;
      }
    }
    var w2 = false;
    t2 = function(r3) {
      return b2(r3);
    }, n2 = function(r3) {
      c2 && (c2._balanced = true);
      return e2(r3);
    }, i2 = function(b3) {
      return r2.attempt(Zt, t2, n2)(b3);
    };
    var o2 = function(b3) {
      if (40 === b3) {
        var e3 = w2 ? t2 : n2;
        return r2.attempt(Qt, t2, e3)(b3);
      }
      return 91 === b3 ? (e3 = w2 ? i2 : n2, r2.attempt(Yt, t2, e3)(b3)) : w2 ? t2(b3) : n2(b3);
    };
    return function(b3) {
      if (!c2) return e2(b3);
      if (c2._inactive) return n2(b3);
      var t3 = { start: c2.end, end: f2.now() };
      t3 = de(E(f2.sliceSerialize(t3))), w2 = M(f2.parser.defined, t3), r2.enter("labelEnd"), r2.enter("labelMarker"), r2.consume(b3), r2.exit("labelMarker"), r2.exit("labelEnd");
      return o2;
    };
  }, resolveAll: t, resolveTo: a }, p = Q("labelStartImage", (0, function(r2, b2, e2) {
    var t2 = this;
    let n2 = function(r3) {
      return 94 === r3 && F(t2.parser.constructs, "_hiddenFootnoteSupport") ? e2(r3) : b2(r3);
    }, c2 = function(b3) {
      return 91 === b3 ? (r2.enter("labelMarker"), r2.consume(b3), r2.exit("labelMarker"), r2.exit("labelImage"), n2) : e2(b3);
    };
    return function(b3) {
      r2.enter("labelImage"), r2.enter("labelImageMarker"), r2.consume(b3), r2.exit("labelImageMarker");
      return c2;
    };
  })), p.resolveAll = a.resolveAll;
  var d = Q("labelStartLink", (0, function(r2, b2, e2) {
    var t2 = this;
    let n2 = function(r3) {
      return 94 === r3 && F(t2.parser.constructs, "_hiddenFootnoteSupport") ? e2(r3) : b2(r3);
    };
    return function(b3) {
      r2.enter("labelLink"), r2.enter("labelMarker"), r2.consume(b3), r2.exit("labelMarker"), r2.exit("labelLink");
      return n2;
    };
  }));
  d.resolveAll = a.resolveAll, o = Q("lineEnding", function(r2, b2, e2) {
    return function(e3) {
      r2.enter("lineEnding"), r2.consume(e3), r2.exit("lineEnding");
      return R(r2, b2, "linePrefix", 0);
    };
  });
  var Te = Q("thematicBreak", function(r2, b2, e2) {
    var t2, c2 = 0, n2 = 0, f2 = function(b3) {
      if (b3 == n2) return r2.consume(b3), c2++, f2;
      r2.exit("thematicBreakSequence");
      return C(b3) ? R(r2, t2, "whitespace", 0)(b3) : t2(b3);
    };
    t2 = function(t3) {
      return t3 == n2 ? (r2.enter("thematicBreakSequence"), f2(t3)) : c2 >= 3 && (A(t3) || T(t3)) ? (r2.exit("thematicBreak"), b2(t3)) : e2(t3);
    };
    let E2 = function(r3) {
      n2 = r3 | 0;
      return t2(r3);
    };
    return function(b3) {
      r2.enter("thematicBreak");
      return E2(b3);
    };
  });
  var Dr = U((0, function(r2, b2, e2) {
    var t2 = this, c2 = M(t2.parser.constructs.disable.null, "codeIndented") ? 0 : 5;
    return R(r2, function(r3) {
      var n2 = t2.events, f2 = q(n2), c3 = n2[f2 - 1];
      return !C(r3) && c3 && "listItemPrefixWhitespace" == c3[1].type ? b2(r3) : e2(r3);
    }, "listItemPrefixWhitespace", c2);
  }));
  Dr.partial = e;
  var Pr = U((0, function(r2, b2, e2) {
    var t2 = this;
    return R(r2, function(r3) {
      var n2 = t2.events, c2 = n2[q(n2) - 1];
      c2 && "listItemIndent" == c2[1].type ? (n2 = E(c2[2].sliceSerialize.call(c2[2], c2[1], true)).length, n2 = n2 == t2.containerState.size) : n2 = false;
      return n2 ? b2(r3) : e2(r3);
    }, "listItemIndent", +t2.containerState.size + 1);
  }));
  Pr.partial = e;
  var te = { name: "list" };
  t = function(r2, b2, e2) {
    var t2 = this;
    t2.containerState._closeFlow = void 0;
    let n2 = function(n3) {
      t2.containerState._closeFlow = true, t2.interrupt = void 0;
      var c2 = M(t2.parser.constructs.disable.null, "codeIndented") ? 0 : 4;
      return R(r2, r2.attempt(te, b2, e2), "linePrefix", c2)(n3);
    };
    return r2.check(me, function(e3) {
      !t2.containerState.furtherBlankLines && (t2.containerState.furtherBlankLines = t2.containerState.initialBlankLine);
      return R(r2, b2, "listItemIndent", +t2.containerState.size + 1)(e3);
    }, function(e3) {
      if (t2.containerState.furtherBlankLines || !C(e3)) return t2.containerState.furtherBlankLines = void 0, t2.containerState.initialBlankLine = void 0, n2(e3);
      t2.containerState.furtherBlankLines = void 0, t2.containerState.initialBlankLine = void 0;
      return r2.attempt(Pr, b2, n2)(e3);
    });
  };
  var h = function(r2) {
    let b2 = r2.exit;
    r2.exit(this.containerState.type);
  };
  te.tokenize = function(r2, b2, e2) {
    var t2 = this, c2 = t2.events, n2 = c2[q(c2) - 1], i2 = 0;
    n2 && "linePrefix" == n2[1].type && (i2 = E(n2[2].sliceSerialize.call(n2[2], n2[1], true)).length);
    var f2 = 0;
    n2 = function(e3) {
      let n3 = r2.exit("listItemPrefix"), c3 = t2.containerState;
      c3.size = i2 + E(t2.sliceSerialize(n3, true)).length;
      return b2(e3);
    };
    var o2 = function(b3) {
      return C(b3) ? (r2.enter("listItemPrefixWhitespace"), r2.consume(b3), r2.exit("listItemPrefixWhitespace"), n2) : e2(b3);
    }, a2 = function(r3) {
      t2.containerState.initialBlankLine = true, i2++;
      return n2(r3);
    };
    c2 = function(b3) {
      r2.enter("listItemMarker"), r2.consume(b3), r2.exit("listItemMarker"), t2.containerState.marker || (t2.containerState.marker = b3);
      var f3 = t2.interrupt ? e2 : a2, c3 = r2.check;
      return r2.check(me, f3, r2.attempt(Dr, n2, o2));
    };
    var w2 = function(b3) {
      if (P(Ne, b3) && (f2 + 1 | 0) < 10) return f2 = f2 + 1 | 0, r2.consume(b3), w2;
      var E2 = !t2.interrupt || f2 < 2, n3 = t2.containerState.marker, i3 = n3 ? b3 == n3 : 41 === b3 || 46 === b3;
      return E2 && i3 ? (r2.exit("listItemValue"), c2(b3)) : e2(b3);
    };
    return function(b3) {
      var n3 = t2.containerState, f3 = n3.type ? E(n3.type) : 42 === b3 || 43 === b3 || 45 === b3 ? "listUnordered" : "listOrdered";
      if ("listUnordered" == f3 ? !n3.marker || b3 == n3.marker : P(Ne, b3)) {
        n3.type || (n3.type = f3, n3 = { _container: true }, r2.enter(f3, n3));
        if ("listUnordered" == f3) {
          var i3 = r2.enter;
          r2.enter("listItemPrefix");
          return 42 === b3 || 45 === b3 ? r2.check(Te, e2, c2)(b3) : c2(b3);
        }
        if (!t2.interrupt || 49 === b3) return r2.enter("listItemPrefix"), r2.enter("listItemValue"), w2(b3);
      }
      return e2(b3);
    };
  }, e = { tokenize: t }, te.continuation = e, te.exit = h, h = { name: "setextUnderline", tokenize: function(r2, b2, e2) {
    var t2 = this, n2 = 0;
    let c2 = function(t3) {
      return A(t3) || T(t3) ? (r2.exit("setextHeadingLine"), b2(t3)) : e2(t3);
    };
    var f2 = function(b3) {
      if (b3 == n2) return r2.consume(b3), f2;
      r2.exit("setextHeadingLineSequence");
      return C(b3) ? R(r2, c2, "lineSuffix", 0)(b3) : c2(b3);
    };
    let E2 = function(b3) {
      r2.enter("setextHeadingLineSequence");
      return f2(b3);
    };
    return function(b3) {
      for (var c3 = q(t2.events); ; ) {
        if (false) {
          c3 = false;
          break;
        }
        c3--;
        if (c3 < 0) {
          c3 = false;
          break;
        }
        var f3 = t2.events[c3][1].type + "";
        if ("lineEnding" != f3 && "linePrefix" != f3 && "content" != f3) {
          c3 = "paragraph" == f3;
          break;
        }
      }
      f3 = t2.parser.lazy;
      return !f3[t2.now().line] && (!!t2.interrupt || c3) ? (r2.enter("setextHeadingLine"), n2 = b3 | 0, E2(b3)) : e2(b3);
    };
  }, resolveTo: function(r2, b2) {
    for (var c2, e2 = q(r2), t2 = -1, n2 = -1; ; ) {
      if (false) {
        e2 = -1;
        break;
      }
      e2--;
      if (e2 < 0) {
        e2 = -1;
        break;
      }
      if ("enter" == r2[e2][0]) {
        if ("content" == r2[e2][1].type) break;
        "paragraph" == r2[e2][1].type && (t2 = e2);
      } else {
        "content" == r2[e2][1].type && r2.splice(e2, 1);
        n2 < 0 && "definition" == r2[e2][1].type && (n2 = e2);
      }
    }
    var f2 = z(r2[e2][1].start);
    c2 = { type: "setextHeading", start: f2, end: z(r2[q(r2) - 1][1].end) }, r2[t2][1].type = "setextHeadingText", n2 > 0 ? (r2.splice(t2, 0, ["enter", c2, b2]), t2 = ["exit", r2[e2][1], b2], r2.splice(n2 + 1 | 0, 0, t2), t2 = r2[e2][1], t2.end = z(r2[n2][1].end)) : r2[e2][1] = c2, r2.push(["exit", c2, b2]);
    return r2;
  } }, t = {}, D(t, 42, te), D(t, 43, te), D(t, 45, te);
  for (e = 48; e <= 57; e++) D(t, e, te);
  D(t, 62, Ae);
  var g = {};
  D(g, 91, y), n = {}, D(n, -2, r), D(n, -1, r), D(n, 32, r), r = {}, D(r, 35, x), D(r, 42, Te), u = Te, e = [], e[0] = h, e[1] = u, D(r, 45, e), D(r, 60, w), D(r, 61, h), D(r, 95, Te), D(r, 96, v), D(r, 126, v), u = {}, D(u, 38, f), D(u, 92, s), e = {}, D(e, -5, o), D(e, -4, o), D(e, -3, o), D(e, 33, p), D(e, 38, f), D(e, 42, i), o = [], o[0] = m, o[1] = S, D(e, 60, o), D(e, 91, d), o = [], o[0] = k, o[1] = s, D(e, 92, o), D(e, 93, a), D(e, 95, i), D(e, 96, b), a = [], a[0] = i, a[1] = c, o = { null: a }, i = [], i[0] = 42, i[1] = 95, a = { null: i }, i = { null: [] };
  var X = { document: t, contentInitial: g, flowInitial: n, flow: r, string: u, text: e, insideSpan: o, attentionMarkers: a, disable: i };
  var Rr = new RegExp("\\\\([!-/:-@[-`{-~])|&(#(?:\\d{1,7}|x[\\da-f]{1,6})|[\\da-z]{1,31});", "gi");
  var Me = function(r2, b2, e2) {
    if (r2 && "object" == typeof r2) {
      if (F(r2, "value")) return "html" == E(r2.type) && !e2 ? "" : r2.value;
      if (b2 && F(r2, "alt") && r2.alt) return r2.alt;
      if (F(r2, "children")) return pt(r2.children, b2, e2);
    }
    return !V(r2) && Array.isArray(r2) ? pt(r2, b2, e2) : "";
  };
  var Jt = Me;
  var Wt = /^(\r?\n|\r)|(\r?\n|\r)$/g;
  var $t = /(\r?\n|\r)$/g;
  var Fr = function(r2, b2) {
    if (r2) {
      var e2 = "Cannot close `" + J(r2) + "` (", t2 = r2.start;
      e2 = e2 + Fe({ start: t2, end: r2.end }) + "): a different token (`" + J(b2) + "`, ", t2 = b2.start;
      throw new ve(e2 + Fe({ start: t2, end: b2.end }) + ") is open");
    }
    r2 = "Cannot close document, a token (`" + J(b2) + "`, ";
    e2 = b2.start;
    throw new ve(r2 + Fe({ start: e2, end: b2.end }) + ") is still open");
  };
  var Kt = function(r2) {
    return { type: "blockquote", children: [], position: void 0 };
  };
  var zr = function(r2) {
    let b2 = null;
    b2 = { type: "code", lang: b2, meta: b2, value: "", position: void 0 };
    return b2;
  };
  var Xt = function(r2) {
    return { type: "inlineCode", value: "", position: void 0 };
  };
  var en = function(r2) {
    let b2 = null;
    b2 = { type: "definition", identifier: "", label: b2, title: b2, url: "", position: void 0 };
    return b2;
  };
  var rn = function(r2) {
    return { type: "emphasis", children: [], position: void 0 };
  };
  var Or = function(r2) {
    return { type: "heading", children: [], position: void 0, depth: 0 };
  };
  var _r = function(r2) {
    return { type: "break", position: void 0 };
  };
  var Nr = function(r2) {
    return { type: "html", value: "", position: void 0 };
  };
  var tn = function(r2) {
    let b2 = null;
    b2 = { type: "image", title: b2, url: "", alt: b2, position: void 0 };
    return b2;
  };
  var Br = function(r2) {
    return { type: "link", children: [], position: void 0, title: null, url: "" };
  };
  var Vr = function(r2) {
    let b2 = [], e2 = void 0;
    return { type: "list", children: b2, position: e2, ordered: "listOrdered" == J(r2), start: null, spread: r2._spread };
  };
  var nn = function(r2) {
    return { type: "listItem", children: [], position: void 0, spread: r2._spread, checked: null };
  };
  var an = function(r2) {
    return { type: "paragraph", children: [], position: void 0 };
  };
  var on = function(r2) {
    return { type: "strong", children: [], position: void 0 };
  };
  var un = function(r2) {
    return { type: "thematicBreak", position: void 0 };
  };
  var ra = (0, function(r2) {
    var b2 = this;
    b2.parser = function(e2) {
      var t2 = L({}, b2.data("settings"));
      L(t2, r2), t2.extensions = gt(b2.data("micromarkExtensions")), t2.mdastExtensions = gt(b2.data("fromMarkdownExtensions"));
      var n2;
      t2 && "object" == typeof t2 && (n2 = t2, t2 = void 0);
      var f2 = ((r3) => {
        var t3, n3, c3, f3, i2, w2, e3 = {}, b3 = {}, o2 = { transforms: [], canContainEols: ["emphasis", "fragment", "heading", "paragraph", "strong"], enter: e3, exit: b3 };
        c3 = (0, function() {
          let r4 = this.stack;
          r4.push({ type: "fragment", children: [] });
        }), f3 = (0, function() {
          let r4 = this.stack;
          return ar(r4.pop(), void 0);
        }), t3 = (0, function(r4, b4, e4) {
          var t4 = O(this).children;
          t4.push(r4), t4 = this.stack, t4.push(r4);
          var n4 = [b4];
          e4 ? n4.push(e4) : n4.push(void 0), e4 = this.tokenStack, e4.push(n4), r4.position = { start: ae(b4.start), end: void 0 };
        }), n3 = (0, function(r4, b4) {
          var t4 = this.stack, n4 = t4.pop(), c4 = this.tokenStack, e4 = c4.pop();
          V(e4) ? ((r5) => {
            let b5 = "Cannot close `" + J(r5) + "` (", e5 = r5.start;
            throw new ve(b5 + Fe({ start: e5, end: r5.end }) + "): it\u2019s not open");
          })(r4) : J(e4[0]) != J(r4) && (b4 ? b4.call(this, r4, e4[0]) : (b4 = e4[1] || Fr, b4.call(this, r4, e4[0]))), n4.position.end = ae(r4.end);
        });
        var u2 = (0, function(r4) {
          this.data.expectingFirstListItemValue = true;
        }), y2 = (0, function(r4) {
          let b4 = E(f3.call(this));
          r4 = O(this), r4.value = b4.replace(Wt, ""), this.data.flowCodeInside = void 0;
        }), T2 = (0, function(r4) {
          let b4 = E(f3.call(this)), e4 = O(this);
          e4.value = b4.replace($t, "");
        }), d2 = (0, function(r4) {
          this.data.setextHeadingSlurpLineEnding = void 0;
        });
        i2 = (0, function(r4) {
          var e4 = O(this).children, b4;
          q(e4) > 0 && (b4 = e4[q(e4) - 1]), (V(b4) || "text" != E(b4.type)) && (b4 = { type: "text", value: "", position: void 0 }, b4.position = { start: ae(r4.start), end: void 0 }, e4.push(b4)), r4 = this.stack, r4.push(b4);
        }), w2 = (0, function(r4) {
          let t4 = this.stack, b4 = t4.pop(), e4 = E(b4.value);
          b4.value = e4 + E(this.sliceSerialize(r4)), e4 = b4.position, e4.end = ae(r4.end);
        });
        var a2 = (0, function(r4) {
          this.data.atHardBreak = true;
        }), R2 = (0, function(r4) {
          let b4 = f3.call(this);
          O(this).value = b4;
        }), k2 = (0, function(r4) {
          let b4 = f3.call(this);
          O(this).value = b4;
        }), D2 = (0, function(r4) {
          let b4 = f3.call(this);
          O(this).value = b4;
        }), H2 = (0, function(r4) {
          ht(this);
        }), Q2 = (0, function(r4) {
          ht(this);
        }), x2 = (0, function(r4) {
          let b4 = this.data;
          b4.characterReferenceType = r4.type;
        });
        e3.autolink = N(t3, Br, void 0), e3.autolinkProtocol = i2, e3.autolinkEmail = i2, e3.atxHeading = N(t3, Or, void 0), e3.blockQuote = N(t3, Kt, void 0), e3.characterEscape = i2, e3.characterReference = i2, e3.codeFenced = N(t3, zr, void 0), e3.codeFencedFenceInfo = c3, e3.codeFencedFenceMeta = c3, e3.codeIndented = N(t3, zr, c3), e3.codeText = N(t3, Xt, c3), e3.codeTextData = i2, e3.data = i2, e3.codeFlowValue = i2, e3.definition = N(t3, en, void 0), e3.definitionDestinationString = c3, e3.definitionLabelString = c3, e3.definitionTitleString = c3, e3.emphasis = N(t3, rn, void 0), e3.hardBreakEscape = N(t3, _r, void 0), e3.hardBreakTrailing = N(t3, _r, void 0), e3.htmlFlow = N(t3, Nr, c3), e3.htmlFlowData = i2, e3.htmlText = N(t3, Nr, c3), e3.htmlTextData = i2, e3.image = N(t3, tn, void 0), e3.label = c3, e3.link = N(t3, Br, void 0), e3.listItem = N(t3, nn, void 0), e3.listItemValue = function(r4) {
          if (this.data.expectingFirstListItemValue) {
            var b4 = this.stack, e4 = b4[q(b4) - 2];
            r4 = E(this.sliceSerialize(r4)), e4.start = +Number.parseInt(r4, 10), this.data.expectingFirstListItemValue = void 0;
          }
        }, e3.listOrdered = N(t3, Vr, u2), e3.listUnordered = N(t3, Vr, void 0), e3.paragraph = N(t3, an, void 0), e3.reference = function(r4) {
          this.data.referenceType = "collapsed";
        }, e3.referenceString = c3, e3.resourceDestinationString = c3, e3.resourceTitleString = c3, e3.setextHeading = N(t3, Or, void 0), e3.strong = N(t3, on, void 0), e3.thematicBreak = N(t3, un, void 0), b3.atxHeading = B(n3, void 0), b3.atxHeadingSequence = function(r4) {
          var b4 = O(this);
          b4.depth || (b4.depth = E(this.sliceSerialize(r4)).length);
        }, b3.autolink = B(n3, void 0), b3.autolinkEmail = function(r4) {
          w2.call(this, r4);
          let b4 = O(this);
          b4.url = "mailto:" + E(this.sliceSerialize(r4));
        }, b3.autolinkProtocol = function(r4) {
          w2.call(this, r4);
          let b4 = O(this);
          b4.url = this.sliceSerialize(r4);
        }, b3.blockQuote = B(n3, void 0), b3.characterEscapeValue = w2, b3.characterReferenceMarkerHexadecimal = x2, b3.characterReferenceMarkerNumeric = x2, b3.characterReferenceValue = function(r4) {
          r4 = E(this.sliceSerialize(r4));
          var b4 = this.data.characterReferenceType;
          b4 ? (b4 = "characterReferenceMarkerNumeric" == E(b4) ? 10 : 16, r4 = ir(r4, b4), this.data.characterReferenceType = void 0) : (r4 = nr(r4), false === r4 && at("expected reference to decode"), r4 = E(r4));
          var e4 = O(this), t4 = E(e4.value) + r4;
          e4.value = t4;
        }, b3.characterReference = function(r4) {
          let b4 = this.stack, e4 = b4.pop().position;
          e4.end = ae(r4.end);
        }, b3.codeFenced = B(n3, y2), b3.codeFencedFence = function(r4) {
          if (!this.data.flowCodeInside) c3.call(this), this.data.flowCodeInside = true;
        }, b3.codeFencedFenceInfo = function(r4) {
          let b4 = f3.call(this);
          O(this).lang = b4;
        }, b3.codeFencedFenceMeta = function(r4) {
          let b4 = f3.call(this);
          O(this).meta = b4;
        }, b3.codeFlowValue = w2, b3.codeIndented = B(n3, T2), b3.codeText = B(n3, D2), b3.codeTextData = w2, b3.data = w2, b3.definition = B(n3, void 0), b3.definitionDestinationString = function(r4) {
          let b4 = f3.call(this);
          O(this).url = b4;
        }, b3.definitionLabelString = function(r4) {
          let e4 = f3.call(this), b4 = O(this);
          b4.label = e4, b4.identifier = de(E(this.sliceSerialize(r4))).toLowerCase();
        }, b3.definitionTitleString = function(r4) {
          let b4 = f3.call(this);
          O(this).title = b4;
        }, b3.emphasis = B(n3, void 0), b3.hardBreakEscape = B(n3, a2), b3.hardBreakTrailing = B(n3, a2), b3.htmlFlow = B(n3, R2), b3.htmlFlowData = w2, b3.htmlText = B(n3, k2), b3.htmlTextData = w2, b3.image = B(n3, Q2), b3.label = function(r4) {
          r4 = O(this);
          var e4 = f3.call(this), b4 = O(this);
          this.data.inReference = true, "link" == E(b4.type) ? b4.children = r4.children : b4.alt = e4;
        }, b3.labelText = function(r4) {
          r4 = E(this.sliceSerialize(r4));
          let b4 = this.stack, e4 = b4[q(b4) - 2];
          e4.label = E(r4.replace(Rr, function(r5, b5, e5) {
            var c4 = E(r5), t4 = b5;
            if (t4) return t4;
            t4 = E(e5);
            if (35 == (t4.charCodeAt(0) | 0)) {
              var n4 = t4.charCodeAt(1) | 0;
              return 120 == n4 || 88 == n4 ? ir(t4.slice(2), 16) : ir(t4.slice(1), 10);
            }
            t4 = nr(t4);
            return t4 || c4;
          })), e4.identifier = de(r4).toLowerCase();
        }, b3.lineEnding = function(r4) {
          var e4 = O(this);
          if (this.data.atHardBreak) {
            var t4 = e4.children, n4 = t4[q(t4) - 1].position;
            n4.end = ae(r4.end), this.data.atHardBreak = void 0;
            return;
          }
          if (!this.data.setextHeadingSlurpLineEnding) {
            var b4 = o2.canContainEols;
            b4 = M(b4, E(e4.type));
          } else {
            b4 = false;
          }
          b4 && (i2.call(this, r4), w2.call(this, r4));
        }, b3.link = B(n3, H2), b3.listItem = B(n3, void 0), b3.listOrdered = B(n3, void 0), b3.listUnordered = B(n3, void 0), b3.paragraph = B(n3, void 0), b3.referenceString = function(r4) {
          let e4 = f3.call(this), b4 = O(this);
          b4.label = e4, b4.identifier = de(E(this.sliceSerialize(r4))).toLowerCase(), this.data.referenceType = "full";
        }, b3.resourceDestinationString = function(r4) {
          let b4 = f3.call(this);
          O(this).url = b4;
        }, b3.resourceTitleString = function(r4) {
          let b4 = f3.call(this);
          O(this).title = b4;
        }, b3.resource = function(r4) {
          this.data.inReference = void 0;
        }, b3.setextHeading = B(n3, d2), b3.setextHeadingLineSequence = function(r4) {
          r4 = E(this.sliceSerialize(r4)), r4 = r4.length > 0 ? 61 == (+r4.codePointAt(0) | 0) ? 1 : 2 : 2, O(this).depth = r4;
        }, b3.setextHeadingText = function(r4) {
          this.data.setextHeadingSlurpLineEnding = true;
        }, b3.strong = B(n3, void 0), b3.thematicBreak = B(n3, void 0), V(r3) && (r3 = {}), b3 = r3.mdastExtensions, V(b3) && (b3 = []), dt(o2, b3), r3 = {};
        return function(b4) {
          var w3 = { type: "root", children: [], position: void 0 };
          for (var x3, A2, u3, l2, a3 = [], S2 = { stack: [w3], tokenStack: a3, config: o2, enter: t3, exit: n3, buffer: c3, resume: f3, data: r3 }, i3 = [], e4 = -1; ++e4 < q(b4); ) or(J(b4[e4][1])) && ("enter" == E(b4[e4][0]) ? i3.push(e4) : e4 = ((r4, b5, e5) => {
            for (var i4, n4, w4, t4, u4, c4, f4 = b5 - 1 | 0, a4 = -1, l3 = false, o3 = void 0, x4 = 0, q2 = false; ++f4 <= e5; ) {
              i4 = r4[f4], n4 = J(i4[1]), t4 = E(i4[0]), or(n4) || "blockQuote" == n4 ? (a4 = "enter" == t4 ? a4 + 1 | 0 : a4 - 1 | 0, q2 = false) : "lineEndingBlank" == n4 ? "enter" == t4 && (o3 && !q2 && 0 == a4 && 0 == x4 && (x4 = f4), q2 = false) : ("linePrefix" == n4 || "listItemValue" == n4 || "listItemMarker" == n4 ? true : "listItemPrefix" == n4 || "listItemPrefixWhitespace" == n4) || (q2 = false);
              if (0 == a4 && "enter" == t4 && "listItemPrefix" == n4 || a4 == -1 && "exit" == t4 && or(n4)) {
                if (o3) {
                  for (t4 = 0, w4 = f4; --w4 >= 0; ) {
                    u4 = r4[w4], c4 = J(u4[1]);
                    if ("lineEnding" == c4 || "lineEndingBlank" == c4) {
                      if ("exit" == E(u4[0])) continue;
                      t4 && (r4[t4][1].type = "lineEndingBlank", l3 = true), u4[1].type = "lineEnding", t4 = w4;
                    } else if (!("linePrefix" == c4 || "blockQuotePrefix" == c4 || "blockQuotePrefixWhitespace" == c4 ? true : "blockQuoteMarker" == c4 || "listItemIndent" == c4)) break;
                  }
                  0 != x4 && (0 != t4 ? x4 < t4 : true) && (o3._spread = true);
                  w4 = i4[1].end, 0 != t4 ? w4 = r4[t4][1].start : t4 = f4, o3.end = L({}, w4), Y(r4, t4, 0, [["exit", o3, i4[2]]]), e5++, f4++;
                }
                "listItemPrefix" == n4 && (o3 = { type: "listItem", _spread: false, start: L({}, i4[1].start), end: void 0 }, Y(r4, f4, 0, [["enter", o3, i4[2]]]), e5++, f4++, x4 = 0, q2 = true);
              }
            }
            r4[b5][1]._spread = l3;
            return e5;
          })(b4, i3.pop(), e4));
          for (e4 = -1; ++e4 < q(b4); ) i3 = b4[e4], l2 = o2[E(i3[0])], x3 = J(i3[1]), F(l2, x3) && (A2 = L({ sliceSerialize: i3[2].sliceSerialize }, S2), l2[x3].call(A2, i3[1]));
          q(a3) > 0 && (i3 = a3[q(a3) - 1], e4 = i3[1] || Fr, e4.call(S2, void 0, i3[0]));
          for (e4 = { line: 1, column: 1, offset: 0 }, i3 = { line: 1, column: 1, offset: 0 }, q(b4) > 0 && (e4 = b4[0][1].start, i3 = b4[q(b4) - 2][1].end), e4 = ae(e4), w3.position = { start: e4, end: ae(i3) }, i3 = o2.transforms, u3 = -1; ++u3 < q(i3); ) e4 = i3[u3](w3), e4 && (w3 = e4);
          return w3;
        };
      })(n2), c2 = ((r3) => {
        var b3 = it(r3);
        r3 = [];
        var e3 = X;
        r3.push(e3);
        if (e3 = b3.extensions) {
          var t3 = q(e3);
          for (b3 = 0; b3 < t3; b3++) {
            var n3 = e3[b3];
            r3.push(n3);
          }
        }
        for (e3 = {}, t3 = q(r3), b3 = -1; ++b3 < t3; ) ((r4, b4) => {
          for (var e4 in b4) if (!!F(b4, e4)) {
            var t4;
            F(r4, e4) && (t4 = r4[e4]), V(t4) && (t4 = {}, r4[e4] = t4);
            var c3 = b4[e4];
            if (c3) {
              for (e4 in c3) if (!!F(c3, e4)) {
                F(t4, e4) || (t4[e4] = []);
                var n4 = c3[e4], o2 = [];
                if (Array.isArray(n4)) var E2, i2, f3, w2 = n4;
                else {
                  !n4 || o2.push(n4), w2 = o2;
                }
                for (E2 = t4[e4], i2 = [], n4 = q(w2), e4 = -1; ++e4 < n4; ) f3 = w2[e4], "after" == f3.add ? E2.push(f3) : i2.push(f3);
                Y(E2, 0, 0, i2);
              }
            }
          }
        })(e3, r3[b3]);
        r3 = { constructs: e3, defined: [], lazy: {} }, b3 = _t, r3.content = function(e4) {
          return Hn(r3, b3, e4);
        }, e3 = Nt, r3.document = function(b4) {
          return Hn(r3, e3, b4);
        }, t3 = Vt, r3.flow = function(b4) {
          return Hn(r3, t3, b4);
        }, n3 = Mt, r3.string = function(b4) {
          return Hn(r3, n3, b4);
        }, r3.text = function(b4) {
          return Hn(r3, jt, b4);
        };
        return r3;
      })(n2);
      n2 = c2.document(), t2 = (/* @__PURE__ */ (() => {
        var r3 = 1, e3 = "", t3 = true, b3 = false;
        return function(n3, c3, f3) {
          var w2 = [], o2 = e3;
          o2 += ((r4, b4) => {
            if ("string" == typeof r4) return r4;
            V(b4) && (b4 = void 0), b4 = new TextDecoder(b4);
            return E(b4.decode(r4));
          })(n3, c3), e3 = "", t3 ? (n3 = o2.length > 0 && 65279 == (o2.charCodeAt(0) | 0) ? 1 : 0, t3 = false) : n3 = 0;
          var x2 = o2.length;
          while (n3 < x2) {
            for (c3 = n3; ; c3++) {
              if (c3 >= x2) {
                var a2, q2, i2 = -1;
                break;
              }
              i2 = o2.charCodeAt(c3) | 0;
              if (0 == i2 || 9 == i2 || 10 == i2 || 13 == i2) break;
            }
            if (i2 < 0) {
              e3 = o2.slice(n3);
              break;
            }
            if (10 == i2 && n3 == c3 && b3) w2.push(-3), b3 = false;
            else {
              b3 && (w2.push(-5), b3 = false), n3 < c3 && (a2 = o2.slice(n3, c3), w2.push(a2), r3 = r3 + (c3 - n3 | 0));
              if (0 == i2) w2.push(65533), r3++;
              else if (9 == i2) {
                for (i2 = qe.ceil, n3 = qe, a2 = r3, i2 = +i2.call(n3, a2 / 4) * 4, w2.push(-2); r3 < i2; r3++) w2.push(-1);
                r3++;
              } else 10 == i2 ? (w2.push(-4), r3 = 1) : (b3 = true, r3 = 1);
            }
            n3 = c3 + 1 | 0;
          }
          f3 && (b3 && w2.push(-5), e3.length > 0 && (q2 = e3, w2.push(q2)), w2.push(null));
          return w2;
        };
      })())(e2, t2, true), t2 = n2.write(t2), e2 = void 0;
      while (!ct(t2)) {
      }
      c2 = f2(t2);
      return c2;
    };
  });
  f = function(r2, b2, e2, t2) {
    let c2 = r2, n2 = t2, f2 = e2.enter("blockquote");
    n2 = e2.createTracker(n2), n2.move("> "), n2.shift(2);
    let i2 = e2.indentLines, w2 = e2.containerFlow;
    c2 = E(e2.indentLines(e2.containerFlow(c2, n2.current()), function(r3, b3, e3) {
      return !!e3 ? ">" + E(r3) : "> " + E(r3);
    })), f2();
    return c2;
  };
  var ln = /[ \t]/;
  e = function(r2, b2, e2, t2) {
    for (var n2 = e2.unsafe, f2 = q(n2), c2 = -1; ++c2 < f2; ) if ("\n" == E(n2[c2].character) && kt(e2.stack, n2[c2])) return ln.test(E(t2.before)) ? "" : " ";
    return "\\\n";
  };
  var cn = /[^ \r\n]/;
  var sn = /^[\t ]*(?:[\r\n]|$)|(?:^|[\r\n])[\t ]*$/;
  v = function(r2, b2, t2, n2) {
    var i2, u2, l2, A2, a2, o2 = r2, c2 = t2, w2 = n2, f2 = ((r3) => {
      var b3 = G(r3).fence;
      r3 = b3 ? E(b3) : "`", "`" != r3 && "~" != r3 && W("Cannot serialize code with `" + r3 + "` for `options.fence`, expected `` ` `` or `~`");
      return r3;
    })(c2), x2 = o2.value ? E(o2.value) : "", q2 = "`" == f2 ? "GraveAccent" : "Tilde";
    if (ur(o2, c2)) return i2 = c2.enter("codeIndented"), o2 = E(c2.indentLines(x2, function(r3, b3, e2) {
      return !!e2 ? E(r3) : "    " + E(r3);
    })), i2(), o2;
    i2 = c2.createTracker(w2), w2 = ((r3, e2) => {
      if ("string" != typeof e2) throw new TypeError("Expected substring");
      var c3 = E(r3), t3 = E(e2);
      r3 = c3.indexOf(t3);
      var f3 = t3.length, n3 = r3, b3 = 0, e2 = 0;
      while (r3 != -1) r3 == n3 ? (b3 = b3 + 1 | 0, b3 > e2 && (e2 = b3)) : b3 = 1, n3 = r3 + f3 | 0, r3 = c3.indexOf(t3, n3);
      return e2;
    })(x2, f2) + 1 | 0, u2 = f2.repeat(+Math.max(w2, 3) | 0), l2 = c2.enter("codeFenced"), f2 = E(i2.move(u2)), o2.lang && (A2 = c2.enter("codeFencedLang" + q2), w2 = L({}, i2.current()), w2.before = f2, w2.after = " ", a2 = ["`"], w2.encode = a2, a2 = i2.move, f2 = f2 + E(i2.move(c2.safe(o2.lang, w2))), A2()), o2.lang && o2.meta && (q2 = c2.enter("codeFencedMeta" + q2), w2 = f2 + E(i2.move(" ")), f2 = L({}, i2.current()), f2.before = w2, f2.after = "\n", a2 = ["`"], f2.encode = a2, a2 = i2.move, f2 = w2 + E(i2.move(c2.safe(o2.meta, f2))), q2()), c2 = f2 + E(i2.move("\n")), x2.length > 0 && (c2 = c2 + E(i2.move(x2 + "\n"))), c2 += E(i2.move(u2)), l2();
    return c2;
  };
  var fn = /[\x00- \x7F]/;
  p = function(r2, b2, e2, t2) {
    var i2, c2, q2, u2, f2, w2, n2 = t2, a2 = lr(e2), l2 = '"' == a2 ? "Quote" : "Apostrophe", x2 = e2.enter("definition"), o2 = e2.enter("label");
    n2 = e2.createTracker(n2), i2 = E(n2.move("[")), c2 = L({}, n2.current()), c2.before = i2, c2.after = "]", q2 = n2.move, u2 = e2.safe, c2 = i2 + E(n2.move(e2.safe(e2.associationId(r2), c2))), i2 = c2 + E(n2.move("]: ")), o2(), f2 = r2.url, w2 = !f2, !w2 && "string" == typeof f2 && fn.test(f2) && (w2 = true), w2 ? (w2 = e2.enter("destinationLiteral"), i2 = i2 + E(n2.move("<")), c2 = L({}, n2.current()), c2.before = i2, c2.after = ">", o2 = n2.move, f2 = i2 + E(n2.move(e2.safe(f2, c2))) + E(n2.move(">"))) : (w2 = e2.enter("destinationRaw"), c2 = L({}, n2.current()), c2.before = i2, c2.after = r2.title ? " " : "\n", o2 = n2.move, f2 = i2 + E(n2.move(e2.safe(f2, c2)))), w2(), r2.title && (i2 = e2.enter("title" + l2), c2 = f2 + E(n2.move(" " + a2)), f2 = L({}, n2.current()), f2.before = c2, f2.after = a2, w2 = n2.move, f2 = c2 + E(n2.move(e2.safe(r2.title, f2))), f2 += E(n2.move(a2)), i2()), x2();
    return f2;
  }, r = function(r2, b2, e2, t2) {
    var c2 = r2, f2 = ((r3) => {
      var b3 = G(r3).emphasis;
      r3 = b3 ? E(b3) : "*", "*" != r3 && "_" != r3 && W("Cannot serialize emphasis with `" + r3 + "` for `options.emphasis`, expected `*`, or `_`");
      return r3;
    })(e2), q2 = e2.enter("emphasis"), i2 = e2.createTracker(t2), x2 = E(i2.move(f2)), n2 = L({}, i2.current());
    n2.after = f2, n2.before = x2;
    var w2 = i2.move;
    n2 = E(i2.move(e2.containerPhrasing(c2, n2)));
    var o2 = n2.charCodeAt(0);
    c2 = E(t2.before), w2 = c2.length - 1, w2 = ze(c2.charCodeAt(w2), o2, f2), w2.inside && (n2 = oe(o2) + n2.slice(1));
    var a2 = n2.length - 1;
    o2 = n2.charCodeAt(a2), a2 = E(t2.after), c2 = ze(a2.charCodeAt(0), o2, f2), c2.inside && (n2 = n2.slice(0, n2.length - 1) + oe(o2)), a2 = E(i2.move(f2)), q2(), f2 = c2.outside, e2.attentionEncodeSurroundingInfo = { after: f2, before: w2.outside };
    return x2 + n2 + a2;
  }, r.peek = function(r2, b2, e2) {
    return e2.options.emphasis || "*";
  }, t = true;
  var vn = /\r?\n|\r/;
  var pn = /^[\t ]/;
  d = function(r2, b2, e2, t2) {
    var w2 = r2, c2 = e2, o2 = t2, f2 = w2.depth, n2 = f2 ? +f2 : 1;
    n2 = +Math.min(6, n2);
    var i2 = +Math.max(n2, 1) | 0;
    f2 = c2.createTracker(o2);
    if (St(w2, c2)) {
      o2 = c2.enter("headingSetext");
      var a2 = c2.enter("phrasing");
      n2 = L({}, f2.current()), n2.before = "\n", n2.after = "\n", n2 = E(c2.containerPhrasing(w2, n2)), a2(), o2(), w2 = 1 == i2 ? "=" : "-", c2 = n2.lastIndexOf("\r"), f2 = n2.lastIndexOf("\n");
      return n2 + "\n" + w2.repeat(n2.length - ((+Math.max(c2, f2) | 0) + 1 | 0) | 0);
    }
    i2 = "#".repeat(i2);
    o2 = c2.enter("headingAtx"), a2 = c2.enter("phrasing"), f2.move(i2 + " "), n2 = L({}, f2.current()), n2.before = "# ", n2.after = "\n", n2 = E(c2.containerPhrasing(w2, n2)), pn.test(n2) && (n2 = oe(n2.charCodeAt(0) | 0) + n2.slice(1)), n2 = n2.length > 0 ? i2 + " " + n2 : i2, c2.options.closeAtx && (n2 = n2 + " " + i2), a2(), o2();
    return n2;
  }, n = function(r2) {
    return r2.value ? E(r2.value) : "";
  }, n.peek = function() {
    return "<";
  };
  var dn = /[\x00- \x7F]/;
  i = function(r2, b2, e2, t2) {
    var o2, n2, w2, f2, i2 = e2, c2 = t2, x2 = lr(i2), u2 = '"' == x2 ? "Quote" : "Apostrophe", q2 = i2.enter("image"), a2 = i2.enter("label");
    c2 = i2.createTracker(c2), o2 = E(c2.move("![")), n2 = L({}, c2.current()), n2.before = o2, n2.after = "]", w2 = c2.move, n2 = o2 + E(c2.move(i2.safe(r2.alt, n2))), w2 = n2 + E(c2.move("](")), a2(), f2 = r2.url, n2 = !f2 && r2.title, !n2 && "string" == typeof f2 && dn.test(f2) && (n2 = true), n2 ? (o2 = i2.enter("destinationLiteral"), w2 = w2 + E(c2.move("<")), n2 = L({}, c2.current()), n2.before = w2, n2.after = ">", a2 = c2.move, f2 = w2 + E(c2.move(i2.safe(f2, n2))) + E(c2.move(">"))) : (o2 = i2.enter("destinationRaw"), n2 = L({}, c2.current()), n2.before = w2, n2.after = r2.title ? " " : ")", a2 = c2.move, f2 = w2 + E(c2.move(i2.safe(f2, n2)))), o2(), r2.title && (o2 = i2.enter("title" + u2), n2 = f2 + E(c2.move(" " + x2)), f2 = L({}, c2.current()), f2.before = n2, f2.after = x2, w2 = c2.move, f2 = n2 + E(c2.move(i2.safe(r2.title, f2))), f2 += E(c2.move(x2)), o2()), i2 = f2 + E(c2.move(")")), q2();
    return i2;
  }, i.peek = function() {
    return "!";
  }, a = function(r2, b2, e2, t2) {
    var i2 = r2, f2 = t2, a2 = E(i2.referenceType), x2 = e2.enter("imageReference"), w2 = e2.enter("label");
    f2 = e2.createTracker(f2);
    var n2 = E(f2.move("![")), c2 = L({}, f2.current());
    c2.before = n2, c2.after = "]";
    var o2 = E(e2.safe(i2.alt, c2));
    c2 = n2 + E(f2.move(o2 + "][")), w2(), w2 = e2.stack, e2.stack = [];
    var q2 = e2.enter("reference");
    n2 = L({}, f2.current()), n2.before = c2, n2.after = "]";
    var u2 = e2.safe;
    n2 = E(e2.safe(e2.associationId(i2), n2)), q2(), e2.stack = w2, x2(), i2 = "full" == a2 || 0 == o2.length || o2 != n2 ? c2 + E(f2.move(n2 + "]")) : "shortcut" == a2 ? c2.slice(0, c2.length - 1) : c2 + E(f2.move("]"));
    return i2;
  }, a.peek = function() {
    return "!";
  };
  var hn = /[^ \r\n]/;
  var gn = /^[ \r\n]/;
  var mn = /[ \r\n]$/;
  var bn = /^`|`$/;
  o = function(r2, b2, e2) {
    var n2, o2, a2, c2, i2, f2 = r2, t2 = f2.value ? E(f2.value) : "", w2 = "`";
    while (true) {
      n2 = new RegExp("(^|[^`])" + w2 + "([^`]|$)", "");
      if (!n2.test(t2)) break;
      w2 += "`";
    }
    hn.test(t2) && (gn.test(t2) && mn.test(t2) || bn.test(t2)) && (t2 = " " + t2 + " ");
    for (o2 = e2.unsafe, a2 = q(o2), f2 = -1; ++f2 < a2; ) {
      c2 = o2[f2];
      if (!!c2.atBreak) {
        c2 = e2.compilePattern(c2);
        while (true) {
          i2 = c2.exec(t2);
          if (i2 == null) break;
          n2 = i2.index | 0, n2 > 0 && 10 == (t2.charCodeAt(n2) | 0) && 13 == (t2.charCodeAt(n2 - 1) | 0) && (n2 = n2 - 1 | 0), t2 = t2.slice(0, n2) + " " + t2.slice((i2.index | 0) + 1 | 0);
        }
      }
    }
    return w2 + t2 + w2;
  }, o.peek = function() {
    return "`";
  };
  var yn = /^[a-z][a-z+.-]+:/i;
  var kn = /[\x00- <>\x7F]/;
  var xn = /[\x00- \x7F]/;
  u = function(r2, b2, e2, t2) {
    var c2, i2, f2, w2, x2, a2, n2 = t2, o2 = lr(e2), q2 = '"' == o2 ? "Quote" : "Apostrophe";
    n2 = e2.createTracker(n2);
    if (Et(r2, e2)) return c2 = e2.stack, e2.stack = [], i2 = e2.enter("autolink"), o2 = E(n2.move("<")), f2 = L({}, n2.current()), f2.before = o2, f2.after = ">", f2 = o2 + E(n2.move(e2.containerPhrasing(r2, f2))) + E(n2.move(">")), i2(), e2.stack = c2, f2;
    x2 = e2.enter("link"), a2 = e2.enter("label"), i2 = E(n2.move("[")), c2 = L({}, n2.current()), c2.before = i2, c2.after = "](", w2 = n2.move, c2 = i2 + E(n2.move(e2.containerPhrasing(r2, c2))), w2 = c2 + E(n2.move("](")), a2(), f2 = r2.url, c2 = !f2 && r2.title, !c2 && "string" == typeof f2 && xn.test(f2) && (c2 = true), c2 ? (i2 = e2.enter("destinationLiteral"), w2 = w2 + E(n2.move("<")), c2 = L({}, n2.current()), c2.before = w2, c2.after = ">", a2 = n2.move, f2 = w2 + E(n2.move(e2.safe(f2, c2))) + E(n2.move(">"))) : (i2 = e2.enter("destinationRaw"), c2 = L({}, n2.current()), c2.before = w2, c2.after = r2.title ? " " : ")", a2 = n2.move, f2 = w2 + E(n2.move(e2.safe(f2, c2)))), i2(), r2.title && (i2 = e2.enter("title" + q2), c2 = f2 + E(n2.move(" " + o2)), f2 = L({}, n2.current()), f2.before = c2, f2.after = o2, w2 = n2.move, f2 = c2 + E(n2.move(e2.safe(r2.title, f2))), f2 += E(n2.move(o2)), i2()), f2 += E(n2.move(")")), x2();
    return f2;
  }, u.peek = function(r2, b2, e2) {
    return Et(r2, e2) ? "<" : "[";
  }, c = function(r2, b2, e2, t2) {
    var i2 = r2, f2 = t2, a2 = E(i2.referenceType), x2 = e2.enter("linkReference"), w2 = e2.enter("label");
    f2 = e2.createTracker(f2);
    var n2 = E(f2.move("[")), c2 = L({}, f2.current());
    c2.before = n2, c2.after = "]";
    var o2 = E(e2.containerPhrasing(i2, c2));
    c2 = n2 + E(f2.move(o2 + "][")), w2(), w2 = e2.stack, e2.stack = [];
    var q2 = e2.enter("reference");
    n2 = L({}, f2.current()), n2.before = c2, n2.after = "]";
    var u2 = e2.safe;
    n2 = E(e2.safe(e2.associationId(i2), n2)), q2(), e2.stack = w2, x2(), i2 = "full" == a2 || 0 == o2.length || o2 != n2 ? c2 + E(f2.move(n2 + "]")) : "shortcut" == a2 ? c2.slice(0, c2.length - 1) : c2 + E(f2.move("]"));
    return i2;
  }, c.peek = function() {
    return "[";
  }, h = function(r2, b2, e2, t2) {
    var n2 = b2, a2 = e2.enter("list"), x2 = e2.bulletCurrent, f2 = cr(e2);
    r2.ordered && (f2 = ((r3) => {
      var b3 = G(r3).bulletOrdered;
      r3 = b3 ? E(b3) : ".", "." != r3 && ")" != r3 && W(Di + r3 + "` for `options.bulletOrdered`, expected `.` or `)`");
      return r3;
    })(e2));
    var i2, c2, o2, u2 = r2.ordered ? "." == f2 ? ")" : "." : ((r3) => {
      var b3 = cr(r3), e3 = G(r3).bulletOther;
      if (!e3) return "*" == b3 ? "-" : "*";
      r3 = E(e3), "*" != r3 && "+" != r3 && "-" != r3 && W(Di + r3 + "` for `options.bulletOther`, expected `*`, `+`, or `-`"), r3 == b3 && W("Expected `bullet` (`" + b3 + "`) and `bulletOther` (`" + r3 + "`) to be different");
      return r3;
    })(e2), w2 = n2 && e2.bulletLastUsed && f2 == E(e2.bulletLastUsed);
    if (!r2.ordered) {
      c2 = void 0, n2 = r2.children, n2 && q(n2) > 0 && (c2 = n2[0]), ("*" == f2 || "-" == f2) && c2 && (i2 = c2.children, !i2 || 0 == q(i2) || !i2[0] ? (n2 = e2.stack, n2 = ((r3, b3) => {
        var e3 = q(r3), t3 = q(b3);
        if (e3 < 4) return false;
        if (t3 < 3) return false;
        var n3 = e3 - 2, c3 = e3 - 3, f3 = e3 - 4;
        if ("list" != E(r3[e3 - 1])) return false;
        if ("listItem" != E(r3[n3])) return false;
        if ("list" != E(r3[c3])) return false;
        if ("listItem" != E(r3[f3])) return false;
        r3 = t3 - 2, e3 = t3 - 3;
        return "0" != E(b3[t3 - 1]) ? false : "0" != E(b3[r3]) ? false : "0" != E(b3[e3]) ? false : true;
      })(n2, e2.indexStack)) : n2 = false, n2 && (w2 = true));
      if (qt(e2) == f2 && c2) for (o2 = q(r2.children), n2 = -1; ++n2 < o2; ) {
        i2 = r2.children[n2];
        if (i2 && "listItem" == E(i2.type)) {
          c2 = i2.children;
          if (c2 && q(c2) > 0 && c2[0] && "thematicBreak" == E(c2[0].type)) {
            w2 = true;
            break;
          }
        }
      }
    }
    w2 && (f2 = u2);
    e2.bulletCurrent = f2, w2 = E(e2.containerFlow(r2, t2)), e2.bulletLastUsed = f2, e2.bulletCurrent = x2, a2();
    return w2;
  }, g = function(r2, b2, e2, t2) {
    var f2 = b2, a2 = t2, o2 = ((r3) => {
      var b3 = G(r3).listItemIndent;
      r3 = b3 ? E(b3) : "one", "tab" != r3 && "one" != r3 && "mixed" != r3 && W(Di + r3 + "` for `options.listItemIndent`, expected `tab`, `one`, or `mixed`");
      return r3;
    })(e2), i2 = cr(e2), n2 = e2.bulletCurrent;
    n2 && (i2 = E(n2));
    var c2;
    f2 && "list" == E(f2.type) && f2.ordered && (c2 = f2.start, c2 = "number" == typeof c2 && +c2 > -1 ? c2 | 0 : 1, n2 = false !== e2.options.incrementListMarker ? +f2.children.indexOf(r2) : 0, i2 = (c2 + n2 | 0).toString(10) + i2);
    var w2 = i2.length + 1 | 0;
    n2 = "tab" == o2, !n2 && "mixed" == o2 && (f2 && "list" == E(f2.type) && f2.spread || r2.spread) && (n2 = true), n2 && (c2 = w2 / 4, w2 = (+Math.ceil(c2) | 0) * 4 | 0), c2 = e2.createTracker(a2), c2.move(i2 + " ".repeat(w2 - i2.length)), c2.shift(w2), f2 = e2.enter("listItem"), n2 = function(r3, b3, e3) {
      var t3 = E(r3);
      return 0 != +b3 ? e3 ? t3 : " ".repeat(w2) + t3 : e3 ? i2 + t3 : i2 + " ".repeat(w2 - i2.length) + t3;
    }, o2 = e2.indentLines, a2 = e2.containerFlow, n2 = E(e2.indentLines(e2.containerFlow(r2, c2.current()), n2)), f2();
    return n2;
  }, m = function(r2, b2, e2, t2) {
    let n2 = r2, c2 = e2.enter("paragraph"), f2 = e2.enter("phrasing");
    n2 = E(e2.containerPhrasing(n2, t2)), f2(), c2();
    return n2;
  }, b = function(r2, b2, e2, t2) {
    for (var c2 = r2.children, f2 = q(c2), n2 = 0; ; ) {
      if (n2 >= f2) {
        n2 = false;
        break;
      }
      if (((r3) => {
        if (V(r3) || "object" != typeof r3) return false;
        var b3 = E(r3.type);
        return "break" == b3 ? true : "delete" == b3 ? true : "emphasis" == b3 ? true : "footnote" == b3 ? true : "footnoteReference" == b3 ? true : "image" == b3 ? true : "imageReference" == b3 ? true : "inlineCode" == b3 ? true : "inlineMath" == b3 ? true : "link" == b3 ? true : "linkReference" == b3 ? true : "mdxJsxTextElement" == b3 ? true : "mdxTextExpression" == b3 ? true : "strong" == b3 ? true : "text" == b3 ? true : "textDirective" == b3 ? true : false;
      })(c2[n2])) {
        n2 = true;
        break;
      }
      n2++;
    }
    return n2 ? e2.containerPhrasing(r2, t2) : e2.containerFlow(r2, t2);
  }, s = function(r2, b2, e2, t2) {
    var f2 = r2, c2 = ((r3) => {
      var b3 = G(r3).strong;
      r3 = b3 ? E(b3) : "*", "*" != r3 && "_" != r3 && W("Cannot serialize strong with `" + r3 + "` for `options.strong`, expected `*`, or `_`");
      return r3;
    })(e2), q2 = e2.enter("strong"), i2 = e2.createTracker(t2), x2 = E(i2.move(c2 + c2)), n2 = L({}, i2.current());
    n2.after = c2, n2.before = x2;
    var w2 = i2.move;
    n2 = E(i2.move(e2.containerPhrasing(f2, n2)));
    var o2 = n2.charCodeAt(0);
    f2 = E(t2.before), w2 = f2.length - 1, w2 = ze(f2.charCodeAt(w2), o2, c2), w2.inside && (n2 = oe(o2) + n2.slice(1));
    var a2 = n2.length - 1;
    o2 = n2.charCodeAt(a2), a2 = E(t2.after), f2 = ze(a2.charCodeAt(0), o2, c2), f2.inside && (n2 = n2.slice(0, n2.length - 1) + oe(o2)), a2 = E(i2.move(c2 + c2)), q2(), c2 = f2.outside, e2.attentionEncodeSurroundingInfo = { after: c2, before: w2.outside };
    return x2 + n2 + a2;
  }, s.peek = function(r2, b2, e2) {
    return e2.options.strong || "*";
  }, y = function(r2, b2, e2, t2) {
    return e2.safe(r2.value, t2);
  }, k = function(r2, b2, e2) {
    var t2 = qt(e2);
    e2.options.ruleSpaces && (t2 = t2 + " ");
    var n2 = t2.repeat(((r3) => {
      var b3 = G(r3).ruleRepetition;
      r3 = b3 ? b3 | 0 : 3, r3 < 3 && W("Cannot serialize rules with repetition `" + r3.toString(10) + "` for `options.ruleRepetition`, expected `3` or more");
      return r3;
    })(e2));
    return e2.options.ruleSpaces ? n2.slice(0, n2.length - 1) : n2;
  };
  var _ = { blockquote: f };
  _.break = e, _.code = v, _.definition = p, _.emphasis = r, _.hardBreak = e, _.heading = d, _.html = n, _.image = i, _.imageReference = a, _.inlineCode = o, _.link = u, _.linkReference = c, _.list = h, _.listItem = g, _.paragraph = m, _.root = b, _.strong = s, _.text = y, _.thematicBreak = k;
  var Mr = [function(r2, b2, e2, t2) {
    var n2 = E(b2.type), c2 = E(r2.type);
    if ("code" == n2 && ur(b2, t2) && ("list" == c2 || c2 == n2 && ur(r2, t2))) return false;
    if ("spread" in e2 && "boolean" == typeof e2.spread) return "paragraph" == c2 && (c2 == n2 || "definition" == n2 || "heading" == n2 && St(b2, t2)) ? void 0 : e2.spread ? 1 : 0;
  }];
  e = ["autolink", "destinationLiteral", "destinationRaw", "reference", "titleQuote", "titleApostrophe"], n = ["codeFencedLangGraveAccent", "codeFencedLangTilde"], r = ["codeFencedLangGraveAccent", "codeFencedLangTilde", "codeFencedMetaGraveAccent", "codeFencedMetaTilde", "destinationLiteral", "headingAtx"], i = ["label", "reference"], a = [], a.push("codeFencedLangGraveAccent"), a.push("codeFencedMetaGraveAccent");
  var I = [{ character: "	", after: "[\\r\\n]", inConstruct: "phrasing" }, { character: "	", before: "[\\r\\n]", inConstruct: "phrasing" }, { character: "	", inConstruct: n }, { character: "\r", inConstruct: r }, { character: "\n", inConstruct: r }, { character: " ", after: "[\\r\\n]", inConstruct: "phrasing" }, { character: " ", before: "[\\r\\n]", inConstruct: "phrasing" }];
  d = I, d.push({ character: " ", inConstruct: n }), I.push({ character: "!", after: "\\[", inConstruct: "phrasing", notInConstruct: e }), I.push({ character: '"', inConstruct: "titleQuote" }), I.push({ atBreak: t, character: "#" }), I.push({ character: "#", inConstruct: "headingAtx", after: "(?:[\r\n]|$)" }), I.push({ character: "&", after: "[#A-Za-z]", inConstruct: "phrasing" }), I.push({ character: "'", inConstruct: "titleApostrophe" }), I.push({ character: "(", inConstruct: "destinationRaw" }), I.push({ before: "\\]", character: "(", inConstruct: "phrasing", notInConstruct: e }), I.push({ atBreak: t, before: "\\d+", character: ")" }), I.push({ character: ")", inConstruct: "destinationRaw" }), I.push({ atBreak: t, character: "*", after: "(?:[ 	\r\n*])" }), I.push({ character: "*", inConstruct: "phrasing", notInConstruct: e }), I.push({ atBreak: t, character: "+", after: "(?:[ 	\r\n])" }), I.push({ atBreak: t, character: "-", after: "(?:[ 	\r\n-])" }), I.push({ atBreak: t, before: "\\d+", character: ".", after: "(?:[ 	\r\n]|$)" }), I.push({ atBreak: t, character: "<", after: "[!/?A-Za-z]" }), I.push({ character: "<", after: "[!/?A-Za-z]", inConstruct: "phrasing", notInConstruct: e }), I.push({ character: "<", inConstruct: "destinationLiteral" }), I.push({ atBreak: t, character: "=" }), I.push({ atBreak: t, character: ">" }), I.push({ character: ">", inConstruct: "destinationLiteral" }), I.push({ atBreak: t, character: "[" }), I.push({ character: "[", inConstruct: "phrasing", notInConstruct: e }), I.push({ character: "[", inConstruct: i }), I.push({ character: "\\", after: "[\\r\\n]", inConstruct: "phrasing" }), I.push({ character: "]", inConstruct: i }), I.push({ atBreak: t, character: "_" }), I.push({ character: "_", inConstruct: "phrasing", notInConstruct: e }), I.push({ atBreak: t, character: "`" }), I.push({ character: "`", inConstruct: a }), I.push({ character: "`", inConstruct: "phrasing", notInConstruct: e }), I.push({ atBreak: t, character: "~" });
  var wn = new RegExp("[|\\\\{}()[\\]^$+*?.-]", "");
  var Sn = /(\r?\n|\r)$/;
  var En = /\r?\n|\r/g;
  var qn = new RegExp("[!-/:-@[-`{-~]", "");
  var An = function(r2, b2) {
    return +r2 - +b2;
  };
  var Tn = /\r?\n|\r/g;
  t = (0, function(r2) {
    var b2 = this;
    b2.compiler = function(e2, t2) {
      let c2 = {}, n2 = L(c2, It(b2, "settings"));
      L(n2, r2), n2.extensions = /* @__PURE__ */ ((r3) => !!r3 ? r3 : [])(It(b2, "toMarkdownExtensions"));
      return Tt(e2, n2);
    };
  }), l = l(), ra = l.use(ra);
  var ta = ra.use(t);
  var na = ta.freeze();
  return __toCommonJS(remark_esm_exports);
})();
globalThis.remark=remark.remark||remark;
