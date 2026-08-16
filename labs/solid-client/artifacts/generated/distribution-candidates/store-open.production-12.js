//#region packages/solidlil/reactive.generated.js
var e = "Potential Infinite Loop Detected.";
function t(e, t, n) {
	e[0] = t, e[1] = [], e[2] = n, e[3] = 0, e[4] = -1;
}
function n(e, t) {
	for (var n = 0; n < e[1].length; n++) if ((e[1][n] | 0) == t) return !1;
	return e[1].push(t), !0;
}
function r(e, t) {
	for (var n = 0; n < e[1].length; n++) if ((e[1][n] | 0) == t) {
		e[1][n] = e[1][e[1].length - 1] | 0, e[1].pop();
		return;
	}
}
function i(e) {
	var t;
	if (T < 0 && e[4] < 0) return e[0];
	if (T >= 0 && e[4] >= 0 && e[4] == E) throw "Circular memo dependency detected.";
	if (e[4] >= 0 && e[4] != E ? (t = E >= 0 && !C[E][9] && !C[E][10], t = !t) : t = !1, t && _(e[4]), T >= 0) {
		t = T;
		var r = e[3] + 1 | 0;
		r > C[t][12] && (C[t][12] = r), e[4] >= 0 && s(C[t], e[4]), n(e, t) && C[t][4].push(e);
	}
	return e[0];
}
function a(e, t) {
	return E >= 0 && C[E][9] && (e[3] = C[E][12], e[4] = E), e[2](e[0], t) ? t : (e[0] = t, ae(e[1]), t);
}
function o(e, t) {
	return a(e, t(e[0]));
}
function s(e, t) {
	for (var n = 0; n < e[13].length; n++) if ((e[13][n] | 0) == t) return;
	e[13].push(t);
}
function c(e) {
	for (; e[6].length > 0;) {
		var t = e[6][e[6].length - 1] | 0;
		e[6].pop(), b(t);
	}
	for (; e[5].length > 0;) t = e[5][e[5].length - 1] | 0, e[5].pop(), oe(t);
	for (; e[7].length > 0;) t = e[7][e[7].length - 1], e[7].pop(), t[0]();
}
function l(e) {
	for (; e[4].length > 0;) {
		var t = e[4][e[4].length - 1];
		e[4].pop(), r(t, e[0]);
	}
}
function u(e) {
	if (!e[11]) {
		c(e), l(e);
		var t = T, n = E, r = D;
		T = e[0], E = e[0], D = -1, e[12] = 0, e[13] = [], k = k + 1 | 0, e[3](), e[10] = !0, k = k - 1 | 0, T = t, E = n, D = r, v();
	}
}
var d = (e) => {
	for (var t = 0; t < M.length; t++) if ((M[t] | 0) == e) return !0;
	return !1;
}, f = (e) => {
	for (var t = M[e] | 0; e < M.length - 1; e = e + 1 | 0) M[e] = M[e + 1 | 0] | 0;
	return M.pop(), t;
}, p = (e) => {
	for (var t = 0; t < M.length; t++) if ((M[t] | 0) == e) return f(t), !0;
	return !1;
}, m = (e, t) => {
	for (var n = e.length - 1; n >= 0; n--) if ((e[n] | 0) == t) {
		e[n] = e[e.length - 1] | 0, e.pop();
		return;
	}
}, ee = (e) => {
	e[1] == 1 ? m(w[e[2]][3], e[0]) : e[1] == 2 && m(C[e[2]][5], e[0]);
}, te = (e) => {
	e[1] == 1 ? m(w[e[2]][4], e[0]) : e[1] == 2 && m(C[e[2]][6], e[0]);
}, h = (e, t, n = 0) => {
	if (e == t) return !0;
	if (n >= C.length) return !1;
	for (var r = 0; r < C[e][13].length; r++) if (h(C[e][13][r] | 0, t, n + 1 | 0)) return !0;
	return !1;
}, ne = (e) => {
	if (E >= 0 && C[E][9] && !j && h(E, e) && !g(e, E)) throw "Reactive dependency cycle detected.";
	!C[e][11] && !d(e) && M.push(e);
}, re = (e) => {
	for (var t = 0; t < C[e][13].length; t++) if (d(C[e][13][t] | 0)) return !0;
	return !1;
}, g = (e, t) => {
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
}, _ = (e) => {
	var t, n;
	if (!(C[e][11] || e == E)) {
		for (n = 0; n < C[e][13].length; n++) t = C[e][13][n] | 0, (d(t) || re(t)) && _(t);
		p(e) && u(C[e]);
	}
}, ie = () => {
	for (var e = -1, t = -1, n = -1, r = 0, i, a, o, s, c; r < M.length; r++) i = M[r] | 0, C[i][9] ? (e < 0 || C[i][12] > C[M[e] | 0][12]) && (e = r) : !C[i][10] && t < 0 ? t = r : n < 0 ? n = r : (o = M[n] | 0, a = C[i], s = C[o], c = a[1] == s[1] && a[2] == s[2], (c && a[12] > s[12] || !c && (g(i, o) || !g(o, i) && a[12] > s[12])) && (n = r));
	return t >= 0 && (e = t), f(e < 0 ? n : e);
}, v = () => {
	if (!(A || O > 0 || k > 0)) {
		A = !0;
		var t = 0;
		try {
			for (; M.length > 0;) {
				if (t = t + 1 | 0, t > 1e3) throw e;
				u(C[ie()]);
			}
			A = !1;
		} catch (e) {
			throw M = [], A = !1, e;
		}
	}
}, y = () => {
	if (!(A || O > 0)) {
		A = !0;
		var t = 0;
		try {
			for (;;) {
				for (var n = -1, r = 0; r < M.length; r = r + 1 | 0) {
					var i = M[r] | 0;
					i != E && C[i][9] && (n < 0 || C[i][12] > C[M[n] | 0][12]) && (n = r);
				}
				if (n < 0) {
					A = !1;
					return;
				}
				if (t = t + 1 | 0, t > 1e3) throw e;
				r = f(n), u(C[r]);
			}
		} catch (e) {
			throw M = [], A = !1, e;
		}
	}
}, ae = (e) => {
	var t = O == 0 && !A;
	t && (O = O + 1 | 0);
	try {
		for (var n = 0; n < e.length; n = n + 1 | 0) ne(e[n] | 0);
	} catch (e) {
		throw t && (O = O - 1 | 0), e;
	}
	t && (O = O - 1 | 0, k > 0 ? y() : v());
}, oe = (e) => {
	var t = C[e];
	if (!t[11]) {
		c(t), l(t), p(e), ee(t), t[3] = () => {}, t[13] = [];
		var n = t[8];
		n && (n[0] = 0, t[8] = null), t[11] = !0, de.push(e);
	}
}, b = (e) => {
	var t = w[e];
	if (!t[7]) {
		for (; t[4].length > 0;) {
			var n = t[4][t[4].length - 1] | 0;
			t[4].pop(), b(n);
		}
		for (; t[3].length > 0;) n = t[3][t[3].length - 1] | 0, t[3].pop(), oe(n);
		for (; t[5].length > 0;) n = t[5][t[5].length - 1], t[5].pop(), n[0]();
		te(t), (n = t[6]) && (n[0] = 0, t[6] = null), t[7] = !0, fe.push(e);
	}
}, se = (e, n) => {
	let r = [
		null,
		[],
		null,
		0,
		0
	];
	return t(r, e, n), r;
}, x = (e) => i(e), ce = (e, t, n = !1) => {
	var r = j;
	r && (n = !0), j = n;
	try {
		return a(e, t);
	} finally {
		j = r;
	}
}, le = (e, t, n = !1) => {
	var r = j;
	r && (n = !0), j = n;
	try {
		return o(e, t);
	} finally {
		j = r;
	}
}, S = () => T >= 0, ue = (e) => {
	O = O + 1 | 0;
	var t = !1;
	try {
		var n = e();
		return t = !0, n;
	} finally {
		O = O - 1 | 0, t && O == 0 && (k > 0 ? y() : v());
	}
}, C = [], w = [], de = [], fe = [], T = -1, E = -1, D = -1, O = 0, k = 0, A = !1, j = !1, M = [], pe = (e, t) => e === t, N = Symbol("solid-proxy"), P = Symbol("solid-track"), me = Symbol("solidlil-signal"), F;
function he(e) {
	return e?.equals === !1 ? () => !1 : e?.equals ?? pe;
}
function ge(e, t) {
	let n = () => {
		if (F?.[2].has(e)) return F[2].get(e);
		if (F && t) {
			if (F[1].has(e)) return F[1].get(e);
			let n = t();
			return F[1].set(e, n), n;
		}
		return x(e);
	};
	return n[me] = e, n;
}
function _e(e, t) {
	if (F) {
		let n = F[2].has(e) ? F[2].get(e) : x(e), r = typeof t == "function" ? t(n) : t;
		return F[2].set(e, r), F[1].clear(), r;
	}
	return typeof t == "function" ? le(e, t) : ce(e, t);
}
function ve(e, t) {
	let n = se(e, he(t));
	return [ge(n), (e) => _e(n, e)];
}
var I = ue, L = Symbol("store-raw"), R = Symbol("store-node"), z = Symbol("store-has"), B = Symbol("store-self");
function V(e) {
	let t = e[N];
	if (!t && (Object.defineProperty(e, N, { value: t = new Proxy(e, be) }), !Array.isArray(e))) {
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
	return typeof e == "object" && !!e && (e[N] || !(t = Object.getPrototypeOf(e)) || t === Object.prototype || Array.isArray(e));
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
	let [r, i] = ve(n, { equals: !1 });
	return r.$ = i, e[t] = r;
}
function ye(e, t) {
	let n = Reflect.getOwnPropertyDescriptor(e, t);
	return !n || n.get || !n.configurable || t === N || t === R ? n : (delete n.value, delete n.writable, n.get = () => e[N][t], n);
}
function K(e) {
	S() && G(W(e, R), B)();
}
function q(e) {
	return K(e), Reflect.ownKeys(e);
}
var be = {
	get(e, t, n) {
		if (t === L) return e;
		if (t === N) return n;
		if (t === P) return K(e), n;
		let r = W(e, R), i = r[t], a = i ? i() : e[t];
		if (t === R || t === z || t === "__proto__") return a;
		if (!i) {
			let n = Object.getOwnPropertyDescriptor(e, t);
			S() && (typeof a != "function" || e.hasOwnProperty(t)) && !(n && n.get) && (a = G(r, t, a)());
		}
		return H(a) ? V(a) : a;
	},
	has(e, t) {
		return t === L || t === N || t === P || t === R || t === z || t === "__proto__" || (S() && G(W(e, z), t)(), t in e);
	},
	set() {
		return !0;
	},
	deleteProperty() {
		return !0;
	},
	ownKeys: q,
	getOwnPropertyDescriptor: ye
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
function xe(e, t) {
	let n = Object.keys(t);
	for (let r = 0; r < n.length; r += 1) {
		let i = n[r];
		Y(i) || J(e, i, t[i]);
	}
}
function Y(e) {
	return e === "__proto__" || e === "constructor" || e === "prototype";
}
function Se(e, t) {
	if (typeof t == "function" && (t = t(e)), t = U(t), Array.isArray(t)) {
		if (e === t) return;
		let n = 0, r = t.length;
		for (; n < r; n++) {
			let r = t[n];
			e[n] !== r && J(e, n, r);
		}
		J(e, "length", r);
	} else xe(e, t);
}
function X(e, t, n = []) {
	let r, i = e;
	if (t.length > 1) {
		r = t.shift();
		let a = typeof r, o = Array.isArray(e);
		if (a === "string" && (r === "__proto__" || t.length > 1 && Y(r))) return;
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
	typeof a == "function" && (a = a(i, n), a === i) || (r !== void 0 || a != null) && (a = U(a), r === void 0 || H(i) && H(a) && !Array.isArray(a) ? xe(i, a) : J(e, r, a));
}
function Ce(...[e, t]) {
	let n = U(e || {}), r = Array.isArray(n), i = V(n);
	function a(...e) {
		I(() => {
			r && e.length === 1 ? Se(n, e[0]) : X(n, e);
		});
	}
	return [i, a];
}
function we(e, t) {
	let n = Reflect.getOwnPropertyDescriptor(e, t);
	return !n || n.get || n.set || !n.configurable || t === N || t === R ? n : (delete n.value, delete n.writable, n.get = () => e[N][t], n.set = (n) => e[N][t] = n, n);
}
var Te = {
	get(e, t, n) {
		if (t === L) return e;
		if (t === N) return n;
		if (t === P) return K(e), n;
		let r = W(e, R), i = r[t], a = i ? i() : e[t];
		if (t === R || t === z || t === "__proto__") return a;
		if (!i) {
			let i = Object.getOwnPropertyDescriptor(e, t), o = typeof a == "function";
			if (S() && (!o || e.hasOwnProperty(t)) && !(i && i.get)) a = G(r, t, a)();
			else if (a != null && o && a === Array.prototype[t]) return (...e) => I(() => Array.prototype[t].apply(n, e));
		}
		return H(a) ? Ee(a) : a;
	},
	has(e, t) {
		return t === L || t === N || t === P || t === R || t === z || t === "__proto__" || (S() && G(W(e, z), t)(), t in e);
	},
	set(e, t, n) {
		return I(() => J(e, t, U(n))), !0;
	},
	deleteProperty(e, t) {
		return I(() => J(e, t, void 0, !0)), !0;
	},
	ownKeys: q,
	getOwnPropertyDescriptor: we
};
function Ee(e) {
	let t = e[N];
	if (!t) {
		Object.defineProperty(e, N, { value: t = new Proxy(e, Te) });
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
						set: (e) => I(() => n.call(t, e)),
						configurable: !0
					});
				}
			}
		}
	}
	return t;
}
function De(e, t) {
	return Ee(U(e || {}));
}
function Oe(e, t) {
	I(() => t(U(e)));
}
var Z = Symbol("store-root");
function Q(e, t, n, r, i) {
	if (Y(n)) return;
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
	for (let t = 0, n = s.length; t < n; t++) Y(s[t]) || Q(e[s[t]], a, s[t], r, i);
	let c = Object.keys(a);
	for (let t = 0, n = c.length; t < n; t++) e[c[t]] === void 0 && J(a, c[t], void 0);
}
function ke(e, t = {}) {
	let { merge: n, key: r = "id" } = t, i = U(e);
	return (e) => {
		if (!H(e) || !H(i)) return i;
		let t = Q(i, { [Z]: e }, Z, n, r);
		return t === void 0 ? e : t;
	};
}
var $ = /* @__PURE__ */ new WeakMap(), Ae = {
	get(e, t) {
		if (t === L) return e;
		let n = e[t];
		if (t === N || t === P || t === R || t === z || t === "__proto__") return n;
		let r;
		return H(n) ? $.get(n) || ($.set(n, r = new Proxy(n, Ae)), r) : n;
	},
	set(e, t, n) {
		return J(e, t, U(n)), !0;
	},
	deleteProperty(e, t) {
		return J(e, t, void 0, !0), !0;
	}
};
function je(e) {
	return (t) => {
		if (H(t)) {
			let n;
			(n = $.get(t)) || $.set(t, n = new Proxy(t, Ae)), e(n);
		}
		return t;
	};
}
var Me = void 0;
//#endregion
export { L as $RAW, Me as DEV, De as createMutable, Ce as createStore, Oe as modifyMutable, je as produce, ke as reconcile, U as unwrap };
