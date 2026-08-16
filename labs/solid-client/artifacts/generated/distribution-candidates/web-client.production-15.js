//#region packages/solidlil/reactive.generated.js
var e = "Potential Infinite Loop Detected.", t = (e) => e[2], n = () => k >= 0 ? T(2, k) : null, r = () => A >= 0 ? T(2, A) : j >= 0 ? T(1, j) : null, i = (e) => _(e, r(), !1), a = (e) => ne(e, !1, !1), o = (e) => ne(e, !1, !0), s = (e) => C(e), c = (e) => {
	e[1] == 1 ? v(O[e[2]][4], e[0]) : e[1] == 2 && v(D[e[2]][6], e[0]);
}, l = (e) => {
	e[1] == 1 ? v(O[e[2]][3], e[0]) : e[1] == 2 && v(D[e[2]][5], e[0]);
}, u = (e, t) => e == t, d = (e, t, n) => {
	let r = [
		[],
		[],
		[]
	];
	we(r);
	let i = [[], null];
	return Se(i), g(() => {
		Te(r), E(i);
	}), () => {
		let a = C(e);
		return S(() => a.length == 0 ? (Te(r), je(i, n)) : (E(i), ae(r, a, t)));
	};
}, f = (e, t, n) => {
	let r = [
		[],
		[],
		[],
		[]
	];
	Ce(r);
	let i = [[], null];
	return Se(i), g(() => {
		Ee(r), E(i);
	}), () => {
		let a = C(e);
		return S(() => a.length == 0 ? (Ee(r), je(i, n)) : (E(i), ie(r, a, t)));
	};
}, p = (e, t) => {
	let n = [
		[],
		[],
		[],
		[]
	];
	return Ce(n), g(() => {
		Ee(n);
	}), () => {
		let r = C(e);
		return S(() => ie(n, r, t));
	};
}, m = (e, t) => {
	let n = [
		[],
		[],
		[]
	];
	return we(n), g(() => {
		Te(n);
	}), () => {
		let r = C(e);
		return S(() => ae(n, r, t));
	};
}, h = (e, t) => {
	let n = [
		null,
		[],
		null,
		0,
		0
	];
	return x(n, e, t), n;
}, g = (e) => {
	var t = [e];
	return A >= 0 ? D[A][7].push(t) : j >= 0 && O[j][5].push(t), e;
}, ee = (e) => {
	var t = M == 0 && !P;
	t && (M = M + 1 | 0);
	try {
		for (var n = 0; n < e.length; n = n + 1 | 0) Ae(e[n] | 0);
	} catch (e) {
		throw t && (M = M - 1 | 0), e;
	}
	t && (M = M - 1 | 0, N > 0 ? he() : y());
}, te = (e, t) => {
	var n = k, r = A, i = j;
	k = -1, e && e[0] != 0 ? e[0] == 2 ? (A = e[1], j = -1) : (A = -1, j = e[1]) : (A = -1, j = -1), N = N + 1 | 0;
	var a = !1;
	try {
		var o = t();
		return a = !0, o;
	} finally {
		N = N - 1 | 0, k = n, A = r, j = i, a && y();
	}
}, _ = (e, t = null, n = !1) => {
	var r = O.length;
	Ne.length > 0 && (r = Ne.pop());
	var i;
	t && t[0] != 0 ? (i = t[0], t = t[1]) : (i = 0, t = -1);
	var a = [
		r,
		i,
		t,
		[],
		[],
		[],
		null,
		!1
	];
	a[3] = [], a[4] = [], a[5] = [], a[6] = null, a[7] = !1, r == O.length ? O.push(a) : O[r] = a, n && (i == 1 ? O[t][4].push(r) : i == 2 && D[t][6].push(r));
	var o = k, s = j, c = A;
	k = -1, j = r, A = -1, N = N + 1 | 0, t = () => {
		O[r] == a && be(r);
	};
	var l = !1;
	try {
		var u = e(t);
		return l = !0, u;
	} finally {
		N = N - 1 | 0, k = o, A = c, j = s, l && y();
	}
}, ne = (e, t, n) => {
	var r = D.length;
	if (Me.length > 0 && (r = Me.pop()), A >= 0) var i = A, a = 2, o;
	else j >= 0 ? (i = j, a = 1) : (a = 0, i = -1);
	return o = [
		r,
		a,
		i,
		e,
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
	], o[4] = [], o[5] = [], o[6] = [], o[7] = [], o[8] = null, o[9] = t, o[10] = !1, o[11] = !1, o[12] = 0, o[13] = [], r == D.length ? D.push(o) : D[r] = o, A >= 0 ? D[A][5].push(r) : j >= 0 && O[j][3].push(r), n && (N > 0 || P) ? Ae(r) : b(o), r;
}, re = (e, t, n) => {
	let r = [
		null,
		[],
		null,
		0,
		0
	];
	return x(r, e, n), ne(() => {
		w(r, t(r[0]));
	}, !0, !1), r;
};
function ie(e, t, n) {
	var r = /* @__PURE__ */ new Set(), i = e[0].length;
	t.length < i && (i = t.length);
	for (var a = 0, o, s, c, l, d, f, p, m, h, g; o = a < i && e[0][a] == t[a], o;) a += 1;
	for (s = e[0].length - 1, c = t.length - 1; i = s >= a && c >= a && e[0][s] == t[c], i;) --s, c = c - 1 | 0;
	for (l = [], d = [], f = [], p = [], o = 0; o < t.length; o += 1) {
		if (m = t[o], o < a) i = o;
		else if (o > c) i = (s + o | 0) - c | 0;
		else for (i = a;;) {
			if (i > s) {
				i = -1;
				break;
			}
			if (!r.has(i) && e[0][i] == m) break;
			i = i + 1 | 0;
		}
		i >= 0 ? (r.add(i), w(e[3][i], o), d.push(e[1][i]), f.push(e[2][i]), p.push(e[3][i])) : (h = [
			null,
			[],
			null,
			0,
			0
		], x(h, o, u), g = [() => {}], d.push(_(((e, t, n, r) => (i) => (r[0] = i, e(t, n)))(n, m, h, g), null, !1)), f.push(g), p.push(h)), l.push(m);
	}
	for (t = 0; t < e[0].length; t += 1) !r.has(t) && e[2][t][0]();
	return e[0] = l, e[1] = d, e[2] = f, e[3] = p, e[1];
}
function ae(e, t, n) {
	var r = e[2].length;
	t.length < r && (r = t.length);
	for (var i = 0, a, o; i < r; i += 1) e[2][i][0] != t[i] && w(e[2][i], t[i]);
	for (var s = e[2].length; s < t.length; s = s + 1 | 0) i = t[s], a = [
		null,
		[],
		null,
		0,
		0
	], x(a, i, (e, t) => e == t), o = [() => {}], i = _(((e, t, n) => (r) => (n[0] = r, e(t, s)))(n, a, o), null, !1), e[2].push(a), e[1].push(o), e[0].push(i);
	for (; e[2].length > t.length;) e[1][e[2].length - 1][0](), e[1].pop(), e[2].pop(), e[0].pop();
	return e[0].slice();
}
function oe(e, t) {
	for (var n = 0; n < e[1].length; n += 1) if ((e[1][n] | 0) == t) {
		e[1][n] = e[1][e[1].length - 1] | 0, e[1].pop();
		return;
	}
}
function se(e, t) {
	for (var n = 0; n < e[1].length; n += 1) if ((e[1][n] | 0) == t) return !1;
	return e[1].push(t), !0;
}
function ce(e, t) {
	for (var n = 0; n < e[13].length; n += 1) if ((e[13][n] | 0) == t) return;
	e[13].push(t);
}
var le = (e) => {
	var t, n;
	if (!(D[e][11] || e == A)) {
		for (n = 0; n < D[e][13].length; n += 1) t = D[e][13][n] | 0, (fe(t) || de(t)) && le(t);
		pe(e) && b(D[e]);
	}
}, ue = (e, t, n) => {
	if (e == t) return !0;
	if (n >= D.length) return !1;
	for (var r = 0; r < D[e][13].length; r += 1) if (ue(D[e][13][r] | 0, t, n + 1 | 0)) return !0;
	return !1;
}, de = (e) => {
	for (var t = 0; t < D[e][13].length; t += 1) if (fe(D[e][13][t] | 0)) return !0;
	return !1;
}, fe = (e) => {
	for (var t = 0; t < I.length; t += 1) if ((I[t] | 0) == e) return !0;
	return !1;
}, pe = (e) => {
	for (var t = 0; t < I.length; t += 1) if ((I[t] | 0) == e) return _e(t), !0;
	return !1;
}, v = (e, t) => {
	for (var n = e.length - 1; n >= 0; --n) if ((e[n] | 0) == t) {
		e[n] = e[e.length - 1] | 0, e.pop();
		return;
	}
}, me = (e, t) => {
	var n = D[t][1];
	t = D[t][2];
	for (var r = 0, i; i = n != 0 && r <= (D.length + O.length | 0), i;) {
		if (n == 2) {
			if (t == e) return !0;
			n = D[t][1], t = D[t][2];
		} else n = O[t][1], t = O[t][2];
		r = r + 1 | 0;
	}
	return !1;
}, y = () => {
	if (!(P || M > 0 || N > 0)) {
		P = !0;
		var t = 0;
		try {
			for (; I.length > 0;) {
				if (t = t + 1 | 0, t > 1e3) throw e;
				b(D[ge()]);
			}
			P = !1;
		} catch (e) {
			throw I = [], P = !1, e;
		}
	}
}, he = () => {
	if (!(P || M > 0)) {
		P = !0;
		var t = 0;
		try {
			for (;;) {
				for (var n = -1, r = 0; r < I.length; r = r + 1 | 0) {
					var i = I[r] | 0;
					i != A && D[i][9] && (n < 0 || D[i][12] > D[I[n] | 0][12]) && (n = r);
				}
				if (n < 0) {
					P = !1;
					return;
				}
				if (t = t + 1 | 0, t > 1e3) throw e;
				r = _e(n), b(D[r]);
			}
		} catch (e) {
			throw I = [], P = !1, e;
		}
	}
}, ge = () => {
	for (var e = -1, t = -1, n = -1, r = 0, i, a, o, s, c; r < I.length; r += 1) i = I[r] | 0, D[i][9] ? (e < 0 || D[i][12] > D[I[e] | 0][12]) && (e = r) : !D[i][10] && t < 0 ? t = r : n < 0 ? n = r : (o = I[n] | 0, a = D[i], s = D[o], c = a[1] == s[1] && a[2] == s[2], (c && a[12] > s[12] || !c && (me(i, o) || !me(o, i) && a[12] > s[12])) && (n = r));
	return t >= 0 && (e = t), _e(e < 0 ? n : e);
}, _e = (e) => {
	for (var t = I[e] | 0; e < I.length - 1; e = e + 1 | 0) I[e] = I[e + 1 | 0] | 0;
	return I.pop(), t;
};
function ve(e) {
	for (; e[4].length > 0;) {
		var t = e[4][e[4].length - 1];
		e[4].pop(), oe(t, e[0]);
	}
}
function ye(e) {
	for (; e[6].length > 0;) {
		var t = e[6][e[6].length - 1] | 0;
		e[6].pop(), be(t);
	}
	for (; e[5].length > 0;) t = e[5][e[5].length - 1] | 0, e[5].pop(), xe(t);
	for (; e[7].length > 0;) t = e[7][e[7].length - 1], e[7].pop(), t[0]();
}
var be = (e) => {
	var t = O[e];
	if (!t[7]) {
		for (; t[4].length > 0;) {
			var n = t[4][t[4].length - 1] | 0;
			t[4].pop(), be(n);
		}
		for (; t[3].length > 0;) n = t[3][t[3].length - 1] | 0, t[3].pop(), xe(n);
		for (; t[5].length > 0;) n = t[5][t[5].length - 1], t[5].pop(), n[0]();
		c(t), (n = t[6]) && (n[0] = 0, t[6] = null), t[7] = !0, Ne.push(e);
	}
}, xe = (e) => {
	var t = D[e];
	if (!t[11]) {
		ye(t), ve(t), pe(e), l(t), t[3] = () => {}, t[13] = [];
		var n = t[8];
		n && (n[0] = 0, t[8] = null), t[11] = !0, Me.push(e);
	}
};
function b(e) {
	if (!e[11]) {
		ye(e), ve(e);
		var t = k, n = A, r = j;
		k = e[0], A = e[0], j = -1, e[12] = 0, e[13] = [], N = N + 1 | 0, e[3](), e[10] = !0, N = N - 1 | 0, k = t, A = n, j = r, y();
	}
}
function Se(e) {
	e[0] = [], e[1] = () => {};
}
function Ce(e) {
	e[0] = [], e[1] = [], e[2] = [], e[3] = [];
}
function we(e) {
	e[0] = [], e[1] = [], e[2] = [];
}
function Te(e) {
	for (var t = 0; t < e[1].length; t += 1) e[1][t][0]();
	e[0] = [], e[1] = [], e[2] = [];
}
function Ee(e) {
	for (var t = 0; t < e[2].length; t += 1) e[2][t][0]();
	e[0] = [], e[1] = [], e[2] = [], e[3] = [];
}
function x(e, t, n) {
	e[0] = t, e[1] = [], e[2] = n, e[3] = 0, e[4] = -1;
}
function De(e, t) {
	return w(e, t(e[0]));
}
var Oe = (e, t, n = !1) => {
	var r = F;
	r && (n = !0), F = n;
	try {
		return w(e, t);
	} finally {
		F = r;
	}
}, ke = (e, t, n = !1) => {
	var r = F;
	r && (n = !0), F = n;
	try {
		return De(e, t);
	} finally {
		F = r;
	}
}, S = (e) => {
	var t = k;
	k = -1;
	try {
		return e();
	} finally {
		k = t;
	}
};
function C(e) {
	var t;
	if (k < 0 && e[4] < 0) return e[0];
	if (k >= 0 && e[4] >= 0 && e[4] == A) throw "Circular memo dependency detected.";
	if (e[4] >= 0 && e[4] != A ? (t = A >= 0 && !D[A][9] && !D[A][10], t = !t) : t = !1, t && le(e[4]), k >= 0) {
		t = k;
		var n = e[3] + 1 | 0;
		n > D[t][12] && (D[t][12] = n), e[4] >= 0 && ce(D[t], e[4]), se(e, t) && D[t][4].push(e);
	}
	return e[0];
}
var Ae = (e) => {
	if (A >= 0 && D[A][9] && !F && ue(A, e, 0) && !me(e, A)) throw "Reactive dependency cycle detected.";
	!D[e][11] && !fe(e) && I.push(e);
};
function w(e, t) {
	return A >= 0 && D[A][9] && (e[3] = D[A][12], e[4] = A), e[2](e[0], t) ? t : (e[0] = t, ee(e[1]), t);
}
var T = (e, t) => {
	if (e == 1) {
		var n = O[t][6];
		return n || (n = [
			e,
			t,
			null
		], O[t][6] = n, n[2] = T(O[t][1], O[t][2]), n);
	}
	return e == 2 ? (n = D[t][8]) ? n : (n = [
		e,
		t,
		null
	], D[t][8] = n, n[2] = T(D[t][1], D[t][2]), n) : null;
};
function je(e, t) {
	return e[0].length == 0 && (e[0] = [_((n) => (e[1] = n, t()), null, !1)]), e[0];
}
function E(e) {
	e[0].length > 0 && e[1](), e[0] = [];
}
var D = [], O = [], Me = [], Ne = [], k = -1, A = -1, j = -1, M = 0, N = 0, P = !1, F = !1, I = [], Pe = (e, t) => e === t, L = Symbol("solid-proxy"), Fe = Symbol("solid-track"), R = Symbol("solidlil-signal"), z, B, Ie, Le, Re, ze = /* @__PURE__ */ new WeakMap();
function Be(e) {
	return e?.equals === !1 ? () => !1 : e?.equals ?? Pe;
}
function V(e, t) {
	let n = () => {
		if (z?.[2].has(e)) return z[2].get(e);
		if (z && t) {
			if (z[1].has(e)) return z[1].get(e);
			let n = t();
			return z[1].set(e, n), n;
		}
		return s(e);
	};
	return n[R] = e, n;
}
function Ve(e, t) {
	if (z) {
		let n = z[2].has(e) ? z[2].get(e) : s(e), r = typeof t == "function" ? t(n) : t;
		return z[2].set(e, r), z[1].clear(), r;
	}
	return typeof t == "function" ? ke(e, t) : Oe(e, t);
}
function He(e, t) {
	return Oe(e[R], t, !0);
}
function Ue(e, t) {
	return ke(e[R], t, !0);
}
function We(e) {
	return e instanceof Error ? e : Error(typeof e == "string" ? e : "Unknown error", { cause: e });
}
function H(e) {
	return e && e.owner === void 0 && (e.owner = H(t(e))), e;
}
function U(e, n = r()) {
	let i = We(e), a = n;
	for (; a;) {
		let e = ze.get(a);
		if (e?.length) {
			for (let n of e) try {
				n(i);
			} catch (e) {
				return U(e, t(a));
			}
			return;
		}
		a = t(a);
	}
	throw i;
}
function Ge(e) {
	let t = Ke(e);
	return (...e) => {
		try {
			return t(...e);
		} catch (e) {
			return U(e);
		}
	};
}
function Ke(e) {
	if (!B) return e;
	let t = h(0, () => !1), n = B.factory(e, () => ke(t, (e) => e + 1));
	return g(() => n.dispose()), (...e) => (s(t), n.track(...e));
}
function qe(e, t = Pe) {
	let n = e[R];
	return n === void 0 && (n = G(e, void 0, { equals: t })[R]), n;
}
function Je(e) {
	for (; typeof e == "function" && !e.length;) e = e();
	if (!Array.isArray(e)) return e;
	let t = [];
	for (let n of e) {
		let e = Je(n);
		Array.isArray(e) ? t.push(...e) : t.push(e);
	}
	return t;
}
function W(e, t) {
	let n = h(e, Be(t));
	return [V(n), (e) => Ve(n, e)];
}
function G(e, t, n) {
	let r = Ge(e), i = Be(n), a = !1, o = re(t, r, (e, t) => a ? i(e, t) : (a = !0, !1));
	return V(o, () => r(s(o)));
}
function Ye(e, t, n) {
	let r = t, i = Ge(e);
	o(() => {
		r = i(r);
	});
}
function K(e, t, n) {
	let r = t, i = Ge(e);
	a(() => {
		r = i(r);
	});
}
function Xe(e, t) {
	let n = !e.length, r = (t) => {
		try {
			return n ? e() : e(() => J(t));
		} catch (e) {
			return U(e);
		}
	};
	return n ? _(r, null) : t === void 0 ? i(r) : _(r, t);
}
function Ze() {
	return H(r());
}
function Qe(e, t) {
	return te(e, () => {
		try {
			return t();
		} catch (e) {
			return U(e);
		}
	});
}
function $e(e, t) {
	return _((n) => {
		let i = r();
		ze.set(i, [t]);
		try {
			return e();
		} catch (e) {
			return U(e, i);
		}
	}, r(), !0);
}
function et(e, t, n) {
	return _((i) => {
		let a = H(r());
		return a.context = { [e.id]: t }, n();
	}, r(), !0);
}
function tt(e, t) {
	let n = {
		id: Symbol("context"),
		defaultValue: e,
		Provider(e) {
			return t?.name, et(n, e.value, () => rt(() => e.children));
		}
	};
	return n;
}
function nt(e) {
	let t = H(r());
	for (; t;) {
		let n = t.context?.[e.id];
		if (n !== void 0) return n;
		t = t.owner;
	}
	return e.defaultValue;
}
function rt(e) {
	let t = G(e), n = G(() => Je(t()));
	return n.toArray = () => {
		let e = n();
		return Array.isArray(e) ? e : e == null ? [] : [e];
	}, n;
}
function it(e, t) {
	return J(() => e(t || {}));
}
var at = typeof Proxy == "function", q = () => !0, ot = {
	get(e, t, n) {
		return t === L ? n : e.get(t);
	},
	has(e, t) {
		return t === L || e.has(t);
	},
	set: q,
	deleteProperty: q,
	getOwnPropertyDescriptor(e, t) {
		return {
			configurable: !0,
			enumerable: !0,
			get() {
				return e.get(t);
			},
			set: q,
			deleteProperty: q
		};
	},
	ownKeys(e) {
		return e.keys();
	}
};
function st(e) {
	return (e = typeof e == "function" ? e() : e) ? e : {};
}
function ct() {
	for (let e = 0; e < this.length; e += 1) {
		let t = this[e]();
		if (t !== void 0) return t;
	}
}
function lt(...e) {
	let t = !1;
	for (let n = 0; n < e.length; n += 1) {
		let r = e[n];
		t ||= !!r && L in r, typeof r == "function" && (t = !0, e[n] = G(r));
	}
	if (at && t) return new Proxy({
		get(t) {
			for (let n = e.length - 1; n >= 0; --n) {
				let r = st(e[n])[t];
				if (r !== void 0) return r;
			}
		},
		has(t) {
			for (let n = e.length - 1; n >= 0; --n) if (t in st(e[n])) return !0;
			return !1;
		},
		keys() {
			let t = [];
			for (let n of e) t.push(...Object.keys(st(n)));
			return [...new Set(t)];
		}
	}, ot);
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
				get: ct.bind(n[t] = [o.get.bind(i)])
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
function ut(e, ...t) {
	let n = t.length;
	if (at && L in e) {
		let r = n > 1 ? t.flat() : t[0], i = t.map((t) => new Proxy({
			get: (n) => t.includes(n) ? e[n] : void 0,
			has: (n) => t.includes(n) && n in e,
			keys: () => t.filter((t) => t in e)
		}, ot));
		return i.push(new Proxy({
			get: (t) => r.includes(t) ? void 0 : e[t],
			has: (t) => !r.includes(t) && t in e,
			keys: () => Object.keys(e).filter((e) => !r.includes(e))
		}, ot)), i;
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
function dt(e, t, n = {}) {
	let r = qe(() => {
		let t = e() || [];
		return t[Fe], t;
	}, () => !1), i = (e, n) => t(e, V(n));
	return n.fallback ? f(r, i, n.fallback) : p(r, i);
}
function ft(e, t, n = {}) {
	let r = qe(() => {
		let t = e() || [];
		return t[Fe], t;
	}, () => !1), i = (e, n) => t(V(e), n);
	return n.fallback ? d(r, i, n.fallback) : m(r, i);
}
function pt(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return G(dt(() => e.each, e.children, t));
}
function mt(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return G(ft(() => e.each, e.children, t));
}
function ht(e) {
	let t = G(() => e.when), n = e.keyed ? t : G(t, void 0, { equals: (e, t) => !e == !t });
	return G(() => {
		let r = n();
		if (!r) return e.fallback;
		let i = e.children;
		return typeof i != "function" || !i.length ? i : J(() => i(e.keyed ? r : () => {
			if (!J(n)) throw Error("Stale read from <Show>.");
			return t();
		}));
	});
}
function gt(e) {
	return e;
}
function _t(e) {
	let t = rt(() => e.children), r = G(() => {
		let e = t(), n = Array.isArray(e) ? e : [e], r = () => void 0;
		for (let e = 0; e < n.length; e += 1) {
			let t = n[e], i = r, a = G(() => i() ? void 0 : t.when), o = t.keyed ? a : G(a, void 0, { equals: (e, t) => !e == !t });
			r = () => i() || (o() ? [
				e,
				a,
				t
			] : void 0);
		}
		return r;
	});
	return G(() => {
		let t = r()();
		if (!t) return e.fallback;
		let [i, a, o] = t, s = o.children;
		return typeof s != "function" || !s.length ? s : J(() => s(o.keyed ? a() : () => {
			if (J(r)()?.[0] !== i && !n()) throw Error("Stale read from <Match>.");
			return a();
		}));
	});
}
function vt(e) {
	let [t, n] = W(void 0);
	Re ||= /* @__PURE__ */ new Set(), Re.add(n), Y(() => Re.delete(n));
	let r = (t) => {
		let r = e.fallback;
		return typeof r == "function" && r.length ? J(() => r(t, () => n(void 0))) : r;
	};
	return G(() => {
		let i = t();
		if (i !== void 0) return r(i);
		let a, o = $e(() => e.children, (e) => {
			a = e, n(e);
		});
		return a === void 0 ? o : r(a);
	});
}
function yt() {
	return Ie ||= tt();
}
function bt() {
	return Le ||= tt();
}
var xt = (e, t) => e.showContent === t.showContent && e.showFallback === t.showFallback;
function St(e) {
	let [t] = W(() => ({ inFallback: !1 })), n = nt(bt()), r = [], [i] = W(0), a = !0, o = n ? n.register(G(() => t()().inFallback)) : null, s = G((t) => {
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
	return He(t, s), et(bt(), { register(e) {
		let t = r.length;
		return r.push(e), a || Ue(i, (e) => e + 1), G(() => s()[t] ?? {
			showContent: !0,
			showFallback: !0
		}, void 0, { equals: xt });
	} }, () => {
		let t = e.children;
		return a = !1, Ue(i, (e) => e + 1), t;
	});
}
function Ct(e) {
	let [t] = W(!1), n = 0, i = {
		effects: [],
		inFallback: t,
		resolved: !1,
		increment() {
			++n === 1 && He(t, !0);
		},
		decrement() {
			--n === 0 && He(t, !1);
		}
	}, a = nt(bt())?.register(i.inFallback), o = r(), s, c;
	return Y(() => s?.()), et(yt(), i, () => {
		let t = G(() => e.children);
		return G((n) => {
			let r = a ? a() : {
				showContent: !0,
				showFallback: !0
			};
			if (!i.inFallback() && r.showContent) return i.resolved = !0, s?.(), s = void 0, t();
			if (r.showFallback) return s ? c : Xe((t) => (s = t, c = e.fallback), o);
		});
	});
}
function J(e) {
	return S(B ? () => B.untrack(e) : e);
}
var Y = g, wt = /*#__PURE__*/ new Set("className value readOnly noValidate formNoValidate isMap noModule playsInline adAuctionHeaders allowFullscreen browsingTopics defaultChecked defaultMuted defaultSelected disablePictureInPicture disableRemotePlayback preservesPitch shadowRootClonable shadowRootCustomElementRegistry shadowRootDelegatesFocus shadowRootSerializable sharedStorageWritable allowfullscreen async alpha autofocus autoplay checked controls default disabled formnovalidate hidden indeterminate inert ismap loop multiple muted nomodule novalidate open playsinline readonly required reversed seamless selected adauctionheaders browsingtopics credentialless defaultchecked defaultmuted defaultselected defer disablepictureinpicture disableremoteplayback preservespitch shadowrootclonable shadowrootcustomelementregistry shadowrootdelegatesfocus shadowrootserializable sharedstoragewritable".split(" ")), Tt = /*#__PURE__*/ new Set("innerHTML textContent innerText children".split(" ")), Et = /*#__PURE__*/ Object.assign(Object.create(null), {
	className: "class",
	htmlFor: "for"
}), Dt = /*#__PURE__*/ Object.assign(Object.create(null), {
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
function Ot(e, t) {
	let n = Dt[e];
	return typeof n == "object" ? n[t] ? n.$ : void 0 : n;
}
var kt = /*#__PURE__*/ new Set("beforeinput click dblclick contextmenu focusin focusout input keydown keyup mousedown mousemove mouseout mouseover mouseup pointerdown pointermove pointerout pointerover pointerup touchend touchmove touchstart".split(" ")), At = /*#__PURE__*/ new Set("altGlyph altGlyphDef altGlyphItem animate animateColor animateMotion animateTransform circle clipPath color-profile cursor defs desc ellipse feBlend feColorMatrix feComponentTransfer feComposite feConvolveMatrix feDiffuseLighting feDisplacementMap feDistantLight feDropShadow feFlood feFuncA feFuncB feFuncG feFuncR feGaussianBlur feImage feMerge feMergeNode feMorphology feOffset fePointLight feSpecularLighting feSpotLight feTile feTurbulence filter font font-face font-face-format font-face-name font-face-src font-face-uri foreignObject g glyph glyphRef hkern image line linearGradient marker mask metadata missing-glyph mpath path pattern polygon polyline radialGradient rect set stop svg switch symbol text textPath tref tspan use view vkern".split(" ")), jt = {
	xlink: "http://www.w3.org/1999/xlink",
	xml: "http://www.w3.org/XML/1998/namespace"
}, Mt = /*#__PURE__*/ new Set("html base head link meta style title body address article aside footer header main nav section blockquote dd div dl dt figcaption figure hr li ol p pre ul a abbr b bdi bdo br cite code data dfn em i kbd mark q rp rt ruby s samp small span strong sub sup time u var wbr area audio img map track video embed iframe object param picture portal source svg math canvas noscript script del ins caption col colgroup table tbody td tfoot th thead tr button datalist fieldset form input label legend meter optgroup option output progress select textarea details dialog menu summary slot template acronym applet basefont bgsound big blink center content dir font frame frameset hgroup image keygen marquee menuitem nobr noembed noframes plaintext rb rtc shadow spacer strike tt xmp h1 h2 h3 h4 h5 h6 webview isindex listing multicol nextid noindex search".split(" ")), Nt = (e) => G(() => e());
function Pt(e, t, n) {
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
var X = "_$DX_DELEGATE";
function Ft(e, t, n, r = {}) {
	let i;
	return Xe((r) => {
		i = r, t === document ? e() : Zt(t, e(), t.firstChild ? null : void 0, n);
	}, r.owner), () => {
		i(), t.textContent = "";
	};
}
function It(e, t, n, r) {
	let i, a = () => {
		let t = r ? document.createElementNS("http://www.w3.org/1998/Math/MathML", "template") : document.createElement("template");
		return t.innerHTML = e, n ? t.content.firstChild.firstChild : r ? t.firstChild : t.content.firstChild;
	}, o = t ? () => J(() => document.importNode(i ||= a(), !0)) : () => (i ||= a()).cloneNode(!0);
	return o.cloneNode = o, o;
}
function Lt(e, t = window.document) {
	let n = t[X] || (t[X] = /* @__PURE__ */ new Set());
	for (let r = 0, i = e.length; r < i; r++) {
		let i = e[r];
		n.has(i) || (n.add(i), t.addEventListener(i, nn));
	}
}
function Rt(e = window.document) {
	if (e[X]) {
		for (let t of e[X].keys()) e.removeEventListener(t, nn);
		delete e[X];
	}
}
function zt(e, t, n) {
	Z(e) || (e[t] = n);
}
function Bt(e, t, n) {
	Z(e) || (n == null ? e.removeAttribute(t) : e.setAttribute(t, n));
}
function Vt(e, t, n, r) {
	Z(e) || (r == null ? e.removeAttributeNS(t, n) : e.setAttributeNS(t, n, r));
}
function Ht(e, t, n) {
	Z(e) || (n ? e.setAttribute(t, "") : e.removeAttribute(t));
}
function Ut(e, t) {
	Z(e) || (t == null ? e.removeAttribute("class") : e.className = t);
}
function Wt(e, t, n, r) {
	if (r) Array.isArray(n) ? (e[`$$${t}`] = n[0], e[`$$${t}Data`] = n[1]) : e[`$$${t}`] = n;
	else if (Array.isArray(n)) {
		let r = n[0];
		e.addEventListener(t, n[0] = (t) => r.call(e, n[1], t));
	} else e.addEventListener(t, n, typeof n != "function" && n);
}
function Gt(e, t, n = {}) {
	let r = Object.keys(t || {}), i = Object.keys(n), a, o;
	for (a = 0, o = i.length; a < o; a++) {
		let r = i[a];
		!r || r === "undefined" || t[r] || (en(e, r, !1), delete n[r]);
	}
	for (a = 0, o = r.length; a < o; a++) {
		let i = r[a], o = !!t[i];
		!i || i === "undefined" || n[i] === o || !o || (en(e, i, !0), n[i] = o);
	}
	return n;
}
function Kt(e, t, n) {
	if (!t) return n ? Bt(e, "style") : t;
	let r = e.style;
	if (typeof t == "string") return r.cssText = t;
	typeof n == "string" && (r.cssText = n = void 0), n ||= {};
	let i, a;
	for (a in n) t[a] ?? r.removeProperty(a), delete n[a];
	for (a in t) i = t[a], i !== n[a] && (r.setProperty(a, i), n[a] = i);
	return n;
}
function qt(e, t, n) {
	n == null ? e.style.removeProperty(t) : e.style.setProperty(t, n);
}
function Jt(e, t = {}, n, r) {
	let i = {};
	return r || K(() => i.children = Q(e, t.children, i.children)), K(() => typeof t.ref == "function" && Xt(t.ref, e)), K(() => Qt(e, t, n, !0, i, !0)), i;
}
function Yt(e, t) {
	let n = e[t];
	return Object.defineProperty(e, t, {
		get() {
			return n();
		},
		enumerable: !0
	}), e;
}
function Xt(e, t, n) {
	return J(() => e(t, n));
}
function Zt(e, t, n, r) {
	if (n !== void 0 && !r && (r = []), typeof t != "function") return Q(e, t, r, n);
	K((r) => Q(e, t(), r, n), r);
}
function Qt(e, t, n, r, i = {}, a = !1) {
	t ||= {};
	for (let r in i) if (!(r in t)) {
		if (r === "children") continue;
		i[r] = tn(e, r, null, i[r], n, a, t);
	}
	for (let o in t) {
		if (o === "children") {
			r || Q(e, t.children);
			continue;
		}
		let s = t[o];
		i[o] = tn(e, o, s, i[o], n, a, t);
	}
}
function Z(e) {
	return !1;
}
function $t(e) {
	return e.toLowerCase().replace(/-([a-z])/g, (e, t) => t.toUpperCase());
}
function en(e, t, n) {
	let r = t.trim().split(/\s+/);
	for (let t = 0, i = r.length; t < i; t++) e.classList.toggle(r[t], n);
}
function tn(e, t, n, r, i, a, o) {
	let s, c, l, u, d;
	if (t === "style") return Kt(e, n, r);
	if (t === "classList") return Gt(e, n, r);
	if (n === r) return r;
	if (t === "ref") a || n(e);
	else if (t.slice(0, 3) === "on:") {
		let i = t.slice(3);
		r && e.removeEventListener(i, r, typeof r != "function" && r), n && e.addEventListener(i, n, typeof n != "function" && n);
	} else if (t.slice(0, 10) === "oncapture:") {
		let i = t.slice(10);
		r && e.removeEventListener(i, r, !0), n && e.addEventListener(i, n, !0);
	} else if (t.slice(0, 2) === "on") {
		let i = t.slice(2).toLowerCase(), a = kt.has(i);
		if (!a && r) {
			let t = Array.isArray(r) ? r[0] : r;
			e.removeEventListener(i, t);
		}
		(a || n) && (Wt(e, i, n, a), a && Lt([i]));
	} else if (t.slice(0, 5) === "attr:") Bt(e, t.slice(5), n);
	else if (t.slice(0, 5) === "bool:") Ht(e, t.slice(5), n);
	else if ((d = t.slice(0, 5) === "prop:") || (l = Tt.has(t)) || !i && ((u = Ot(t, e.tagName)) || (c = wt.has(t))) || (s = e.nodeName.includes("-") || "is" in o)) {
		if (d) t = t.slice(5), c = !0;
		else if (Z(e)) return n;
		t === "class" || t === "className" ? Ut(e, n) : s && !c && !l ? e[$t(t)] = n : e[u || t] = n;
	} else {
		let r = i && t.indexOf(":") > -1 && jt[t.split(":")[0]];
		r ? Vt(e, r, t, n) : Bt(e, Et[t] || t, n);
	}
	return n;
}
function nn(e) {
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
	} else if (o === "function") return K(() => {
		let i = t();
		for (; typeof i == "function";) i = i();
		n = Q(e, i, n, r);
	}), () => n;
	else if (Array.isArray(t)) {
		let o = [], c = n && Array.isArray(n);
		if (rn(o, t, n, i)) return K(() => n = Q(e, o, n, r, !0)), () => n;
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
		} else c ? n.length === 0 ? an(e, o, r) : Pt(e, n, o) : (n && $(e), an(e, o));
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
function rn(e, t, n, r) {
	let i = !1;
	for (let a = 0, o = t.length; a < o; a++) {
		let o = t[a], s = n && n[e.length], c;
		if (o != null && o !== !0 && o !== !1) {
			if ((c = typeof o) == "object" && o.nodeType) e.push(o);
			else if (Array.isArray(o)) i = rn(e, o, s) || i;
			else if (c === "function") {
				if (r) {
					for (; typeof o == "function";) o = o();
					i = rn(e, Array.isArray(o) ? o : [o], Array.isArray(s) ? s : [s]) || i;
				} else e.push(o), i = !0;
			} else {
				let t = String(o);
				s && s.nodeType === 3 && s.data === t ? e.push(s) : e.push(document.createTextNode(t));
			}
		}
	}
	return i;
}
function an(e, t, n) {
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
function on(e, t) {
	e.innerHTML = t;
}
var sn = !1, cn = !1, ln = "http://www.w3.org/2000/svg";
function un(e, t, n) {
	return t ? document.createElementNS(ln, e) : document.createElement(e, { is: n });
}
function dn(e) {
	let { useShadow: t } = e, n = document.createTextNode(""), r = () => e.mount || document.body, i = Ze(), a, o = !1;
	return Ye(() => {
		o && (Ze().user = o = !1), a ||= Qe(i, () => G(() => e.children));
		let s = r();
		if (s instanceof HTMLHeadElement) {
			let [e, t] = W(!1);
			Xe((t) => Zt(s, () => e() ? t() : a(), null)), Y(() => t(!0));
		} else {
			let r = un(e.isSVG ? "g" : "div", e.isSVG), i = t && r.attachShadow ? r.attachShadow({ mode: "open" }) : r;
			Object.defineProperty(r, "_$host", {
				get() {
					return n.parentNode;
				},
				configurable: !0
			}), Zt(i, a), s.appendChild(r), e.ref && e.ref(r), Y(() => s.removeChild(r));
		}
	}, void 0, { render: !o }), n;
}
function fn(e, t) {
	let n = G(e);
	return G(() => {
		let e = n();
		switch (typeof e) {
			case "function": return J(() => e(t));
			case "string":
				let n = At.has(e), r = un(e, n, J(() => t.is));
				return Jt(r, t, n), r;
		}
	});
}
function pn(e) {
	let [, t] = ut(e, ["component"]);
	return fn(() => e.component, t);
}
//#endregion
export { Et as Aliases, Tt as ChildProperties, Mt as DOMElements, kt as DelegatedEvents, pn as Dynamic, vt as ErrorBoundary, pt as For, mt as Index, gt as Match, dn as Portal, wt as Properties, At as SVGElements, jt as SVGNamespace, ht as Show, Ct as Suspense, St as SuspenseList, _t as Switch, Wt as addEventListener, Qt as assign, Gt as classList, Ut as className, Rt as clearDelegatedEvents, it as createComponent, fn as createDynamic, Lt as delegateEvents, Yt as dynamicProperty, K as effect, Ze as getOwner, Ot as getPropAlias, on as innerHTML, Zt as insert, cn as isDev, sn as isServer, Nt as memo, lt as mergeProps, Ft as render, Bt as setAttribute, Vt as setAttributeNS, Ht as setBoolAttribute, zt as setProperty, qt as setStyleProperty, Jt as spread, Kt as style, It as template, J as untrack, Xt as use };
