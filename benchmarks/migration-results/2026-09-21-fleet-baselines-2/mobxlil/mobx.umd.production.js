"use strict";
var mobx = (() => {
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

  // dist/mobx.esm.js
  var mobx_esm_exports = {};
  __export(mobx_esm_exports, {
    $mobx: () => u,
    FlowCancellationError: () => Y,
    ObservableMap: () => d,
    ObservableSet: () => _,
    Reaction: () => A,
    _allowStateChanges: () => ma,
    _allowStateChangesInsideComputed: () => An,
    _allowStateReadsEnd: () => Mt,
    _allowStateReadsStart: () => Kt,
    _autoAction: () => Ee,
    _autoActionBound: () => Ye,
    _endAction: () => Ja,
    _getAdministration: () => W,
    _getGlobalState: () => xn,
    _interceptReads: () => si,
    _isComputingDerivation: () => Lt,
    _resetGlobalState: () => Cn,
    _startAction: () => Qa,
    action: () => de,
    actionBound: () => qn,
    autorun: () => ra,
    compareDefault: () => Pt,
    compareIdentity: () => Et,
    compareShallow: () => Tt,
    compareStructural: () => It,
    computed: () => le,
    computedStruct: () => Mn,
    configure: () => Hn,
    createAtom: () => $e,
    defineProperty: () => $n,
    entries: () => Xn,
    extendObservable: () => Pe,
    flow: () => ne,
    flowBound: () => ea,
    flowResult: () => kn,
    get: () => Jn,
    getAtom: () => ee,
    getDebugName: () => Zn,
    getDependencyTree: () => ni,
    getObserverTree: () => ii,
    has: () => Na,
    intercept: () => ei,
    isAction: () => oe,
    isBoxedObservable: () => Gt,
    isComputed: () => ci,
    isComputedProp: () => oi,
    isFlow: () => In,
    isFlowCancellationError: () => Ln,
    isObservable: () => Bn,
    isObservableArray: () => C,
    isObservableMap: () => g,
    isObservableObject: () => m,
    isObservableProp: () => Vn,
    isObservableSet: () => y,
    keys: () => Ne,
    makeAutoObservable: () => Fn,
    makeObservable: () => Un,
    observable: () => b,
    observableDeep: () => Kn,
    observableRef: () => De,
    observableShallow: () => zn,
    observableStruct: () => Nn,
    observe: () => Yn,
    onBecomeObserved: () => ai,
    onBecomeUnobserved: () => ti,
    onReactionError: () => En,
    override: () => Sn,
    ownKeys: () => za,
    reaction: () => Tn,
    remove: () => Qn,
    runInAction: () => An,
    set: () => Va,
    spy: () => Rn,
    toJS: () => ri,
    transaction: () => Pn,
    untracked: () => Dn,
    values: () => Wn,
    when: () => Gn
  });
  function z(e) {
    f(), Ga(e), v();
  }
  function p(e) {
    ce(e, []);
  }
  function S(e, a, t) {
    l.defineProperty(e, a, t);
  }
  function V(e, a) {
    ce(e, [a]);
  }
  function Ae(e, a) {
    return { annotationType_: e, options_: a, make_: xt, extend_: Ct };
  }
  function ba(e, a) {
    return { annotationType_: e, options_: a, make_: xa, extend_: At };
  }
  function We(e, a) {
    return { annotationType_: e, options_: a, make_: xa, extend_: St };
  }
  function Ht(e) {
    if (e) {
      if (e.defaultDecorator !== void 0) return e.defaultDecorator;
      var a;
      if (e.autoBind || false === e.deep) return a = { annotationType_: "true", options_: e, make_: ka, extend_: Sa }, a;
    }
  }
  function Ge(e) {
    return true === e.deep ? ie : false === e.deep ? Oe : e.defaultDecorator && e.defaultDecorator.options_ && e.defaultDecorator.options_.enhancer_ ? e.defaultDecorator.options_.enhancer_ : ie;
  }
  function la(e, a, t, r) {
    var n = fe(e)[u];
    n.lazyObservableKeys_ || (n.lazyObservableKeys_ = /* @__PURE__ */ new Map()), n.lazyObservableKeys_.set(t, function() {
      var i = ie;
      a.options_ && a.options_.enhancer_ && (i = a.options_.enhancer_);
      var O2 = "ObservableObject." + B(t);
      return new h(r, i, O2, false);
    });
    return n;
  }
  function J(e, a, t, r) {
    var n = (0, function() {
      var n2 = r == null ? this : r;
      return $a(e, t, a, n2, arguments);
    });
    n.isMobxAction = true, n.toString = function() {
      return a.toString();
    }, wt && (wa.value = e, S(n, "name", wa));
    return n;
  }
  function bt(t, r) {
    var e, a = r[1];
    r.length > 2 && "function" == typeof r[2] ? (e = ee(r[0], r[1]), a = r[2]) : e = ee(r[0]), e[t] ? e[t].add(a) : (r = e, r[t] = /* @__PURE__ */ new Set(), e[t].add(a));
    return function() {
      var r2 = e[t];
      r2 && (r2.delete(a), 0 == r2.size && delete e[t]);
    };
  }
  function oe(e) {
    return "function" == typeof e && true === e.isMobxAction;
  }
  function Le(e) {
    return ae(e) && true === e.isMobXReaction;
  }
  function Ie(e) {
    return ae(e) && true === e.isMobXCaughtException;
  }
  function N(e) {
    return ae(e) && true === e.isMobXComputedValue;
  }
  function ca(e) {
    return ae(e) && true === e.isMobXAtom;
  }
  function pe(e) {
    return e == null ? false : true === e.isMobXFlow;
  }
  function x(e) {
    return Array.from(e);
  }
  function Gt(e) {
    return !!me(e);
  }
  function y(e) {
    return !!pa(e);
  }
  function g(e) {
    return !!ha(e);
  }
  function He(e) {
    return !!e ? e : ja;
  }
  function R(e) {
    return !!e.proxy_ ? e.proxy_ : e.target_;
  }
  function ct(e) {
    return !!e.scheduler ? e.scheduler : !!e.delay ? function(t) {
      return setTimeout(t, e.delay);
    } : function(a) {
      return a();
    };
  }
  function Fa(e) {
    !e.onBUOL || e.onBUOL.forEach(function(a) {
      a();
    });
  }
  function Ua(e) {
    !e.onBOL || e.onBOL.forEach(function(a) {
      a();
    });
  }
  function U(e, a) {
    return P(function(r, n) {
      if (n && "string" == typeof n.kind) return a(e, r, n);
    }, e);
  }
  function P(e, a) {
    return l.assign(e, a);
  }
  function Pt(e, a) {
    return true === ye(e, a);
  }
  function Et(e, a) {
    return e === a;
  }
  function It(e, a) {
    return ge(e, a, -1, void 0, void 0);
  }
  function Tt(e, a) {
    return ge(e, a, 1, void 0, void 0);
  }
  function Fe(e, a) {
    return e.dehancer !== void 0 ? e.dehancer(a) : a;
  }
  function Jt(e, a) {
    return !e ? a : function() {
      try {
        return a.apply(this, arguments);
      } catch (n) {
        e.call(this, n);
        return;
      }
    };
  }
  function qa(a) {
    var e = false;
    return function() {
      if (!e) return e = true, a.apply(this, arguments);
    };
  }
  function te(e, a) {
    e = "isMobX" + e, a.prototype[e] = true;
    return function(t) {
      return ae(t) && true === t[e];
    };
  }
  function ue(e) {
    _.prototype[e] = function(t) {
      k(this.atom_);
      return ("intersection" == e || "union" == e || "symmetricDifference" == e || "isDisjointFrom" == e) && he(t) && !y(t) && "function" == typeof t[e] ? t[e](this) : new Set(this)[e](t);
    };
  }
  function G(e) {
    "function" == typeof Array.prototype[e] && (j[e] = function(p2, j2) {
      var f2 = this, h2 = f2[u];
      k(h2.atom_);
      var g2 = h2.dehanceValues_(h2.values_), l2 = j2;
      return g2[e](function(c, o) {
        return p2.call(l2, c, o, f2);
      });
    });
  }
  function rt(e) {
    "function" == typeof Array.prototype[e] && (j[e] = function() {
      var d2 = this;
      let v2 = d2[u];
      k(v2.atom_);
      let h2 = v2.dehanceValues_(v2.values_), p2 = arguments[0];
      arguments[0] = function(n, i, c) {
        return p2(n, i, c, d2);
      };
      return h2[e].apply(h2, arguments);
    });
  }
  function E(e) {
    if ("function" == typeof Array.prototype[e]) j[e] = function() {
      let r = this[u];
      k(r.atom_);
      let n = r.dehanceValues_(r.values_);
      return n[e].apply(n, arguments);
    };
  }
  function $t(e, a) {
    var t;
    if (a && a.signal && a.signal.aborted) return e = Promise.reject(new Error("WHEN_ABORTED")), e.cancel = function() {
      return null;
    }, e;
    t = { cancel: void 0, abort: void 0 };
    var r = new Promise(function(L2, K2) {
      var M2 = P({}, a);
      M2.onError = K2, M2 = ot(e, L2, M2), t.cancel = function() {
        M2(), K2(new Error("WHEN_CANCELLED"));
      }, t.abort = function() {
        M2(), K2(new Error("WHEN_ABORTED"));
      }, a && a.signal && "function" == typeof a.signal.addEventListener && a.signal.addEventListener("abort", t.abort);
    });
    a && a.signal && "function" == typeof a.signal.removeEventListener && (r = r.finally(function() {
      a.signal.removeEventListener("abort", t.abort);
    })), r.cancel = t.cancel;
    return r;
  }
  function ot(r, n, i) {
    i = i || {};
    var e, a;
    if ("number" == typeof i.timeout) {
      var c = new Error("WHEN_TIMEOUT");
      a = setTimeout(function() {
        if (!e[u].isDisposed) e(), i.onError ? i.onError(c) : Ke(c);
      }, i.timeout);
    }
    i.name || (i.name = "When");
    var t = J("When-effect", n, false, void 0);
    e = ra(function(n2) {
      ma(false, r) && (n2.dispose(), !a || clearTimeout(a), t());
    }, i);
    return e;
  }
  function ut(t, r, n) {
    if (N(t)) {
      var a, e = true;
      return ra(function() {
        var i, c = t.get();
        (!e || n) && (i = Q(), r({ observableKind: "computed", debugObjectName: t.name_, type: "update", object: t, newValue: c, oldValue: a }), K(i)), e = false, a = c;
      });
    }
    n && r({ observableKind: "value", debugObjectName: t.name_, object: t, type: "update", newValue: t.value_, oldValue: void 0 });
    return Se(t, r);
  }
  function Ft(e, a, t, r) {
    var i = I(e), n = null;
    i && (n = { observableKind: "array", object: e.proxy_, type: "update", debugObjectName: e.atom_.name_, index: a, newValue: t, oldValue: r }), z(e.atom_), i && T(e, n);
  }
  function Wt(e, a, t, r) {
    var c, o, i = I(e), n = null;
    i && (n = e.proxy_, c = e.atom_.name_, o = r.length, n = { observableKind: "array", object: n, debugObjectName: c, type: "splice", index: a, removed: r, added: t, removedCount: o, addedCount: t.length }), z(e.atom_), i && T(e, n);
  }
  function Ue(e, a, t, r) {
    var n = +e.values_.length;
    a > n ? a = n : a < 0 && (a = n + a | 0, a < 0 && (a = 0)), t < 0 && (t = 0), n = n - a | 0, t > n || (n = t), r == null ? r = [] : Array.isArray(r) || (r = Array.prototype.slice.call(r));
    if (M(e)) {
      t = L(e, { object: e.proxy_, type: "splice", index: a, removedCount: n, added: r });
      if (!t) return fa;
      n = t.removedCount | 0, r = t.added;
    }
    if (0 != r.length) {
      t = [];
      for (var c = r.length, i = 0; i < c; i++) t.push(e.enhancer_(r[i], void 0));
    } else t = r;
    i = Ut(e, a, n, t);
    (0 != n || 0 != t.length) && Wt(e, a, t, i);
    return e.dehanceValues_(i);
  }
  function Ut(e, a, t, r) {
    var n = e.values_, i = r.length;
    if (0 == t && a == n.length) {
      for (e = 0; e < i; e++) n.push(r[e]);
      return fa;
    }
    if (i < 1e4) {
      for (e = [], e.push(a), e.push(t), a = 0; a < r.length; a++) e.push(r[a]);
      return n.splice.apply(n, e);
    }
    e = a + t | 0;
    var c = re.call(n, a, e);
    for (i = re.call(n, e, n.length), e = n.length, n.length = e + r.length - t | 0, e = 0; e < r.length; e++) n[a + e | 0] = r[e];
    for (e = 0; e < i.length; e++) n[a + r.length + e | 0] = i[e];
    return c;
  }
  function _e(e, a) {
    var t, r, n, i;
    if (e == null || "object" != typeof e || Ka(Date, e) || !Te(e)) return e;
    if (me(e) || N(e)) return _e(e.get(), a);
    if (a.has(e)) return a.get(e);
    if (C(e)) {
      for (r = [], a.set(e, r), t = 0; t < e.length; t++) r[t] = _e(e[t], a);
      return r;
    }
    if (y(e)) {
      for (t = /* @__PURE__ */ new Set(), a.set(e, t), r = x(e.values()), e = 0; e < r.length; e++) t.add(_e(r[e], a));
      return t;
    }
    if (g(e)) {
      for (r = /* @__PURE__ */ new Map(), a.set(e, r), t = x(e.entries()), e = 0; e < t.length; e++) r.set(t[e][0], _e(t[e][1], a));
      return r;
    }
    n = {};
    a.set(e, n), r = za(e), t = 0;
    for (; t < r.length; t++) true === l.prototype.propertyIsEnumerable.call(e, r[t]) && (i = r[t], n[i] = _e(e[r[t]], a));
    return n;
  }
  function Dt(e) {
    var t = l.keys(e), a = l.getOwnPropertySymbols(e);
    if (0 == a.length) return t;
    var n = re.call(t), i = a.length;
    for (t = 0; t < i; t++) {
      var r = a[t];
      true === l.prototype.propertyIsEnumerable.call(e, r) && n.push(r);
    }
    return n;
  }
  function st(e, a, t) {
    true === t && (t = e.defaultAnnotation_);
    if (false !== t) {
      if (true !== a in e.target_) {
        var n = t.annotationType_, i = e.name_ + "." + B(a);
        ce(1, [n, i]);
      }
      var r;
      for (r = e.target_; ; ) {
        n = r && r !== l.prototype;
        if (!n) break;
        if (n = l.getOwnPropertyDescriptor(r, a)) {
          n = t.make_(e, a, n, r);
          if (0 === n) return;
          if (1 === n) break;
        }
        r = l.getPrototypeOf(r);
      }
    }
  }
  function tt(e, a, t, r) {
    if ($.call(e.target_, a)) {
      if (e.values_.has(a)) return e.setObservablePropValue_(a, t);
      if (r) return true === Reflect.set(e.target_, a, t);
      e.target_[a] = t;
      return true;
    }
    return e.extend_(a, { value: t, enumerable: true, writable: true, configurable: true }, e.defaultAnnotation_, r);
  }
  function at(e) {
    var a = Aa[e];
    if (a) return a;
    a = { get: function() {
      return this[u].getObservablePropValue_(e);
    }, set: function(t) {
      return this[u].setObservablePropValue_(e, t);
    } }, Aa[e] = a;
    return a;
  }
  function it(e, a, t, r) {
    var n = a.value;
    pe(n) || (n = ne(n)), t && (n = n.bind(R(e)), n.isMobXFlow = true), r ? (a = !!e.isPlainObject_, e = false) : (a = true, e = true);
    return { value: n, configurable: a, enumerable: false, writable: e };
  }
  function nt(e, a, t, r, n) {
    var i = r.value;
    a.options_ && a.options_.bound && (i = i.bind(R(e)));
    var c = B(t);
    a.options_ && a.options_.name && (c = a.options_.name + ""), r = a.options_ && a.options_.autoAction;
    var o;
    a.options_ && a.options_.bound && (o = R(e)), n ? (a = !!e.isPlainObject_, e = false) : (a = true, e = true);
    return { value: J(c, i, r, o), configurable: a, enumerable: false, writable: e };
  }
  function $a(e, a, t, r, n) {
    var l2 = Qa(e, a, r, n);
    try {
      return t.apply(r, n);
    } catch (e2) {
      l2.error_ = e2;
      throw e2;
    } finally {
      Ja(l2);
    }
  }
  function se(e) {
    var _2 = Q(), y2 = true;
    f();
    try {
      return e();
    } finally {
      v(), K(_2);
    }
  }
  function L(e, a) {
    var q2 = Q();
    try {
      var I2 = [];
      e.interceptors_ && (I2 = e.interceptors_);
      for (var T2 = re.call(I2), G2 = T2.length, H = 0; H < G2; H++) {
        a = T2[H](a), a && !a.type && p(14);
        if (!a) break;
      }
      return a;
    } finally {
      K(q2);
    }
  }
  function T(e, a) {
    var t = Q(), r = e.changeListeners_;
    if (!r) {
      K(t);
      return;
    }
    r = re.call(r);
    var n = r.length;
    for (e = 0; e < n; e++) r[e](a);
    K(t);
  }
  function Se(e, a) {
    e.changeListeners_ === void 0 && (e.changeListeners_ = []);
    var t = e.changeListeners_;
    t.push(a);
    return qa(function() {
      var u2 = +t.indexOf(a);
      u2 != -1 && t.splice(u2, 1);
    });
  }
  function Za(e, a) {
    e.interceptors_ === void 0 && (e.interceptors_ = []);
    var t = e.interceptors_;
    t.push(a);
    return qa(function() {
      var u2 = +t.indexOf(a);
      u2 != -1 && t.splice(u2, 1);
    });
  }
  function M(e) {
    return e.interceptors_ !== void 0 && e.interceptors_.length > 0;
  }
  function I(e) {
    return e.changeListeners_ !== void 0 && e.changeListeners_.length > 0;
  }
  function ce(e, a) {
    a = a.length > 0 ? " " + a.map(String).join(",") : "", Ke(new Error("[MobX] minified error nr: " + e + a + ". See mobx.js.org/errors"));
  }
  function f() {
    s.inBatch++;
  }
  function Ha() {
    if (!((s.inBatch | 0) > 0 || s.isRunningReactions)) Je(mt);
  }
  function Ga(e) {
    if (2 !== e.lowestObserverState_) e.lowestObserverState_ = 2, e.observers_.forEach(gt);
  }
  function Nt(e) {
    if (2 !== e.lowestObserverState_) {
      e.lowestObserverState_ = 2;
      var a = Ce;
      Ce = e, e.observers_.forEach(yt), Ce = a;
    }
  }
  function zt(e) {
    if (!e.lowestObserverState_) e.lowestObserverState_ = 1, e.observers_.forEach(_t);
  }
  function Vt(e, a) {
    e.observers_.add(a);
    var t = e.lowestObserverState_ | 0;
    t > (a.dependenciesState_ | 0) && (e.lowestObserverState_ = a.dependenciesState_);
  }
  function sa(e) {
    var a = e.dependenciesState_ | 0;
    if (0 == a) return false;
    var t, n, r, i;
    if (a == -1 || 2 == a) return true;
    if (1 == a) {
      a = true, a = Q(), n = e.observing_, i = n.length, r = 0;
      while (r < i) {
        t = n[r];
        if (N(t)) {
          if (true === s.disableErrorBoundaries) t.get();
          else try {
            t.get();
          } catch {
            K(a);
            return true;
          }
          if (2 === e.dependenciesState_) return K(a), true;
        }
        r++;
      }
      Wa(e);
      K(a);
      return false;
    }
    return false;
  }
  function Wa(e) {
    if (0 != (e.dependenciesState_ | 0)) {
      e.dependenciesState_ = 0;
      var t = e.observing_, a = t.length;
      while (a > 0) a--, t[a].lowestObserverState_ = 0;
    }
  }
  function qt(e) {
    var o = e.observing_, r = e.newObserving_;
    e.observing_ = r;
    for (var n, c, s2 = e.unboundDepsCount_ | 0, t = 0, a = 0, i = 0; i < s2; i++) n = r[i], 0 == (n.diffValue | 0) && (n.diffValue = 1, a != i && (r[a] = n), a++), c = n.dependenciesState_, c !== void 0 && (c | 0) > t && (t = c | 0);
    r.length = a, e.newObserving_ = null, n = o.length;
    while (n > 0) n--, i = o[n], 0 == (i.diffValue | 0) && Ta(i, e), i.diffValue = 0;
    while (a > 0) a--, n = r[a], 1 == (n.diffValue | 0) && (n.diffValue = 0, Vt(n, e));
    0 != t && (e.dependenciesState_ = t, e.onBecomeStale_());
  }
  function ua(e) {
    var t = e.observing_;
    e.observing_ = [];
    var a = t.length;
    while (a > 0) a--, Ta(t[a], e);
    e.dependenciesState_ = -1;
  }
  function lt(e) {
    var a, t = { name: e.name_ };
    if (e.observing_ && e.observing_.length > 0) {
      var r = [];
      for (a = 0; a < e.observing_.length; a++) r.push(lt(e.observing_[a]));
      t.dependencies = r;
    }
    return t;
  }
  function dt(e) {
    var a = { name: e.name_ };
    if (Bt(e)) {
      var t = x(e.observers_.values()), r = [];
      for (e = 0; e < t.length; e++) r.push(dt(t[e]));
      a.observers = r;
    }
    return a;
  }
  function Bt(e) {
    return e.observers_ && e.observers_.size;
  }
  function Ta(e, a) {
    e.observers_.delete(a), e.observers_.size || Ia(e);
  }
  function k(e) {
    var a = s.trackingDerivation;
    if (a != null) {
      if (a.runId_ !== e.lastAccessedBy_) {
        e.lastAccessedBy_ = a.runId_;
        var t = a.unboundDepsCount_ | 0;
        a.newObserving_[t] = e, a.unboundDepsCount_ = t + 1 | 0, !e.isBeingObserved && s.trackingContext && (e.isBeingObserved = true, e.onBO());
      }
      return !!e.isBeingObserved;
    } else !e.observers_.size && s.inBatch && Ia(e);
    return false;
  }
  function Xa(e, a, t) {
    var r = true;
    Wa(e), r = 0 != (e.runId_ | 0) ? +e.observing_.length : 100, e.newObserving_ = new Array(r), e.unboundDepsCount_ = 0, r = (s.runId | 0) + 1 | 0, s.runId = r, e.runId_ = r, r = s.trackingDerivation, s.trackingDerivation = e, s.inBatch++;
    var ee2;
    if (true === s.disableErrorBoundaries) ee2 = a.call(t);
    else try {
      ee2 = a.call(t);
    } catch (w2) {
      ee2 = new we(w2);
    }
    s.inBatch--;
    s.trackingDerivation = r, qt(e);
    return ee2;
  }
  function Q() {
    let e = s.trackingDerivation;
    s.trackingDerivation = null;
    return e;
  }
  function Lt() {
    return s.trackingDerivation != null;
  }
  function K(e) {
    s.trackingDerivation = e;
  }
  function Ke(e) {
    throw e;
  }
  function Rt(e) {
    throw new TypeError(e);
  }
  function Me(e) {
    e[Symbol.iterator] = ht;
    if (!da) {
      da = true;
      var a = globalThis.Iterator;
      Xe = a ? a.prototype : {};
    }
    a = Xe;
    return P(l.create(a), e);
  }
  function Mt(e) {
    s.allowStateReads = e;
  }
  function Kt(e) {
    let a = !!s.allowStateReads;
    s.allowStateReads = e;
    return a;
  }
  function Qa(e, a) {
    var n = s.trackingDerivation;
    a = !a || n == null, f();
    var r = !!s.allowStateChanges;
    a && Q();
    var i = !!s.allowStateReads, t = Ze;
    Ze++;
    var c = Re;
    Re = t, e = { runAsAction_: a, prevDerivation_: n, prevAllowStateChanges_: r, prevAllowStateReads_: i, notifySpy_: false, startTime_: 0, actionId_: t, parentActionId_: c };
    return e;
  }
  function Ja(e) {
    Re != (e.actionId_ | 0) && p(30), Re = e.parentActionId_ | 0, e.error_ === void 0 || (s.suppressReactionErrors = true), v(), !e.runAsAction_ || K(e.prevDerivation_), s.suppressReactionErrors = false;
  }
  function qe() {
    let e = { version: 7, UNCHANGED: {} }, a = null;
    e.trackingDerivation = a, e.trackingContext = a, e.runId = 0, e.mobxGuid = 0, e.inBatch = 0, e.pendingUnobservations = [], e.pendingReactions = [], e.isRunningReactions = false, e.allowStateChanges = false, e.allowStateReads = true, e.enforceActions = true, e.spyListeners = [], e.globalReactionErrorHandlers = [], e.computedRequiresReaction = false, e.reactionRequiresObservable = false, e.observableRequiresReaction = false, e.disableErrorBoundaries = false, e.suppressReactionErrors = false, e.safeDescriptors = true;
    return e;
  }
  function Ia(e) {
    e.isPendingUnobservation || (e.isPendingUnobservation = true, s.pendingUnobservations.push(e));
  }
  function v() {
    var e = --s.inBatch;
    if (0 == e) {
      Ha();
      for (var t = s.pendingUnobservations, a = 0; a < t.length; a++) e = t[a], e.isPendingUnobservation = false, e.observers_.size || (!e.isBeingObserved || (e.isBeingObserved = false, e.onBUO()), N(e) && e.suspend_());
      s.pendingUnobservations = [];
    }
  }
  function Xt(e) {
    var a, r, t;
    if (ve(e) || g(e)) return e;
    if (Array.isArray(e)) return new Map(e);
    if (X(e)) {
      for (r = /* @__PURE__ */ new Map(), t = l.keys(e), a = 0; a < t.length; a++) r.set(t[a], e[t[a]]);
      return r;
    }
    V(21, e);
    return /* @__PURE__ */ new Map();
  }
  function Qt(e, a, t) {
    return a && "string" == typeof a.kind ? je(aa, e, a) : Te(e) ? e : X(e) ? b.object(e, a, t) : Array.isArray(e) ? b.array(e, a) : ve(e) ? b.map(e, a) : he(e) ? b.set.call(b, e, a) : "object" == typeof e && e != null ? e : b.box(e, a);
  }
  function ae(e) {
    e = e != null && "object" == typeof e;
    return e;
  }
  function La(e) {
    return null === e ? null : "object" == typeof e ? "" + e : e;
  }
  function B(e) {
    return "string" == typeof e ? e : "symbol" == typeof e ? e.toString() : new String(e) + "";
  }
  function ia(e) {
    e = typeof e;
    return "string" == e || "symbol" == e || "number" == e;
  }
  function Te(e) {
    return !e ? false : m(e) || e[u] || ca(e) || Le(e) || N(e);
  }
  function Ya(e, a) {
    if (!e) return false;
    if ("function" == typeof e.isPrototypeOf) return true === e.isPrototypeOf(a);
    true === "constructor" in a ? (e = a.constructor == e, e = true === e) : e = false;
    return e;
  }
  function X(e) {
    if (!ae(e)) return false;
    var a = l.getPrototypeOf(e);
    if (a == null) return true;
    e = void 0, !$.call(a, "constructor") || (e = a.constructor), e = "function" == typeof e && e.toString() === vt;
    return e;
  }
  function Ma(e) {
    if (e == null) return false;
    var a = e.constructor;
    return !a ? false : "GeneratorFunction" == a.name + "" ? true : "GeneratorFunction" == a.displayName + "" ? true : false;
  }
  function ge(e, a, t, r, n) {
    if (e === a) return 0 !== e ? e = true : (e = 1 / +e, e = e === 1 / +a), e;
    var i, c, o, s2;
    if (e == null || a == null) return false;
    if (e !== e) return a !== a;
    i = typeof e;
    if ("function" != i && "object" != i && "object" != typeof a) return false;
    i = l.prototype.toString.call(e) + "";
    if (i != l.prototype.toString.call(a) + "") return false;
    if ("[object RegExp]" == i || "[object String]" == i) return "" + e == "" + a;
    if ("[object Number]" == i) {
      e = Number(e), a = Number(a);
      return true !== ye(e, e) ? true !== ye(a, a) : 0 === e ? (e = 1 / +e, true === ye(e, 1 / +a)) : e === a;
    }
    if ("[object Date]" == i || "[object Boolean]" == i) return e = Number(e), e === Number(a);
    if ("[object Symbol]" == i) return e = Symbol.valueOf.call(e), e === Symbol.valueOf.call(a);
    ("[object Map]" == i || "[object Set]" == i) && t >= 0 && (t = t + 1 | 0), c = et(e), e = et(a), o = "[object Array]" == i;
    if (!o) {
      if ("object" != typeof c || "object" != typeof e) return false;
      a = c.constructor, i = e.constructor, a !== i ? (a = "function" == typeof a && Ya(a, a) && "function" == typeof i && Ya(i, i), a = !a) : a = false;
      if (a && true === "constructor" in c && true === "constructor" in e) return false;
    }
    if (0 == t) return false;
    else t < 0 && (t = -1);
    r === void 0 && (r = [], n = []);
    for (a = r.length; a--; ) if (r[a] === c) return n[a] === e;
    r.push(c), n.push(e);
    if (o) {
      a = c.length;
      if (a != e.length) return false;
      while (a > 0) {
        a--;
        if (!ge(c[a], e[a], t - 1 | 0, r, n)) return false;
      }
    } else {
      o = l.keys(c), s2 = o.length;
      if (l.keys(e).length != s2) return false;
      for (i = 0; i < s2; i++) {
        a = o[i], a = $.call(e, a) && ge(c[a], e[a], t - 1 | 0, r, n);
        if (!a) return false;
      }
    }
    r.pop();
    n.pop();
    return true;
  }
  function ve(e) {
    return e == null ? false : "[object Map]" == l.prototype.toString.call(e) + "";
  }
  function he(e) {
    return e == null ? false : "[object Set]" == l.prototype.toString.call(e) + "";
  }
  function Ka(e, a) {
    return a == null ? false : true === ft.call(e.prototype, a);
  }
  function C(e) {
    return !ae(e) ? false : !!ga(e[u]);
  }
  function m(e) {
    return !ae(e) ? false : !!_a(e[u]);
  }
  function et(e) {
    return C(e) ? e.slice() : ve(e) || g(e) ? x(e.entries()) : he(e) || y(e) ? x(e.entries()) : e;
  }
  var l = Object;
  var $ = l.prototype.hasOwnProperty;
  var ye = l.is;
  var re = Array.prototype.slice;
  var ft = l.prototype.isPrototypeOf;
  var vt = l.toString();
  var Xe = void 0;
  var da = false;
  var ht = (0, function() {
    return this;
  });
  var be = function() {
  };
  var kn = [];
  l.freeze(kn);
  var fa = kn;
  kn = {}, l.freeze(kn);
  var va = kn;
  var u = Symbol("mobx administration");
  var Qe = function(a, t) {
    return true === ye(a, t);
  };
  var me;
  var ha;
  var pa;
  var ga;
  var _a;
  var xe = true;
  var ya = false;
  var q = [];
  (function() {
    q.push("mobxGuid"), q.push("spyListeners"), q.push("enforceActions"), q.push("computedRequiresReaction"), q.push("reactionRequiresObservable"), q.push("observableRequiresReaction"), q.push("allowStateReads"), q.push("disableErrorBoundaries"), q.push("runId"), q.push("UNCHANGED");
  })();
  var s = (function() {
    var a;
    globalThis.__mobxInstanceCount && (globalThis.__mobxInstanceCount | 0) > 0 && !globalThis.__mobxGlobals && (xe = false), a = globalThis.__mobxGlobals, a && 7 != (a.version | 0) && (xe = false);
    if (!xe) return setTimeout(function() {
      ya || p(35);
    }, 1), qe();
    else if (a) {
      var _2 = (globalThis.__mobxInstanceCount | 0) + 1 | 0;
      globalThis.__mobxInstanceCount = _2, a.UNCHANGED || (a.UNCHANGED = {});
      return a;
    }
    globalThis.__mobxInstanceCount = 1;
    a = qe(), globalThis.__mobxGlobals = a;
    return a;
  })();
  var pt = function() {
    var e, a;
    (0 != s.pendingReactions.length || 0 != (s.inBatch | 0) || s.isRunningReactions) && p(36), ya = true, xe && (e = globalThis, a = --e.__mobxInstanceCount, 0 == a && (e.__mobxGlobals = void 0), s = qe());
  };
  var xn = function() {
    return s;
  };
  var Cn = function() {
    for (var e, t = qe(), r = l.keys(t), n = r.length, a = 0; a < n; a++) e = r[a], +q.indexOf(e) == -1 && (s[e] = t[e]);
    s.allowStateChanges = !s.enforceActions;
  };
  var Rn = function(a) {
    console.warn("[mobx.spy] Is a no-op in production builds");
    return function() {
    };
  };
  var gt = function(a) {
    a.dependenciesState_ || a.onBecomeStale_(), a.dependenciesState_ = 2;
  };
  var _t = function(a) {
    a.dependenciesState_ || (a.dependenciesState_ = 1, a.onBecomeStale_());
  };
  var Ce = void 0;
  var yt = function(a) {
    var e = a.dependenciesState_;
    1 === e ? a.dependenciesState_ = 2 : e || (Ce.lowestObserverState_ = 0);
  };
  var Je = function(e) {
    return e();
  };
  var mt = function() {
    s.isRunningReactions = true;
    var t, r, n, e = s.pendingReactions, a = 0;
    while (e.length > 0) {
      a++, 100 == a && (t = "[mobx] cycle in reaction: " + e[0], console.error(t), e.splice(0, e.length)), r = e.splice(0, e.length), n = r.length, t = 0;
      for (; t < n; t++) r[t].runReaction_();
    }
    s.isRunningReactions = false;
  };
  var Dn = function(a) {
    var o = Q();
    try {
      return a();
    } finally {
      K(o);
    }
  };
  var ma = function(a, t) {
    var e = !!s.allowStateChanges;
    s.allowStateChanges = !!a;
    try {
      return t();
    } finally {
      s.allowStateChanges = e;
    }
  };
  var we = class {
    constructor(a) {
      this.cause = a;
    }
  };
  S(we, "name", { value: "CaughtException", configurable: true });
  we.prototype.isMobXCaughtException = true;
  var D = class {
    constructor(a = "Atom") {
      a = a + "", this.name_ = a, this.observers_ = /* @__PURE__ */ new Set(), this.lastAccessedBy_ = 0, this.lowestObserverState_ = -1, this.flags_ = 0;
    }
    onBO() {
      Ua(this);
    }
    onBUO() {
      Fa(this);
    }
    reportObserved() {
      return k(this);
    }
    reportChanged() {
      z(this);
    }
    toString() {
      return this.name_;
    }
    get isBeingObserved() {
      return 0 != (this.flags_ & 1);
    }
    set isBeingObserved(a) {
      a ? this.flags_ |= 1 : this.flags_ &= ~1;
    }
    get isPendingUnobservation() {
      return 0 != (this.flags_ & 2);
    }
    set isPendingUnobservation(a) {
      a ? this.flags_ |= 2 : this.flags_ &= ~2;
    }
    get diffValue() {
      return 0 != (this.flags_ & 4) ? 1 : 0;
    }
    set diffValue(t) {
      1 == (t | 0) ? this.flags_ |= 4 : this.flags_ &= ~4;
    }
  };
  te("Atom", D);
  kn = D.prototype;
  var $e = function(a, t = be, r = be) {
    var e = a !== void 0 ? new D(a) : new D();
    t === be || (e.onBOL = /* @__PURE__ */ new Set(), e.onBOL.add(t)), r === be || (e.onBUOL = /* @__PURE__ */ new Set(), e.onBUOL.add(r));
    return e;
  };
  var Re = 0;
  var Ze = 1;
  var wa = { value: "action", configurable: true, writable: false, enumerable: false };
  var Sn = l.getOwnPropertyDescriptor(function() {
  }, "name");
  kn = Sn != null && Sn.configurable;
  var wt = kn;
  var A = class {
    constructor(i = "Reaction", o, u2, d2) {
      var t = i + "";
      this.name_ = t, this.onInvalidate_ = void 0, o !== void 0 && (this.onInvalidate_ = o), u2 && (this.errorHandler_ = u2), d2 !== void 0 && (this.requiresObservable_ = d2), this.observing_ = [], this.newObserving_ = null, this.dependenciesState_ = -1, this.runId_ = 0, this.unboundDepsCount_ = 0, this.flags_ = 0;
    }
    onBecomeStale_() {
      this.schedule_();
    }
    schedule_() {
      this.isScheduled || (this.isScheduled = true, s.pendingReactions.push(this), Ha());
    }
    runReaction_() {
      if (!this.isDisposed) {
        f(), this.isScheduled = false;
        var t = s.trackingContext;
        s.trackingContext = this;
        if (sa(this)) {
          this.isTrackPending = true;
          try {
            this.onInvalidate_();
            if (false) {
            }
          } catch (l2) {
            this.reportExceptionInDerivation_(l2);
          }
        }
        s.trackingContext = t;
        v();
      }
    }
    track(a) {
      if (!this.isDisposed) {
        f(), this.isRunning = true;
        var t = s.trackingContext;
        s.trackingContext = this, a = Xa(this, a, void 0), s.trackingContext = t, this.isRunning = false, this.isTrackPending = false, !this.isDisposed || ua(this), Ie(a) && this.reportExceptionInDerivation_(a.cause), v();
      }
    }
    reportExceptionInDerivation_(a) {
      if (this.errorHandler_) {
        this.errorHandler_(a, this);
        return;
      }
      !s.disableErrorBoundaries || Ke(a);
      var t = "[mobx] uncaught error in '" + this + "'";
      s.suppressReactionErrors || console.error(t, a);
      var r = s.globalReactionErrorHandlers, n = r.length;
      for (t = 0; t < n; t++) r[t](a, this);
    }
    dispose() {
      this.isDisposed || (this.isDisposed = true, this.isRunning || (f(), ua(this), v()));
    }
    getDisposer_(t) {
      var a = this, e = function() {
        a.dispose(), t != null && "function" == typeof t.removeEventListener && t.removeEventListener("abort", e);
      };
      t != null && "function" == typeof t.addEventListener && t.addEventListener("abort", e), e[u] = a, true === "dispose" in Symbol && "symbol" == typeof Symbol.dispose && (e[Symbol.dispose] = e);
      return e;
    }
    toString() {
      return "Reaction[" + this.name_ + "]";
    }
    get isDisposed() {
      return 0 != (this.flags_ & 1);
    }
    set isDisposed(a) {
      a ? this.flags_ |= 1 : this.flags_ &= ~1;
    }
    get isScheduled() {
      return 0 != (this.flags_ & 2);
    }
    set isScheduled(a) {
      a ? this.flags_ |= 2 : this.flags_ &= ~2;
    }
    get isTrackPending() {
      return 0 != (this.flags_ & 4);
    }
    set isTrackPending(a) {
      a ? this.flags_ |= 4 : this.flags_ &= ~4;
    }
    get isRunning() {
      return 0 != (this.flags_ & 8);
    }
    set isRunning(a) {
      a ? this.flags_ |= 8 : this.flags_ &= ~8;
    }
    get diffValue() {
      return 0 != (this.flags_ & 16) ? 1 : 0;
    }
    set diffValue(t) {
      1 == (t | 0) ? this.flags_ |= 16 : this.flags_ &= ~16;
    }
  };
  te("Reaction", A);
  var En = function(a) {
    s.globalReactionErrorHandlers.push(a);
    return function() {
      var f2 = +s.globalReactionErrorHandlers.indexOf(a);
      f2 >= 0 && s.globalReactionErrorHandlers.splice(f2, 1);
    };
  };
  var w = class {
    constructor(a) {
      a.get || p(31), this.derivation = a.get;
      var t = a.name ? a.name + "" : "ComputedValue";
      this.name_ = t, a.set ? this.setter_ = J("ComputedValue-setter", a.set, false, void 0) : this.setter_ = void 0, this.equals_ = Qe, a.equals && (this.equals_ = a.equals), this.scope_ = a.context, this.requiresReaction_ = a.requiresReaction, this.keepAlive_ = !!a.keepAlive, this.dependenciesState_ = -1, this.observing_ = [], t = null, this.newObserving_ = t, this.observers_ = /* @__PURE__ */ new Set(), this.runId_ = 0, this.lastAccessedBy_ = 0, this.lowestObserverState_ = 0, this.unboundDepsCount_ = 0, this.value_ = new we(t), this.flags_ = 0;
    }
    onBecomeStale_() {
      zt(this);
    }
    onBO() {
      Ua(this);
    }
    onBUO() {
      Fa(this);
    }
    computeValue_(a) {
      this.isComputing = true;
      var C2, t = false;
      if (a) C2 = Xa(this, this.derivation, this.scope_);
      else if (true === s.disableErrorBoundaries) C2 = this.derivation.call(this.scope_);
      else try {
        C2 = this.derivation.call(this.scope_);
      } catch (h2) {
        C2 = new we(h2);
      }
      this.isComputing = false;
      return C2;
    }
    trackAndCompute() {
      var r = this.value_, a = (this.dependenciesState_ | 0) == -1, t = this.computeValue_(true);
      a = a || Ie(r) || Ie(t) || true !== this.equals_(r, t), a && (this.value_ = t);
      return a;
    }
    get() {
      var a;
      if (this.isComputing) a = [this.name_, this.derivation], ce(32, a);
      !s.inBatch && !this.observers_.size && !this.keepAlive_ ? sa(this) && (f(), this.value_ = this.computeValue_(false), v()) : (k(this), sa(this) && (a = s.trackingContext, this.keepAlive_ && !a && (s.trackingContext = this), !this.trackAndCompute() || Nt(this), s.trackingContext = a)), a = this.value_, Ie(a) && Ke(a.cause);
      return a;
    }
    get isComputing() {
      return 0 != (this.flags_ & 1);
    }
    set isComputing(a) {
      a ? this.flags_ |= 1 : this.flags_ &= ~1;
    }
    get isRunningSetter() {
      return 0 != (this.flags_ & 2);
    }
    set isRunningSetter(a) {
      a ? this.flags_ |= 2 : this.flags_ &= ~2;
    }
    get isBeingObserved() {
      return 0 != (this.flags_ & 4);
    }
    set isBeingObserved(a) {
      a ? this.flags_ |= 4 : this.flags_ &= ~4;
    }
    get isPendingUnobservation() {
      return 0 != (this.flags_ & 8);
    }
    set isPendingUnobservation(a) {
      a ? this.flags_ |= 8 : this.flags_ &= ~8;
    }
    get diffValue() {
      return 0 != (this.flags_ & 16) ? 1 : 0;
    }
    set diffValue(t) {
      1 == (t | 0) ? this.flags_ |= 16 : this.flags_ &= ~16;
    }
    set(a) {
      if (this.setter_) {
        !this.isRunningSetter || V(33, this.name_), this.isRunningSetter = true;
        try {
          this.setter_.call(this.scope_, a);
        } finally {
          this.isRunningSetter = false;
        }
      } else V(34, this.name_);
    }
    suspend_() {
      this.keepAlive_ || (ua(this), this.value_ = void 0);
    }
    warnAboutUntrackedRead_() {
    }
    toString() {
      let a = this.name_ + "[";
      return a + this.derivation.toString() + "]";
    }
    valueOf() {
      return La(this.get());
    }
  };
  w.prototype[Symbol.toPrimitive] = function() {
    return this.valueOf();
  }, te("ComputedValue", w), kn = w.prototype;
  var Pn = function(a, t) {
    f();
    try {
      return a.apply(t);
    } finally {
      v();
    }
  };
  var Bn = function(a) {
    return Te(a);
  };
  var Vn = function(a, t) {
    if (!m(a)) return false;
    var e = a[u];
    return e.values_.has(t) ? true : e.lazyComputedKeys_ && e.lazyComputedKeys_.has(t) ? true : e.lazyObservableKeys_ && e.lazyObservableKeys_.has(t) ? true : false;
  };
  var b = void 0;
  var De;
  var le;
  var de;
  var Ee;
  var Ye;
  var ne = void 0;
  var ea;
  var Oa;
  var Pe;
  var Oe = function(a) {
    return a;
  };
  var ie = function(a, t, r) {
    return Te(a) ? a : Array.isArray(a) ? r ? b.array(a, { name: r }) : b.array(a) : X(a) ? r ? b.object(a, void 0, { name: r }) : b.object(a) : ve(a) ? r ? b.map(a, { name: r }) : b.map(a) : he(a) ? r ? b.set.call(b, a, { name: r }) : b.set.call(b, a) : "function" == typeof a && !oe(a) && !pe(a) ? Ma(a) ? ne(a) : Ee(r, a) : a;
  };
  var An = function(a, t, r) {
    return a == null ? a : m(a) || C(a) || g(a) || y(a) ? a : Array.isArray(a) ? b.array(a, { name: r, deep: false }) : X(a) ? b.object(a, void 0, { name: r, deep: false }) : ve(a) ? b.map(a, { name: r, deep: false }) : he(a) ? b.set.call(b, a, { name: r, deep: false }) : a;
  };
  var Nn = function(a, t) {
    return ge(a, t, -1, void 0, void 0) ? t : a;
  };
  var ja = { deep: true, name: void 0, defaultDecorator: void 0 };
  l.freeze(ja);
  var h = class extends D {
    constructor(c, n, s2 = "ObservableValue", C2, f2) {
      var t = s2 + "";
      if (arguments.length > 3) {
      }
      var r = Qe;
      f2 && (r = f2), super(t), this.enhancer_ = n, this.name_ = t, this.equals_ = r, this.hasUnreportedChange_ = false, this.value_ = n(c, void 0, t);
    }
    prepareNewValue_(a) {
      if (M(this)) {
        var t = L(this, { object: this, type: "update", newValue: a });
        if (!t) return s.UNCHANGED;
        a = t.newValue;
      }
      a = this.enhancer_(a, this.value_, this.name_);
      return true === this.equals_(this.value_, a) ? s.UNCHANGED : a;
    }
    setNewValue_(a) {
      var t = this.value_;
      this.value_ = a, z(this), I(this) && T(this, { type: "update", object: this, newValue: a, oldValue: t });
    }
    set(a) {
      a = this.prepareNewValue_(a), a === s.UNCHANGED || this.setNewValue_(a);
    }
    get() {
      k(this);
      return this.dehancer === void 0 ? this.value_ : this.dehancer(this.value_);
    }
    raw() {
      return this.value_;
    }
    toJSON() {
      return this.get();
    }
    toString() {
      let a = this.name_ + "[";
      return a + this.value_ + "]";
    }
    valueOf() {
      return La(this.get());
    }
  };
  h.prototype[Symbol.toPrimitive] = function() {
    return this.valueOf();
  };
  me = te("ObservableValue", h), Sn = { annotationType_: "override", make_: function(a, t) {
    return 0;
  }, extend_: function() {
    V(44, this.annotationType_);
    return false;
  } };
  var ka = (0, function(r, n, t, i) {
    if (t.get) return le.make_(r, n, t, i);
    if (t.set) {
      var e = t.set;
      oe(e) || (e = J(B(n), e, false, void 0));
      if (i === r.target_) return r.defineProperty_(n, { configurable: true, set: e }) == null ? 0 : 2;
      S(i, n, { configurable: true, set: e });
      return 2;
    }
    if (i !== r.target_ && "function" == typeof t.value) {
      if (Ma(t.value)) {
        var c = ne;
        this.options_ && this.options_.autoBind && (c = ea);
        return c.make_(r, n, t, i);
      }
      c = Ee;
      this.options_ && this.options_.autoBind && (c = Ye);
      return c.make_(r, n, t, i);
    }
    c = b;
    this.options_ && false === this.options_.deep && (c = De), "function" == typeof t.value && this.options_ && this.options_.autoBind && (e = t.value.bind(R(r)), t.value = e);
    return c.make_(r, n, t, i);
  });
  var Sa = (0, function(r, n, t, c) {
    if (t.get) return le.extend_(r, n, t, c);
    if (t.set) {
      var e = r.defineProperty_;
      return r.defineProperty_(n, { configurable: true, set: J(B(n), t.set, false, void 0) }, c);
    }
    "function" == typeof t.value && this.options_ && this.options_.autoBind && (e = t.value.bind(R(r)), t.value = e);
    var i = b;
    this.options_ && false === this.options_.deep && (i = De);
    return i.extend_(r, n, t, c);
  });
  Oa = kn = { annotationType_: "true", options_: void 0, make_: ka, extend_: Sa };
  var Aa = l.create(null);
  var O = class {
    constructor(r, i, o, b2) {
      this.target_ = r, i ? this.values_ = i : this.values_ = /* @__PURE__ */ new Map(), this.name_ = o + "", this.defaultAnnotation_ = Oa, b2 && (this.defaultAnnotation_ = b2), this.keysAtom_ = new D("ObservableObject.keys"), this.isPlainObject_ = X(this.target_);
    }
    materializeLazyComputed_(a) {
      if (!!this.lazyComputedKeys_) {
        var t = this.lazyComputedKeys_.get(a);
        if (t) return this.lazyComputedKeys_.delete(a), 0 == this.lazyComputedKeys_.size && (this.lazyComputedKeys_ = void 0), t = t(), this.values_.set(a, t), t;
      }
    }
    materializeLazyObservable_(a) {
      if (!!this.lazyObservableKeys_) {
        var t = this.lazyObservableKeys_.get(a);
        if (t) return this.lazyObservableKeys_.delete(a), 0 == this.lazyObservableKeys_.size && (this.lazyObservableKeys_ = void 0), t = t(), this.values_.set(a, t), t;
      }
    }
    getObservablePropValue_(a) {
      var t = this.values_.get(a) || this.materializeLazyComputed_(a) || this.materializeLazyObservable_(a);
      return t.get();
    }
    setObservablePropValue_(a, t) {
      var r = this.values_.get(a) || this.materializeLazyComputed_(a) || this.materializeLazyObservable_(a);
      if (N(r)) return r.set(t), true;
      if (M(this)) {
        var n = L(this, { type: "update", object: R(this), name: a, newValue: t });
        if (!n) return null;
        t = n.newValue;
      }
      t = r.prepareNewValue_(t);
      if (t !== s.UNCHANGED) {
        var i = I(this);
        n = null, i && (n = this.name_, n = { type: "update", observableKind: "object", debugObjectName: n, object: R(this), oldValue: r.value_, name: a, newValue: t }), r.setNewValue_(t), i && T(this, n);
      }
      return true;
    }
    get_(a) {
      s.trackingDerivation && !$.call(this.target_, a) && this.has_(a);
      return this.target_[a];
    }
    set_(a, t) {
      return tt(this, a, t, false);
    }
    has_(a) {
      if (!s.trackingDerivation) return true === a in this.target_;
      this.pendingKeys_ || (this.pendingKeys_ = /* @__PURE__ */ new Map());
      var t = this.pendingKeys_.get(a);
      t || (t = h, t = new t(true === a in this.target_, Oe, "ObservableObject.key?", false), this.pendingKeys_.set(a, t));
      return t.get();
    }
    extend_(a, b2, c, d2) {
      var t = c;
      true === t && (t = this.defaultAnnotation_);
      if (false === t) return this.defineProperty_(a, b2, d2);
      var e = t.extend_(this, a, b2, d2);
      if (e) {
      }
      return e;
    }
    notifyPropertyAddition_(a, t) {
      var n, r = I(this);
      r && (n = this.name_, t = { type: "add", observableKind: "object", debugObjectName: n, object: R(this), name: a, newValue: t }, r && T(this, t)), this.pendingKeys_ && (a = this.pendingKeys_.get(a), !a || a.set(true)), z(this.keysAtom_);
    }
    defineProperty_(a, t, r) {
      r = !!r;
      try {
        f();
        var se2 = this.delete_(a);
        if (!se2) return se2;
        if (M(this)) {
          var ue2 = L(this, { object: R(this), name: a, type: "add", newValue: t.value });
          if (!ue2) return null;
          t.value === ue2.newValue || (t = P({}, t), t.value = ue2.newValue);
        }
        if (r) {
          if (true !== Reflect.defineProperty(this.target_, a, t)) return false;
        } else S(this.target_, a, t);
        this.notifyPropertyAddition_(a, t.value);
      } finally {
        v();
      }
      return true;
    }
    defineObservableProperty_(a, b2, c, d2) {
      var Pe2 = b2;
      try {
        f();
        var Ne2 = this.delete_(a);
        if (!Ne2) return Ne2;
        if (M(this)) {
          var ze = L(this, { object: R(this), name: a, type: "add", newValue: Pe2 });
          if (!ze) return null;
          Pe2 = ze.newValue;
        }
        var Ke2 = at(a), Me2 = true;
        s.safeDescriptors && (Me2 = !!this.isPlainObject_);
        var Le2 = { configurable: Me2, enumerable: true, get: Ke2.get, set: Ke2.set };
        if (d2) {
          if (true !== Reflect.defineProperty(this.target_, a, Le2)) return false;
        } else S(this.target_, a, Le2);
        var Ie2 = "ObservableObject.key", Te2 = new h(Pe2, c, Ie2, false);
        this.values_.set(a, Te2), this.notifyPropertyAddition_(a, Te2.value_);
      } finally {
        v();
      }
      return true;
    }
    defineComputedProperty_(a, t, r) {
      r = !!r;
      try {
        f();
        var je2 = this.delete_(a);
        if (!je2) return je2;
        if (M(this)) {
          var ke = L(this, { object: R(this), name: a, type: "add", newValue: void 0 });
          if (!ke) return null;
        }
        t.name || (t.name = "ObservableObject.key");
        t.context = R(this);
        var Se2 = at(a), Ae2 = true;
        s.safeDescriptors && (Ae2 = !!this.isPlainObject_);
        var Ce2 = { configurable: Ae2, enumerable: false, get: Se2.get, set: Se2.set };
        if (r) {
          if (true !== Reflect.defineProperty(this.target_, a, Ce2)) return false;
        } else S(this.target_, a, Ce2);
        this.values_.set(a, new w(t));
        this.notifyPropertyAddition_(a, void 0);
      } finally {
        v();
      }
      return true;
    }
    delete_(a, t) {
      var r = !!t;
      if (!$.call(this.target_, a)) return true;
      if (M(this) && !L(this, { object: R(this), name: a, type: "remove" })) return null;
      try {
        f();
        var ra2 = I(this);
        t = false;
        var ca2, na2 = t, ia2 = this.values_.get(a);
        if (!ia2 && (ra2 || na2)) {
          var oa = l.getOwnPropertyDescriptor(this.target_, a);
          oa && (ca2 = oa.value);
        }
        if (r) {
          if (true !== Reflect.deleteProperty(this.target_, a)) return false;
        } else true === Reflect.deleteProperty(this.target_, a) || Rt("Cannot delete property '" + B(a) + "'");
        t = false;
        t && delete this.appliedAnnotations_[a], ia2 && (this.values_.delete(a), !me(ia2) || (ca2 = ia2.value_), Ga(ia2)), z(this.keysAtom_), this.pendingKeys_ && (oa = this.pendingKeys_.get(a), oa && (t = oa.set, r = oa, t.call(oa, true === a in this.target_)));
        if (ra2 || na2) {
          oa = { type: "remove", observableKind: "object", object: R(this), debugObjectName: this.name_, oldValue: ca2, name: a };
          if (a = false) {
          }
          ra2 && T(this, oa);
          if (false) {
          }
        }
      } finally {
        v();
      }
      return true;
    }
    ownKeys_() {
      k(this.keysAtom_);
      return Reflect.ownKeys(this.target_);
    }
    keys_() {
      k(this.keysAtom_);
      return l.keys(this.target_);
    }
  };
  _a = te("ObservableObjectAdministration", O);
  var fe = function(a, t) {
    if ($.call(a, u)) return a;
    var e;
    e = t && t.name ? t.name + "" : "ObservableObject", e = new O(a, /* @__PURE__ */ new Map(), e, Ht(t)), S(a, u, { enumerable: false, writable: true, configurable: true, value: e });
    return a;
  };
  var Z = { has: function(a, t) {
    return a[u].has_.call(a[u], t);
  } };
  Z.get = function(a, t) {
    return a[u].get_.call(a[u], t);
  }, Z.set = function(a, t, r) {
    if (!ia(t)) return false;
    var e = tt(a[u], t, r, true);
    return e == null ? true : !!e;
  }, Z.deleteProperty = function(a, t) {
    if (!ia(t)) return false;
    var e = a[u].delete_.call(a[u], t, true);
    return e == null ? true : !!e;
  }, Z.defineProperty = function(a, t, r) {
    var e = a[u].defineProperty_.call(a[u], t, r);
    return e == null ? true : !!e;
  }, Z.ownKeys = function(a) {
    return a[u].ownKeys_.call(a[u]);
  }, Z.preventExtensions = function(a) {
    p(13);
    return false;
  };
  var j = {};
  var Be = {};
  Be.get = function(a, t) {
    var e = a[u];
    return t === u ? e : "length" === t ? e.getArrayLength_() : "string" == typeof t && true !== isNaN(t) ? e.get_(parseInt(t)) : $.call(j, t) ? j[t] : a[t];
  }, Be.set = function(a, t, r) {
    var e = a[u];
    "length" === t && e.setArrayLength_(r);
    "symbol" == typeof t || true === isNaN(t) ? a[t] = r : e.set_(parseInt(t), r);
    return true;
  }, Be.preventExtensions = function() {
    p(15);
    return false;
  };
  var F = class {
    constructor(o = "ObservableArray", n, u2) {
      var r = o + "";
      this.owned_ = false, u2 !== void 0 && (this.owned_ = !!u2), this.atom_ = new D(r), this.values_ = [], this.interceptors_ = void 0, this.changeListeners_ = void 0, this.dehancer = void 0, this.proxy_ = void 0, this.lastKnownLength_ = 0;
      var e = "ObservableArray[..]";
      this.enhancer_ = function(r2, t, i) {
        return n(r2, t, e);
      };
    }
    dehanceValue_(a) {
      return this.dehancer !== void 0 ? this.dehancer(a) : a;
    }
    dehanceValues_(a) {
      return this.dehancer !== void 0 && a.length > 0 ? a.map(this.dehancer) : a;
    }
    getArrayLength_() {
      k(this.atom_);
      return this.values_.length;
    }
    setArrayLength_(a) {
      var t;
      ("number" != typeof a || true === Number.isNaN(a) || (a | 0) < 0) && V(40, a), a = a | 0, t = +this.values_.length;
      if (a != t) a > t ? (a = new Array(a - t | 0), this.spliceWithArray_(t, 0, a)) : this.spliceWithArray_(a, t - a | 0);
    }
    spliceWithArray_(a = 0, b2, c) {
      var r = +this.values_.length, n = a | 0, t = 1 == arguments.length ? r - n | 0 : b2 !== void 0 && b2 != null ? b2 | 0 : 0;
      r = void 0, r = c;
      return Ue(this, n, t, c);
    }
    get_(a) {
      k(this.atom_);
      return this.dehanceValue_(this.values_[a]);
    }
    set_(a, t) {
      a |= 0;
      var r = this.values_;
      if (a < r.length) {
        var n = r[a];
        if (M(this)) {
          var i = L(this, { type: "update", object: this.proxy_, index: a, newValue: t });
          if (!i) return;
          t = i.newValue;
        }
        t = this.enhancer_(t, n);
        t === n || (r[a] = t, Ft(this, a, t, n));
      } else {
        a++, a = new Array(a - r.length);
        var g2 = a.length - 1 | 0;
        a[g2] = t, Ue(this, r.length, 0, a);
      }
    }
  };
  var Ot = function(n, i, o, h2) {
    var e = "ObservableArray";
    o !== void 0 && (e = o + "");
    var a = false;
    a = !!h2;
    return se(function() {
      var C2 = new F(e, i, a);
      S(C2.values_, u, { enumerable: false, writable: false, configurable: true, value: C2 });
      var c = new Proxy(C2.values_, Be);
      C2.proxy_ = c, n && n.length > 0 && C2.spliceWithArray_(0, 0, n);
      return c;
    });
  };
  ga = te("ObservableArrayAdministration", F), j.clear = function() {
    return this.splice(0);
  }, j.replace = function(a) {
    let t = this[u], r = t.spliceWithArray_;
    return t.spliceWithArray_(0, +t.values_.length, a);
  }, j.toJSON = function() {
    return this.slice();
  }, j.splice = function(s2, g2, ...c) {
    var t = this[u];
    return 0 == arguments.length ? [] : 1 == arguments.length ? t.spliceWithArray_(s2) : 2 == arguments.length ? t.spliceWithArray_(s2, g2) : t.spliceWithArray_(s2, g2, c);
  }, j.spliceWithArray = function() {
    return this[u].spliceWithArray_.apply(this[u], arguments);
  }, j.push = function() {
    let t = this[u];
    Ue(t, +t.values_.length, 0, arguments);
    return t.values_.length;
  }, j.pop = function() {
    var a = this[u].values_.length - 1 | 0;
    a < 0 && (a = 0);
    return this.splice(a, 1)[0];
  }, j.shift = function() {
    return this.splice(0, 1)[0];
  }, j.unshift = function() {
    let t = this[u];
    Ue(t, 0, 0, arguments);
    return t.values_.length;
  }, j.reverse = function() {
    return !s.trackingDerivation || V(37, "reverse"), this.replace(this.slice().reverse()), this;
  }, j.sort = function() {
    !s.trackingDerivation || V(37, "sort");
    var t = this.slice();
    t.sort.apply(t, arguments), this.replace(t);
    return this;
  }, j.remove = function(a) {
    var t = this[u];
    a = +t.dehanceValues_(t.values_).indexOf(a);
    return a > -1 ? (this.splice(a, 1), true) : false;
  }, E("at"), E("concat"), E("flat"), E("includes"), E("indexOf"), E("join"), E("lastIndexOf"), E("slice"), E("toString"), E("toLocaleString"), E("toSorted"), E("toSpliced"), E("with"), G("every"), G("filter"), G("find"), G("findIndex"), G("findLast"), G("findLastIndex"), G("flatMap"), G("forEach"), G("map"), G("some"), G("toReversed"), rt("reduce"), rt("reduceRight");
  var jt = {};
  var d = class {
    constructor(a, t, r = "ObservableMap") {
      var e = this;
      e[u] = jt, e.enhancer_ = ie, t && (e.enhancer_ = t), t = r + "", e.name_ = t, e.interceptors_ = void 0, e.changeListeners_ = void 0, e.dehancer = void 0, se(function() {
        e.keysAtom_ = $e("ObservableMap.keys()"), e.data_ = /* @__PURE__ */ new Map(), e.hasMap_ = /* @__PURE__ */ new Map(), !a || e.merge(a);
      });
    }
    has_(a) {
      return !!this.data_.has(a);
    }
    has(a) {
      var e = this;
      if (!s.trackingDerivation) return e.has_(a);
      var t = e.hasMap_.get(a);
      t || (t = new h(e.has_(a), Oe, "ObservableMap.key?", false), e.hasMap_.set(a, t), t.onBUOL = /* @__PURE__ */ new Set(), t.onBUOL.add(function() {
        e.hasMap_.delete(a);
      }));
      return t.get();
    }
    set(a, t) {
      var n = !!this.data_.has(a);
      if (M(this)) {
        var r = n ? "update" : "add";
        r = L(this, { type: r, object: this, newValue: t, name: a });
        if (!r) return this;
        t = r.newValue;
      }
      n ? this.updateValue_(a, t) : this.addValue_(a, t);
      return this;
    }
    updateValue_(a, t) {
      var r = this.data_.get(a);
      t = r.prepareNewValue_(t);
      if (t !== s.UNCHANGED) {
        var i = I(this), n = null;
        i && (n = { observableKind: "map", debugObjectName: this.name_, type: "update", object: this, oldValue: r.value_, name: a, newValue: t }), r.setNewValue_(t), i && T(this, n);
      }
    }
    addValue_(a, t) {
      f();
      try {
        var J2 = "ObservableMap.key", $2 = new h(t, this.enhancer_, J2, false);
        this.data_.set(a, $2), t = $2.value_;
        var Z2 = this.hasMap_.get(a);
        !Z2 || Z2.setNewValue_(true), z(this.keysAtom_);
      } finally {
        v();
      }
      var r = false, i = I(this), n = null;
      i && (r = true), r && (n = { observableKind: "map", debugObjectName: this.name_, type: "add", object: this, name: a, newValue: t }), i && T(this, n);
    }
    delete(a) {
      if (M(this) && !L(this, { type: "delete", object: this, name: a })) return false;
      if (this.data_.has(a)) {
        var t = false, n = I(this), r = null;
        n && (t = true), t && (t = this.name_, r = { observableKind: "map", debugObjectName: t, type: "delete", object: this, oldValue: this.data_.get(a).value_, name: a }), f();
        try {
          z(this.keysAtom_);
          var ae2 = this.hasMap_.get(a);
          !ae2 || ae2.setNewValue_(false);
          var te2 = this.data_.get(a);
          te2.setNewValue_(void 0), this.data_.delete(a);
        } finally {
          v();
        }
        n && T(this, r);
        return true;
      }
      return false;
    }
    get(a) {
      return this.has(a) ? (a = this.data_.get(a), Fe(this, a.get())) : Fe(this, void 0);
    }
    getOrInsert(a, t) {
      this.has(a) || this.set(a, t);
      return this.get(a);
    }
    getOrInsertComputed(a, t) {
      this.has(a) || this.set(a, t(a));
      return this.get(a);
    }
    keys() {
      k(this.keysAtom_);
      return this.data_.keys();
    }
    values() {
      var e = this;
      let t = e.keys(), a = { next: function() {
        var d2 = t.next();
        return d2.done ? { done: true, value: void 0 } : { done: false, value: e.get(d2.value) };
      } };
      a[Symbol.toStringTag] = "MapIterator";
      return Me(a);
    }
    entries() {
      var e = this;
      let t = e.keys(), a = { next: function() {
        var p2 = t.next();
        if (p2.done) return { done: true, value: void 0 };
        var g2 = [p2.value, e.get(p2.value)];
        return { done: false, value: g2 };
      } };
      a[Symbol.toStringTag] = "MapIterator";
      return Me(a);
    }
    forEach(a, t) {
      var n = this.entries(), r = n.next();
      while (!r.done) {
        a.call(t, r.value[1], r.value[0], this);
        var i = n.next();
        r = i;
      }
    }
    merge(a) {
      var e = this;
      g(a) && (a = new Map(a)), f();
      try {
        if (X(a)) for (var he2 = Dt(a), pe2 = 0; ; pe2++) {
          if (pe2 >= he2.length) break;
          e.set(he2[pe2], a[he2[pe2]]);
        }
        else if (Array.isArray(a)) for (var ge2 = 0; ; ge2++) {
          if (ge2 >= a.length) break;
          e.set(a[ge2][0], a[ge2][1]);
        }
        else ve(a) ? (l.getPrototypeOf(l.getPrototypeOf(l.getPrototypeOf(a))) == null || V(19, a), a.forEach(function(i, n) {
          e.set(n, i);
        })) : a == null || V(20, a);
      } finally {
        v();
      }
      return e;
    }
    clear() {
      f();
      try {
        var A2 = Q();
        try {
          for (var C2 = x(this.keys()), R2 = 0; ; R2++) {
            if (R2 >= C2.length) break;
            this.delete(C2[R2]);
          }
        } finally {
          K(A2);
        }
      } finally {
        v();
      }
    }
    replace(a) {
      f();
      try {
        for (var fa2 = Xt(a), va2 = /* @__PURE__ */ new Map(), ha2 = false, pa2 = x(this.data_.keys()), ga2 = 0; ; ga2++) {
          a = ga2;
          if (a >= pa2.length) break;
          var _a2 = pa2[ga2];
          fa2.has(_a2) || (this.delete(_a2) ? ha2 = true : va2.set(_a2, this.data_.get(_a2)));
        }
        var ya2 = x(fa2.entries());
        for (ga2 = 0; ; ga2++) {
          a = ga2;
          if (a >= ya2.length) break;
          var ma2 = ya2[ga2][0], wa2 = ya2[ga2][1], Oa2 = !!this.data_.has(ma2);
          this.set(ma2, wa2), !this.data_.has(ma2) || (va2.set(ma2, this.data_.get(ma2)), Oa2 || (ha2 = true));
        }
        if (!ha2) {
          a = +this.data_.size;
          if (a != va2.size) z(this.keysAtom_);
          else {
            var ja2 = this.data_.keys(), ka2 = va2.keys(), Sa2 = ja2.next(), Aa2 = ka2.next();
            while (!Sa2.done) {
              if (Sa2.value !== Aa2.value) {
                z(this.keysAtom_);
                break;
              }
              Sa2 = ja2.next();
              Aa2 = ka2.next();
            }
          }
        }
        this.data_ = va2;
      } finally {
        v();
      }
      return this;
    }
    toJSON() {
      return x(this);
    }
    toString() {
      return "[object ObservableMap]";
    }
    get size() {
      k(this.keysAtom_);
      return this.data_.size;
    }
  };
  d.prototype[Symbol.iterator] = function() {
    return this.entries();
  };
  S(d.prototype, Symbol.toStringTag, { enumerable: false, configurable: true, get: function() {
    return "Map";
  } }), ha = te("ObservableMap", d);
  var kt = {};
  var _ = class {
    constructor(r, n, i) {
      var t = this;
      t[u] = kt;
      var e = "ObservableSet";
      i === void 0 || (e = i + ""), t.name_ = e;
      var a = ie;
      n && (a = n), t.enhancer_ = function(r2, n2, i2) {
        return a(r2, n2, e);
      }, t.data_ = /* @__PURE__ */ new Set(), t.changeListeners_ = void 0, t.interceptors_ = void 0, t.dehancer = void 0, se(function() {
        t.atom_ = $e(t.name_), !r || t.replace(r);
      });
    }
    has(a) {
      k(this.atom_);
      return !!this.data_.has(Fe(this, a));
    }
    add(a) {
      if (M(this)) {
        var t = L(this, { type: "add", object: this, newValue: a });
        if (!t) return this;
        a = t.newValue;
      }
      if (!this.has(a)) {
        f();
        try {
          this.data_.add(this.enhancer_(a, void 0)), z(this.atom_);
        } finally {
          v();
        }
        t = false;
        var n = I(this), r = null;
        n && (t = true), t && (r = { observableKind: "set", debugObjectName: this.name_, type: "add", object: this, newValue: a }), n && T(this, r);
      }
      return this;
    }
    delete(a) {
      if (M(this) && !L(this, { type: "delete", object: this, oldValue: a })) return false;
      if (this.has(a)) {
        var t = false, n = I(this), r = null;
        n && (t = true), t && (r = { observableKind: "set", debugObjectName: this.name_, type: "delete", object: this, oldValue: a }), f();
        try {
          z(this.atom_), this.data_.delete(a);
        } finally {
          v();
        }
        n && T(this, r);
        return true;
      }
      return false;
    }
    values() {
      var e = this;
      k(e.atom_);
      let t = e.data_.values(), a = { next: function() {
        var l2 = t.next();
        return l2.done ? { done: true, value: void 0 } : { done: false, value: Fe(e, l2.value) };
      } };
      a[Symbol.toStringTag] = "SetIterator";
      return Me(a);
    }
    keys() {
      return this.values();
    }
    entries() {
      let t = this.values(), a = { next: function() {
        var f2 = t.next();
        if (f2.done) return { done: true, value: void 0 };
        var a2 = [f2.value, f2.value];
        return { done: false, value: a2 };
      } };
      a[Symbol.toStringTag] = "SetIterator";
      return Me(a);
    }
    forEach(a, t) {
      var n = this.values(), r = n.next();
      while (!r.done) {
        a.call(t, r.value, r.value, this);
        var i = n.next();
        r = i;
      }
    }
    replace(a) {
      var e = this;
      y(a) && (a = new Set(a)), f();
      try {
        if (Array.isArray(a)) {
          e.clear();
          for (var z2 = 0; ; z2++) {
            var t = z2;
            if (t >= a.length) break;
            e.add(a[z2]);
          }
        } else he(a) ? (e.clear(), a.forEach(function(t2) {
          e.add(t2);
        })) : a == null || V(41, a);
      } finally {
        v();
      }
      return e;
    }
    clear() {
      f();
      try {
        var A2 = Q();
        try {
          for (var C2 = x(this.data_.values()), R2 = 0; ; R2++) {
            if (R2 >= C2.length) break;
            this.delete(C2[R2]);
          }
        } finally {
          K(A2);
        }
      } finally {
        v();
      }
    }
    toJSON() {
      return x(this);
    }
    toString() {
      return "[object ObservableSet]";
    }
    get size() {
      k(this.atom_);
      return this.data_.size;
    }
  };
  _.prototype[Symbol.iterator] = function() {
    return this.values();
  };
  S(_.prototype, Symbol.toStringTag, { enumerable: false, configurable: true, get: function() {
    return "Set";
  } }), ue("intersection"), ue("union"), ue("difference"), ue("symmetricDifference"), ue("isSubsetOf"), ue("isSupersetOf"), ue("isDisjointFrom"), pa = te("ObservableSet", _);
  var xa = (0, function(a, t, r) {
    return this.extend_(a, t, r, false) == null ? 0 : 1;
  });
  var St = (0, function(r, n, i, c) {
    var t = ie;
    this.options_ && this.options_.enhancer_ && (t = this.options_.enhancer_);
    return r.defineObservableProperty_(n, i.value, t, c);
  });
  var At = (0, function(t, n, r, i) {
    var a = P({}, this.options_);
    a.get = r.get, a.set = r.set;
    return t.defineComputedProperty_(n, a, i);
  });
  var xt = (0, function(t, r, n, i) {
    if (this.options_ && this.options_.bound) return this.extend_(t, r, n, false) == null ? 0 : 1;
    if (i === t.target_) return this.extend_(t, r, n, false) == null ? 0 : 2;
    if (oe(n.value)) return 1;
    S(i, r, nt(t, this, r, n, false));
    return 2;
  });
  var Ct = (0, function(t, n, c, d2) {
    return t.defineProperty_(n, nt(t, this, n, c, !!s.safeDescriptors), d2);
  });
  var Ca = function(a, t, r) {
    var n = r.name;
    pe(t) || (t = ne(t)), a.options_ && a.options_.bound && r.addInitializer(function() {
      let t2 = this[n].bind(this);
      t2.isMobXFlow = true, this[n] = t2;
    });
    return t;
  };
  var Ra = (0, function(t, r, n, i) {
    if (i === t.target_) return this.extend_(t, r, n, false) == null ? 0 : 2;
    if (this.options_ && this.options_.bound && (!$.call(t.target_, r) || !pe(t.target_[r])) && this.extend_(t, r, n, false) == null) return 0;
    if (pe(n.value)) return 1;
    S(i, r, it(t, n, false, false));
    return 2;
  });
  var Da = (0, function(t, b2, l2, g2) {
    var e = this.options_ && this.options_.bound;
    return t.defineProperty_(b2, it(t, l2, e, !!s.safeDescriptors), g2);
  });
  Pe = function(t, n, c, b2) {
    var e = c, a = b2, r = l.getOwnPropertyDescriptors(n);
    se(function() {
      for (var L2, M2, i = fe(t, a)[u], c2 = Reflect.ownKeys(r), n2 = 0; n2 < c2.length; n2++) L2 = c2[n2], M2 = e ? true === L2 in e ? e[L2] : true : true, i.extend_(L2, r[L2], M2);
    });
    return t;
  };
  var je = function(a, t, r) {
    if ("accessor" == r.kind + "") {
      var n = r.name;
      return { get: function() {
        var r2 = this[u] || la(this, a, n, t.get.call(this));
        return r2.getObservablePropValue_(n);
      }, set: function(r2) {
        var t2 = this[u] || la(this, a, n, r2);
        return t2.setObservablePropValue_(n, r2);
      }, init: function(r2) {
        la(this, a, n, r2);
        return r2;
      } };
    }
  };
  var aa = We("observable", void 0);
  kn = We("observable.ref", { enhancer_: Oe }), An = We("observable.shallow", { enhancer_: An }), Nn = We("observable.struct", { enhancer_: Nn }), b = P(function(a, t, r) {
    return Qt(a, t, r);
  }, aa), b.box = function(a, t) {
    let e = He(t);
    return new h(a, Ge(e), e.name, true, e.equals);
  }, b.array = function(a, t) {
    let e = He(t);
    return Ot(a, Ge(e), e.name);
  }, b.map = function(a, t) {
    let e = He(t);
    return new d(a, Ge(e), e.name);
  }, b.set = function(a, t) {
    let e = He(t);
    return new _(a, Ge(e), e.name);
  }, b.object = function(a, t, r) {
    return se(function() {
      var w2 = {};
      w2 = fe(w2, r);
      var j2 = w2[u];
      j2.proxy_ || (j2.proxy_ = new Proxy(w2, Z)), w2 = j2.proxy_;
      return Pe(w2, a, t);
    });
  }, De = U(kn, je);
  var zn = U(An, je);
  var Kn = U(aa, je);
  Nn = U(Nn, je);
  var ta = function(t, r, n) {
    var e, a = n.name, i = function(n2, i2) {
      var _2 = P({}, t.options_);
      _2.get = r, _2.context = n2, _2.name || (_2.name = "ObservableObject." + B(a));
      return new w(_2);
    };
    n.addInitializer(function() {
      var k2 = this, A2 = fe(k2)[u], x2 = A2.values_.get(a);
      N(x2) && x2.derivation !== r && A2.values_.delete(a), A2.lazyComputedKeys_ || (A2.lazyComputedKeys_ = /* @__PURE__ */ new Map()), A2.lazyComputedKeys_.set(a, function() {
        return i(k2, A2);
      });
    });
    return function() {
      var n2, E2 = this[u], c = E2.values_.get(a);
      return N(c) && c.derivation !== r ? (e = e || /* @__PURE__ */ new WeakMap(), n2 = e.get(this), n2 || (n2 = i(this, E2), e.set(this, n2)), n2.get()) : E2.getObservablePropValue_(a);
    };
  };
  var Ea = ba("computed", void 0);
  kn = ba("computed.struct", { equals: function(a, t) {
    return ge(a, t, -1, void 0, void 0);
  } }), le = P(function(a, t) {
    var e;
    if (t && "string" == typeof t.kind) return ta(Ea, a, t);
    if (X(a)) return U(ba("computed", a), ta);
    e = {}, X(t) && (e = P({}, t)), e.get = a, e.name || (e.name = a.name);
    return new w(e);
  }, Ea);
  var Mn = U(kn, ta);
  An = (0, function(a) {
    var t = a.name + "";
    "" == t && (t = "<unnamed action>");
    return $a(t, false, a, this, void 0);
  });
  var Ve = function(a, t, r) {
    var n = r.name, e = function(r2) {
      var t2 = B(n);
      a.options_ && a.options_.name && (t2 = a.options_.name + "");
      return J(t2, r2, a.options_ && a.options_.autoAction, void 0);
    };
    if ("field" == r.kind + "") return function(r2) {
      oe(r2) || (r2 = e(r2)), a.options_ && a.options_.bound && (r2 = r2.bind(this), r2.isMobxAction = true);
      return r2;
    };
    if ("method" == r.kind + "") return oe(t) || (t = e(t)), a.options_ && a.options_.bound && r.addInitializer(function() {
      let t2 = this[n].bind(this);
      t2.isMobxAction = true, this[n] = t2;
    }), t;
    t = a.annotationType_;
    var i = B(n), c = r.kind;
    e = [t, i, c], ce(43, e);
  };
  var Pa = Ae("action", void 0);
  kn = Ae("action.bound", { bound: true });
  var Ba = Ae("autoAction", { autoAction: true });
  var Ln = Ae("autoAction.bound", { autoAction: true, bound: true });
  de = P(function(t, r) {
    var a;
    if (r && "string" == typeof r.kind) return a = Pa, Ve(a, t, r);
    if ("function" == typeof t) return a = t.name + "", "" == a && (a = "<unnamed action>"), J(a, t, false, void 0);
    if ("function" == typeof r) return J(t + "", r, false, void 0);
    if (ia(t)) return a = "action", U(Ae(a, { name: t, autoAction: false }), Ve);
  }, Pa), Ee = P(function(t, r) {
    var a;
    if (r && "string" == typeof r.kind) return a = Ba, Ve(a, t, r);
    if ("function" == typeof t) return a = t.name + "", "" == a && (a = "<unnamed action>"), J(a, t, true, void 0);
    if ("function" == typeof r) return J(t + "", r, true, void 0);
    if (ia(t)) return a = "autoAction", U(Ae(a, { name: t, autoAction: true }), Ve);
  }, Ba);
  var qn = U(kn, Ve);
  Ye = U(Ln, Ve);
  var Y = class extends Error {
    constructor() {
      super(), this.message = "FLOW_CANCELLED", this.name = "FlowCancellationError";
    }
    toString() {
      return "Error: " + this.message;
    }
  };
  S(Y, "name", { value: "FlowCancellationError", configurable: true });
  Ln = function(a) {
    return Ka(Y, a);
  };
  var In = function(t, r) {
    var a;
    if (r && "string" == typeof r.kind) return Ca(ne, t, r);
    var e = t.name + "";
    "" == e && (e = "flow");
    a = (0, function() {
      var y2, w2, _2, x2 = e, k2 = de(x2, t).apply(this, arguments), me2 = { rejector: void 0, pending: void 0, stepId: 0 };
      y2 = function(c) {
        me2.pending = void 0;
        try {
          var L2 = e, I2 = de(L2, k2.next).call(k2, c);
          _2(I2);
        } catch (G2) {
          me2.rejector(G2);
        }
      }, w2 = function(c) {
        me2.pending = void 0;
        try {
          var L2 = e, I2 = de(L2, k2.throw).call(k2, c);
          _2(I2);
        } catch (G2) {
          me2.rejector(G2);
        }
      }, _2 = function(i) {
        if ("function" == typeof i.then) {
          i.then(_2, me2.rejector);
          return;
        }
        if (i.done) {
          me2.resolve(i.value);
          return;
        }
        me2.pending = Promise.resolve(i.value);
        me2.pending.then(y2, w2);
      };
      var A2 = new Promise(function(n, i) {
        me2.resolve = n, me2.rejector = i, y2(void 0);
      });
      x2 = e, A2.cancel = de(x2, function() {
        try {
          me2.pending && "function" == typeof me2.pending.cancel && me2.pending.cancel();
          var q2 = k2.return(void 0), I2 = Promise.resolve(q2.value);
          I2.then(be, be), "function" == typeof I2.cancel && I2.cancel(), me2.rejector(new Y());
        } catch (T2) {
          me2.rejector(T2);
        }
      });
      return A2;
    }), a.isMobXFlow = true;
    return a;
  };
  kn = { annotationType_: "flow", options_: void 0, make_: Ra, extend_: Da }, ne = P(In, kn), kn = { annotationType_: "flow.bound", options_: { bound: true }, make_: Ra, extend_: Da }, ea = U(kn, Ca), kn = function(a) {
    return a;
  }, In = function(a) {
    return pe(a);
  };
  var ra = function(r, n) {
    var t = va;
    n = n || t, t = n.name ? n.name + "" : "Autorun";
    var e, i = !n.scheduler && !n.delay, c = function() {
      r(e);
    };
    if (i) e = new A(t, function() {
      this.track(c);
    }, n.onError, n.requiresObservable);
    else {
      i = ct(n);
      var a = false;
      e = new A(t, function() {
        var $2 = this;
        a || (a = true, i(function() {
          a = false, $2.isDisposed || $2.track(c);
        }));
      }, n.onError, n.requiresObservable);
    }
    t = n.signal && n.signal.aborted;
    t || e.schedule_();
    return e.getDisposer_(n.signal);
  };
  var Tn = function(u2, b2, d2) {
    var e = va;
    d2 && (e = d2);
    var o = e.name ? e.name + "" : "Reaction", c = Qe;
    e.equals && (c = e.equals);
    var r, a, f2 = J(o, Jt(e.onError, b2), false, void 0), t = true, n = false, v2 = function() {
      var E2 = !!s.allowStateChanges;
      s.allowStateChanges = false;
      var R2;
      try {
        R2 = u2(a);
      } finally {
        s.allowStateChanges = E2;
      }
      n = t || true !== c(r, R2);
      r = R2;
    };
    b2 = !e.scheduler && !e.delay;
    var i = false, h2 = ct(e), g2 = function() {
      i = false;
      if (!a.isDisposed) {
        var s2 = r;
        a.track(v2), t && e.fireImmediately ? f2(r, s2, a) : !t && false, t = false;
      }
    };
    a = new A(o, function() {
      t || b2 ? g2() : i || (i = true, h2(g2));
    }, e.onError, e.requiresObservable), o = e.signal && e.signal.aborted, o || a.schedule_();
    return a.getDisposer_(e.signal);
  };
  var Gn = function(a, b2, c) {
    var e;
    return 1 == arguments.length || b2 && "object" == typeof b2 ? (e = void 0, e = b2, $t(a, b2)) : ot(a, b2, c);
  };
  var Hn = function(a) {
    true === a.isolateGlobalState && pt();
    if (a.enforceActions !== void 0) {
      var e = a.enforceActions;
      "always" === e ? (s.enforceActions = "always", s.allowStateChanges = false) : "observed" === e ? (s.enforceActions = true, s.allowStateChanges = false) : (s.enforceActions = false, s.allowStateChanges = true);
    }
    true === "computedRequiresReaction" in a && (s.computedRequiresReaction = !!a.computedRequiresReaction);
    true === "reactionRequiresObservable" in a && (s.reactionRequiresObservable = !!a.reactionRequiresObservable), true === "observableRequiresReaction" in a && (s.observableRequiresReaction = !!a.observableRequiresReaction), true === "disableErrorBoundaries" in a && (s.disableErrorBoundaries = !!a.disableErrorBoundaries), true === "safeDescriptors" in a && (s.safeDescriptors = !!a.safeDescriptors), s.allowStateReads = !s.observableRequiresReaction;
    if (a.reactionScheduler) {
      e = a.reactionScheduler;
      var t = Je;
      Je = function(te2) {
        return e(function() {
          return t(te2);
        });
      };
    }
  };
  var na = Symbol("mobx-keys");
  var Un = function(a, t, r) {
    se(function() {
      for (var n = fe(a, r)[u], C2 = Reflect.ownKeys(t), x2 = 0; x2 < C2.length; x2++) st(n, C2[x2], t[C2[x2]]);
    });
    return a;
  };
  var Fn = function(a, t, r) {
    if (X(a)) return Pe(a, a, t, r);
    se(function() {
      var m2 = fe(a, r)[u];
      if (true !== na in a) {
        for (var g2 = l.getPrototypeOf(a), ye2 = /* @__PURE__ */ new Set(), _2 = Reflect.ownKeys(a), y2 = Reflect.ownKeys(g2), v2 = 0; v2 < _2.length; v2++) ye2.add(_2[v2]);
        for (v2 = 0; v2 < y2.length; v2++) ye2.add(y2[v2]);
        ye2.delete("constructor"), ye2.delete(u), S(g2, na, { enumerable: false, writable: true, configurable: true, value: ye2 });
      }
      a[na].forEach(function(r2) {
        var u2;
        u2 = t && true === r2 in t ? t[r2] : true, st(m2, r2, u2);
      });
    });
    return a;
  };
  var Ne = function(a) {
    if (m(a)) return a[u].keys_.call(a[u]);
    var e, t;
    if (g(a) || y(a)) return x(a.keys());
    if (C(a)) {
      for (t = [], e = 0; e < a.length; e++) t.push(e);
      return t;
    }
    p(5);
  };
  var Wn = function(a) {
    if (m(a)) {
      for (var r = Ne(a), t = [], e = 0; e < r.length; e++) t.push(a[r[e]]);
      return t;
    }
    if (g(a)) {
      for (r = Ne(a), t = [], e = 0; e < r.length; e++) t.push(a.get(r[e]));
      return t;
    }
    if (y(a)) return x(a.values());
    if (C(a)) return a.slice();
    p(6);
  };
  var Xn = function(a) {
    var e, n, t, r;
    if (m(a) || g(a)) {
      for (r = Ne(a), n = [], e = 0; e < r.length; e++) t = [r[e]], g(a) ? t.push(a.get(r[e])) : t.push(a[r[e]]), n.push(t);
      return n;
    }
    if (y(a)) return x(a.entries());
    if (C(a)) {
      for (r = [], e = 0; e < a.length; e++) t = [e, a[e]], r.push(t);
      return r;
    }
    p(7);
  };
  var Va = function(b2, c, d2) {
    var a, t = c;
    if (2 == arguments.length && !y(b2)) {
      f();
      try {
        for (var je2 = l.keys(t), ke = 0; ; ke++) {
          a = ke;
          if (a >= je2.length) break;
          Va(b2, je2[ke], t[je2[ke]]);
        }
      } finally {
        v();
      }
      return;
    }
    m(b2) ? b2[u].set_.call(b2[u], t, d2) : g(b2) ? b2.set(t, d2) : y(b2) ? b2.add(t) : C(b2) ? (f(), a = t | 0, a >= b2.length && (b2.length = (t | 0) + 1 | 0), b2[t] = d2, v()) : p(8);
  };
  var Qn = function(a, t) {
    m(a) ? a[u].delete_.call(a[u], t) : g(a) || y(a) ? a.delete(t) : C(a) ? a.splice(t, 1) : p(9);
  };
  var Na = function(a, t) {
    if (m(a)) return a[u].has_.call(a[u], t);
    var e;
    if (g(a) || y(a)) return a.has(t);
    if (C(a)) return (t | 0) >= 0 ? (e = t | 0, e = e < a.length) : e = false, e;
    p(10);
    return false;
  };
  var Jn = function(a, t) {
    if (!!Na(a, t)) {
      if (m(a)) return a[u].get_.call(a[u], t);
      if (g(a)) return a.get(t);
      if (C(a)) return a[t];
      p(11);
    }
  };
  var za = function(a) {
    if (m(a)) return a[u].ownKeys_.call(a[u]);
    p(38);
  };
  var $n = function(a, t, r) {
    if (m(a)) return a[u].defineProperty_.call(a[u], t, r);
    p(39);
  };
  var ee = function(a, t) {
    var e, n, r;
    if ("object" == typeof a && a != null) {
      if (C(a)) return t === void 0 || p(23), a[u].atom_;
      if (y(a)) return a.atom_;
      if (g(a)) {
        if (t === void 0) return a.keysAtom_;
        e = a.data_.get(t) || a.hasMap_.get(t), e || (n = a.name_, ce(25, [t, n]));
        return e;
      }
      if (t && !a[u] && a[t] === void 0) {
      }
      if (m(a)) return t || p(26), r = a[u], e = r.values_.get(t) || r.materializeLazyComputed_(t) || r.materializeLazyObservable_(t), e || (n = r.name_, ce(27, [t, n])), e;
      if (ca(a) || N(a) || Le(a)) return a;
    } else {
      if ("function" == typeof a && Le(a[u])) return a[u];
    }
    V(28, a);
  };
  var W = function(a, t) {
    a || p(29);
    if (t !== void 0) return W(ee(a, t));
    if (ca(a) || N(a) || Le(a) || g(a) || y(a)) return a;
    if (a[u]) return a[u];
    V(24, a);
  };
  var Zn = function(a, t) {
    if (t !== void 0) var e = ee(a, t);
    else if (oe(a)) return a.name;
    else e = m(a) || g(a) || y(a) ? W(a) : ee(a);
    return e.name_;
  };
  var Yn = function(b2, c, d2, f2) {
    var e, t = b2;
    if (arguments.length > 2 && "function" == typeof d2) return e = f2, ut(W(t, c), d2, e);
    var n = arguments.length > 2 && d2;
    e = W(t);
    if (C(t)) {
      if (n) {
        var a = re.call(e.values_);
        t = e.proxy_, n = e.atom_.name_, c({ observableKind: "array", object: t, debugObjectName: n, type: "splice", index: 0, added: a, addedCount: a.length, removed: [], removedCount: 0 });
      }
      e = Se(e, c);
      return e;
    }
    if (g(t)) return e = Se(e, c), e;
    if (y(t)) return e = Se(e, c), e;
    if (m(t)) return e = Se(e, c), e;
    e = ut(e, c, n);
    return e;
  };
  var ei = function(t, c, n) {
    return arguments.length > 2 && "function" == typeof n ? Za(W(t, c), n) : Za(W(t), c);
  };
  var ai = function() {
    return bt("onBOL", arguments);
  };
  var ti = function() {
    return bt("onBUOL", arguments);
  };
  var ri = function(e) {
    return _e(e, /* @__PURE__ */ new Map());
  };
  var ni = function(a, t) {
    return lt(ee(a, t));
  };
  var ii = function(a, t) {
    return dt(ee(a, t));
  };
  var ci = function(a) {
    return N(a);
  };
  var oi = function(a, t) {
    if (!m(a)) return false;
    var e = a[u];
    return e.lazyComputedKeys_ && e.lazyComputedKeys_.has(t) ? true : !e.values_.has(t) ? false : N(e.values_.get(t));
  };
  var si = function(b2, c, d2) {
    var e, n;
    g(b2) || C(b2) || me(b2) || y(b2) ? (e = W(b2), n = c) : m(b2) && (e = W(b2, c), n = d2), e.dehancer = n;
    return function() {
      e.dehancer = void 0;
    };
  };
  return __toCommonJS(mobx_esm_exports);
})();
typeof module!=="undefined"&&module.exports&&(module.exports=mobx);
