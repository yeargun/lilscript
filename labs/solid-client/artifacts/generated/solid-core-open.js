//#region node_modules/solid-js/dist/solid.js
var e = 1, t = !1, n = !1, r = [], i = null, a = null, o = 5, s = 0, c = 300, l = 0, u = null, d = null, f = 1073741823;
function p() {
	let e = new MessageChannel(), t = e.port1, n = e.port2;
	if (typeof t.unref == "function" && t.unref(), typeof n.unref == "function" && n.unref(), u = () => n.postMessage(null), t.onmessage = () => {
		if (d !== null) {
			let e = performance.now();
			s = e + o, l = e + c;
			try {
				d(e) ? n.postMessage(null) : d = null;
			} catch (e) {
				throw n.postMessage(null), e;
			}
		}
	}, navigator && navigator.scheduling && navigator.scheduling.isInputPending) {
		let e = navigator.scheduling;
		a = () => {
			let t = performance.now();
			return t >= s ? e.isInputPending() ? !0 : t >= l : !1;
		};
	} else a = () => performance.now() >= s;
}
function m(e, t) {
	function n() {
		let n = 0, r = e.length - 1;
		for (; n <= r;) {
			let i = r + n >> 1, a = t.expirationTime - e[i].expirationTime;
			if (a > 0) n = i + 1;
			else if (a < 0) r = i - 1;
			else return i;
		}
		return n;
	}
	e.splice(n(), 0, t);
}
function h(i, a) {
	u || p();
	let o = performance.now(), s = f;
	a && a.timeout && (s = a.timeout);
	let c = {
		id: e++,
		fn: i,
		startTime: o,
		expirationTime: o + s
	};
	return m(r, c), !t && !n && (t = !0, d = _, u()), c;
}
function g(e) {
	e.fn = null;
}
function _(e) {
	t = !1, n = !0;
	try {
		return v(e);
	} finally {
		i = null, n = !1;
	}
}
function v(e) {
	let t = e;
	for (i = r[0] || null; i !== null && !(i.expirationTime > t && a());) {
		let e = i.fn;
		e === null ? r.shift() : (i.fn = null, e(i.expirationTime <= t), t = performance.now(), i === r[0] && r.shift()), i = r[0] || null;
	}
	return i !== null;
}
var y = {
	context: void 0,
	registry: void 0,
	effects: void 0,
	done: !1,
	getContextId() {
		return ee(this.context.count);
	},
	getNextContextId() {
		return ee(this.context.count++);
	}
};
function ee(e) {
	let t = String(e), n = t.length - 1;
	return y.context.id + (n ? String.fromCharCode(96 + n) : "") + t;
}
function b(e) {
	y.context = e;
}
function te() {
	return {
		...y.context,
		id: y.getNextContextId(),
		count: 0
	};
}
var x = (e, t) => e === t, S = Symbol("solid-proxy"), C = typeof Proxy == "function", w = Symbol("solid-track"), ne = Symbol("solid-dev-component"), T = { equals: x }, E = null, re = Fe, D = 1, O = 2, ie = {
	owned: null,
	cleanups: null,
	context: null,
	owner: null
}, ae = {}, k = null, A = null, j = null, M = null, N = null, P = null, F = null, I = 0;
function L(e, t) {
	let n = N, r = k, i = e.length === 0, a = t === void 0 ? r : t, o = i ? ie : {
		owned: null,
		cleanups: null,
		context: a ? a.context : null,
		owner: a
	}, s = i ? e : () => e(() => B(() => Y(o)));
	k = o, N = null;
	try {
		return q(s, !0);
	} finally {
		N = n, k = r;
	}
}
function R(e, t) {
	t = t ? Object.assign({}, T, t) : T;
	let n = {
		value: e,
		observers: null,
		observerSlots: null,
		comparator: t.equals || void 0
	};
	return [je.bind(n), (e) => (typeof e == "function" && (e = A && A.running && A.sources.has(n) ? e(n.tValue) : e(n.value)), Me(n, e))];
}
function oe(e, t, n) {
	let r = G(e, t, !0, D);
	j && A && A.running ? P.push(r) : W(r);
}
function se(e, t, n) {
	let r = G(e, t, !1, D);
	j && A && A.running ? P.push(r) : W(r);
}
function ce(e, t, n) {
	re = Le;
	let r = G(e, t, !1, D), i = U && H(U);
	i && (r.suspense = i), (!n || !n.render) && (r.user = !0), F ? F.push(r) : W(r);
}
function le(e, t) {
	let n, r = G(() => {
		n ? n() : B(e), n = void 0;
	}, void 0, !1, 0), i = U && H(U);
	return i && (r.suspense = i), r.user = !0, (e) => {
		n = e, W(r);
	};
}
function z(e, t, n) {
	n = n ? Object.assign({}, T, n) : T;
	let r = G(e, t, !0, 0);
	return r.observers = null, r.observerSlots = null, r.comparator = n.equals || void 0, j && A && A.running ? (r.tState = D, P.push(r)) : W(r), je.bind(r);
}
function ue(e) {
	return e && typeof e == "object" && "then" in e;
}
function de(e, t, n) {
	let r, i, a;
	typeof t == "function" ? (r = e, i = t, a = n || {}) : (r = !0, i = e, a = t || {});
	let o = null, s = ae, c = null, l = !1, u = !1, d = "initialValue" in a, f = typeof r == "function" && z(r), p = /* @__PURE__ */ new Set(), [m, h] = (a.storage || R)(a.initialValue), [g, _] = R(void 0), [v, ee] = R(void 0, { equals: !1 }), [b, te] = R(d ? "ready" : "unresolved");
	y.context && (c = y.getNextContextId(), a.ssrLoadFrom === "initial" ? s = a.initialValue : y.load && y.has(c) && (s = y.load(c)));
	function x(e, t, n, r) {
		return o === e && (o = null, r !== void 0 && (d = !0), (e === s || t === s) && a.onHydrated && queueMicrotask(() => a.onHydrated(r, { value: t })), s = ae, A && e && l ? (A.promises.delete(e), l = !1, q(() => {
			A.running = !0, S(t, n);
		}, !1)) : S(t, n)), t;
	}
	function S(e, t) {
		q(() => {
			t === void 0 && h(() => e), te(t === void 0 ? d ? "ready" : "unresolved" : "errored"), _(t);
			for (let e of p.keys()) e.decrement();
			p.clear();
		}, !1);
	}
	function C() {
		let e = U && H(U), t = m(), n = g();
		if (n !== void 0 && !o) throw n;
		return N && !N.user && e && oe(() => {
			v(), o && (e.resolved && A && l ? A.promises.add(o) : p.has(e) || (e.increment(), p.add(e)));
		}), t;
	}
	function w(e = !0) {
		if (e !== !1 && u) return;
		u = !1;
		let t = f ? f() : r;
		if (l = A && A.running, t == null || t === !1) {
			x(o, B(m));
			return;
		}
		A && o && A.promises.delete(o);
		let n, a = s === ae ? B(() => {
			try {
				return i(t, {
					value: m(),
					refetching: e
				});
			} catch (e) {
				n = e;
			}
		}) : s;
		if (n !== void 0) {
			x(o, void 0, X(n), t);
			return;
		}
		return ue(a) ? (o = a, "v" in a ? (a.s === 1 ? x(o, a.v, void 0, t) : x(o, void 0, X(a.v), t), a) : (u = !0, queueMicrotask(() => u = !1), q(() => {
			te(d ? "refreshing" : "pending"), ee();
		}, !1), a.then((e) => x(a, e, void 0, t), (e) => x(a, void 0, X(e), t)))) : (x(o, a, void 0, t), a);
	}
	Object.defineProperties(C, {
		state: { get: () => b() },
		error: { get: () => g() },
		loading: { get() {
			let e = b();
			return e === "pending" || e === "refreshing";
		} },
		latest: { get() {
			if (!d) return C();
			let e = g();
			if (e && !o) throw e;
			return m();
		} }
	});
	let ne = k;
	return f ? oe(() => (ne = k, w(!1))) : w(!1), [C, {
		refetch: (e) => be(ne, () => w(e)),
		mutate: h
	}];
}
function fe(e, t) {
	let n, r = t ? t.timeoutMs : void 0, i = G(() => ((!n || !n.fn) && (n = h(() => o(() => i.value), r === void 0 ? void 0 : { timeout: r })), e()), void 0, !0), [a, o] = R(A && A.running && A.sources.has(i) ? i.tValue : i.value, t);
	return W(i), o(() => A && A.running && A.sources.has(i) ? i.tValue : i.value), a;
}
function pe(e, t = x, n) {
	let r = /* @__PURE__ */ new Map(), i = G((n) => {
		let i = e();
		for (let [e, a] of r.entries()) if (t(e, i) !== t(e, n)) for (let e of a.values()) e.state = D, e.pure ? P.push(e) : F.push(e);
		return i;
	}, void 0, !0, D);
	return W(i), (e) => {
		let n = N;
		if (n) {
			let t;
			(t = r.get(e)) ? t.add(n) : r.set(e, t = /* @__PURE__ */ new Set([n])), V(() => {
				t.delete(n), !t.size && r.delete(e);
			});
		}
		return t(e, A && A.running && A.sources.has(i) ? i.tValue : i.value);
	};
}
function me(e) {
	return q(e, !1);
}
function B(e) {
	if (!M && N === null) return e();
	let t = N;
	N = null;
	try {
		return M ? M.untrack(e) : e();
	} finally {
		N = t;
	}
}
function he(e, t, n) {
	let r = Array.isArray(e), i, a = n && n.defer;
	return (n) => {
		let o;
		if (r) {
			o = Array(e.length);
			for (let t = 0; t < e.length; t++) o[t] = e[t]();
		} else o = e();
		if (a) return a = !1, n;
		let s = B(() => t(o, i, n));
		return i = o, s;
	};
}
function ge(e) {
	ce(() => B(e));
}
function V(e) {
	return k === null || (k.cleanups === null ? k.cleanups = [e] : k.cleanups.push(e)), e;
}
function _e(e, t) {
	E ||= Symbol("error"), k = G(void 0, void 0, !0), k.context = {
		...k.context,
		[E]: [t]
	}, A && A.running && A.sources.add(k);
	try {
		return e();
	} catch (e) {
		Z(e);
	} finally {
		k = k.owner;
	}
}
function ve() {
	return N;
}
function ye() {
	return k;
}
function be(e, t) {
	let n = k, r = N;
	k = e, N = null;
	try {
		return q(t, !0);
	} catch (e) {
		Z(e);
	} finally {
		k = n, N = r;
	}
}
function xe(e = h) {
	j = e;
}
function Se(e) {
	if (A && A.running) return e(), A.done;
	let t = N, n = k;
	return Promise.resolve().then(() => {
		N = t, k = n;
		let r;
		return (j || U) && (r = A ||= {
			sources: /* @__PURE__ */ new Set(),
			effects: [],
			promises: /* @__PURE__ */ new Set(),
			disposed: /* @__PURE__ */ new Set(),
			queue: /* @__PURE__ */ new Set(),
			running: !0
		}, r.done || (r.done = new Promise((e) => r.resolve = e)), r.running = !0), q(e, !1), N = k = null, r ? r.done : void 0;
	});
}
var [Ce, we] = /*@__PURE__*/ R(!1);
function Te() {
	return [Ce, Se];
}
function Ee(e) {
	F.push.apply(F, e), e.length = 0;
}
function De(e, t) {
	let n = Symbol("context");
	return {
		id: n,
		Provider: He(n),
		defaultValue: e
	};
}
function H(e) {
	let t;
	return k && k.context && (t = k.context[e.id]) !== void 0 ? t : e.defaultValue;
}
function Oe(e) {
	let t = z(e), n = z(() => Ve(t()));
	return n.toArray = () => {
		let e = n();
		return Array.isArray(e) ? e : e == null ? [] : [e];
	}, n;
}
var U;
function ke() {
	return U ||= De();
}
function Ae(e, t = (e) => e()) {
	if (M) {
		let { factory: n, untrack: r } = M;
		M = {
			factory: (t, r) => {
				let i = n(t, r), a = e((e) => i.track(e), r);
				return {
					track: (e) => a.track(e),
					dispose() {
						a.dispose(), i.dispose();
					}
				};
			},
			untrack: (e) => r(() => t(e))
		};
	} else M = {
		factory: e,
		untrack: t
	};
}
function je() {
	let e = A && A.running;
	if (this.sources && (e ? this.tState : this.state)) {
		if ((e ? this.tState : this.state) === D) W(this);
		else {
			let e = P;
			P = null, q(() => J(this), !1), P = e;
		}
	}
	if (N) {
		let e = this.observers;
		if (!e || e[e.length - 1] !== N) {
			let t = e ? e.length : 0;
			N.sources ? (N.sources.push(this), N.sourceSlots.push(t)) : (N.sources = [this], N.sourceSlots = [t]), e ? (e.push(N), this.observerSlots.push(N.sources.length - 1)) : (this.observers = [N], this.observerSlots = [N.sources.length - 1]);
		}
	}
	return e && A.sources.has(this) ? this.tValue : this.value;
}
function Me(e, t, n) {
	let r = A && A.running && A.sources.has(e) ? e.tValue : e.value;
	if (!e.comparator || !e.comparator(r, t)) {
		if (A) {
			let r = A.running;
			(r || !n && A.sources.has(e)) && (A.sources.add(e), e.tValue = t), r || (e.value = t);
		} else e.value = t;
		e.observers && e.observers.length && q(() => {
			for (let t = 0; t < e.observers.length; t += 1) {
				let n = e.observers[t], r = A && A.running;
				r && A.disposed.has(n) || ((r ? !n.tState : !n.state) && (n.pure ? P.push(n) : F.push(n), n.observers && Re(n)), r ? n.tState = D : n.state = D);
			}
			if (P.length > 1e6) throw P = [], Error();
		}, !1);
	}
	return t;
}
function W(e) {
	if (!e.fn) return;
	Y(e);
	let t = I;
	Ne(e, A && A.running && A.sources.has(e) ? e.tValue : e.value, t), A && !A.running && A.sources.has(e) && queueMicrotask(() => {
		q(() => {
			A && (A.running = !0), N = k = e, Ne(e, e.tValue, t), N = k = null;
		}, !1);
	});
}
function Ne(e, t, n) {
	let r, i = k, a = N;
	N = k = e;
	try {
		r = e.fn(t);
	} catch (t) {
		return e.pure && (A && A.running ? (e.tState = D, e.tOwned && e.tOwned.forEach(Y), e.tOwned = void 0) : (e.state = D, e.owned && e.owned.forEach(Y), e.owned = null)), e.updatedAt = n + 1, Z(t);
	} finally {
		N = a, k = i;
	}
	(!e.updatedAt || e.updatedAt <= n) && (e.updatedAt != null && "observers" in e ? Me(e, r, !0) : A && A.running && e.pure ? (A.sources.has(e) || (e.value = r), A.sources.add(e), e.tValue = r) : e.value = r, e.updatedAt = n);
}
function G(e, t, n, r = D, i) {
	let a = {
		fn: e,
		state: r,
		updatedAt: null,
		owned: null,
		sources: null,
		sourceSlots: null,
		cleanups: null,
		value: t,
		owner: k,
		context: k ? k.context : null,
		pure: n
	};
	if (A && A.running && (a.state = 0, a.tState = r), k === null || k !== ie && (A && A.running && k.pure ? k.tOwned ? k.tOwned.push(a) : k.tOwned = [a] : k.owned ? k.owned.push(a) : k.owned = [a]), M && a.fn) {
		let e = a.fn, [t, n] = R(void 0, { equals: !1 }), r = M.factory(e, n);
		V(() => r.dispose());
		let i, o = () => Se(n).then(() => {
			i &&= (i.dispose(), void 0);
		});
		a.fn = (n) => (t(), A && A.running ? (i ||= M.factory(e, o), i.track(n)) : r.track(n));
	}
	return a;
}
function K(e) {
	let t = A && A.running;
	if ((t ? e.tState : e.state) === 0) return;
	if ((t ? e.tState : e.state) === O) return J(e);
	if (e.suspense && B(e.suspense.inFallback)) return e.suspense.effects.push(e);
	let n = [e];
	for (; (e = e.owner) && (!e.updatedAt || e.updatedAt < I);) {
		if (t && A.disposed.has(e)) return;
		(t ? e.tState : e.state) && n.push(e);
	}
	for (let r = n.length - 1; r >= 0; r--) {
		if (e = n[r], t) {
			let t = e, i = n[r + 1];
			for (; (t = t.owner) && t !== i;) if (A.disposed.has(t)) return;
		}
		if ((t ? e.tState : e.state) === D) W(e);
		else if ((t ? e.tState : e.state) === O) {
			let t = P;
			P = null, q(() => J(e, n[0]), !1), P = t;
		}
	}
}
function q(e, t) {
	if (P) return e();
	let n = !1;
	t || (P = []), F ? n = !0 : F = [], I++;
	try {
		let t = e();
		return Pe(n), t;
	} catch (e) {
		n || (F = null), P = null, Z(e);
	}
}
function Pe(e) {
	if (P &&= (j && A && A.running ? Ie(P) : Fe(P), null), e) return;
	let t;
	if (A) {
		if (!A.promises.size && !A.queue.size) {
			let e = A.sources, n = A.disposed;
			F.push.apply(F, A.effects), t = A.resolve;
			for (let e of F) "tState" in e && (e.state = e.tState), delete e.tState;
			A = null, q(() => {
				for (let e of n) Y(e);
				for (let t of e) {
					if (t.value = t.tValue, t.owned) for (let e = 0, n = t.owned.length; e < n; e++) Y(t.owned[e]);
					t.tOwned && (t.owned = t.tOwned), delete t.tValue, delete t.tOwned, t.tState = 0;
				}
				we(!1);
			}, !1);
		} else if (A.running) {
			A.running = !1, A.effects.push.apply(A.effects, F), F = null, we(!0);
			return;
		}
	}
	let n = F;
	F = null, n.length && q(() => re(n), !1), t && t();
}
function Fe(e) {
	for (let t = 0; t < e.length; t++) K(e[t]);
}
function Ie(e) {
	for (let t = 0; t < e.length; t++) {
		let n = e[t], r = A.queue;
		r.has(n) || (r.add(n), j(() => {
			r.delete(n), q(() => {
				A.running = !0, K(n);
			}, !1), A && (A.running = !1);
		}));
	}
}
function Le(e) {
	let t, n = 0;
	for (t = 0; t < e.length; t++) {
		let r = e[t];
		r.user ? e[n++] = r : K(r);
	}
	if (y.context) {
		if (y.count) {
			y.effects ||= [], y.effects.push(...e.slice(0, n));
			return;
		}
		b();
	}
	for (y.effects && (y.done || !y.count) && (e = [...y.effects, ...e], n += y.effects.length, delete y.effects), t = 0; t < n; t++) K(e[t]);
}
function J(e, t) {
	let n = A && A.running;
	n ? e.tState = 0 : e.state = 0;
	for (let r = 0; r < e.sources.length; r += 1) {
		let i = e.sources[r];
		if (i.sources) {
			let e = n ? i.tState : i.state;
			e === D ? i !== t && (!i.updatedAt || i.updatedAt < I) && K(i) : e === O && J(i, t);
		}
	}
}
function Re(e) {
	let t = A && A.running;
	for (let n = 0; n < e.observers.length; n += 1) {
		let r = e.observers[n];
		(t ? !r.tState : !r.state) && (t ? r.tState = O : r.state = O, r.pure ? P.push(r) : F.push(r), r.observers && Re(r));
	}
}
function Y(e) {
	let t;
	if (e.sources) for (; e.sources.length;) {
		let t = e.sources.pop(), n = e.sourceSlots.pop(), r = t.observers;
		if (r && r.length) {
			let e = r.pop(), i = t.observerSlots.pop();
			n < r.length && (e.sourceSlots[i] = n, r[n] = e, t.observerSlots[n] = i);
		}
	}
	if (e.tOwned) {
		for (t = e.tOwned.length - 1; t >= 0; t--) Y(e.tOwned[t]);
		delete e.tOwned;
	}
	if (A && A.running && e.pure) ze(e, !0);
	else if (e.owned) {
		for (t = e.owned.length - 1; t >= 0; t--) Y(e.owned[t]);
		e.owned = null;
	}
	if (e.cleanups) {
		for (t = e.cleanups.length - 1; t >= 0; t--) e.cleanups[t]();
		e.cleanups = null;
	}
	A && A.running ? e.tState = 0 : e.state = 0;
}
function ze(e, t) {
	if (t || (e.tState = 0, A.disposed.add(e)), e.owned) for (let t = 0; t < e.owned.length; t++) ze(e.owned[t]);
}
function X(e) {
	return e instanceof Error ? e : Error(typeof e == "string" ? e : "Unknown error", { cause: e });
}
function Be(e, t, n) {
	try {
		for (let n of t) n(e);
	} catch (e) {
		Z(e, n && n.owner || null);
	}
}
function Z(e, t = k) {
	let n = E && t && t.context && t.context[E], r = X(e);
	if (!n) throw r;
	F ? F.push({
		fn() {
			Be(r, n, t);
		},
		state: D
	}) : Be(r, n, t);
}
function Ve(e) {
	if (typeof e == "function" && !e.length) return Ve(e());
	if (Array.isArray(e)) {
		let t = [];
		for (let n = 0; n < e.length; n++) {
			let r = Ve(e[n]);
			if (Array.isArray(r)) {
				if (r.length < 32768) t.push.apply(t, r);
				else for (let e = 0; e < r.length; e++) t.push(r[e]);
			} else t.push(r);
		}
		return t;
	}
	return e;
}
function He(e, t) {
	return function(t) {
		let n;
		return se(() => n = B(() => (k.context = {
			...k.context,
			[e]: t.value
		}, Oe(() => t.children))), void 0), n;
	};
}
function Ue(e) {
	E ||= Symbol("error"), k === null || (k.context === null || !k.context[E] ? (k.context = {
		...k.context,
		[E]: [e]
	}, Q(k, E, [e])) : k.context[E].push(e));
}
function Q(e, t, n) {
	if (e.owned) for (let r = 0; r < e.owned.length; r++) e.owned[r].context === e.context && Q(e.owned[r], t, n), e.owned[r].context ? e.owned[r].context[t] || (e.owned[r].context[t] = n, Q(e.owned[r], t, n)) : (e.owned[r].context = e.context, Q(e.owned[r], t, n));
}
function We(e) {
	return {
		subscribe(t) {
			if (!(t instanceof Object) || t == null) throw TypeError("Expected the observer to be an object.");
			let n = typeof t == "function" ? t : t.next && t.next.bind(t);
			if (!n) return { unsubscribe() {} };
			let r = L((t) => (ce(() => {
				let t = e();
				B(() => n(t));
			}), t));
			return ye() && V(r), { unsubscribe() {
				r();
			} };
		},
		[Symbol.observable || "@@observable"]() {
			return this;
		}
	};
}
function Ge(e, t = void 0) {
	let [n, r] = R(t, { equals: !1 });
	if ("subscribe" in e) {
		let t = e.subscribe((e) => r(() => e));
		V(() => "unsubscribe" in t ? t.unsubscribe() : t());
	} else V(e(r));
	return n;
}
var Ke = Symbol("fallback");
function qe(e) {
	for (let t = 0; t < e.length; t++) e[t]();
}
function Je(e, t, n = {}) {
	let r = [], i = [], a = [], o = 0, s = t.length > 1 ? [] : null;
	return V(() => qe(a)), () => {
		let c = e() || [], l = c.length, u, d;
		return c[w], B(() => {
			let e, t, p, m, h, g, _, v, y;
			if (l === 0) o !== 0 && (qe(a), a = [], r = [], i = [], o = 0, s &&= []), n.fallback && (r = [Ke], i[0] = L((e) => (a[0] = e, n.fallback())), o = 1);
			else if (o === 0) {
				for (i = Array(l), d = 0; d < l; d++) r[d] = c[d], i[d] = L(f);
				o = l;
			} else {
				for (p = Array(l), m = Array(l), s && (h = Array(l)), g = 0, _ = Math.min(o, l); g < _ && r[g] === c[g]; g++);
				for (_ = o - 1, v = l - 1; _ >= g && v >= g && r[_] === c[v]; _--, v--) p[v] = i[_], m[v] = a[_], s && (h[v] = s[_]);
				for (e = /* @__PURE__ */ new Map(), t = Array(v + 1), d = v; d >= g; d--) y = c[d], u = e.get(y), t[d] = u === void 0 ? -1 : u, e.set(y, d);
				for (u = g; u <= _; u++) y = r[u], d = e.get(y), d !== void 0 && d !== -1 ? (p[d] = i[u], m[d] = a[u], s && (h[d] = s[u]), d = t[d], e.set(y, d)) : a[u]();
				for (d = g; d < l; d++) d in p ? (i[d] = p[d], a[d] = m[d], s && (s[d] = h[d], s[d](d))) : i[d] = L(f);
				i = i.slice(0, o = l), r = c.slice(0);
			}
			return i;
		});
		function f(e) {
			if (a[d] = e, s) {
				let [e, n] = R(d);
				return s[d] = n, t(c[d], e);
			}
			return t(c[d]);
		}
	};
}
function Ye(e, t, n = {}) {
	let r = [], i = [], a = [], o = [], s = 0, c;
	return V(() => qe(a)), () => {
		let l = e() || [], u = l.length;
		return l[w], B(() => {
			if (u === 0) return s !== 0 && (qe(a), a = [], r = [], i = [], s = 0, o = []), n.fallback && (r = [Ke], i[0] = L((e) => (a[0] = e, n.fallback())), s = 1), i;
			for (r[0] === Ke && (a[0](), a = [], r = [], i = [], s = 0), c = 0; c < u; c++) c < r.length && r[c] !== l[c] ? o[c](() => l[c]) : c >= r.length && (i[c] = L(d));
			for (; c < r.length; c++) a[c]();
			return s = o.length = a.length = u, r = l.slice(0), i = i.slice(0, s);
		});
		function d(e) {
			a[c] = e;
			let [n, r] = R(l[c]);
			return o[c] = r, t(n, c);
		}
	};
}
var Xe = !1;
function Ze() {
	Xe = !0;
}
function Qe(e, t) {
	if (Xe && y.context) {
		let n = y.context;
		b(te());
		let r = B(() => e(t || {}));
		return b(n), r;
	}
	return B(() => e(t || {}));
}
function $e() {
	return !0;
}
var et = {
	get(e, t, n) {
		return t === S ? n : e.get(t);
	},
	has(e, t) {
		return t === S || e.has(t);
	},
	set: $e,
	deleteProperty: $e,
	getOwnPropertyDescriptor(e, t) {
		return {
			configurable: !0,
			enumerable: !0,
			get() {
				return e.get(t);
			},
			set: $e,
			deleteProperty: $e
		};
	},
	ownKeys(e) {
		return e.keys();
	}
};
function tt(e) {
	return (e = typeof e == "function" ? e() : e) ? e : {};
}
function nt() {
	for (let e = 0, t = this.length; e < t; ++e) {
		let t = this[e]();
		if (t !== void 0) return t;
	}
}
function rt(...e) {
	let t = !1;
	for (let n = 0; n < e.length; n++) {
		let r = e[n];
		t ||= !!r && S in r, e[n] = typeof r == "function" ? (t = !0, z(r)) : r;
	}
	if (C && t) return new Proxy({
		get(t) {
			for (let n = e.length - 1; n >= 0; n--) {
				let r = tt(e[n])[t];
				if (r !== void 0) return r;
			}
		},
		has(t) {
			for (let n = e.length - 1; n >= 0; n--) if (t in tt(e[n])) return !0;
			return !1;
		},
		keys() {
			let t = [];
			for (let n = 0; n < e.length; n++) t.push(...Object.keys(tt(e[n])));
			return [...new Set(t)];
		}
	}, et);
	let n = {}, r = Object.create(null);
	for (let t = e.length - 1; t >= 0; t--) {
		let i = e[t];
		if (!i) continue;
		let a = Object.getOwnPropertyNames(i);
		for (let e = a.length - 1; e >= 0; e--) {
			let t = a[e];
			if (t === "__proto__" || t === "constructor") continue;
			let o = Object.getOwnPropertyDescriptor(i, t);
			if (!r[t]) r[t] = o.get ? {
				enumerable: !0,
				configurable: !0,
				get: nt.bind(n[t] = [o.get.bind(i)])
			} : o.value === void 0 ? void 0 : o;
			else {
				let e = n[t];
				e && (o.get ? e.push(o.get.bind(i)) : o.value !== void 0 && e.push(() => o.value));
			}
		}
	}
	let i = {}, a = Object.keys(r);
	for (let e = a.length - 1; e >= 0; e--) {
		let t = a[e], n = r[t];
		n && n.get ? Object.defineProperty(i, t, n) : i[t] = n ? n.value : void 0;
	}
	return i;
}
function it(e, ...t) {
	let n = t.length;
	if (C && S in e) {
		let r = n > 1 ? t.flat() : t[0], i = t.map((t) => new Proxy({
			get(n) {
				return t.includes(n) ? e[n] : void 0;
			},
			has(n) {
				return t.includes(n) && n in e;
			},
			keys() {
				return t.filter((t) => t in e);
			}
		}, et));
		return i.push(new Proxy({
			get(t) {
				return r.includes(t) ? void 0 : e[t];
			},
			has(t) {
				return !r.includes(t) && t in e;
			},
			keys() {
				return Object.keys(e).filter((e) => !r.includes(e));
			}
		}, et)), i;
	}
	let r = [];
	for (let e = 0; e <= n; e++) r[e] = {};
	for (let i of Object.getOwnPropertyNames(e)) {
		let a = n;
		for (let e = 0; e < t.length; e++) if (t[e].includes(i)) {
			a = e;
			break;
		}
		let o = Object.getOwnPropertyDescriptor(e, i);
		!o.get && !o.set && o.enumerable && o.writable && o.configurable ? r[a][i] = o.value : Object.defineProperty(r[a], i, o);
	}
	return r;
}
function at(e) {
	let t, n, r = (r) => {
		let i = y.context;
		if (i) {
			let [r, a] = R();
			y.count ||= 0, y.count++, (n ||= e()).then((e) => {
				!y.done && b(i), y.count--, a(() => e.default), b();
			}), t = r;
		} else if (!t) {
			let [r] = de(() => (n ||= e()).then((e) => e.default));
			t = r;
		}
		let a;
		return z(() => (a = t()) ? B(() => {
			if (!i || y.done) return a(r);
			let e = y.context;
			b(i);
			let t = a(r);
			return b(e), t;
		}) : "");
	};
	return r.preload = () => n || ((n = e()).then((e) => t = () => e.default), n), r;
}
var ot = 0;
function st() {
	return y.context ? y.getNextContextId() : `cl-${ot++}`;
}
var ct = (e) => `Stale read from <${e}>.`;
function lt(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return z(Je(() => e.each, e.children, t || void 0));
}
function ut(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return z(Ye(() => e.each, e.children, t || void 0));
}
function dt(e) {
	let t = e.keyed, n = z(() => e.when, void 0, void 0), r = t ? n : z(n, void 0, { equals: (e, t) => !e == !t });
	return z(() => {
		let i = r();
		if (i) {
			let a = e.children;
			return typeof a == "function" && a.length > 0 ? B(() => a(t ? i : () => {
				if (!B(r)) throw ct("Show");
				return n();
			})) : a;
		}
		return e.fallback;
	}, void 0, void 0);
}
function ft(e) {
	let t = Oe(() => e.children), n = z(() => {
		let e = t(), n = Array.isArray(e) ? e : [e], r = () => void 0;
		for (let e = 0; e < n.length; e++) {
			let t = e, i = n[e], a = r, o = z(() => a() ? void 0 : i.when, void 0, void 0), s = i.keyed ? o : z(o, void 0, { equals: (e, t) => !e == !t });
			r = () => a() || (s() ? [
				t,
				o,
				i
			] : void 0);
		}
		return r;
	});
	return z(() => {
		let t = n()();
		if (!t) return e.fallback;
		let [r, i, a] = t, o = a.children;
		return typeof o == "function" && o.length > 0 ? B(() => o(a.keyed ? i() : () => {
			if (B(n)()?.[0] !== r) throw ct("Match");
			return i();
		})) : o;
	}, void 0, void 0);
}
function pt(e) {
	return e;
}
var $;
function mt() {
	$ && [...$].forEach((e) => e());
}
function ht(e) {
	let t;
	y.context && y.load && (t = y.load(y.getContextId()));
	let [n, r] = R(t, void 0);
	return $ ||= /* @__PURE__ */ new Set(), $.add(r), V(() => $.delete(r)), z(() => {
		let t;
		if (t = n()) {
			let n = e.fallback;
			return typeof n == "function" && n.length ? B(() => n(t, () => r())) : n;
		}
		return _e(() => e.children, r);
	}, void 0, void 0);
}
var gt = (e, t) => e.showContent === t.showContent && e.showFallback === t.showFallback, _t = /* #__PURE__ */ De();
function vt(e) {
	let [t, n] = R(() => ({ inFallback: !1 })), r, i = H(_t), [a, o] = R([]);
	i && (r = i.register(z(() => t()().inFallback)));
	let s = z((t) => {
		let n = e.revealOrder, i = e.tail, { showContent: o = !0, showFallback: s = !0 } = r ? r() : {}, c = a(), l = n === "backwards";
		if (n === "together") {
			let e = c.every((e) => !e()), t = c.map(() => ({
				showContent: e && o,
				showFallback: s
			}));
			return t.inFallback = !e, t;
		}
		let u = !1, d = t.inFallback, f = [];
		for (let e = 0, t = c.length; e < t; e++) {
			let n = l ? t - e - 1 : e, r = c[n]();
			if (!u && !r) f[n] = {
				showContent: o,
				showFallback: s
			};
			else {
				let e = !u;
				e && (d = !0), f[n] = {
					showContent: e,
					showFallback: !i || e && i === "collapsed" ? s : !1
				}, u = !0;
			}
		}
		return u || (d = !1), f.inFallback = d, f;
	}, { inFallback: !1 });
	return n(() => s), Qe(_t.Provider, {
		value: { register: (e) => {
			let t;
			return o((n) => (t = n.length, [...n, e])), z(() => s()[t], void 0, { equals: gt });
		} },
		get children() {
			return e.children;
		}
	});
}
function yt(e) {
	let t = 0, n, r, i, a, o, [s, c] = R(!1), l = ke(), u = {
		increment: () => {
			++t === 1 && c(!0);
		},
		decrement: () => {
			--t === 0 && c(!1);
		},
		inFallback: s,
		effects: [],
		resolved: !1
	}, d = ye();
	if (y.context && y.load) {
		let e = y.getContextId(), t = y.load(e);
		if (t && (typeof t != "object" || t.s !== 1 ? i = t : y.gather(e)), i && i !== "$$f") {
			let [t, n] = R(void 0, { equals: !1 });
			a = t, i.then(() => {
				if (y.done) return n();
				y.gather(e), b(r), n(), b();
			}, (e) => {
				o = e, n();
			});
		}
	}
	let f = H(_t);
	f && (n = f.register(u.inFallback));
	let p;
	return V(() => p && p()), Qe(l.Provider, {
		value: u,
		get children() {
			return z(() => {
				if (o) throw o;
				if (r = y.context, a) {
					a(), a = void 0;
					return;
				}
				r && i === "$$f" && b();
				let t = z(() => e.children);
				return z((a) => {
					let o = u.inFallback(), { showContent: s = !0, showFallback: c = !0 } = n ? n() : {};
					if ((!o || i && i !== "$$f") && s) return u.resolved = !0, p && p(), p = r = i = void 0, Ee(u.effects), t();
					if (c) return p ? a : L((t) => (p = t, r &&= (b({
						id: r.id + "F",
						count: 0
					}), void 0), e.fallback), d);
				});
			});
		}
	});
}
var bt = void 0;
//#endregion
export { ne as $DEVCOMP, S as $PROXY, w as $TRACK, bt as DEV, ht as ErrorBoundary, lt as For, ut as Index, pt as Match, dt as Show, yt as Suspense, vt as SuspenseList, ft as Switch, me as batch, g as cancelCallback, _e as catchError, Oe as children, Qe as createComponent, oe as createComputed, De as createContext, fe as createDeferred, ce as createEffect, z as createMemo, le as createReaction, se as createRenderEffect, de as createResource, L as createRoot, pe as createSelector, R as createSignal, st as createUniqueId, Ae as enableExternalSource, Ze as enableHydration, xe as enableScheduling, x as equalFn, Ge as from, ve as getListener, ye as getOwner, Ye as indexArray, at as lazy, Je as mapArray, rt as mergeProps, We as observable, he as on, V as onCleanup, Ue as onError, ge as onMount, h as requestCallback, mt as resetErrorBoundaries, be as runWithOwner, y as sharedConfig, it as splitProps, Se as startTransition, B as untrack, H as useContext, Te as useTransition };
