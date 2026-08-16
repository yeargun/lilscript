//#region packages/solidlil/reactive.generated.js
var e = "Potential Infinite Loop Detected.";
function t(e, t, n, r) {
	e[0] = t, e[1] = n, e[2] = r, e[3] = [], e[4] = [], e[5] = [], e[6] = null, e[7] = !1;
}
function n(e, t, n) {
	e[0] = t, e[1] = [], e[2] = n, e[3] = 0, e[4] = -1;
}
function r(e, t) {
	for (var n = 0; n < e[1].length; n += 1) if ((e[1][n] | 0) == t) return !1;
	return e[1].push(t), !0;
}
function i(e, t) {
	for (var n = 0; n < e[1].length; n += 1) if ((e[1][n] | 0) == t) {
		e[1][n] = e[1][e[1].length - 1] | 0, e[1].pop();
		return;
	}
}
function a(e) {
	var t;
	if (O < 0 && e[4] < 0) return e[0];
	if (O >= 0 && e[4] >= 0 && e[4] == k) throw "Circular memo dependency detected.";
	if (e[4] >= 0 && e[4] != k ? (t = k >= 0 && !E[k][9] && !E[k][10], t = !t) : t = !1, t && pe(e[4]), O >= 0) {
		t = O;
		var n = e[3] + 1 | 0;
		n > E[t][12] && (E[t][12] = n), e[4] >= 0 && l(E[t], e[4]), r(e, t) && E[t][4].push(e);
	}
	return e[0];
}
function o(e, t) {
	return k >= 0 && E[k][9] && (e[3] = E[k][12], e[4] = k), e[2](e[0], t) ? t : (e[0] = t, _e(e[1]), t);
}
function s(e, t) {
	return o(e, t(e[0]));
}
function c(e, t, n, r, i, a) {
	e[0] = t, e[1] = n, e[2] = r, e[3] = i, e[4] = [], e[5] = [], e[6] = [], e[7] = [], e[8] = null, e[9] = a, e[10] = !1, e[11] = !1, e[12] = 0, e[13] = [];
}
function l(e, t) {
	for (var n = 0; n < e[13].length; n += 1) if ((e[13][n] | 0) == t) return;
	e[13].push(t);
}
function u(e) {
	for (; e[6].length > 0;) {
		var t = e[6][e[6].length - 1] | 0;
		e[6].pop(), be(t);
	}
	for (; e[5].length > 0;) t = e[5][e[5].length - 1] | 0, e[5].pop(), ye(t);
	for (; e[7].length > 0;) t = e[7][e[7].length - 1], e[7].pop(), t[0]();
}
function d(e) {
	for (; e[4].length > 0;) {
		var t = e[4][e[4].length - 1];
		e[4].pop(), i(t, e[0]);
	}
}
function f(e) {
	if (!e[11]) {
		u(e), d(e);
		var t = O, n = k, r = A;
		O = e[0], k = e[0], A = -1, e[12] = 0, e[13] = [], M = M + 1 | 0, e[3](), e[10] = !0, M = M - 1 | 0, O = t, k = n, A = r, he();
	}
}
function p(e) {
	e[0] = [], e[1] = [], e[2] = [], e[3] = [];
}
function m(e) {
	for (var t = 0; t < e[2].length; t += 1) e[2][t][0]();
	e[0] = [], e[1] = [], e[2] = [], e[3] = [];
}
function h(e, t, r) {
	var i = /* @__PURE__ */ new Set(), a = e[0].length;
	t.length < a && (a = t.length);
	for (var s = 0, c, l, u, d, f, p, m, h, g, _; c = s < a && e[0][s] == t[s], c;) s += 1;
	for (l = e[0].length - 1, u = t.length - 1; a = l >= s && u >= s && e[0][l] == t[u], a;) --l, u = u - 1 | 0;
	for (d = [], f = [], p = [], m = [], c = 0; c < t.length; c += 1) {
		if (h = t[c], c < s) a = c;
		else if (c > u) a = (l + c | 0) - u | 0;
		else for (a = s;;) {
			if (a > l) {
				a = -1;
				break;
			}
			if (!i.has(a) && e[0][a] == h) break;
			a = a + 1 | 0;
		}
		a >= 0 ? (i.add(a), o(e[3][a], c), f.push(e[1][a]), p.push(e[2][a]), m.push(e[3][a])) : (g = [
			null,
			[],
			null,
			0,
			0
		], n(g, c, re), _ = [null], _[0] = () => {}, f.push(S(((e, t, n, r) => (i) => (r[0] = i, e(t, n)))(r, h, g, _), null, !1)), p.push(_), m.push(g)), d.push(h);
	}
	for (t = 0; t < e[0].length; t += 1) !i.has(t) && e[2][t][0]();
	return e[0] = d, e[1] = f, e[2] = p, e[3] = m, e[1];
}
function g(e) {
	e[0] = [], e[1] = [], e[2] = [];
}
function _(e) {
	for (var t = 0; t < e[1].length; t += 1) e[1][t][0]();
	e[0] = [], e[1] = [], e[2] = [];
}
function ee(e, t, r) {
	var i = e[2].length;
	t.length < i && (i = t.length);
	for (var a = 0, s, c; a < i; a += 1) e[2][a][0] != t[a] && o(e[2][a], t[a]);
	for (var l = e[2].length; l < t.length; l = l + 1 | 0) a = t[l], s = [
		null,
		[],
		null,
		0,
		0
	], n(s, a, (e, t) => e == t), c = [null], c[0] = () => {}, a = S(((e, t, n) => (r) => (n[0] = r, e(t, l)))(r, s, c), null, !1), e[2].push(s), e[1].push(c), e[0].push(a);
	for (; e[2].length > t.length;) e[1][e[2].length - 1][0](), e[1].pop(), e[2].pop(), e[0].pop();
	return e[0].slice();
}
function te(e) {
	e[0] = [], e[1] = () => {};
}
function ne(e, t) {
	return e[0].length == 0 && (e[0] = [S((n) => (e[1] = n, t()), null, !1)]), e[0];
}
function v(e) {
	e[0].length > 0 && e[1](), e[0] = [];
}
var re = (e, t) => e == t, ie = (e) => {
	for (var t = 0; t < F.length; t += 1) if ((F[t] | 0) == e) return !0;
	return !1;
}, ae = (e) => {
	for (var t = F[e] | 0; e < F.length - 1; e = e + 1 | 0) F[e] = F[e + 1 | 0] | 0;
	return F.pop(), t;
}, oe = (e) => {
	for (var t = 0; t < F.length; t += 1) if ((F[t] | 0) == e) return ae(t), !0;
	return !1;
}, y = (e, t) => {
	for (var n = e.length - 1; n >= 0; --n) if ((e[n] | 0) == t) {
		e[n] = e[e.length - 1] | 0, e.pop();
		return;
	}
}, b = (e, t) => {
	if (e == 1) {
		var n = D[t][6];
		return n || (n = [
			0,
			0,
			null
		], n[0] = e, n[1] = t, n[2] = null, D[t][6] = n, n[2] = b(D[t][1], D[t][2]), n);
	}
	return e == 2 ? (n = E[t][8]) ? n : (n = [
		0,
		0,
		null
	], n[0] = e, n[1] = t, n[2] = null, E[t][8] = n, n[2] = b(E[t][1], E[t][2]), n) : null;
}, se = (e) => {
	e[1] == 1 ? y(D[e[2]][3], e[0]) : e[1] == 2 && y(E[e[2]][5], e[0]);
}, ce = (e) => {
	e[1] == 1 ? y(D[e[2]][4], e[0]) : e[1] == 2 && y(E[e[2]][6], e[0]);
}, le = (e, t, n = 0) => {
	if (e == t) return !0;
	if (n >= E.length) return !1;
	for (var r = 0; r < E[e][13].length; r += 1) if (le(E[e][13][r] | 0, t, n + 1 | 0)) return !0;
	return !1;
}, ue = (e) => {
	if (k >= 0 && E[k][9] && !P && le(k, e) && !fe(e, k)) throw "Reactive dependency cycle detected.";
	!E[e][11] && !ie(e) && F.push(e);
}, de = (e) => {
	for (var t = 0; t < E[e][13].length; t += 1) if (ie(E[e][13][t] | 0)) return !0;
	return !1;
}, fe = (e, t) => {
	var n = E[t][1];
	t = E[t][2];
	for (var r = 0, i; i = n != 0 && r <= (E.length + D.length | 0), i;) {
		if (n == 2) {
			if (t == e) return !0;
			n = E[t][1], t = E[t][2];
		} else n = D[t][1], t = D[t][2];
		r = r + 1 | 0;
	}
	return !1;
}, pe = (e) => {
	var t, n;
	if (!(E[e][11] || e == k)) {
		for (n = 0; n < E[e][13].length; n += 1) t = E[e][13][n] | 0, (ie(t) || de(t)) && pe(t);
		oe(e) && f(E[e]);
	}
}, me = () => {
	for (var e = -1, t = -1, n = -1, r = 0, i, a, o, s, c; r < F.length; r += 1) i = F[r] | 0, E[i][9] ? (e < 0 || E[i][12] > E[F[e] | 0][12]) && (e = r) : !E[i][10] && t < 0 ? t = r : n < 0 ? n = r : (o = F[n] | 0, a = E[i], s = E[o], c = a[1] == s[1] && a[2] == s[2], (c && a[12] > s[12] || !c && (fe(i, o) || !fe(o, i) && a[12] > s[12])) && (n = r));
	return t >= 0 && (e = t), e >= 0 && (n = e), ae(n);
}, he = () => {
	if (!(N || j > 0 || M > 0)) {
		N = !0;
		var t = 0;
		try {
			for (; F.length > 0;) {
				if (t = t + 1 | 0, t > 1e3) throw e;
				f(E[me()]);
			}
			N = !1;
		} catch (e) {
			throw F = [], N = !1, e;
		}
	}
}, ge = () => {
	if (!(N || j > 0)) {
		N = !0;
		var t = 0;
		try {
			for (;;) {
				for (var n = -1, r = 0; r < F.length; r = r + 1 | 0) {
					var i = F[r] | 0;
					i != k && E[i][9] && (n < 0 || E[i][12] > E[F[n] | 0][12]) && (n = r);
				}
				if (n < 0) {
					N = !1;
					return;
				}
				if (t = t + 1 | 0, t > 1e3) throw e;
				r = ae(n), f(E[r]);
			}
		} catch (e) {
			throw F = [], N = !1, e;
		}
	}
}, _e = (e) => {
	var t = j == 0 && !N;
	t && (j = j + 1 | 0);
	try {
		for (var n = 0; n < e.length; n = n + 1 | 0) ue(e[n] | 0);
	} catch (e) {
		throw t && (j = j - 1 | 0), e;
	}
	t && (j = j - 1 | 0, M > 0 ? ge() : he());
}, ve = (e, t, n) => {
	var r = E.length;
	if (Fe.length > 0 && (r = Fe.pop()), k >= 0) var i = k, a = 2, o;
	else A >= 0 ? (i = A, a = 1) : (a = 0, i = -1);
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
	], c(o, r, a, i, e, t), r == E.length ? E.push(o) : E[r] = o, k >= 0 ? E[k][5].push(r) : A >= 0 && D[A][3].push(r), n && (M > 0 || N) ? ue(r) : f(o), r;
}, ye = (e) => {
	var t = E[e];
	if (!t[11]) {
		u(t), d(t), oe(e), se(t), t[3] = () => {}, t[13] = [];
		var n = t[8];
		n && (n[0] = 0, t[8] = null), t[11] = !0, Fe.push(e);
	}
}, be = (e) => {
	var t = D[e];
	if (!t[7]) {
		for (; t[4].length > 0;) {
			var n = t[4][t[4].length - 1] | 0;
			t[4].pop(), be(n);
		}
		for (; t[3].length > 0;) n = t[3][t[3].length - 1] | 0, t[3].pop(), ye(n);
		for (; t[5].length > 0;) n = t[5][t[5].length - 1], t[5].pop(), n[0]();
		ce(t), (n = t[6]) && (n[0] = 0, t[6] = null), t[7] = !0, Ie.push(e);
	}
}, xe = (e, t) => {
	let r = [
		null,
		[],
		null,
		0,
		0
	];
	return n(r, e, t), r;
}, x = (e) => a(e), Se = (e, t, n = !1) => {
	var r = P;
	r && (n = !0), P = n;
	try {
		return o(e, t);
	} finally {
		P = r;
	}
}, Ce = (e, t, n = !1) => {
	var r = P;
	r && (n = !0), P = n;
	try {
		return s(e, t);
	} finally {
		P = r;
	}
}, we = (e) => ve(e, !1, !0), Te = (e) => ve(e, !1, !1), Ee = (e, t, r) => {
	let i = [
		null,
		[],
		null,
		0,
		0
	];
	return n(i, e, r), ve(() => {
		o(i, t(i[0]));
	}, !0, !1), i;
}, De = (e, t) => {
	let n = [
		[],
		[],
		[],
		[]
	];
	return p(n), w(() => {
		m(n);
	}), () => {
		let r = a(e);
		return T(() => h(n, r, t));
	};
}, Oe = (e, t, n) => {
	let r = [
		[],
		[],
		[],
		[]
	];
	p(r);
	let i = [[], null];
	return te(i), w(() => {
		m(r), v(i);
	}), () => {
		let o = a(e);
		return T(() => o.length == 0 ? (m(r), ne(i, n)) : (v(i), h(r, o, t)));
	};
}, ke = (e, t) => {
	let n = [
		[],
		[],
		[]
	];
	return g(n), w(() => {
		_(n);
	}), () => {
		let r = a(e);
		return T(() => ee(n, r, t));
	};
}, Ae = (e, t, n) => {
	let r = [
		[],
		[],
		[]
	];
	g(r);
	let i = [[], null];
	return te(i), w(() => {
		_(r), v(i);
	}), () => {
		let o = a(e);
		return T(() => o.length == 0 ? (_(r), ne(i, n)) : (v(i), ee(r, o, t)));
	};
}, S = (e, n = null, r = !1) => {
	var i = D.length;
	Ie.length > 0 && (i = Ie.pop());
	var a;
	n && n[0] != 0 ? (a = n[0], n = n[1]) : (a = 0, n = -1);
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
	t(s, o, a, n), i == D.length ? D.push(s) : D[i] = s, r && (a == 1 ? D[n][4].push(i) : a == 2 && E[n][6].push(i));
	var c = O, l = A, u = k;
	O = -1, A = i, k = -1, M = M + 1 | 0, n = () => {
		D[i] == s && be(i);
	};
	var d = !1;
	try {
		var f = e(n);
		return d = !0, f;
	} finally {
		M = M - 1 | 0, O = c, k = u, A = l, d && he();
	}
}, je = (e) => S(e, C(), !1), C = () => k >= 0 ? b(2, k) : A >= 0 ? b(1, A) : null, Me = () => O >= 0 ? b(2, O) : null, Ne = (e, t) => {
	var n = O, r = k, i = A;
	O = -1, e && e[0] != 0 ? e[0] == 2 ? (k = e[1], A = -1) : (k = -1, A = e[1]) : (k = -1, A = -1), M = M + 1 | 0;
	var a = !1;
	try {
		var o = t();
		return a = !0, o;
	} finally {
		M = M - 1 | 0, O = n, k = r, A = i, a && he();
	}
}, Pe = (e) => e[2], w = (e) => {
	var t = [null];
	return t[0] = e, k >= 0 ? E[k][7].push(t) : A >= 0 && D[A][5].push(t), e;
}, T = (e) => {
	var t = O;
	O = -1;
	try {
		return e();
	} finally {
		O = t;
	}
}, E = [], D = [], Fe = [], Ie = [], O = -1, k = -1, A = -1, j = 0, M = 0, N = !1, P = !1, F = [], Le = (e, t) => e === t, I = Symbol("solid-proxy"), Re = Symbol("solid-track"), L = Symbol("solidlil-signal"), R, z, ze, Be, Ve, He = /* @__PURE__ */ new WeakMap();
function Ue(e) {
	return e?.equals === !1 ? () => !1 : e?.equals ?? Le;
}
function B(e, t) {
	let n = () => {
		if (R?.[2].has(e)) return R[2].get(e);
		if (R && t) {
			if (R[1].has(e)) return R[1].get(e);
			let n = t();
			return R[1].set(e, n), n;
		}
		return x(e);
	};
	return n[L] = e, n;
}
function We(e, t) {
	if (R) {
		let n = R[2].has(e) ? R[2].get(e) : x(e), r = typeof t == "function" ? t(n) : t;
		return R[2].set(e, r), R[1].clear(), r;
	}
	return typeof t == "function" ? Ce(e, t) : Se(e, t);
}
function Ge(e, t) {
	return Se(e[L], t, !0);
}
function Ke(e, t) {
	return Ce(e[L], t, !0);
}
function qe(e) {
	return e instanceof Error ? e : Error(typeof e == "string" ? e : "Unknown error", { cause: e });
}
function V(e) {
	return e && e.owner === void 0 && (e.owner = V(Pe(e))), e;
}
function H(e, t = C()) {
	let n = qe(e), r = t;
	for (; r;) {
		let e = He.get(r);
		if (e?.length) {
			for (let t of e) try {
				t(n);
			} catch (e) {
				return H(e, Pe(r));
			}
			return;
		}
		r = Pe(r);
	}
	throw n;
}
function Je(e) {
	let t = Ye(e);
	return (...e) => {
		try {
			return t(...e);
		} catch (e) {
			return H(e);
		}
	};
}
function Ye(e) {
	if (!z) return e;
	let t = xe(0, () => !1), n = z.factory(e, () => Ce(t, (e) => e + 1));
	return w(() => n.dispose()), (...e) => (x(t), n.track(...e));
}
function Xe(e, t = Le) {
	let n = e[L];
	return n === void 0 && (n = W(e, void 0, { equals: t })[L]), n;
}
function Ze(e) {
	for (; typeof e == "function" && !e.length;) e = e();
	if (!Array.isArray(e)) return e;
	let t = [];
	for (let n of e) {
		let e = Ze(n);
		Array.isArray(e) ? t.push(...e) : t.push(e);
	}
	return t;
}
function U(e, t) {
	let n = xe(e, Ue(t));
	return [B(n), (e) => We(n, e)];
}
function W(e, t, n) {
	let r = Je(e), i = Ue(n), a = !1, o = Ee(t, r, (e, t) => a ? i(e, t) : (a = !0, !1));
	return B(o, () => r(x(o)));
}
function Qe(e, t, n) {
	let r = t, i = Je(e);
	we(() => {
		r = i(r);
	});
}
function G(e, t, n) {
	let r = t, i = Je(e);
	Te(() => {
		r = i(r);
	});
}
function $e(e, t) {
	let n = !e.length, r = (t) => {
		try {
			return n ? e() : e(() => q(t));
		} catch (e) {
			return H(e);
		}
	};
	return n ? S(r, null) : t === void 0 ? je(r) : S(r, t);
}
function et() {
	return V(C());
}
function tt(e, t) {
	return Ne(e, () => {
		try {
			return t();
		} catch (e) {
			return H(e);
		}
	});
}
function nt(e, t) {
	return S((n) => {
		let r = C();
		He.set(r, [t]);
		try {
			return e();
		} catch (e) {
			return H(e, r);
		}
	}, C(), !0);
}
function rt(e, t, n) {
	return S((r) => {
		let i = V(C());
		return i.context = { [e.id]: t }, n();
	}, C(), !0);
}
function it(e, t) {
	let n = {
		id: Symbol("context"),
		defaultValue: e,
		Provider(e) {
			return t?.name, rt(n, e.value, () => ot(() => e.children));
		}
	};
	return n;
}
function at(e) {
	let t = V(C());
	for (; t;) {
		let n = t.context?.[e.id];
		if (n !== void 0) return n;
		t = t.owner;
	}
	return e.defaultValue;
}
function ot(e) {
	let t = W(e), n = W(() => Ze(t()));
	return n.toArray = () => {
		let e = n();
		return Array.isArray(e) ? e : e == null ? [] : [e];
	}, n;
}
function st(e, t) {
	return q(() => e(t || {}));
}
var ct = typeof Proxy == "function", K = () => !0, lt = {
	get(e, t, n) {
		return t === I ? n : e.get(t);
	},
	has(e, t) {
		return t === I || e.has(t);
	},
	set: K,
	deleteProperty: K,
	getOwnPropertyDescriptor(e, t) {
		return {
			configurable: !0,
			enumerable: !0,
			get() {
				return e.get(t);
			},
			set: K,
			deleteProperty: K
		};
	},
	ownKeys(e) {
		return e.keys();
	}
};
function ut(e) {
	return (e = typeof e == "function" ? e() : e) ? e : {};
}
function dt() {
	for (let e = 0; e < this.length; e += 1) {
		let t = this[e]();
		if (t !== void 0) return t;
	}
}
function ft(...e) {
	let t = !1;
	for (let n = 0; n < e.length; n += 1) {
		let r = e[n];
		t ||= !!r && I in r, typeof r == "function" && (t = !0, e[n] = W(r));
	}
	if (ct && t) return new Proxy({
		get(t) {
			for (let n = e.length - 1; n >= 0; --n) {
				let r = ut(e[n])[t];
				if (r !== void 0) return r;
			}
		},
		has(t) {
			for (let n = e.length - 1; n >= 0; --n) if (t in ut(e[n])) return !0;
			return !1;
		},
		keys() {
			let t = [];
			for (let n of e) t.push(...Object.keys(ut(n)));
			return [...new Set(t)];
		}
	}, lt);
	let n = {}, r = Object.create(null);
	for (let t = e.length - 1; t >= 0; --t) {
		let i = e[t];
		if (!i) continue;
		let a = Object.getOwnPropertyNames(i);
		for (let e = a.length - 1; e >= 0; --e) {
			let t = a[e];
			if (t === "__proto__" || t === "constructor") continue;
			let o = Object.getOwnPropertyDescriptor(i, t);
			r[t] ? n[t] && (o.get ? n[t].push(o.get.bind(i)) : o.value !== void 0 && n[t].push(() => o.value)) : r[t] = o.get ? {
				enumerable: !0,
				configurable: !0,
				get: dt.bind(n[t] = [o.get.bind(i)])
			} : o.value === void 0 ? void 0 : o;
		}
	}
	let i = {};
	for (let e of Object.keys(r).reverse()) {
		let t = r[e];
		t?.get ? Object.defineProperty(i, e, t) : i[e] = t?.value;
	}
	return i;
}
function pt(e, ...t) {
	let n = t.length;
	if (ct && I in e) {
		let r = n > 1 ? t.flat() : t[0], i = t.map((t) => new Proxy({
			get: (n) => t.includes(n) ? e[n] : void 0,
			has: (n) => t.includes(n) && n in e,
			keys: () => t.filter((t) => t in e)
		}, lt));
		return i.push(new Proxy({
			get: (t) => r.includes(t) ? void 0 : e[t],
			has: (t) => !r.includes(t) && t in e,
			keys: () => Object.keys(e).filter((e) => !r.includes(e))
		}, lt)), i;
	}
	let r = Array.from({ length: n + 1 }, () => ({}));
	for (let i of Object.getOwnPropertyNames(e)) {
		let a = n;
		for (let e = 0; e < t.length; e += 1) if (t[e].includes(i)) {
			a = e;
			break;
		}
		let o = Object.getOwnPropertyDescriptor(e, i);
		!o.get && !o.set && o.enumerable && o.writable && o.configurable ? r[a][i] = o.value : Object.defineProperty(r[a], i, o);
	}
	return r;
}
function mt(e, t, n = {}) {
	let r = Xe(() => {
		let t = e() || [];
		return t[Re], t;
	}, () => !1), i = (e, n) => t(e, B(n));
	return n.fallback ? Oe(r, i, n.fallback) : De(r, i);
}
function ht(e, t, n = {}) {
	let r = Xe(() => {
		let t = e() || [];
		return t[Re], t;
	}, () => !1), i = (e, n) => t(B(e), n);
	return n.fallback ? Ae(r, i, n.fallback) : ke(r, i);
}
function gt(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return W(mt(() => e.each, e.children, t));
}
function _t(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return W(ht(() => e.each, e.children, t));
}
function vt(e) {
	let t = W(() => e.when), n = e.keyed ? t : W(t, void 0, { equals: (e, t) => !e == !t });
	return W(() => {
		let r = n();
		if (!r) return e.fallback;
		let i = e.children;
		return typeof i != "function" || !i.length ? i : q(() => i(e.keyed ? r : () => {
			if (!q(n)) throw Error("Stale read from <Show>.");
			return t();
		}));
	});
}
function yt(e) {
	return e;
}
function bt(e) {
	let t = ot(() => e.children), n = W(() => {
		let e = t(), n = Array.isArray(e) ? e : [e], r = () => void 0;
		for (let e = 0; e < n.length; e += 1) {
			let t = n[e], i = r, a = W(() => i() ? void 0 : t.when), o = t.keyed ? a : W(a, void 0, { equals: (e, t) => !e == !t });
			r = () => i() || (o() ? [
				e,
				a,
				t
			] : void 0);
		}
		return r;
	});
	return W(() => {
		let t = n()();
		if (!t) return e.fallback;
		let [r, i, a] = t, o = a.children;
		return typeof o != "function" || !o.length ? o : q(() => o(a.keyed ? i() : () => {
			if (q(n)()?.[0] !== r && !Me()) throw Error("Stale read from <Match>.");
			return i();
		}));
	});
}
function xt(e) {
	let [t, n] = U(void 0);
	Ve ||= /* @__PURE__ */ new Set(), Ve.add(n), J(() => Ve.delete(n));
	let r = (t) => {
		let r = e.fallback;
		return typeof r == "function" && r.length ? q(() => r(t, () => n(void 0))) : r;
	};
	return W(() => {
		let i = t();
		if (i !== void 0) return r(i);
		let a, o = nt(() => e.children, (e) => {
			a = e, n(e);
		});
		return a === void 0 ? o : r(a);
	});
}
function St() {
	return ze ||= it();
}
function Ct() {
	return Be ||= it();
}
var wt = (e, t) => e.showContent === t.showContent && e.showFallback === t.showFallback;
function Tt(e) {
	let [t] = U(() => ({ inFallback: !1 })), n = at(Ct()), r = [], [i] = U(0), a = !0, o = n ? n.register(W(() => t()().inFallback)) : null, s = W((t) => {
		let n = e.revealOrder, a = e.tail, s = o ? o() : {
			showContent: !0,
			showFallback: !0
		};
		i();
		let c = r, l = n === "backwards";
		if (n === "together") {
			let e = c.every((e) => !e()), t = c.map(() => ({
				showContent: e && s.showContent,
				showFallback: s.showFallback
			}));
			return t.inFallback = !e, t;
		}
		let u = !1, d = t.inFallback, f = [];
		for (let e = 0; e < c.length; e += 1) {
			let t = l ? c.length - e - 1 : e, n = c[t]();
			if (!u && !n) f[t] = {
				showContent: s.showContent,
				showFallback: s.showFallback
			};
			else {
				let e = !u;
				e && (d = !0), f[t] = {
					showContent: e,
					showFallback: (!a || e && a === "collapsed") && s.showFallback
				}, u = !0;
			}
		}
		return u || (d = !1), f.inFallback = d, f;
	}, { inFallback: !1 });
	return Ge(t, s), rt(Ct(), { register(e) {
		let t = r.length;
		return r.push(e), a || Ke(i, (e) => e + 1), W(() => s()[t] ?? {
			showContent: !0,
			showFallback: !0
		}, void 0, { equals: wt });
	} }, () => {
		let t = e.children;
		return a = !1, Ke(i, (e) => e + 1), t;
	});
}
function Et(e) {
	let [t] = U(!1), n = 0, r = {
		effects: [],
		inFallback: t,
		resolved: !1,
		increment() {
			++n === 1 && Ge(t, !0);
		},
		decrement() {
			--n === 0 && Ge(t, !1);
		}
	}, i = at(Ct())?.register(r.inFallback), a = C(), o, s;
	return J(() => o?.()), rt(St(), r, () => {
		let t = W(() => e.children);
		return W((n) => {
			let c = i ? i() : {
				showContent: !0,
				showFallback: !0
			};
			if (!r.inFallback() && c.showContent) return r.resolved = !0, o?.(), o = void 0, t();
			if (c.showFallback) return o ? s : $e((t) => (o = t, s = e.fallback), a);
		});
	});
}
function q(e) {
	return T(z ? () => z.untrack(e) : e);
}
var J = w, Dt = /*#__PURE__*/ new Set("className value readOnly noValidate formNoValidate isMap noModule playsInline adAuctionHeaders allowFullscreen browsingTopics defaultChecked defaultMuted defaultSelected disablePictureInPicture disableRemotePlayback preservesPitch shadowRootClonable shadowRootCustomElementRegistry shadowRootDelegatesFocus shadowRootSerializable sharedStorageWritable allowfullscreen async alpha autofocus autoplay checked controls default disabled formnovalidate hidden indeterminate inert ismap loop multiple muted nomodule novalidate open playsinline readonly required reversed seamless selected adauctionheaders browsingtopics credentialless defaultchecked defaultmuted defaultselected defer disablepictureinpicture disableremoteplayback preservespitch shadowrootclonable shadowrootcustomelementregistry shadowrootdelegatesfocus shadowrootserializable sharedstoragewritable".split(" ")), Ot = /*#__PURE__*/ new Set("innerHTML textContent innerText children".split(" ")), kt = /*#__PURE__*/ Object.assign(Object.create(null), {
	className: "class",
	htmlFor: "for"
}), At = /*#__PURE__*/ Object.assign(Object.create(null), {
	class: "className",
	novalidate: {
		$: "noValidate",
		FORM: 1
	},
	formnovalidate: {
		$: "formNoValidate",
		BUTTON: 1,
		INPUT: 1
	},
	ismap: {
		$: "isMap",
		IMG: 1
	},
	nomodule: {
		$: "noModule",
		SCRIPT: 1
	},
	playsinline: {
		$: "playsInline",
		VIDEO: 1
	},
	readonly: {
		$: "readOnly",
		INPUT: 1,
		TEXTAREA: 1
	},
	adauctionheaders: {
		$: "adAuctionHeaders",
		IFRAME: 1
	},
	allowfullscreen: {
		$: "allowFullscreen",
		IFRAME: 1
	},
	browsingtopics: {
		$: "browsingTopics",
		IMG: 1
	},
	defaultchecked: {
		$: "defaultChecked",
		INPUT: 1
	},
	defaultmuted: {
		$: "defaultMuted",
		AUDIO: 1,
		VIDEO: 1
	},
	defaultselected: {
		$: "defaultSelected",
		OPTION: 1
	},
	disablepictureinpicture: {
		$: "disablePictureInPicture",
		VIDEO: 1
	},
	disableremoteplayback: {
		$: "disableRemotePlayback",
		AUDIO: 1,
		VIDEO: 1
	},
	preservespitch: {
		$: "preservesPitch",
		AUDIO: 1,
		VIDEO: 1
	},
	shadowrootclonable: {
		$: "shadowRootClonable",
		TEMPLATE: 1
	},
	shadowrootdelegatesfocus: {
		$: "shadowRootDelegatesFocus",
		TEMPLATE: 1
	},
	shadowrootserializable: {
		$: "shadowRootSerializable",
		TEMPLATE: 1
	},
	sharedstoragewritable: {
		$: "sharedStorageWritable",
		IFRAME: 1,
		IMG: 1
	}
});
function jt(e, t) {
	let n = At[e];
	return typeof n == "object" ? n[t] ? n.$ : void 0 : n;
}
var Mt = /*#__PURE__*/ new Set("beforeinput click dblclick contextmenu focusin focusout input keydown keyup mousedown mousemove mouseout mouseover mouseup pointerdown pointermove pointerout pointerover pointerup touchend touchmove touchstart".split(" ")), Nt = /*#__PURE__*/ new Set("altGlyph altGlyphDef altGlyphItem animate animateColor animateMotion animateTransform circle clipPath color-profile cursor defs desc ellipse feBlend feColorMatrix feComponentTransfer feComposite feConvolveMatrix feDiffuseLighting feDisplacementMap feDistantLight feDropShadow feFlood feFuncA feFuncB feFuncG feFuncR feGaussianBlur feImage feMerge feMergeNode feMorphology feOffset fePointLight feSpecularLighting feSpotLight feTile feTurbulence filter font font-face font-face-format font-face-name font-face-src font-face-uri foreignObject g glyph glyphRef hkern image line linearGradient marker mask metadata missing-glyph mpath path pattern polygon polyline radialGradient rect set stop svg switch symbol text textPath tref tspan use view vkern".split(" ")), Pt = {
	xlink: "http://www.w3.org/1999/xlink",
	xml: "http://www.w3.org/XML/1998/namespace"
}, Ft = /*#__PURE__*/ new Set("html base head link meta style title body address article aside footer header main nav section blockquote dd div dl dt figcaption figure hr li ol p pre ul a abbr b bdi bdo br cite code data dfn em i kbd mark q rp rt ruby s samp small span strong sub sup time u var wbr area audio img map track video embed iframe object param picture portal source svg math canvas noscript script del ins caption col colgroup table tbody td tfoot th thead tr button datalist fieldset form input label legend meter optgroup option output progress select textarea details dialog menu summary slot template acronym applet basefont bgsound big blink center content dir font frame frameset hgroup image keygen marquee menuitem nobr noembed noframes plaintext rb rtc shadow spacer strike tt xmp h1 h2 h3 h4 h5 h6 webview isindex listing multicol nextid noindex search".split(" ")), It = (e) => W(() => e());
function Lt(e, t, n) {
	let r = n.length, i = t.length, a = r, o = 0, s = 0, c = t[i - 1].nextSibling, l = null;
	for (; o < i || s < a;) {
		if (t[o] === n[s]) {
			o++, s++;
			continue;
		}
		for (; t[i - 1] === n[a - 1];) i--, a--;
		if (i === o) {
			let t = a < r ? s ? n[s - 1].nextSibling : n[a - s] : c;
			for (; s < a;) e.insertBefore(n[s++], t);
		} else if (a === s) for (; o < i;) (!l || !l.has(t[o])) && t[o].remove(), o++;
		else if (t[o] === n[a - 1] && n[s] === t[i - 1]) {
			let r = t[--i].nextSibling;
			e.insertBefore(n[s++], t[o++].nextSibling), e.insertBefore(n[--a], r), t[i] = n[a];
		} else {
			if (!l) {
				l = /* @__PURE__ */ new Map();
				let e = s;
				for (; e < a;) l.set(n[e], e++);
			}
			let r = l.get(t[o]);
			if (r != null) {
				if (s < r && r < a) {
					let c = o, u = 1, d;
					for (; ++c < i && c < a && (d = l.get(t[c])) != null && d === r + u;) u++;
					if (u > r - s) {
						let i = t[o];
						for (; s < r;) e.insertBefore(n[s++], i);
					} else e.replaceChild(n[s++], t[o++]);
				} else o++;
			} else t[o++].remove();
		}
	}
}
var Y = "_$DX_DELEGATE";
function Rt(e, t, n, r = {}) {
	let i;
	return $e((r) => {
		i = r, t === document ? e() : $t(t, e(), t.firstChild ? null : void 0, n);
	}, r.owner), () => {
		i(), t.textContent = "";
	};
}
function zt(e, t, n, r) {
	let i, a = () => {
		let t = r ? document.createElementNS("http://www.w3.org/1998/Math/MathML", "template") : document.createElement("template");
		return t.innerHTML = e, n ? t.content.firstChild.firstChild : r ? t.firstChild : t.content.firstChild;
	}, o = t ? () => q(() => document.importNode(i ||= a(), !0)) : () => (i ||= a()).cloneNode(!0);
	return o.cloneNode = o, o;
}
function Bt(e, t = window.document) {
	let n = t[Y] || (t[Y] = /* @__PURE__ */ new Set());
	for (let r = 0, i = e.length; r < i; r++) {
		let i = e[r];
		n.has(i) || (n.add(i), t.addEventListener(i, an));
	}
}
function Vt(e = window.document) {
	if (e[Y]) {
		for (let t of e[Y].keys()) e.removeEventListener(t, an);
		delete e[Y];
	}
}
function Ht(e, t, n) {
	Z(e) || (e[t] = n);
}
function X(e, t, n) {
	Z(e) || (n == null ? e.removeAttribute(t) : e.setAttribute(t, n));
}
function Ut(e, t, n, r) {
	Z(e) || (r == null ? e.removeAttributeNS(t, n) : e.setAttributeNS(t, n, r));
}
function Wt(e, t, n) {
	Z(e) || (n ? e.setAttribute(t, "") : e.removeAttribute(t));
}
function Gt(e, t) {
	Z(e) || (t == null ? e.removeAttribute("class") : e.className = t);
}
function Kt(e, t, n, r) {
	if (r) Array.isArray(n) ? (e[`$$${t}`] = n[0], e[`$$${t}Data`] = n[1]) : e[`$$${t}`] = n;
	else if (Array.isArray(n)) {
		let r = n[0];
		e.addEventListener(t, n[0] = (t) => r.call(e, n[1], t));
	} else e.addEventListener(t, n, typeof n != "function" && n);
}
function qt(e, t, n = {}) {
	let r = Object.keys(t || {}), i = Object.keys(n), a, o;
	for (a = 0, o = i.length; a < o; a++) {
		let r = i[a];
		!r || r === "undefined" || t[r] || (nn(e, r, !1), delete n[r]);
	}
	for (a = 0, o = r.length; a < o; a++) {
		let i = r[a], o = !!t[i];
		!i || i === "undefined" || n[i] === o || !o || (nn(e, i, !0), n[i] = o);
	}
	return n;
}
function Jt(e, t, n) {
	if (!t) return n ? X(e, "style") : t;
	let r = e.style;
	if (typeof t == "string") return r.cssText = t;
	typeof n == "string" && (r.cssText = n = void 0), n ||= {};
	let i, a;
	for (a in n) t[a] ?? r.removeProperty(a), delete n[a];
	for (a in t) i = t[a], i !== n[a] && (r.setProperty(a, i), n[a] = i);
	return n;
}
function Yt(e, t, n) {
	n == null ? e.style.removeProperty(t) : e.style.setProperty(t, n);
}
function Xt(e, t = {}, n, r) {
	let i = {};
	return r || G(() => i.children = Q(e, t.children, i.children)), G(() => typeof t.ref == "function" && Qt(t.ref, e)), G(() => en(e, t, n, !0, i, !0)), i;
}
function Zt(e, t) {
	let n = e[t];
	return Object.defineProperty(e, t, {
		get() {
			return n();
		},
		enumerable: !0
	}), e;
}
function Qt(e, t, n) {
	return q(() => e(t, n));
}
function $t(e, t, n, r) {
	if (n !== void 0 && !r && (r = []), typeof t != "function") return Q(e, t, r, n);
	G((r) => Q(e, t(), r, n), r);
}
function en(e, t, n, r, i = {}, a = !1) {
	t ||= {};
	for (let r in i) if (!(r in t)) {
		if (r === "children") continue;
		i[r] = rn(e, r, null, i[r], n, a, t);
	}
	for (let o in t) {
		if (o === "children") {
			r || Q(e, t.children);
			continue;
		}
		let s = t[o];
		i[o] = rn(e, o, s, i[o], n, a, t);
	}
}
function Z(e) {
	return !1;
}
function tn(e) {
	return e.toLowerCase().replace(/-([a-z])/g, (e, t) => t.toUpperCase());
}
function nn(e, t, n) {
	let r = t.trim().split(/\s+/);
	for (let t = 0, i = r.length; t < i; t++) e.classList.toggle(r[t], n);
}
function rn(e, t, n, r, i, a, o) {
	let s, c, l, u, d;
	if (t === "style") return Jt(e, n, r);
	if (t === "classList") return qt(e, n, r);
	if (n === r) return r;
	if (t === "ref") a || n(e);
	else if (t.slice(0, 3) === "on:") {
		let i = t.slice(3);
		r && e.removeEventListener(i, r, typeof r != "function" && r), n && e.addEventListener(i, n, typeof n != "function" && n);
	} else if (t.slice(0, 10) === "oncapture:") {
		let i = t.slice(10);
		r && e.removeEventListener(i, r, !0), n && e.addEventListener(i, n, !0);
	} else if (t.slice(0, 2) === "on") {
		let i = t.slice(2).toLowerCase(), a = Mt.has(i);
		if (!a && r) {
			let t = Array.isArray(r) ? r[0] : r;
			e.removeEventListener(i, t);
		}
		(a || n) && (Kt(e, i, n, a), a && Bt([i]));
	} else if (t.slice(0, 5) === "attr:") X(e, t.slice(5), n);
	else if (t.slice(0, 5) === "bool:") Wt(e, t.slice(5), n);
	else if ((d = t.slice(0, 5) === "prop:") || (l = Ot.has(t)) || !i && ((u = jt(t, e.tagName)) || (c = Dt.has(t))) || (s = e.nodeName.includes("-") || "is" in o)) {
		if (d) t = t.slice(5), c = !0;
		else if (Z(e)) return n;
		t === "class" || t === "className" ? Gt(e, n) : s && !c && !l ? e[tn(t)] = n : e[u || t] = n;
	} else {
		let r = i && t.indexOf(":") > -1 && Pt[t.split(":")[0]];
		r ? Ut(e, r, t, n) : X(e, kt[t] || t, n);
	}
	return n;
}
function an(e) {
	let t = e.target, n = `$$${e.type}`, r = e.target, i = e.currentTarget, a = (t) => Object.defineProperty(e, "target", {
		configurable: !0,
		value: t
	}), o = () => {
		let r = t[n];
		if (r && !t.disabled) {
			let i = t[`${n}Data`];
			if (i === void 0 ? r.call(t, e) : r.call(t, i, e), e.cancelBubble) return;
		}
		return t.host && typeof t.host != "string" && !t.host._$host && t.contains(e.target) && a(t.host), !0;
	}, s = () => {
		for (; o() && (t = t._$host || t.parentNode || t.host););
	};
	if (Object.defineProperty(e, "currentTarget", {
		configurable: !0,
		get() {
			return t || document;
		}
	}), e.composedPath) {
		let n = e.composedPath();
		a(n[0]);
		for (let e = 0; e < n.length - 2 && (t = n[e], o()); e++) {
			if (t._$host) {
				t = t._$host, s();
				break;
			}
			if (t.parentNode === i) break;
		}
	} else s();
	a(r);
}
function Q(e, t, n, r, i) {
	let a = Z(e);
	if (a) {
		!n && (n = [...e.childNodes]);
		let t = [];
		for (let e = 0; e < n.length; e++) {
			let r = n[e];
			r.nodeType === 8 && r.data.slice(0, 2) === "!$" ? r.remove() : t.push(r);
		}
		n = t;
	}
	for (; typeof n == "function";) n = n();
	if (t === n) return n;
	let o = typeof t, s = r !== void 0;
	if (e = s && n[0] && n[0].parentNode || e, o === "string" || o === "number") {
		if (a || o === "number" && (t = t.toString(), t === n)) return n;
		if (s) {
			let i = n[0];
			i && i.nodeType === 3 ? i.data !== t && (i.data = t) : i = document.createTextNode(t), n = $(e, n, r, i);
		} else n = n !== "" && typeof n == "string" ? e.firstChild.data = t : e.textContent = t;
	} else if (t == null || o === "boolean") {
		if (a) return n;
		n = $(e, n, r);
	} else if (o === "function") return G(() => {
		let i = t();
		for (; typeof i == "function";) i = i();
		n = Q(e, i, n, r);
	}), () => n;
	else if (Array.isArray(t)) {
		let o = [], c = n && Array.isArray(n);
		if (on(o, t, n, i)) return G(() => n = Q(e, o, n, r, !0)), () => n;
		if (a) {
			if (!o.length) return n;
			if (r === void 0) return n = [...e.childNodes];
			let t = o[0];
			if (t.parentNode !== e) return n;
			let i = [t];
			for (; (t = t.nextSibling) !== r;) i.push(t);
			return n = i;
		}
		if (o.length === 0) {
			if (n = $(e, n, r), s) return n;
		} else c ? n.length === 0 ? sn(e, o, r) : Lt(e, n, o) : (n && $(e), sn(e, o));
		n = o;
	} else if (t.nodeType) {
		if (a && t.parentNode) return n = s ? [t] : t;
		if (Array.isArray(n)) {
			if (s) return n = $(e, n, r, t);
			$(e, n, null, t);
		} else n == null || n === "" || !e.firstChild ? e.appendChild(t) : e.replaceChild(t, e.firstChild);
		n = t;
	}
	return n;
}
function on(e, t, n, r) {
	let i = !1;
	for (let a = 0, o = t.length; a < o; a++) {
		let o = t[a], s = n && n[e.length], c;
		if (o != null && o !== !0 && o !== !1) {
			if ((c = typeof o) == "object" && o.nodeType) e.push(o);
			else if (Array.isArray(o)) i = on(e, o, s) || i;
			else if (c === "function") {
				if (r) {
					for (; typeof o == "function";) o = o();
					i = on(e, Array.isArray(o) ? o : [o], Array.isArray(s) ? s : [s]) || i;
				} else e.push(o), i = !0;
			} else {
				let t = String(o);
				s && s.nodeType === 3 && s.data === t ? e.push(s) : e.push(document.createTextNode(t));
			}
		}
	}
	return i;
}
function sn(e, t, n) {
	for (let r = 0, i = t.length; r < i; r++) e.insertBefore(t[r], n);
}
function $(e, t, n, r) {
	if (n === void 0) return e.textContent = "";
	let i = r || document.createTextNode("");
	if (t.length) {
		let r = !1;
		for (let a = t.length - 1; a >= 0; a--) {
			let o = t[a];
			if (i !== o) {
				let t = o.parentNode === e;
				!r && !a ? t ? e.replaceChild(i, o) : e.insertBefore(i, n) : t && o.remove();
			} else r = !0;
		}
	} else e.insertBefore(i, n);
	return [i];
}
function cn(e, t) {
	e.innerHTML = t;
}
var ln = !1, un = !1, dn = "http://www.w3.org/2000/svg";
function fn(e, t, n) {
	return t ? document.createElementNS(dn, e) : document.createElement(e, { is: n });
}
function pn(e) {
	let { useShadow: t } = e, n = document.createTextNode(""), r = () => e.mount || document.body, i = et(), a, o = !1;
	return Qe(() => {
		o && (et().user = o = !1), a ||= tt(i, () => W(() => e.children));
		let s = r();
		if (s instanceof HTMLHeadElement) {
			let [e, t] = U(!1);
			$e((t) => $t(s, () => e() ? t() : a(), null)), J(() => t(!0));
		} else {
			let r = fn(e.isSVG ? "g" : "div", e.isSVG), i = t && r.attachShadow ? r.attachShadow({ mode: "open" }) : r;
			Object.defineProperty(r, "_$host", {
				get() {
					return n.parentNode;
				},
				configurable: !0
			}), $t(i, a), s.appendChild(r), e.ref && e.ref(r), J(() => s.removeChild(r));
		}
	}, void 0, { render: !o }), n;
}
function mn(e, t) {
	let n = W(e);
	return W(() => {
		let e = n();
		switch (typeof e) {
			case "function": return q(() => e(t));
			case "string":
				let n = Nt.has(e), r = fn(e, n, q(() => t.is));
				return Xt(r, t, n), r;
		}
	});
}
function hn(e) {
	let [, t] = pt(e, ["component"]);
	return mn(() => e.component, t);
}
//#endregion
export { kt as Aliases, Ot as ChildProperties, Ft as DOMElements, Mt as DelegatedEvents, hn as Dynamic, xt as ErrorBoundary, gt as For, _t as Index, yt as Match, pn as Portal, Dt as Properties, Nt as SVGElements, Pt as SVGNamespace, vt as Show, Et as Suspense, Tt as SuspenseList, bt as Switch, Kt as addEventListener, en as assign, qt as classList, Gt as className, Vt as clearDelegatedEvents, st as createComponent, mn as createDynamic, Bt as delegateEvents, Zt as dynamicProperty, G as effect, et as getOwner, jt as getPropAlias, cn as innerHTML, $t as insert, un as isDev, ln as isServer, It as memo, ft as mergeProps, Rt as render, X as setAttribute, Ut as setAttributeNS, Wt as setBoolAttribute, Ht as setProperty, Yt as setStyleProperty, Xt as spread, Jt as style, zt as template, q as untrack, Qt as use };
