//#region \0rolldown/runtime.js
var e = Object.defineProperty, t = (t, n) => {
	let r = {};
	for (var i in t) e(r, i, {
		get: t[i],
		enumerable: !0
	});
	return n || e(r, Symbol.toStringTag, { value: "Module" }), r;
}, n = "Potential Infinite Loop Detected.";
function r(e, t, n, r) {
	e[0] = t, e[1] = n, e[2] = r, e[3] = [], e[4] = [], e[5] = [], e[6] = null, e[7] = !1;
}
function i(e, t, n) {
	e[0] = t, e[1] = [], e[2] = n, e[3] = 0, e[4] = -1;
}
function a(e, t) {
	for (var n = 0; n < e[1].length; n++) if ((e[1][n] | 0) == t) return !1;
	return e[1].push(t), !0;
}
function o(e, t) {
	for (var n = 0; n < e[1].length; n++) if ((e[1][n] | 0) == t) {
		e[1][n] = e[1][e[1].length - 1] | 0, e[1].pop();
		return;
	}
}
function s(e) {
	var t;
	if (D < 0 && e[4] < 0) return e[0];
	if (D >= 0 && e[4] >= 0 && e[4] == O) throw "Circular memo dependency detected.";
	if (e[4] >= 0 && e[4] != O ? (t = O >= 0 && !C[O][9] && !C[O][10], t = !t) : t = !1, t && ce(e[4]), D >= 0) {
		t = D;
		var n = e[3] + 1 | 0;
		n > C[t][12] && (C[t][12] = n), e[4] >= 0 && d(C[t], e[4]), a(e, t) && C[t][4].push(e);
	}
	return e[0];
}
function c(e, t) {
	return O >= 0 && C[O][9] && (e[3] = C[O][12], e[4] = O), e[2](e[0], t) ? t : (e[0] = t, de(e[1]), t);
}
function l(e, t) {
	return c(e, t(e[0]));
}
function u(e, t, n, r, i, a) {
	e[0] = t, e[1] = n, e[2] = r, e[3] = i, e[4] = [], e[5] = [], e[6] = [], e[7] = [], e[8] = null, e[9] = a, e[10] = !1, e[11] = !1, e[12] = 0, e[13] = [];
}
function d(e, t) {
	for (var n = 0; n < e[13].length; n++) if ((e[13][n] | 0) == t) return;
	e[13].push(t);
}
function f(e) {
	for (; e[6].length > 0;) {
		var t = e[6][e[6].length - 1] | 0;
		e[6].pop(), y(t);
	}
	for (; e[5].length > 0;) t = e[5][e[5].length - 1] | 0, e[5].pop(), pe(t);
	for (; e[7].length > 0;) t = e[7][e[7].length - 1], e[7].pop(), t[0]();
}
function p(e) {
	for (; e[4].length > 0;) {
		var t = e[4][e[4].length - 1];
		e[4].pop(), o(t, e[0]);
	}
}
function m(e) {
	if (!e[11]) {
		f(e), p(e);
		var t = D, n = O, r = k;
		D = e[0], O = e[0], k = -1, e[12] = 0, e[13] = [], j = j + 1 | 0, e[3](), e[10] = !0, j = j - 1 | 0, D = t, O = n, k = r, v();
	}
}
var ee = (e) => {
	for (var t = 0; t < P.length; t++) if ((P[t] | 0) == e) return !0;
	return !1;
}, te = (e) => {
	for (var t = P[e] | 0; e < P.length - 1; e = e + 1 | 0) P[e] = P[e + 1 | 0] | 0;
	return P.pop(), t;
}, ne = (e) => {
	for (var t = 0; t < P.length; t++) if ((P[t] | 0) == e) return te(t), !0;
	return !1;
}, h = (e, t) => {
	for (var n = e.length - 1; n >= 0; n--) if ((e[n] | 0) == t) {
		e[n] = e[e.length - 1] | 0, e.pop();
		return;
	}
}, g = (e, t) => {
	if (e == 1) {
		var n = w[t][6];
		return n || (n = [
			e,
			t,
			null
		], w[t][6] = n, n[2] = g(w[t][1], w[t][2]), n);
	}
	return e == 2 ? (n = C[t][8]) ? n : (n = [
		e,
		t,
		null
	], C[t][8] = n, n[2] = g(C[t][1], C[t][2]), n) : null;
}, re = (e) => {
	e[1] == 1 ? h(w[e[2]][3], e[0]) : e[1] == 2 && h(C[e[2]][5], e[0]);
}, ie = (e) => {
	e[1] == 1 ? h(w[e[2]][4], e[0]) : e[1] == 2 && h(C[e[2]][6], e[0]);
}, ae = (e, t, n = 0) => {
	if (e == t) return !0;
	if (n >= C.length) return !1;
	for (var r = 0; r < C[e][13].length; r++) if (ae(C[e][13][r] | 0, t, n + 1 | 0)) return !0;
	return !1;
}, oe = (e) => {
	if (O >= 0 && C[O][9] && !N && ae(O, e) && !_(e, O)) throw "Reactive dependency cycle detected.";
	!C[e][11] && !ee(e) && P.push(e);
}, se = (e) => {
	for (var t = 0; t < C[e][13].length; t++) if (ee(C[e][13][t] | 0)) return !0;
	return !1;
}, _ = (e, t) => {
	var n = C[t][1];
	t = C[t][2];
	for (var r = 0, i; i = n != 0 && r <= (C.length + w.length | 0), i;) {
		if (n == 2) {
			if (t == e) return !0;
			n = C[t][1], t = C[t][2];
		} else n = w[t][1], t = w[t][2];
		r = r + 1 | 0;
	}
	return !1;
}, ce = (e) => {
	var t, n;
	if (!(C[e][11] || e == O)) {
		for (n = 0; n < C[e][13].length; n++) t = C[e][13][n] | 0, (ee(t) || se(t)) && ce(t);
		ne(e) && m(C[e]);
	}
}, le = () => {
	for (var e = -1, t = -1, n = -1, r = 0, i, a, o, s, c; r < P.length; r++) i = P[r] | 0, C[i][9] ? (e < 0 || C[i][12] > C[P[e] | 0][12]) && (e = r) : !C[i][10] && t < 0 ? t = r : n < 0 ? n = r : (o = P[n] | 0, a = C[i], s = C[o], c = a[1] == s[1] && a[2] == s[2], (c && a[12] > s[12] || !c && (_(i, o) || !_(o, i) && a[12] > s[12])) && (n = r));
	return t >= 0 && (e = t), te(e < 0 ? n : e);
}, v = () => {
	if (!(M || A > 0 || j > 0)) {
		M = !0;
		var e = 0;
		try {
			for (; P.length > 0;) {
				if (e = e + 1 | 0, e > 1e3) throw n;
				m(C[le()]);
			}
			M = !1;
		} catch (e) {
			throw P = [], M = !1, e;
		}
	}
}, ue = () => {
	if (!(M || A > 0)) {
		M = !0;
		var e = 0;
		try {
			for (;;) {
				for (var t = -1, r = 0; r < P.length; r = r + 1 | 0) {
					var i = P[r] | 0;
					i != O && C[i][9] && (t < 0 || C[i][12] > C[P[t] | 0][12]) && (t = r);
				}
				if (t < 0) {
					M = !1;
					return;
				}
				if (e = e + 1 | 0, e > 1e3) throw n;
				r = te(t), m(C[r]);
			}
		} catch (e) {
			throw P = [], M = !1, e;
		}
	}
}, de = (e) => {
	var t = A == 0 && !M;
	t && (A = A + 1 | 0);
	try {
		for (var n = 0; n < e.length; n = n + 1 | 0) oe(e[n] | 0);
	} catch (e) {
		throw t && (A = A - 1 | 0), e;
	}
	t && (A = A - 1 | 0, j > 0 ? ue() : v());
}, fe = (e, t, n) => {
	var r = C.length;
	if (T.length > 0 && (r = T.pop()), O >= 0) var i = O, a = 2, o;
	else k >= 0 ? (i = k, a = 1) : (a = 0, i = -1);
	return o = [
		0,
		0,
		0,
		null,
		[],
		[],
		[],
		[],
		null,
		!1,
		!1,
		!1,
		0,
		[]
	], u(o, r, a, i, e, t), r == C.length ? C.push(o) : C[r] = o, O >= 0 ? C[O][5].push(r) : k >= 0 && w[k][3].push(r), n && (j > 0 || M) ? oe(r) : m(o), r;
}, pe = (e) => {
	var t = C[e];
	if (!t[11]) {
		f(t), p(t), ne(e), re(t), t[3] = () => {}, t[13] = [];
		var n = t[8];
		n && (n[0] = 0, t[8] = null), t[11] = !0, T.push(e);
	}
}, y = (e) => {
	var t = w[e];
	if (!t[7]) {
		for (; t[4].length > 0;) {
			var n = t[4][t[4].length - 1] | 0;
			t[4].pop(), y(n);
		}
		for (; t[3].length > 0;) n = t[3][t[3].length - 1] | 0, t[3].pop(), pe(n);
		for (; t[5].length > 0;) n = t[5][t[5].length - 1], t[5].pop(), n[0]();
		ie(t), (n = t[6]) && (n[0] = 0, t[6] = null), t[7] = !0, E.push(e);
	}
}, me = (e, t) => {
	let n = [
		null,
		[],
		null,
		0,
		0
	];
	return i(n, e, t), n;
}, b = (e) => s(e), he = (e, t, n = !1) => {
	var r = N;
	r && (n = !0), N = n;
	try {
		return c(e, t);
	} finally {
		N = r;
	}
}, ge = (e, t, n = !1) => {
	var r = N;
	r && (n = !0), N = n;
	try {
		return l(e, t);
	} finally {
		N = r;
	}
}, _e = (e) => fe(e, !1, !0), x = (e, t = null, n = !1) => {
	var i = w.length;
	E.length > 0 && (i = E.pop());
	var a;
	t && t[0] != 0 ? (a = t[0], t = t[1]) : (a = 0, t = -1);
	var o = i, s = [
		0,
		0,
		0,
		[],
		[],
		[],
		null,
		!1
	];
	r(s, o, a, t), i == w.length ? w.push(s) : w[i] = s, n && (a == 1 ? w[t][4].push(i) : a == 2 && C[t][6].push(i));
	var c = D, l = k, u = O;
	D = -1, k = i, O = -1, j = j + 1 | 0, t = () => {
		w[i] == s && y(i);
	};
	var d = !1;
	try {
		var f = e(t);
		return d = !0, f;
	} finally {
		j = j - 1 | 0, D = c, O = u, k = l, d && v();
	}
}, ve = (e) => x(e, ye(), !1), ye = () => O >= 0 ? g(2, O) : k >= 0 ? g(1, k) : null, S = () => D >= 0, be = (e) => e[2], xe = (e) => {
	var t = [e];
	return O >= 0 ? C[O][7].push(t) : k >= 0 && w[k][5].push(t), e;
}, Se = (e) => {
	var t = D;
	D = -1;
	try {
		return e();
	} finally {
		D = t;
	}
}, Ce = (e) => {
	A = A + 1 | 0;
	var t = !1;
	try {
		var n = e();
		return t = !0, n;
	} finally {
		A = A - 1 | 0, t && A == 0 && (j > 0 ? ue() : v());
	}
}, C = [], w = [], T = [], E = [], D = -1, O = -1, k = -1, A = 0, j = 0, M = !1, N = !1, P = [], we = (e, t) => e === t, F = Symbol("solid-proxy"), I = Symbol("solid-track"), Te = Symbol("solidlil-signal"), L, R, Ee = /* @__PURE__ */ new WeakMap();
function De(e) {
	return e?.equals === !1 ? () => !1 : e?.equals ?? we;
}
function Oe(e, t) {
	let n = () => {
		if (L?.[2].has(e)) return L[2].get(e);
		if (L && t) {
			if (L[1].has(e)) return L[1].get(e);
			let n = t();
			return L[1].set(e, n), n;
		}
		return b(e);
	};
	return n[Te] = e, n;
}
function ke(e, t) {
	if (L) {
		let n = L[2].has(e) ? L[2].get(e) : b(e), r = typeof t == "function" ? t(n) : t;
		return L[2].set(e, r), L[1].clear(), r;
	}
	return typeof t == "function" ? ge(e, t) : he(e, t);
}
function Ae(e) {
	return e instanceof Error ? e : Error(typeof e == "string" ? e : "Unknown error", { cause: e });
}
function z(e, t = ye()) {
	let n = Ae(e), r = t;
	for (; r;) {
		let e = Ee.get(r);
		if (e?.length) {
			for (let t of e) try {
				t(n);
			} catch (e) {
				return z(e, be(r));
			}
			return;
		}
		r = be(r);
	}
	throw n;
}
function je(e) {
	let t = Me(e);
	return (...e) => {
		try {
			return t(...e);
		} catch (e) {
			return z(e);
		}
	};
}
function Me(e) {
	if (!R) return e;
	let t = me(0, () => !1), n = R.factory(e, () => ge(t, (e) => e + 1));
	return xe(() => n.dispose()), (...e) => (b(t), n.track(...e));
}
function Ne(e, t) {
	let n = me(e, De(t));
	return [Oe(n), (e) => ke(n, e)];
}
function Pe(e, t, n) {
	let r = t, i = je(e);
	_e(() => {
		r = i(r);
	});
}
function Fe(e, t) {
	let n = !e.length, r = (t) => {
		try {
			return n ? e() : e(() => Ie(t));
		} catch (e) {
			return z(e);
		}
	};
	return n ? x(r, null) : t === void 0 ? ve(r) : x(r, t);
}
var B = Ce;
function Ie(e) {
	return Se(R ? () => R.untrack(e) : e);
}
//#endregion
//#region packages/solidlil/store.js
var Le = /* @__PURE__ */ t({
	$RAW: () => V,
	DEV: () => tt,
	createMutable: () => Ye,
	createStore: () => Ge,
	modifyMutable: () => Xe,
	produce: () => et,
	reconcile: () => Qe,
	unwrap: () => G
}), V = Symbol("store-raw"), H = Symbol("store-node"), U = Symbol("store-has"), Re = Symbol("store-self");
function ze(e) {
	let t = e[F];
	if (!t && (Object.defineProperty(e, F, { value: t = new Proxy(e, He) }), !Array.isArray(e))) {
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
function W(e) {
	let t;
	return typeof e == "object" && !!e && (e[F] || !(t = Object.getPrototypeOf(e)) || t === Object.prototype || Array.isArray(e));
}
function G(e, t = /* @__PURE__ */ new Set()) {
	let n, r, i, a;
	if (n = e != null && e[V]) return n;
	if (!W(e) || t.has(e)) return e;
	if (Array.isArray(e)) {
		Object.isFrozen(e) ? e = e.slice(0) : t.add(e);
		for (let n = 0, a = e.length; n < a; n++) i = e[n], (r = G(i, t)) !== i && (e[n] = r);
	} else {
		Object.isFrozen(e) ? e = Object.assign({}, e) : t.add(e);
		let n = Object.keys(e), o = Object.getOwnPropertyDescriptors(e);
		for (let s = 0, c = n.length; s < c; s++) a = n[s], !o[a].get && (i = e[a], (r = G(i, t)) !== i && (e[a] = r));
	}
	return e;
}
function K(e, t) {
	let n = e[t];
	return n || Object.defineProperty(e, t, { value: n = Object.create(null) }), n;
}
function q(e, t, n) {
	if (e[t]) return e[t];
	let [r, i] = Ne(n, { equals: !1 });
	return r.$ = i, e[t] = r;
}
function Be(e, t) {
	let n = Reflect.getOwnPropertyDescriptor(e, t);
	return !n || n.get || !n.configurable || t === F || t === H ? n : (delete n.value, delete n.writable, n.get = () => e[F][t], n);
}
function J(e) {
	S() && q(K(e, H), Re)();
}
function Ve(e) {
	return J(e), Reflect.ownKeys(e);
}
var He = {
	get(e, t, n) {
		if (t === V) return e;
		if (t === F) return n;
		if (t === I) return J(e), n;
		let r = K(e, H), i = r[t], a = i ? i() : e[t];
		if (t === H || t === U || t === "__proto__") return a;
		if (!i) {
			let n = Object.getOwnPropertyDescriptor(e, t);
			S() && (typeof a != "function" || e.hasOwnProperty(t)) && !(n && n.get) && (a = q(r, t, a)());
		}
		return W(a) ? ze(a) : a;
	},
	has(e, t) {
		return t === V || t === F || t === I || t === H || t === U || t === "__proto__" || (S() && q(K(e, U), t)(), t in e);
	},
	set() {
		return !0;
	},
	deleteProperty() {
		return !0;
	},
	ownKeys: Ve,
	getOwnPropertyDescriptor: Be
};
function Y(e, t, n, r = !1) {
	if (t === "__proto__" || !r && e[t] === n) return;
	let i = e[t], a = e.length;
	n === void 0 ? (delete e[t], e[U] && e[U][t] && i !== void 0 && e[U][t].$()) : (e[t] = n, e[U] && e[U][t] && i === void 0 && e[U][t].$());
	let o = K(e, H), s;
	if ((s = q(o, t, i)) && s.$(() => n), Array.isArray(e) && e.length !== a) {
		for (let t = e.length; t < a; t++) (s = o[t]) && s.$();
		(s = q(o, "length", a)) && s.$(e.length);
	}
	(s = o[Re]) && s.$();
}
function Ue(e, t) {
	let n = Object.keys(t);
	for (let r = 0; r < n.length; r += 1) {
		let i = n[r];
		X(i) || Y(e, i, t[i]);
	}
}
function X(e) {
	return e === "__proto__" || e === "constructor" || e === "prototype";
}
function We(e, t) {
	if (typeof t == "function" && (t = t(e)), t = G(t), Array.isArray(t)) {
		if (e === t) return;
		let n = 0, r = t.length;
		for (; n < r; n++) {
			let r = t[n];
			e[n] !== r && Y(e, n, r);
		}
		Y(e, "length", r);
	} else Ue(e, t);
}
function Z(e, t, n = []) {
	let r, i = e;
	if (t.length > 1) {
		r = t.shift();
		let a = typeof r, o = Array.isArray(e);
		if (a === "string" && (r === "__proto__" || t.length > 1 && X(r))) return;
		if (Array.isArray(r)) {
			for (let i = 0; i < r.length; i++) Z(e, [r[i]].concat(t), n);
			return;
		}
		if (o && a === "function") {
			for (let i = 0; i < e.length; i++) r(e[i], i) && Z(e, [i].concat(t), n);
			return;
		}
		if (o && a === "object") {
			let { from: i = 0, to: a = e.length - 1, by: o = 1 } = r;
			for (let r = i; r <= a; r += o) Z(e, [r].concat(t), n);
			return;
		}
		if (t.length > 1) {
			Z(e[r], t, [r].concat(n));
			return;
		}
		i = e[r], n = [r].concat(n);
	}
	let a = t[0];
	typeof a == "function" && (a = a(i, n), a === i) || (r !== void 0 || a != null) && (a = G(a), r === void 0 || W(i) && W(a) && !Array.isArray(a) ? Ue(i, a) : Y(e, r, a));
}
function Ge(...[e, t]) {
	let n = G(e || {}), r = Array.isArray(n), i = ze(n);
	function a(...e) {
		B(() => {
			r && e.length === 1 ? We(n, e[0]) : Z(n, e);
		});
	}
	return [i, a];
}
function Ke(e, t) {
	let n = Reflect.getOwnPropertyDescriptor(e, t);
	return !n || n.get || n.set || !n.configurable || t === F || t === H ? n : (delete n.value, delete n.writable, n.get = () => e[F][t], n.set = (n) => e[F][t] = n, n);
}
var qe = {
	get(e, t, n) {
		if (t === V) return e;
		if (t === F) return n;
		if (t === I) return J(e), n;
		let r = K(e, H), i = r[t], a = i ? i() : e[t];
		if (t === H || t === U || t === "__proto__") return a;
		if (!i) {
			let i = Object.getOwnPropertyDescriptor(e, t), o = typeof a == "function";
			if (S() && (!o || e.hasOwnProperty(t)) && !(i && i.get)) a = q(r, t, a)();
			else if (a != null && o && a === Array.prototype[t]) return (...e) => B(() => Array.prototype[t].apply(n, e));
		}
		return W(a) ? Je(a) : a;
	},
	has(e, t) {
		return t === V || t === F || t === I || t === H || t === U || t === "__proto__" || (S() && q(K(e, U), t)(), t in e);
	},
	set(e, t, n) {
		return B(() => Y(e, t, G(n))), !0;
	},
	deleteProperty(e, t) {
		return B(() => Y(e, t, void 0, !0)), !0;
	},
	ownKeys: Ve,
	getOwnPropertyDescriptor: Ke
};
function Je(e) {
	let t = e[F];
	if (!t) {
		Object.defineProperty(e, F, { value: t = new Proxy(e, qe) });
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
						set: (e) => B(() => n.call(t, e)),
						configurable: !0
					});
				}
			}
		}
	}
	return t;
}
function Ye(e, t) {
	return Je(G(e || {}));
}
function Xe(e, t) {
	B(() => t(G(e)));
}
var Ze = Symbol("store-root");
function Q(e, t, n, r, i) {
	if (X(n)) return;
	let a = t[n];
	if (e === a) return;
	let o = Array.isArray(e);
	if (n !== Ze && (!W(e) || !W(a) || o !== Array.isArray(a) || i && e[i] !== a[i])) {
		Y(t, n, e);
		return;
	}
	if (o) {
		if (e.length && a.length && (!r || i && e[0] && e[0][i] != null)) {
			let t, n, o, s, c, l, u, d;
			for (o = 0, s = Math.min(a.length, e.length); o < s && (a[o] === e[o] || i && a[o] && e[o] && a[o][i] && a[o][i] === e[o][i]); o++) Q(e[o], a, o, r, i);
			let f = Array(e.length), p = /* @__PURE__ */ new Map();
			for (s = a.length - 1, c = e.length - 1; s >= o && c >= o && (a[s] === e[c] || i && a[s] && e[c] && a[s][i] && a[s][i] === e[c][i]); s--, c--) f[c] = a[s];
			if (o > c || o > s) {
				for (n = o; n <= c; n++) Y(a, n, e[n]);
				for (; n < e.length; n++) Y(a, n, f[n]), Q(e[n], a, n, r, i);
				a.length > e.length && Y(a, "length", e.length);
				return;
			}
			for (u = Array(c + 1), n = c; n >= o; n--) l = e[n], d = i && l ? l[i] : l, t = p.get(d), u[n] = t === void 0 ? -1 : t, p.set(d, n);
			for (t = o; t <= s; t++) l = a[t], d = i && l ? l[i] : l, n = p.get(d), n !== void 0 && n !== -1 && (f[n] = a[t], n = u[n], p.set(d, n));
			for (n = o; n < e.length; n++) n in f ? (Y(a, n, f[n]), Q(e[n], a, n, r, i)) : Y(a, n, e[n]);
		} else for (let t = 0, n = e.length; t < n; t++) Q(e[t], a, t, r, i);
		a.length > e.length && Y(a, "length", e.length);
		return;
	}
	let s = Object.keys(e);
	for (let t = 0, n = s.length; t < n; t++) X(s[t]) || Q(e[s[t]], a, s[t], r, i);
	let c = Object.keys(a);
	for (let t = 0, n = c.length; t < n; t++) e[c[t]] === void 0 && Y(a, c[t], void 0);
}
function Qe(e, t = {}) {
	let { merge: n, key: r = "id" } = t, i = G(e);
	return (e) => {
		if (!W(e) || !W(i)) return i;
		let t = Q(i, { [Ze]: e }, Ze, n, r);
		return t === void 0 ? e : t;
	};
}
var $ = /* @__PURE__ */ new WeakMap(), $e = {
	get(e, t) {
		if (t === V) return e;
		let n = e[t];
		if (t === F || t === I || t === H || t === U || t === "__proto__") return n;
		let r;
		return W(n) ? $.get(n) || ($.set(n, r = new Proxy(n, $e)), r) : n;
	},
	set(e, t, n) {
		return Y(e, t, G(n)), !0;
	},
	deleteProperty(e, t) {
		return Y(e, t, void 0, !0), !0;
	}
};
function et(e) {
	return (t) => {
		if (W(t)) {
			let n;
			(n = $.get(t)) || $.set(t, n = new Proxy(t, $e)), e(n);
		}
		return t;
	};
}
var tt = void 0;
//#endregion
export { Pe as createEffect, Fe as createRoot, Le as store };
