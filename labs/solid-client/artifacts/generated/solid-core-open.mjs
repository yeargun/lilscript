//#region node_modules/solid-js/dist/solid.js
var e = {
	context: void 0,
	registry: void 0,
	effects: void 0,
	done: !1,
	getContextId() {
		return t(this.context.count);
	},
	getNextContextId() {
		return t(this.context.count++);
	}
};
function t(t) {
	let n = String(t), r = n.length - 1;
	return e.context.id + (r ? String.fromCharCode(96 + r) : "") + n;
}
function n(t) {
	e.context = t;
}
var r = (e, t) => e === t, i = { equals: r }, a = null, o = L, s = 1, c = 2, l = {
	owned: null,
	cleanups: null,
	context: null,
	owner: null
}, u = null, d = null, f = null, p = null, m = null, h = 0;
function g(e, t) {
	let n = f, r = u, i = e.length === 0, a = t === void 0 ? r : t, o = i ? l : {
		owned: null,
		cleanups: null,
		context: a ? a.context : null,
		owner: a
	}, s = i ? e : () => e(() => C(() => V(o)));
	u = o, f = null;
	try {
		return F(s, !0);
	} finally {
		f = n, u = r;
	}
}
function _(e, t) {
	t = t ? Object.assign({}, i, t) : i;
	let n = {
		value: e,
		observers: null,
		observerSlots: null,
		comparator: t.equals || void 0
	};
	return [k.bind(n), (e) => (typeof e == "function" && (e = d && d.running && d.sources.has(n) ? e(n.tValue) : e(n.value)), A(n, e))];
}
function v(e, t, n) {
	j(N(e, t, !1, s));
}
function y(e, t, n) {
	o = R;
	let r = N(e, t, !1, s), i = O && D(O);
	i && (r.suspense = i), (!n || !n.render) && (r.user = !0), m ? m.push(r) : j(r);
}
function b(e, t, n) {
	n = n ? Object.assign({}, i, n) : i;
	let r = N(e, t, !0, 0);
	return r.observers = null, r.observerSlots = null, r.comparator = n.equals || void 0, j(r), k.bind(r);
}
function x(e, t = r, n) {
	let i = /* @__PURE__ */ new Map(), a = N((n) => {
		let r = e();
		for (let [e, a] of i.entries()) if (t(e, r) !== t(e, n)) for (let e of a.values()) e.state = s, e.pure ? p.push(e) : m.push(e);
		return r;
	}, void 0, !0, s);
	return j(a), (e) => {
		let n = f;
		if (n) {
			let t;
			(t = i.get(e)) ? t.add(n) : i.set(e, t = /* @__PURE__ */ new Set([n])), w(() => {
				t.delete(n), !t.size && i.delete(e);
			});
		}
		return t(e, d && d.running && d.sources.has(a) ? a.tValue : a.value);
	};
}
function S(e) {
	return F(e, !1);
}
function C(e) {
	if (f === null) return e();
	let t = f;
	f = null;
	try {
		return e();
	} finally {
		f = t;
	}
}
function w(e) {
	return u === null || (u.cleanups === null ? u.cleanups = [e] : u.cleanups.push(e)), e;
}
var [T, E] = /*@__PURE__*/ _(!1);
function D(e) {
	let t;
	return u && u.context && (t = u.context[e.id]) !== void 0 ? t : e.defaultValue;
}
var O;
function k() {
	let e = d && d.running;
	if (this.sources && (e ? this.tState : this.state)) {
		if ((e ? this.tState : this.state) === s) j(this);
		else {
			let e = p;
			p = null, F(() => z(this), !1), p = e;
		}
	}
	if (f) {
		let e = this.observers;
		if (!e || e[e.length - 1] !== f) {
			let t = e ? e.length : 0;
			f.sources ? (f.sources.push(this), f.sourceSlots.push(t)) : (f.sources = [this], f.sourceSlots = [t]), e ? (e.push(f), this.observerSlots.push(f.sources.length - 1)) : (this.observers = [f], this.observerSlots = [f.sources.length - 1]);
		}
	}
	return e && d.sources.has(this) ? this.tValue : this.value;
}
function A(e, t, n) {
	let r = d && d.running && d.sources.has(e) ? e.tValue : e.value;
	if (!e.comparator || !e.comparator(r, t)) {
		if (d) {
			let r = d.running;
			(r || !n && d.sources.has(e)) && (d.sources.add(e), e.tValue = t), r || (e.value = t);
		} else e.value = t;
		e.observers && e.observers.length && F(() => {
			for (let t = 0; t < e.observers.length; t += 1) {
				let n = e.observers[t], r = d && d.running;
				r && d.disposed.has(n) || ((r ? !n.tState : !n.state) && (n.pure ? p.push(n) : m.push(n), n.observers && B(n)), r ? n.tState = s : n.state = s);
			}
			if (p.length > 1e6) throw p = [], Error();
		}, !1);
	}
	return t;
}
function j(e) {
	if (!e.fn) return;
	V(e);
	let t = h;
	M(e, d && d.running && d.sources.has(e) ? e.tValue : e.value, t), d && !d.running && d.sources.has(e) && queueMicrotask(() => {
		F(() => {
			d && (d.running = !0), f = u = e, M(e, e.tValue, t), f = u = null;
		}, !1);
	});
}
function M(e, t, n) {
	let r, i = u, a = f;
	f = u = e;
	try {
		r = e.fn(t);
	} catch (t) {
		return e.pure && (d && d.running ? (e.tState = s, e.tOwned && e.tOwned.forEach(V), e.tOwned = void 0) : (e.state = s, e.owned && e.owned.forEach(V), e.owned = null)), e.updatedAt = n + 1, G(t);
	} finally {
		f = a, u = i;
	}
	(!e.updatedAt || e.updatedAt <= n) && (e.updatedAt != null && "observers" in e ? A(e, r, !0) : d && d.running && e.pure ? (d.sources.has(e) || (e.value = r), d.sources.add(e), e.tValue = r) : e.value = r, e.updatedAt = n);
}
function N(e, t, n, r = s, i) {
	let a = {
		fn: e,
		state: r,
		updatedAt: null,
		owned: null,
		sources: null,
		sourceSlots: null,
		cleanups: null,
		value: t,
		owner: u,
		context: u ? u.context : null,
		pure: n
	};
	return d && d.running && (a.state = 0, a.tState = r), u === null || u !== l && (d && d.running && u.pure ? u.tOwned ? u.tOwned.push(a) : u.tOwned = [a] : u.owned ? u.owned.push(a) : u.owned = [a]), a;
}
function P(e) {
	let t = d && d.running;
	if ((t ? e.tState : e.state) === 0) return;
	if ((t ? e.tState : e.state) === c) return z(e);
	if (e.suspense && C(e.suspense.inFallback)) return e.suspense.effects.push(e);
	let n = [e];
	for (; (e = e.owner) && (!e.updatedAt || e.updatedAt < h);) {
		if (t && d.disposed.has(e)) return;
		(t ? e.tState : e.state) && n.push(e);
	}
	for (let r = n.length - 1; r >= 0; r--) {
		if (e = n[r], t) {
			let t = e, i = n[r + 1];
			for (; (t = t.owner) && t !== i;) if (d.disposed.has(t)) return;
		}
		if ((t ? e.tState : e.state) === s) j(e);
		else if ((t ? e.tState : e.state) === c) {
			let t = p;
			p = null, F(() => z(e, n[0]), !1), p = t;
		}
	}
}
function F(e, t) {
	if (p) return e();
	let n = !1;
	t || (p = []), m ? n = !0 : m = [], h++;
	try {
		let t = e();
		return I(n), t;
	} catch (e) {
		n || (m = null), p = null, G(e);
	}
}
function I(e) {
	if (p &&= (L(p), null), e) return;
	let t;
	if (d) {
		if (!d.promises.size && !d.queue.size) {
			let e = d.sources, n = d.disposed;
			m.push.apply(m, d.effects), t = d.resolve;
			for (let e of m) "tState" in e && (e.state = e.tState), delete e.tState;
			d = null, F(() => {
				for (let e of n) V(e);
				for (let t of e) {
					if (t.value = t.tValue, t.owned) for (let e = 0, n = t.owned.length; e < n; e++) V(t.owned[e]);
					t.tOwned && (t.owned = t.tOwned), delete t.tValue, delete t.tOwned, t.tState = 0;
				}
				E(!1);
			}, !1);
		} else if (d.running) {
			d.running = !1, d.effects.push.apply(d.effects, m), m = null, E(!0);
			return;
		}
	}
	let n = m;
	m = null, n.length && F(() => o(n), !1), t && t();
}
function L(e) {
	for (let t = 0; t < e.length; t++) P(e[t]);
}
function R(t) {
	let r, i = 0;
	for (r = 0; r < t.length; r++) {
		let e = t[r];
		e.user ? t[i++] = e : P(e);
	}
	if (e.context) {
		if (e.count) {
			e.effects ||= [], e.effects.push(...t.slice(0, i));
			return;
		}
		n();
	}
	for (e.effects && (e.done || !e.count) && (t = [...e.effects, ...t], i += e.effects.length, delete e.effects), r = 0; r < i; r++) P(t[r]);
}
function z(e, t) {
	let n = d && d.running;
	n ? e.tState = 0 : e.state = 0;
	for (let r = 0; r < e.sources.length; r += 1) {
		let i = e.sources[r];
		if (i.sources) {
			let e = n ? i.tState : i.state;
			e === s ? i !== t && (!i.updatedAt || i.updatedAt < h) && P(i) : e === c && z(i, t);
		}
	}
}
function B(e) {
	let t = d && d.running;
	for (let n = 0; n < e.observers.length; n += 1) {
		let r = e.observers[n];
		(t ? !r.tState : !r.state) && (t ? r.tState = c : r.state = c, r.pure ? p.push(r) : m.push(r), r.observers && B(r));
	}
}
function V(e) {
	let t;
	if (e.sources) for (; e.sources.length;) {
		let t = e.sources.pop(), n = e.sourceSlots.pop(), r = t.observers;
		if (r && r.length) {
			let e = r.pop(), i = t.observerSlots.pop();
			n < r.length && (e.sourceSlots[i] = n, r[n] = e, t.observerSlots[n] = i);
		}
	}
	if (e.tOwned) {
		for (t = e.tOwned.length - 1; t >= 0; t--) V(e.tOwned[t]);
		delete e.tOwned;
	}
	if (d && d.running && e.pure) H(e, !0);
	else if (e.owned) {
		for (t = e.owned.length - 1; t >= 0; t--) V(e.owned[t]);
		e.owned = null;
	}
	if (e.cleanups) {
		for (t = e.cleanups.length - 1; t >= 0; t--) e.cleanups[t]();
		e.cleanups = null;
	}
	d && d.running ? e.tState = 0 : e.state = 0;
}
function H(e, t) {
	if (t || (e.tState = 0, d.disposed.add(e)), e.owned) for (let t = 0; t < e.owned.length; t++) H(e.owned[t]);
}
function U(e) {
	return e instanceof Error ? e : Error(typeof e == "string" ? e : "Unknown error", { cause: e });
}
function W(e, t, n) {
	try {
		for (let n of t) n(e);
	} catch (e) {
		G(e, n && n.owner || null);
	}
}
function G(e, t = u) {
	let n = a && t && t.context && t.context[a], r = U(e);
	if (!n) throw r;
	m ? m.push({
		fn() {
			W(r, n, t);
		},
		state: s
	}) : W(r, n, t);
}
//#endregion
export { S as batch, y as createEffect, b as createMemo, v as createRenderEffect, g as createRoot, x as createSelector, _ as createSignal, w as onCleanup, C as untrack };
