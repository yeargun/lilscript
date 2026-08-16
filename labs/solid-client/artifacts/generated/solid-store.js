//#region node_modules/solid-js/dist/solid.js
var e = (e, t) => e === t, t = Symbol("solid-proxy"), n = Symbol("solid-track"), r = { equals: e }, i = null, a = w, o = 1, s = 2, c = null, l = null, u = null, d = null, f = null, p = 0;
function m(e, t) {
	t = t ? Object.assign({}, r, t) : r;
	let n = {
		value: e,
		observers: null,
		observerSlots: null,
		comparator: t.equals || void 0
	};
	return [v.bind(n), (e) => (typeof e == "function" && (e = l && l.running && l.sources.has(n) ? e(n.tValue) : e(n.value)), y(n, e))];
}
function h(e) {
	return C(e, !1);
}
function ee(e) {
	if (u === null) return e();
	let t = u;
	u = null;
	try {
		return e();
	} finally {
		u = t;
	}
}
function g() {
	return u;
}
var [te, _] = /*@__PURE__*/ m(!1);
function v() {
	let e = l && l.running;
	if (this.sources && (e ? this.tState : this.state)) {
		if ((e ? this.tState : this.state) === o) b(this);
		else {
			let e = d;
			d = null, C(() => T(this), !1), d = e;
		}
	}
	if (u) {
		let e = this.observers;
		if (!e || e[e.length - 1] !== u) {
			let t = e ? e.length : 0;
			u.sources ? (u.sources.push(this), u.sourceSlots.push(t)) : (u.sources = [this], u.sourceSlots = [t]), e ? (e.push(u), this.observerSlots.push(u.sources.length - 1)) : (this.observers = [u], this.observerSlots = [u.sources.length - 1]);
		}
	}
	return e && l.sources.has(this) ? this.tValue : this.value;
}
function y(e, t, n) {
	let r = l && l.running && l.sources.has(e) ? e.tValue : e.value;
	if (!e.comparator || !e.comparator(r, t)) {
		if (l) {
			let r = l.running;
			(r || !n && l.sources.has(e)) && (l.sources.add(e), e.tValue = t), r || (e.value = t);
		} else e.value = t;
		e.observers && e.observers.length && C(() => {
			for (let t = 0; t < e.observers.length; t += 1) {
				let n = e.observers[t], r = l && l.running;
				r && l.disposed.has(n) || ((r ? !n.tState : !n.state) && (n.pure ? d.push(n) : f.push(n), n.observers && E(n)), r ? n.tState = o : n.state = o);
			}
			if (d.length > 1e6) throw d = [], Error();
		}, !1);
	}
	return t;
}
function b(e) {
	if (!e.fn) return;
	D(e);
	let t = p;
	x(e, l && l.running && l.sources.has(e) ? e.tValue : e.value, t), l && !l.running && l.sources.has(e) && queueMicrotask(() => {
		C(() => {
			l && (l.running = !0), u = c = e, x(e, e.tValue, t), u = c = null;
		}, !1);
	});
}
function x(e, t, n) {
	let r, i = c, a = u;
	u = c = e;
	try {
		r = e.fn(t);
	} catch (t) {
		return e.pure && (l && l.running ? (e.tState = o, e.tOwned && e.tOwned.forEach(D), e.tOwned = void 0) : (e.state = o, e.owned && e.owned.forEach(D), e.owned = null)), e.updatedAt = n + 1, A(t);
	} finally {
		u = a, c = i;
	}
	(!e.updatedAt || e.updatedAt <= n) && (e.updatedAt != null && "observers" in e ? y(e, r, !0) : l && l.running && e.pure ? (l.sources.has(e) || (e.value = r), l.sources.add(e), e.tValue = r) : e.value = r, e.updatedAt = n);
}
function S(e) {
	let t = l && l.running;
	if ((t ? e.tState : e.state) === 0) return;
	if ((t ? e.tState : e.state) === s) return T(e);
	if (e.suspense && ee(e.suspense.inFallback)) return e.suspense.effects.push(e);
	let n = [e];
	for (; (e = e.owner) && (!e.updatedAt || e.updatedAt < p);) {
		if (t && l.disposed.has(e)) return;
		(t ? e.tState : e.state) && n.push(e);
	}
	for (let r = n.length - 1; r >= 0; r--) {
		if (e = n[r], t) {
			let t = e, i = n[r + 1];
			for (; (t = t.owner) && t !== i;) if (l.disposed.has(t)) return;
		}
		if ((t ? e.tState : e.state) === o) b(e);
		else if ((t ? e.tState : e.state) === s) {
			let t = d;
			d = null, C(() => T(e, n[0]), !1), d = t;
		}
	}
}
function C(e, t) {
	if (d) return e();
	let n = !1;
	t || (d = []), f ? n = !0 : f = [], p++;
	try {
		let t = e();
		return ne(n), t;
	} catch (e) {
		n || (f = null), d = null, A(e);
	}
}
function ne(e) {
	if (d &&= (w(d), null), e) return;
	let t;
	if (l) {
		if (!l.promises.size && !l.queue.size) {
			let e = l.sources, n = l.disposed;
			f.push.apply(f, l.effects), t = l.resolve;
			for (let e of f) "tState" in e && (e.state = e.tState), delete e.tState;
			l = null, C(() => {
				for (let e of n) D(e);
				for (let t of e) {
					if (t.value = t.tValue, t.owned) for (let e = 0, n = t.owned.length; e < n; e++) D(t.owned[e]);
					t.tOwned && (t.owned = t.tOwned), delete t.tValue, delete t.tOwned, t.tState = 0;
				}
				_(!1);
			}, !1);
		} else if (l.running) {
			l.running = !1, l.effects.push.apply(l.effects, f), f = null, _(!0);
			return;
		}
	}
	let n = f;
	f = null, n.length && C(() => a(n), !1), t && t();
}
function w(e) {
	for (let t = 0; t < e.length; t++) S(e[t]);
}
function T(e, t) {
	let n = l && l.running;
	n ? e.tState = 0 : e.state = 0;
	for (let r = 0; r < e.sources.length; r += 1) {
		let i = e.sources[r];
		if (i.sources) {
			let e = n ? i.tState : i.state;
			e === o ? i !== t && (!i.updatedAt || i.updatedAt < p) && S(i) : e === s && T(i, t);
		}
	}
}
function E(e) {
	let t = l && l.running;
	for (let n = 0; n < e.observers.length; n += 1) {
		let r = e.observers[n];
		(t ? !r.tState : !r.state) && (t ? r.tState = s : r.state = s, r.pure ? d.push(r) : f.push(r), r.observers && E(r));
	}
}
function D(e) {
	let t;
	if (e.sources) for (; e.sources.length;) {
		let t = e.sources.pop(), n = e.sourceSlots.pop(), r = t.observers;
		if (r && r.length) {
			let e = r.pop(), i = t.observerSlots.pop();
			n < r.length && (e.sourceSlots[i] = n, r[n] = e, t.observerSlots[n] = i);
		}
	}
	if (e.tOwned) {
		for (t = e.tOwned.length - 1; t >= 0; t--) D(e.tOwned[t]);
		delete e.tOwned;
	}
	if (l && l.running && e.pure) O(e, !0);
	else if (e.owned) {
		for (t = e.owned.length - 1; t >= 0; t--) D(e.owned[t]);
		e.owned = null;
	}
	if (e.cleanups) {
		for (t = e.cleanups.length - 1; t >= 0; t--) e.cleanups[t]();
		e.cleanups = null;
	}
	l && l.running ? e.tState = 0 : e.state = 0;
}
function O(e, t) {
	if (t || (e.tState = 0, l.disposed.add(e)), e.owned) for (let t = 0; t < e.owned.length; t++) O(e.owned[t]);
}
function re(e) {
	return e instanceof Error ? e : Error(typeof e == "string" ? e : "Unknown error", { cause: e });
}
function k(e, t, n) {
	try {
		for (let n of t) n(e);
	} catch (e) {
		A(e, n && n.owner || null);
	}
}
function A(e, t = c) {
	let n = i && t && t.context && t.context[i], r = re(e);
	if (!n) throw r;
	f ? f.push({
		fn() {
			k(r, n, t);
		},
		state: o
	}) : k(r, n, t);
}
//#endregion
//#region node_modules/solid-js/store/dist/store.js
var j = Symbol("store-raw"), M = Symbol("store-node"), N = Symbol("store-has"), P = Symbol("store-self");
function F(e) {
	let n = e[t];
	if (!n && (Object.defineProperty(e, t, { value: n = new Proxy(e, H) }), !Array.isArray(e))) {
		let t = Object.keys(e), r = Object.getOwnPropertyDescriptors(e), i = Object.getPrototypeOf(e), a = i !== null && typeof e == "object" && !!e && !Array.isArray(e) && i !== Object.prototype;
		if (a) {
			let e = Object.getOwnPropertyDescriptors(i);
			t.push(...Object.keys(e)), Object.assign(r, e);
		}
		for (let i = 0, o = t.length; i < o; i++) {
			let o = t[i];
			a && o === "constructor" || r[o].get && Object.defineProperty(e, o, {
				configurable: !0,
				enumerable: r[o].enumerable,
				get: r[o].get.bind(n)
			});
		}
	}
	return n;
}
function I(e) {
	let n;
	return typeof e == "object" && !!e && (e[t] || !(n = Object.getPrototypeOf(e)) || n === Object.prototype || Array.isArray(e));
}
function L(e, t = /* @__PURE__ */ new Set()) {
	let n, r, i, a;
	if (n = e != null && e[j]) return n;
	if (!I(e) || t.has(e)) return e;
	if (Array.isArray(e)) {
		Object.isFrozen(e) ? e = e.slice(0) : t.add(e);
		for (let n = 0, a = e.length; n < a; n++) i = e[n], (r = L(i, t)) !== i && (e[n] = r);
	} else {
		Object.isFrozen(e) ? e = Object.assign({}, e) : t.add(e);
		let n = Object.keys(e), o = Object.getOwnPropertyDescriptors(e);
		for (let s = 0, c = n.length; s < c; s++) a = n[s], !o[a].get && (i = e[a], (r = L(i, t)) !== i && (e[a] = r));
	}
	return e;
}
function R(e, t) {
	let n = e[t];
	return n || Object.defineProperty(e, t, { value: n = Object.create(null) }), n;
}
function z(e, t, n) {
	if (e[t]) return e[t];
	let [r, i] = m(n, {
		equals: !1,
		internal: !0
	});
	return r.$ = i, e[t] = r;
}
function ie(e, n) {
	let r = Reflect.getOwnPropertyDescriptor(e, n);
	return !r || r.get || !r.configurable || n === t || n === M ? r : (delete r.value, delete r.writable, r.get = () => e[t][n], r);
}
function B(e) {
	g() && z(R(e, M), P)();
}
function V(e) {
	return B(e), Reflect.ownKeys(e);
}
var H = {
	get(e, r, i) {
		if (r === j) return e;
		if (r === t) return i;
		if (r === n) return B(e), i;
		let a = R(e, M), o = a[r], s = o ? o() : e[r];
		if (r === M || r === N || r === "__proto__") return s;
		if (!o) {
			let t = Object.getOwnPropertyDescriptor(e, r);
			g() && (typeof s != "function" || e.hasOwnProperty(r)) && !(t && t.get) && (s = z(a, r, s)());
		}
		return I(s) ? F(s) : s;
	},
	has(e, r) {
		return r === j || r === t || r === n || r === M || r === N || r === "__proto__" || (g() && z(R(e, N), r)(), r in e);
	},
	set() {
		return !0;
	},
	deleteProperty() {
		return !0;
	},
	ownKeys: V,
	getOwnPropertyDescriptor: ie
};
function U(e, t, n, r = !1) {
	if (t === "__proto__" || !r && e[t] === n) return;
	let i = e[t], a = e.length;
	n === void 0 ? (delete e[t], e[N] && e[N][t] && i !== void 0 && e[N][t].$()) : (e[t] = n, e[N] && e[N][t] && i === void 0 && e[N][t].$());
	let o = R(e, M), s;
	if ((s = z(o, t, i)) && s.$(() => n), Array.isArray(e) && e.length !== a) {
		for (let t = e.length; t < a; t++) (s = o[t]) && s.$();
		(s = z(o, "length", a)) && s.$(e.length);
	}
	(s = o[P]) && s.$();
}
function W(e, t) {
	let n = Object.keys(t);
	for (let r = 0; r < n.length; r += 1) {
		let i = n[r];
		G(i) || U(e, i, t[i]);
	}
}
function G(e) {
	return e === "__proto__" || e === "constructor" || e === "prototype";
}
function ae(e, t) {
	if (typeof t == "function" && (t = t(e)), t = L(t), Array.isArray(t)) {
		if (e === t) return;
		let n = 0, r = t.length;
		for (; n < r; n++) {
			let r = t[n];
			e[n] !== r && U(e, n, r);
		}
		U(e, "length", r);
	} else W(e, t);
}
function K(e, t, n = []) {
	let r, i = e;
	if (t.length > 1) {
		r = t.shift();
		let a = typeof r, o = Array.isArray(e);
		if (a === "string" && (r === "__proto__" || t.length > 1 && G(r))) return;
		if (Array.isArray(r)) {
			for (let i = 0; i < r.length; i++) K(e, [r[i]].concat(t), n);
			return;
		}
		if (o && a === "function") {
			for (let i = 0; i < e.length; i++) r(e[i], i) && K(e, [i].concat(t), n);
			return;
		}
		if (o && a === "object") {
			let { from: i = 0, to: a = e.length - 1, by: o = 1 } = r;
			for (let r = i; r <= a; r += o) K(e, [r].concat(t), n);
			return;
		}
		if (t.length > 1) {
			K(e[r], t, [r].concat(n));
			return;
		}
		i = e[r], n = [r].concat(n);
	}
	let a = t[0];
	typeof a == "function" && (a = a(i, n), a === i) || (r !== void 0 || a != null) && (a = L(a), r === void 0 || I(i) && I(a) && !Array.isArray(a) ? W(i, a) : U(e, r, a));
}
function q(...[e, t]) {
	let n = L(e || {}), r = Array.isArray(n), i = F(n);
	function a(...e) {
		h(() => {
			r && e.length === 1 ? ae(n, e[0]) : K(n, e);
		});
	}
	return [i, a];
}
function oe(e, n) {
	let r = Reflect.getOwnPropertyDescriptor(e, n);
	return !r || r.get || r.set || !r.configurable || n === t || n === M ? r : (delete r.value, delete r.writable, r.get = () => e[t][n], r.set = (r) => e[t][n] = r, r);
}
var se = {
	get(e, r, i) {
		if (r === j) return e;
		if (r === t) return i;
		if (r === n) return B(e), i;
		let a = R(e, M), o = a[r], s = o ? o() : e[r];
		if (r === M || r === N || r === "__proto__") return s;
		if (!o) {
			let t = Object.getOwnPropertyDescriptor(e, r), n = typeof s == "function";
			if (g() && (!n || e.hasOwnProperty(r)) && !(t && t.get)) s = z(a, r, s)();
			else if (s != null && n && s === Array.prototype[r]) return (...e) => h(() => Array.prototype[r].apply(i, e));
		}
		return I(s) ? J(s) : s;
	},
	has(e, r) {
		return r === j || r === t || r === n || r === M || r === N || r === "__proto__" || (g() && z(R(e, N), r)(), r in e);
	},
	set(e, t, n) {
		return h(() => U(e, t, L(n))), !0;
	},
	deleteProperty(e, t) {
		return h(() => U(e, t, void 0, !0)), !0;
	},
	ownKeys: V,
	getOwnPropertyDescriptor: oe
};
function J(e) {
	let n = e[t];
	if (!n) {
		Object.defineProperty(e, t, { value: n = new Proxy(e, se) });
		let r = Object.keys(e), i = Object.getOwnPropertyDescriptors(e), a = Object.getPrototypeOf(e), o = a !== null && typeof e == "object" && !!e && !Array.isArray(e) && a !== Object.prototype;
		if (o) {
			let e = a;
			for (; e != null;) {
				let t = Object.getOwnPropertyDescriptors(e);
				r.push(...Object.keys(t)), Object.assign(i, t), e = Object.getPrototypeOf(e);
			}
		}
		for (let t = 0, a = r.length; t < a; t++) {
			let a = r[t];
			if (!(o && a === "constructor")) {
				if (i[a].get) {
					let t = i[a].get.bind(n);
					Object.defineProperty(e, a, {
						get: t,
						configurable: !0
					});
				}
				if (i[a].set) {
					let t = i[a].set;
					Object.defineProperty(e, a, {
						set: (e) => h(() => t.call(n, e)),
						configurable: !0
					});
				}
			}
		}
	}
	return n;
}
function ce(e, t) {
	return J(L(e || {}));
}
function le(e, t) {
	h(() => t(L(e)));
}
var Y = Symbol("store-root");
function X(e) {
	return e === "__proto__" || e === "constructor" || e === "prototype";
}
function Z(e, t, n, r, i) {
	if (X(n)) return;
	let a = t[n];
	if (e === a) return;
	let o = Array.isArray(e);
	if (n !== Y && (!I(e) || !I(a) || o !== Array.isArray(a) || i && e[i] !== a[i])) {
		U(t, n, e);
		return;
	}
	if (o) {
		if (e.length && a.length && (!r || i && e[0] && e[0][i] != null)) {
			let t, n, o, s, c, l, u, d;
			for (o = 0, s = Math.min(a.length, e.length); o < s && (a[o] === e[o] || i && a[o] && e[o] && a[o][i] && a[o][i] === e[o][i]); o++) Z(e[o], a, o, r, i);
			let f = Array(e.length), p = /* @__PURE__ */ new Map();
			for (s = a.length - 1, c = e.length - 1; s >= o && c >= o && (a[s] === e[c] || i && a[s] && e[c] && a[s][i] && a[s][i] === e[c][i]); s--, c--) f[c] = a[s];
			if (o > c || o > s) {
				for (n = o; n <= c; n++) U(a, n, e[n]);
				for (; n < e.length; n++) U(a, n, f[n]), Z(e[n], a, n, r, i);
				a.length > e.length && U(a, "length", e.length);
				return;
			}
			for (u = Array(c + 1), n = c; n >= o; n--) l = e[n], d = i && l ? l[i] : l, t = p.get(d), u[n] = t === void 0 ? -1 : t, p.set(d, n);
			for (t = o; t <= s; t++) l = a[t], d = i && l ? l[i] : l, n = p.get(d), n !== void 0 && n !== -1 && (f[n] = a[t], n = u[n], p.set(d, n));
			for (n = o; n < e.length; n++) n in f ? (U(a, n, f[n]), Z(e[n], a, n, r, i)) : U(a, n, e[n]);
		} else for (let t = 0, n = e.length; t < n; t++) Z(e[t], a, t, r, i);
		a.length > e.length && U(a, "length", e.length);
		return;
	}
	let s = Object.keys(e);
	for (let t = 0, n = s.length; t < n; t++) X(s[t]) || Z(e[s[t]], a, s[t], r, i);
	let c = Object.keys(a);
	for (let t = 0, n = c.length; t < n; t++) e[c[t]] === void 0 && U(a, c[t], void 0);
}
function ue(e, t = {}) {
	let { merge: n, key: r = "id" } = t, i = L(e);
	return (e) => {
		if (!I(e) || !I(i)) return i;
		let t = Z(i, { [Y]: e }, Y, n, r);
		return t === void 0 ? e : t;
	};
}
var Q = /* @__PURE__ */ new WeakMap(), $ = {
	get(e, r) {
		if (r === j) return e;
		let i = e[r];
		if (r === t || r === n || r === M || r === N || r === "__proto__") return i;
		let a;
		return I(i) ? Q.get(i) || (Q.set(i, a = new Proxy(i, $)), a) : i;
	},
	set(e, t, n) {
		return U(e, t, L(n)), !0;
	},
	deleteProperty(e, t) {
		return U(e, t, void 0, !0), !0;
	}
};
function de(e) {
	return (t) => {
		if (I(t)) {
			let n;
			(n = Q.get(t)) || Q.set(t, n = new Proxy(t, $)), e(n);
		}
		return t;
	};
}
var fe = void 0;
//#endregion
export { j as $RAW, fe as DEV, ce as createMutable, q as createStore, le as modifyMutable, de as produce, ue as reconcile, L as unwrap };
