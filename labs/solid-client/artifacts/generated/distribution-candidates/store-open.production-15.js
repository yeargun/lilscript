//#region packages/solidlil/reactive.generated.js
var e = "Potential Infinite Loop Detected.", t = () => E >= 0, n = (e) => ce(e), r = (e) => {
	e[1] == 1 ? h(T[e[2]][4], e[0]) : e[1] == 2 && h(w[e[2]][6], e[0]);
}, i = (e) => {
	e[1] == 1 ? h(T[e[2]][3], e[0]) : e[1] == 2 && h(w[e[2]][5], e[0]);
}, a = (e, t) => {
	let n = [
		null,
		[],
		null,
		0,
		0
	];
	return ie(n, e, t), n;
}, o = (e) => {
	var t = k == 0 && !j;
	t && (k = k + 1 | 0);
	try {
		for (var n = 0; n < e.length; n = n + 1 | 0) le(e[n] | 0);
	} catch (e) {
		throw t && (k = k - 1 | 0), e;
	}
	t && (k = k - 1 | 0, A > 0 ? v() : _());
}, s = (e) => {
	k = k + 1 | 0;
	var t = !1;
	try {
		var n = e();
		return t = !0, n;
	} finally {
		k = k - 1 | 0, t && k == 0 && (A > 0 ? v() : _());
	}
};
function c(e, t) {
	for (var n = 0; n < e[1].length; n += 1) if ((e[1][n] | 0) == t) {
		e[1][n] = e[1][e[1].length - 1] | 0, e[1].pop();
		return;
	}
}
function l(e, t) {
	for (var n = 0; n < e[1].length; n += 1) if ((e[1][n] | 0) == t) return !1;
	return e[1].push(t), !0;
}
function u(e, t) {
	for (var n = 0; n < e[13].length; n += 1) if ((e[13][n] | 0) == t) return;
	e[13].push(t);
}
var d = (e) => {
	var t, n;
	if (!(w[e][11] || e == D)) {
		for (n = 0; n < w[e][13].length; n += 1) t = w[e][13][n] | 0, (m(t) || p(t)) && d(t);
		ee(e) && S(w[e]);
	}
}, f = (e, t, n) => {
	if (e == t) return !0;
	if (n >= w.length) return !1;
	for (var r = 0; r < w[e][13].length; r += 1) if (f(w[e][13][r] | 0, t, n + 1 | 0)) return !0;
	return !1;
}, p = (e) => {
	for (var t = 0; t < w[e][13].length; t += 1) if (m(w[e][13][t] | 0)) return !0;
	return !1;
}, m = (e) => {
	for (var t = 0; t < N.length; t += 1) if ((N[t] | 0) == e) return !0;
	return !1;
}, ee = (e) => {
	for (var t = 0; t < N.length; t += 1) if ((N[t] | 0) == e) return y(t), !0;
	return !1;
}, h = (e, t) => {
	for (var n = e.length - 1; n >= 0; --n) if ((e[n] | 0) == t) {
		e[n] = e[e.length - 1] | 0, e.pop();
		return;
	}
}, g = (e, t) => {
	var n = w[t][1];
	t = w[t][2];
	for (var r = 0, i; i = n != 0 && r <= (w.length + T.length | 0), i;) {
		if (n == 2) {
			if (t == e) return !0;
			n = w[t][1], t = w[t][2];
		} else n = T[t][1], t = T[t][2];
		r = r + 1 | 0;
	}
	return !1;
}, _ = () => {
	if (!(j || k > 0 || A > 0)) {
		j = !0;
		var t = 0;
		try {
			for (; N.length > 0;) {
				if (t = t + 1 | 0, t > 1e3) throw e;
				S(w[te()]);
			}
			j = !1;
		} catch (e) {
			throw N = [], j = !1, e;
		}
	}
}, v = () => {
	if (!(j || k > 0)) {
		j = !0;
		var t = 0;
		try {
			for (;;) {
				for (var n = -1, r = 0; r < N.length; r = r + 1 | 0) {
					var i = N[r] | 0;
					i != D && w[i][9] && (n < 0 || w[i][12] > w[N[n] | 0][12]) && (n = r);
				}
				if (n < 0) {
					j = !1;
					return;
				}
				if (t = t + 1 | 0, t > 1e3) throw e;
				r = y(n), S(w[r]);
			}
		} catch (e) {
			throw N = [], j = !1, e;
		}
	}
}, te = () => {
	for (var e = -1, t = -1, n = -1, r = 0, i, a, o, s, c; r < N.length; r += 1) i = N[r] | 0, w[i][9] ? (e < 0 || w[i][12] > w[N[e] | 0][12]) && (e = r) : !w[i][10] && t < 0 ? t = r : n < 0 ? n = r : (o = N[n] | 0, a = w[i], s = w[o], c = a[1] == s[1] && a[2] == s[2], (c && a[12] > s[12] || !c && (g(i, o) || !g(o, i) && a[12] > s[12])) && (n = r));
	return t >= 0 && (e = t), y(e < 0 ? n : e);
}, y = (e) => {
	for (var t = N[e] | 0; e < N.length - 1; e = e + 1 | 0) N[e] = N[e + 1 | 0] | 0;
	return N.pop(), t;
};
function b(e) {
	for (; e[4].length > 0;) {
		var t = e[4][e[4].length - 1];
		e[4].pop(), c(t, e[0]);
	}
}
function x(e) {
	for (; e[6].length > 0;) {
		var t = e[6][e[6].length - 1] | 0;
		e[6].pop(), ne(t);
	}
	for (; e[5].length > 0;) t = e[5][e[5].length - 1] | 0, e[5].pop(), re(t);
	for (; e[7].length > 0;) t = e[7][e[7].length - 1], e[7].pop(), t[0]();
}
var ne = (e) => {
	var t = T[e];
	if (!t[7]) {
		for (; t[4].length > 0;) {
			var n = t[4][t[4].length - 1] | 0;
			t[4].pop(), ne(n);
		}
		for (; t[3].length > 0;) n = t[3][t[3].length - 1] | 0, t[3].pop(), re(n);
		for (; t[5].length > 0;) n = t[5][t[5].length - 1], t[5].pop(), n[0]();
		r(t), (n = t[6]) && (n[0] = 0, t[6] = null), t[7] = !0, de.push(e);
	}
}, re = (e) => {
	var t = w[e];
	if (!t[11]) {
		x(t), b(t), ee(e), i(t), t[3] = () => {}, t[13] = [];
		var n = t[8];
		n && (n[0] = 0, t[8] = null), t[11] = !0, ue.push(e);
	}
};
function S(e) {
	if (!e[11]) {
		x(e), b(e);
		var t = E, n = D, r = O;
		E = e[0], D = e[0], O = -1, e[12] = 0, e[13] = [], A = A + 1 | 0, e[3](), e[10] = !0, A = A - 1 | 0, E = t, D = n, O = r, _();
	}
}
function ie(e, t, n) {
	e[0] = t, e[1] = [], e[2] = n, e[3] = 0, e[4] = -1;
}
function ae(e, t) {
	return C(e, t(e[0]));
}
var oe = (e, t, n = !1) => {
	var r = M;
	r && (n = !0), M = n;
	try {
		return C(e, t);
	} finally {
		M = r;
	}
}, se = (e, t, n = !1) => {
	var r = M;
	r && (n = !0), M = n;
	try {
		return ae(e, t);
	} finally {
		M = r;
	}
};
function ce(e) {
	var t;
	if (E < 0 && e[4] < 0) return e[0];
	if (E >= 0 && e[4] >= 0 && e[4] == D) throw "Circular memo dependency detected.";
	if (e[4] >= 0 && e[4] != D ? (t = D >= 0 && !w[D][9] && !w[D][10], t = !t) : t = !1, t && d(e[4]), E >= 0) {
		t = E;
		var n = e[3] + 1 | 0;
		n > w[t][12] && (w[t][12] = n), e[4] >= 0 && u(w[t], e[4]), l(e, t) && w[t][4].push(e);
	}
	return e[0];
}
var le = (e) => {
	if (D >= 0 && w[D][9] && !M && f(D, e, 0) && !g(e, D)) throw "Reactive dependency cycle detected.";
	!w[e][11] && !m(e) && N.push(e);
};
function C(e, t) {
	return D >= 0 && w[D][9] && (e[3] = w[D][12], e[4] = D), e[2](e[0], t) ? t : (e[0] = t, o(e[1]), t);
}
var w = [], T = [], ue = [], de = [], E = -1, D = -1, O = -1, k = 0, A = 0, j = !1, M = !1, N = [], fe = (e, t) => e === t, P = Symbol("solid-proxy"), F = Symbol("solid-track"), pe = Symbol("solidlil-signal"), I;
function me(e) {
	return e?.equals === !1 ? () => !1 : e?.equals ?? fe;
}
function he(e, t) {
	let r = () => {
		if (I?.[2].has(e)) return I[2].get(e);
		if (I && t) {
			if (I[1].has(e)) return I[1].get(e);
			let n = t();
			return I[1].set(e, n), n;
		}
		return n(e);
	};
	return r[pe] = e, r;
}
function ge(e, t) {
	if (I) {
		let r = I[2].has(e) ? I[2].get(e) : n(e), i = typeof t == "function" ? t(r) : t;
		return I[2].set(e, i), I[1].clear(), i;
	}
	return typeof t == "function" ? se(e, t) : oe(e, t);
}
function _e(e, t) {
	let n = a(e, me(t));
	return [he(n), (e) => ge(n, e)];
}
var L = s, R = Symbol("store-raw"), z = Symbol("store-node"), B = Symbol("store-has"), V = Symbol("store-self");
function H(e) {
	let t = e[P];
	if (!t && (Object.defineProperty(e, P, { value: t = new Proxy(e, be) }), !Array.isArray(e))) {
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
function U(e) {
	let t;
	return typeof e == "object" && !!e && (e[P] || !(t = Object.getPrototypeOf(e)) || t === Object.prototype || Array.isArray(e));
}
function W(e, t = /* @__PURE__ */ new Set()) {
	let n, r, i, a;
	if (n = e != null && e[R]) return n;
	if (!U(e) || t.has(e)) return e;
	if (Array.isArray(e)) {
		Object.isFrozen(e) ? e = e.slice(0) : t.add(e);
		for (let n = 0, a = e.length; n < a; n++) i = e[n], (r = W(i, t)) !== i && (e[n] = r);
	} else {
		Object.isFrozen(e) ? e = Object.assign({}, e) : t.add(e);
		let n = Object.keys(e), o = Object.getOwnPropertyDescriptors(e);
		for (let s = 0, c = n.length; s < c; s++) a = n[s], !o[a].get && (i = e[a], (r = W(i, t)) !== i && (e[a] = r));
	}
	return e;
}
function G(e, t) {
	let n = e[t];
	return n || Object.defineProperty(e, t, { value: n = Object.create(null) }), n;
}
function K(e, t, n) {
	if (e[t]) return e[t];
	let [r, i] = _e(n, { equals: !1 });
	return r.$ = i, e[t] = r;
}
function ve(e, t) {
	let n = Reflect.getOwnPropertyDescriptor(e, t);
	return !n || n.get || !n.configurable || t === P || t === z ? n : (delete n.value, delete n.writable, n.get = () => e[P][t], n);
}
function q(e) {
	t() && K(G(e, z), V)();
}
function ye(e) {
	return q(e), Reflect.ownKeys(e);
}
var be = {
	get(e, n, r) {
		if (n === R) return e;
		if (n === P) return r;
		if (n === F) return q(e), r;
		let i = G(e, z), a = i[n], o = a ? a() : e[n];
		if (n === z || n === B || n === "__proto__") return o;
		if (!a) {
			let r = Object.getOwnPropertyDescriptor(e, n);
			t() && (typeof o != "function" || e.hasOwnProperty(n)) && !(r && r.get) && (o = K(i, n, o)());
		}
		return U(o) ? H(o) : o;
	},
	has(e, n) {
		return n === R || n === P || n === F || n === z || n === B || n === "__proto__" || (t() && K(G(e, B), n)(), n in e);
	},
	set() {
		return !0;
	},
	deleteProperty() {
		return !0;
	},
	ownKeys: ye,
	getOwnPropertyDescriptor: ve
};
function J(e, t, n, r = !1) {
	if (t === "__proto__" || !r && e[t] === n) return;
	let i = e[t], a = e.length;
	n === void 0 ? (delete e[t], e[B] && e[B][t] && i !== void 0 && e[B][t].$()) : (e[t] = n, e[B] && e[B][t] && i === void 0 && e[B][t].$());
	let o = G(e, z), s;
	if ((s = K(o, t, i)) && s.$(() => n), Array.isArray(e) && e.length !== a) {
		for (let t = e.length; t < a; t++) (s = o[t]) && s.$();
		(s = K(o, "length", a)) && s.$(e.length);
	}
	(s = o[V]) && s.$();
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
	if (typeof t == "function" && (t = t(e)), t = W(t), Array.isArray(t)) {
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
	typeof a == "function" && (a = a(i, n), a === i) || (r !== void 0 || a != null) && (a = W(a), r === void 0 || U(i) && U(a) && !Array.isArray(a) ? xe(i, a) : J(e, r, a));
}
function Ce(...[e, t]) {
	let n = W(e || {}), r = Array.isArray(n), i = H(n);
	function a(...e) {
		L(() => {
			r && e.length === 1 ? Se(n, e[0]) : X(n, e);
		});
	}
	return [i, a];
}
function we(e, t) {
	let n = Reflect.getOwnPropertyDescriptor(e, t);
	return !n || n.get || n.set || !n.configurable || t === P || t === z ? n : (delete n.value, delete n.writable, n.get = () => e[P][t], n.set = (n) => e[P][t] = n, n);
}
var Te = {
	get(e, n, r) {
		if (n === R) return e;
		if (n === P) return r;
		if (n === F) return q(e), r;
		let i = G(e, z), a = i[n], o = a ? a() : e[n];
		if (n === z || n === B || n === "__proto__") return o;
		if (!a) {
			let a = Object.getOwnPropertyDescriptor(e, n), s = typeof o == "function";
			if (t() && (!s || e.hasOwnProperty(n)) && !(a && a.get)) o = K(i, n, o)();
			else if (o != null && s && o === Array.prototype[n]) return (...e) => L(() => Array.prototype[n].apply(r, e));
		}
		return U(o) ? Ee(o) : o;
	},
	has(e, n) {
		return n === R || n === P || n === F || n === z || n === B || n === "__proto__" || (t() && K(G(e, B), n)(), n in e);
	},
	set(e, t, n) {
		return L(() => J(e, t, W(n))), !0;
	},
	deleteProperty(e, t) {
		return L(() => J(e, t, void 0, !0)), !0;
	},
	ownKeys: ye,
	getOwnPropertyDescriptor: we
};
function Ee(e) {
	let t = e[P];
	if (!t) {
		Object.defineProperty(e, P, { value: t = new Proxy(e, Te) });
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
						set: (e) => L(() => n.call(t, e)),
						configurable: !0
					});
				}
			}
		}
	}
	return t;
}
function De(e, t) {
	return Ee(W(e || {}));
}
function Oe(e, t) {
	L(() => t(W(e)));
}
var Z = Symbol("store-root");
function Q(e, t, n, r, i) {
	if (Y(n)) return;
	let a = t[n];
	if (e === a) return;
	let o = Array.isArray(e);
	if (n !== Z && (!U(e) || !U(a) || o !== Array.isArray(a) || i && e[i] !== a[i])) {
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
	let { merge: n, key: r = "id" } = t, i = W(e);
	return (e) => {
		if (!U(e) || !U(i)) return i;
		let t = Q(i, { [Z]: e }, Z, n, r);
		return t === void 0 ? e : t;
	};
}
var $ = /* @__PURE__ */ new WeakMap(), Ae = {
	get(e, t) {
		if (t === R) return e;
		let n = e[t];
		if (t === P || t === F || t === z || t === B || t === "__proto__") return n;
		let r;
		return U(n) ? $.get(n) || ($.set(n, r = new Proxy(n, Ae)), r) : n;
	},
	set(e, t, n) {
		return J(e, t, W(n)), !0;
	},
	deleteProperty(e, t) {
		return J(e, t, void 0, !0), !0;
	}
};
function je(e) {
	return (t) => {
		if (U(t)) {
			let n;
			(n = $.get(t)) || $.set(t, n = new Proxy(t, Ae)), e(n);
		}
		return t;
	};
}
var Me = void 0;
//#endregion
export { R as $RAW, Me as DEV, De as createMutable, Ce as createStore, Oe as modifyMutable, je as produce, ke as reconcile, W as unwrap };
