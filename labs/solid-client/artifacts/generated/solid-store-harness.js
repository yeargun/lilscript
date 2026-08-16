//#region \0rolldown/runtime.js
var e = Object.defineProperty, t = (t, n) => {
	let r = {};
	for (var i in t) e(r, i, {
		get: t[i],
		enumerable: !0
	});
	return n || e(r, Symbol.toStringTag, { value: "Module" }), r;
}, n = {
	context: void 0,
	registry: void 0,
	effects: void 0,
	done: !1,
	getContextId() {
		return r(this.context.count);
	},
	getNextContextId() {
		return r(this.context.count++);
	}
};
function r(e) {
	let t = String(e), r = t.length - 1;
	return n.context.id + (r ? String.fromCharCode(96 + r) : "") + t;
}
function i(e) {
	n.context = e;
}
var a = (e, t) => e === t, o = Symbol("solid-proxy"), s = Symbol("solid-track"), c = { equals: a }, l = null, u = A, d = 1, f = 2, p = {
	owned: null,
	cleanups: null,
	context: null,
	owner: null
}, m = null, h = null, g = null, _ = null, v = null, y = 0;
function ee(e, t) {
	let n = g, r = m, i = e.length === 0, a = t === void 0 ? r : t, o = i ? p : {
		owned: null,
		cleanups: null,
		context: a ? a.context : null,
		owner: a
	}, s = i ? e : () => e(() => S(() => N(o)));
	m = o, g = null;
	try {
		return k(s, !0);
	} finally {
		g = n, m = r;
	}
}
function b(e, t) {
	t = t ? Object.assign({}, c, t) : c;
	let n = {
		value: e,
		observers: null,
		observerSlots: null,
		comparator: t.equals || void 0
	};
	return [ie.bind(n), (e) => (typeof e == "function" && (e = h && h.running && h.sources.has(n) ? e(n.tValue) : e(n.value)), E(n, e))];
}
function te(e, t, n) {
	u = ce;
	let r = oe(e, t, !1, d), i = T && re(T);
	i && (r.suspense = i), (!n || !n.render) && (r.user = !0), v ? v.push(r) : D(r);
}
function x(e) {
	return k(e, !1);
}
function S(e) {
	if (g === null) return e();
	let t = g;
	g = null;
	try {
		return e();
	} finally {
		g = t;
	}
}
function C() {
	return g;
}
var [ne, w] = /*@__PURE__*/ b(!1);
function re(e) {
	let t;
	return m && m.context && (t = m.context[e.id]) !== void 0 ? t : e.defaultValue;
}
var T;
function ie() {
	let e = h && h.running;
	if (this.sources && (e ? this.tState : this.state)) {
		if ((e ? this.tState : this.state) === d) D(this);
		else {
			let e = _;
			_ = null, k(() => j(this), !1), _ = e;
		}
	}
	if (g) {
		let e = this.observers;
		if (!e || e[e.length - 1] !== g) {
			let t = e ? e.length : 0;
			g.sources ? (g.sources.push(this), g.sourceSlots.push(t)) : (g.sources = [this], g.sourceSlots = [t]), e ? (e.push(g), this.observerSlots.push(g.sources.length - 1)) : (this.observers = [g], this.observerSlots = [g.sources.length - 1]);
		}
	}
	return e && h.sources.has(this) ? this.tValue : this.value;
}
function E(e, t, n) {
	let r = h && h.running && h.sources.has(e) ? e.tValue : e.value;
	if (!e.comparator || !e.comparator(r, t)) {
		if (h) {
			let r = h.running;
			(r || !n && h.sources.has(e)) && (h.sources.add(e), e.tValue = t), r || (e.value = t);
		} else e.value = t;
		e.observers && e.observers.length && k(() => {
			for (let t = 0; t < e.observers.length; t += 1) {
				let n = e.observers[t], r = h && h.running;
				r && h.disposed.has(n) || ((r ? !n.tState : !n.state) && (n.pure ? _.push(n) : v.push(n), n.observers && M(n)), r ? n.tState = d : n.state = d);
			}
			if (_.length > 1e6) throw _ = [], Error();
		}, !1);
	}
	return t;
}
function D(e) {
	if (!e.fn) return;
	N(e);
	let t = y;
	ae(e, h && h.running && h.sources.has(e) ? e.tValue : e.value, t), h && !h.running && h.sources.has(e) && queueMicrotask(() => {
		k(() => {
			h && (h.running = !0), g = m = e, ae(e, e.tValue, t), g = m = null;
		}, !1);
	});
}
function ae(e, t, n) {
	let r, i = m, a = g;
	g = m = e;
	try {
		r = e.fn(t);
	} catch (t) {
		return e.pure && (h && h.running ? (e.tState = d, e.tOwned && e.tOwned.forEach(N), e.tOwned = void 0) : (e.state = d, e.owned && e.owned.forEach(N), e.owned = null)), e.updatedAt = n + 1, I(t);
	} finally {
		g = a, m = i;
	}
	(!e.updatedAt || e.updatedAt <= n) && (e.updatedAt != null && "observers" in e ? E(e, r, !0) : h && h.running && e.pure ? (h.sources.has(e) || (e.value = r), h.sources.add(e), e.tValue = r) : e.value = r, e.updatedAt = n);
}
function oe(e, t, n, r = d, i) {
	let a = {
		fn: e,
		state: r,
		updatedAt: null,
		owned: null,
		sources: null,
		sourceSlots: null,
		cleanups: null,
		value: t,
		owner: m,
		context: m ? m.context : null,
		pure: n
	};
	return h && h.running && (a.state = 0, a.tState = r), m === null || m !== p && (h && h.running && m.pure ? m.tOwned ? m.tOwned.push(a) : m.tOwned = [a] : m.owned ? m.owned.push(a) : m.owned = [a]), a;
}
function O(e) {
	let t = h && h.running;
	if ((t ? e.tState : e.state) === 0) return;
	if ((t ? e.tState : e.state) === f) return j(e);
	if (e.suspense && S(e.suspense.inFallback)) return e.suspense.effects.push(e);
	let n = [e];
	for (; (e = e.owner) && (!e.updatedAt || e.updatedAt < y);) {
		if (t && h.disposed.has(e)) return;
		(t ? e.tState : e.state) && n.push(e);
	}
	for (let r = n.length - 1; r >= 0; r--) {
		if (e = n[r], t) {
			let t = e, i = n[r + 1];
			for (; (t = t.owner) && t !== i;) if (h.disposed.has(t)) return;
		}
		if ((t ? e.tState : e.state) === d) D(e);
		else if ((t ? e.tState : e.state) === f) {
			let t = _;
			_ = null, k(() => j(e, n[0]), !1), _ = t;
		}
	}
}
function k(e, t) {
	if (_) return e();
	let n = !1;
	t || (_ = []), v ? n = !0 : v = [], y++;
	try {
		let t = e();
		return se(n), t;
	} catch (e) {
		n || (v = null), _ = null, I(e);
	}
}
function se(e) {
	if (_ &&= (A(_), null), e) return;
	let t;
	if (h) {
		if (!h.promises.size && !h.queue.size) {
			let e = h.sources, n = h.disposed;
			v.push.apply(v, h.effects), t = h.resolve;
			for (let e of v) "tState" in e && (e.state = e.tState), delete e.tState;
			h = null, k(() => {
				for (let e of n) N(e);
				for (let t of e) {
					if (t.value = t.tValue, t.owned) for (let e = 0, n = t.owned.length; e < n; e++) N(t.owned[e]);
					t.tOwned && (t.owned = t.tOwned), delete t.tValue, delete t.tOwned, t.tState = 0;
				}
				w(!1);
			}, !1);
		} else if (h.running) {
			h.running = !1, h.effects.push.apply(h.effects, v), v = null, w(!0);
			return;
		}
	}
	let n = v;
	v = null, n.length && k(() => u(n), !1), t && t();
}
function A(e) {
	for (let t = 0; t < e.length; t++) O(e[t]);
}
function ce(e) {
	let t, r = 0;
	for (t = 0; t < e.length; t++) {
		let n = e[t];
		n.user ? e[r++] = n : O(n);
	}
	if (n.context) {
		if (n.count) {
			n.effects ||= [], n.effects.push(...e.slice(0, r));
			return;
		}
		i();
	}
	for (n.effects && (n.done || !n.count) && (e = [...n.effects, ...e], r += n.effects.length, delete n.effects), t = 0; t < r; t++) O(e[t]);
}
function j(e, t) {
	let n = h && h.running;
	n ? e.tState = 0 : e.state = 0;
	for (let r = 0; r < e.sources.length; r += 1) {
		let i = e.sources[r];
		if (i.sources) {
			let e = n ? i.tState : i.state;
			e === d ? i !== t && (!i.updatedAt || i.updatedAt < y) && O(i) : e === f && j(i, t);
		}
	}
}
function M(e) {
	let t = h && h.running;
	for (let n = 0; n < e.observers.length; n += 1) {
		let r = e.observers[n];
		(t ? !r.tState : !r.state) && (t ? r.tState = f : r.state = f, r.pure ? _.push(r) : v.push(r), r.observers && M(r));
	}
}
function N(e) {
	let t;
	if (e.sources) for (; e.sources.length;) {
		let t = e.sources.pop(), n = e.sourceSlots.pop(), r = t.observers;
		if (r && r.length) {
			let e = r.pop(), i = t.observerSlots.pop();
			n < r.length && (e.sourceSlots[i] = n, r[n] = e, t.observerSlots[n] = i);
		}
	}
	if (e.tOwned) {
		for (t = e.tOwned.length - 1; t >= 0; t--) N(e.tOwned[t]);
		delete e.tOwned;
	}
	if (h && h.running && e.pure) P(e, !0);
	else if (e.owned) {
		for (t = e.owned.length - 1; t >= 0; t--) N(e.owned[t]);
		e.owned = null;
	}
	if (e.cleanups) {
		for (t = e.cleanups.length - 1; t >= 0; t--) e.cleanups[t]();
		e.cleanups = null;
	}
	h && h.running ? e.tState = 0 : e.state = 0;
}
function P(e, t) {
	if (t || (e.tState = 0, h.disposed.add(e)), e.owned) for (let t = 0; t < e.owned.length; t++) P(e.owned[t]);
}
function le(e) {
	return e instanceof Error ? e : Error(typeof e == "string" ? e : "Unknown error", { cause: e });
}
function F(e, t, n) {
	try {
		for (let n of t) n(e);
	} catch (e) {
		I(e, n && n.owner || null);
	}
}
function I(e, t = m) {
	let n = l && t && t.context && t.context[l], r = le(e);
	if (!n) throw r;
	v ? v.push({
		fn() {
			F(r, n, t);
		},
		state: d
	}) : F(r, n, t);
}
//#endregion
//#region node_modules/solid-js/store/dist/store.js
var ue = /* @__PURE__ */ t({
	$RAW: () => L,
	DEV: () => void 0,
	createMutable: () => ye,
	createStore: () => he,
	modifyMutable: () => be,
	produce: () => we,
	reconcile: () => Se,
	unwrap: () => U
}), L = Symbol("store-raw"), R = Symbol("store-node"), z = Symbol("store-has"), B = Symbol("store-self");
function V(e) {
	let t = e[o];
	if (!t && (Object.defineProperty(e, o, { value: t = new Proxy(e, fe) }), !Array.isArray(e))) {
		let n = Object.keys(e), r = Object.getOwnPropertyDescriptors(e), i = Object.getPrototypeOf(e), a = i !== null && typeof e == "object" && !!e && !Array.isArray(e) && i !== Object.prototype;
		if (a) {
			let e = Object.getOwnPropertyDescriptors(i);
			n.push(...Object.keys(e)), Object.assign(r, e);
		}
		for (let i = 0, o = n.length; i < o; i++) {
			let o = n[i];
			a && o === "constructor" || r[o].get && Object.defineProperty(e, o, {
				configurable: !0,
				enumerable: r[o].enumerable,
				get: r[o].get.bind(t)
			});
		}
	}
	return t;
}
function H(e) {
	let t;
	return typeof e == "object" && !!e && (e[o] || !(t = Object.getPrototypeOf(e)) || t === Object.prototype || Array.isArray(e));
}
function U(e, t = /* @__PURE__ */ new Set()) {
	let n, r, i, a;
	if (n = e != null && e[L]) return n;
	if (!H(e) || t.has(e)) return e;
	if (Array.isArray(e)) {
		Object.isFrozen(e) ? e = e.slice(0) : t.add(e);
		for (let n = 0, a = e.length; n < a; n++) i = e[n], (r = U(i, t)) !== i && (e[n] = r);
	} else {
		Object.isFrozen(e) ? e = Object.assign({}, e) : t.add(e);
		let n = Object.keys(e), o = Object.getOwnPropertyDescriptors(e);
		for (let s = 0, c = n.length; s < c; s++) a = n[s], !o[a].get && (i = e[a], (r = U(i, t)) !== i && (e[a] = r));
	}
	return e;
}
function W(e, t) {
	let n = e[t];
	return n || Object.defineProperty(e, t, { value: n = Object.create(null) }), n;
}
function G(e, t, n) {
	if (e[t]) return e[t];
	let [r, i] = b(n, {
		equals: !1,
		internal: !0
	});
	return r.$ = i, e[t] = r;
}
function de(e, t) {
	let n = Reflect.getOwnPropertyDescriptor(e, t);
	return !n || n.get || !n.configurable || t === o || t === R ? n : (delete n.value, delete n.writable, n.get = () => e[o][t], n);
}
function K(e) {
	C() && G(W(e, R), B)();
}
function q(e) {
	return K(e), Reflect.ownKeys(e);
}
var fe = {
	get(e, t, n) {
		if (t === L) return e;
		if (t === o) return n;
		if (t === s) return K(e), n;
		let r = W(e, R), i = r[t], a = i ? i() : e[t];
		if (t === R || t === z || t === "__proto__") return a;
		if (!i) {
			let n = Object.getOwnPropertyDescriptor(e, t);
			C() && (typeof a != "function" || e.hasOwnProperty(t)) && !(n && n.get) && (a = G(r, t, a)());
		}
		return H(a) ? V(a) : a;
	},
	has(e, t) {
		return t === L || t === o || t === s || t === R || t === z || t === "__proto__" || (C() && G(W(e, z), t)(), t in e);
	},
	set() {
		return !0;
	},
	deleteProperty() {
		return !0;
	},
	ownKeys: q,
	getOwnPropertyDescriptor: de
};
function J(e, t, n, r = !1) {
	if (t === "__proto__" || !r && e[t] === n) return;
	let i = e[t], a = e.length;
	n === void 0 ? (delete e[t], e[z] && e[z][t] && i !== void 0 && e[z][t].$()) : (e[t] = n, e[z] && e[z][t] && i === void 0 && e[z][t].$());
	let o = W(e, R), s;
	if ((s = G(o, t, i)) && s.$(() => n), Array.isArray(e) && e.length !== a) {
		for (let t = e.length; t < a; t++) (s = o[t]) && s.$();
		(s = G(o, "length", a)) && s.$(e.length);
	}
	(s = o[B]) && s.$();
}
function Y(e, t) {
	let n = Object.keys(t);
	for (let r = 0; r < n.length; r += 1) {
		let i = n[r];
		pe(i) || J(e, i, t[i]);
	}
}
function pe(e) {
	return e === "__proto__" || e === "constructor" || e === "prototype";
}
function me(e, t) {
	if (typeof t == "function" && (t = t(e)), t = U(t), Array.isArray(t)) {
		if (e === t) return;
		let n = 0, r = t.length;
		for (; n < r; n++) {
			let r = t[n];
			e[n] !== r && J(e, n, r);
		}
		J(e, "length", r);
	} else Y(e, t);
}
function X(e, t, n = []) {
	let r, i = e;
	if (t.length > 1) {
		r = t.shift();
		let a = typeof r, o = Array.isArray(e);
		if (a === "string" && (r === "__proto__" || t.length > 1 && pe(r))) return;
		if (Array.isArray(r)) {
			for (let i = 0; i < r.length; i++) X(e, [r[i]].concat(t), n);
			return;
		}
		if (o && a === "function") {
			for (let i = 0; i < e.length; i++) r(e[i], i) && X(e, [i].concat(t), n);
			return;
		}
		if (o && a === "object") {
			let { from: i = 0, to: a = e.length - 1, by: o = 1 } = r;
			for (let r = i; r <= a; r += o) X(e, [r].concat(t), n);
			return;
		}
		if (t.length > 1) {
			X(e[r], t, [r].concat(n));
			return;
		}
		i = e[r], n = [r].concat(n);
	}
	let a = t[0];
	typeof a == "function" && (a = a(i, n), a === i) || (r !== void 0 || a != null) && (a = U(a), r === void 0 || H(i) && H(a) && !Array.isArray(a) ? Y(i, a) : J(e, r, a));
}
function he(...[e, t]) {
	let n = U(e || {}), r = Array.isArray(n), i = V(n);
	function a(...e) {
		x(() => {
			r && e.length === 1 ? me(n, e[0]) : X(n, e);
		});
	}
	return [i, a];
}
function ge(e, t) {
	let n = Reflect.getOwnPropertyDescriptor(e, t);
	return !n || n.get || n.set || !n.configurable || t === o || t === R ? n : (delete n.value, delete n.writable, n.get = () => e[o][t], n.set = (n) => e[o][t] = n, n);
}
var _e = {
	get(e, t, n) {
		if (t === L) return e;
		if (t === o) return n;
		if (t === s) return K(e), n;
		let r = W(e, R), i = r[t], a = i ? i() : e[t];
		if (t === R || t === z || t === "__proto__") return a;
		if (!i) {
			let i = Object.getOwnPropertyDescriptor(e, t), o = typeof a == "function";
			if (C() && (!o || e.hasOwnProperty(t)) && !(i && i.get)) a = G(r, t, a)();
			else if (a != null && o && a === Array.prototype[t]) return (...e) => x(() => Array.prototype[t].apply(n, e));
		}
		return H(a) ? ve(a) : a;
	},
	has(e, t) {
		return t === L || t === o || t === s || t === R || t === z || t === "__proto__" || (C() && G(W(e, z), t)(), t in e);
	},
	set(e, t, n) {
		return x(() => J(e, t, U(n))), !0;
	},
	deleteProperty(e, t) {
		return x(() => J(e, t, void 0, !0)), !0;
	},
	ownKeys: q,
	getOwnPropertyDescriptor: ge
};
function ve(e) {
	let t = e[o];
	if (!t) {
		Object.defineProperty(e, o, { value: t = new Proxy(e, _e) });
		let n = Object.keys(e), r = Object.getOwnPropertyDescriptors(e), i = Object.getPrototypeOf(e), a = i !== null && typeof e == "object" && !!e && !Array.isArray(e) && i !== Object.prototype;
		if (a) {
			let e = i;
			for (; e != null;) {
				let t = Object.getOwnPropertyDescriptors(e);
				n.push(...Object.keys(t)), Object.assign(r, t), e = Object.getPrototypeOf(e);
			}
		}
		for (let i = 0, o = n.length; i < o; i++) {
			let o = n[i];
			if (!(a && o === "constructor")) {
				if (r[o].get) {
					let n = r[o].get.bind(t);
					Object.defineProperty(e, o, {
						get: n,
						configurable: !0
					});
				}
				if (r[o].set) {
					let n = r[o].set;
					Object.defineProperty(e, o, {
						set: (e) => x(() => n.call(t, e)),
						configurable: !0
					});
				}
			}
		}
	}
	return t;
}
function ye(e, t) {
	return ve(U(e || {}));
}
function be(e, t) {
	x(() => t(U(e)));
}
var Z = Symbol("store-root");
function xe(e) {
	return e === "__proto__" || e === "constructor" || e === "prototype";
}
function Q(e, t, n, r, i) {
	if (xe(n)) return;
	let a = t[n];
	if (e === a) return;
	let o = Array.isArray(e);
	if (n !== Z && (!H(e) || !H(a) || o !== Array.isArray(a) || i && e[i] !== a[i])) {
		J(t, n, e);
		return;
	}
	if (o) {
		if (e.length && a.length && (!r || i && e[0] && e[0][i] != null)) {
			let t, n, o, s, c, l, u, d;
			for (o = 0, s = Math.min(a.length, e.length); o < s && (a[o] === e[o] || i && a[o] && e[o] && a[o][i] && a[o][i] === e[o][i]); o++) Q(e[o], a, o, r, i);
			let f = Array(e.length), p = /* @__PURE__ */ new Map();
			for (s = a.length - 1, c = e.length - 1; s >= o && c >= o && (a[s] === e[c] || i && a[s] && e[c] && a[s][i] && a[s][i] === e[c][i]); s--, c--) f[c] = a[s];
			if (o > c || o > s) {
				for (n = o; n <= c; n++) J(a, n, e[n]);
				for (; n < e.length; n++) J(a, n, f[n]), Q(e[n], a, n, r, i);
				a.length > e.length && J(a, "length", e.length);
				return;
			}
			for (u = Array(c + 1), n = c; n >= o; n--) l = e[n], d = i && l ? l[i] : l, t = p.get(d), u[n] = t === void 0 ? -1 : t, p.set(d, n);
			for (t = o; t <= s; t++) l = a[t], d = i && l ? l[i] : l, n = p.get(d), n !== void 0 && n !== -1 && (f[n] = a[t], n = u[n], p.set(d, n));
			for (n = o; n < e.length; n++) n in f ? (J(a, n, f[n]), Q(e[n], a, n, r, i)) : J(a, n, e[n]);
		} else for (let t = 0, n = e.length; t < n; t++) Q(e[t], a, t, r, i);
		a.length > e.length && J(a, "length", e.length);
		return;
	}
	let s = Object.keys(e);
	for (let t = 0, n = s.length; t < n; t++) xe(s[t]) || Q(e[s[t]], a, s[t], r, i);
	let c = Object.keys(a);
	for (let t = 0, n = c.length; t < n; t++) e[c[t]] === void 0 && J(a, c[t], void 0);
}
function Se(e, t = {}) {
	let { merge: n, key: r = "id" } = t, i = U(e);
	return (e) => {
		if (!H(e) || !H(i)) return i;
		let t = Q(i, { [Z]: e }, Z, n, r);
		return t === void 0 ? e : t;
	};
}
var $ = /* @__PURE__ */ new WeakMap(), Ce = {
	get(e, t) {
		if (t === L) return e;
		let n = e[t];
		if (t === o || t === s || t === R || t === z || t === "__proto__") return n;
		let r;
		return H(n) ? $.get(n) || ($.set(n, r = new Proxy(n, Ce)), r) : n;
	},
	set(e, t, n) {
		return J(e, t, U(n)), !0;
	},
	deleteProperty(e, t) {
		return J(e, t, void 0, !0), !0;
	}
};
function we(e) {
	return (t) => {
		if (H(t)) {
			let n;
			(n = $.get(t)) || $.set(t, n = new Proxy(t, Ce)), e(n);
		}
		return t;
	};
}
//#endregion
export { te as createEffect, ee as createRoot, ue as store };
