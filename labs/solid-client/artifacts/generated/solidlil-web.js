//#region packages/solidlil/reactive.generated.js
var e = "Potential Infinite Loop Detected.", t = (e) => e[2], n = () => D >= 0 ? w(2, D) : null, r = () => O >= 0 ? w(2, O) : k >= 0 ? w(1, k) : null, i = (e) => _(e, r(), !1), a = (e) => ne(e, !1, !1), o = (e) => ne(e, !1, !0), s = (e) => S(e), c = (e) => {
	e[1] == 1 ? v(E[e[2]][4], e[0]) : e[1] == 2 && v(T[e[2]][6], e[0]);
}, l = (e) => {
	e[1] == 1 ? v(E[e[2]][3], e[0]) : e[1] == 2 && v(T[e[2]][5], e[0]);
}, u = (e, t) => e == t, d = (e, t, n) => {
	let r = [
		[],
		[],
		[]
	];
	Te(r);
	let i = [[], null];
	return Ce(i), g(() => {
		Ee(r), Ne(i);
	}), () => {
		let a = S(e);
		return x(() => a.length == 0 ? (Ee(r), Me(i, n)) : (Ne(i), ae(r, a, t)));
	};
}, f = (e, t, n) => {
	let r = [
		[],
		[],
		[],
		[]
	];
	we(r);
	let i = [[], null];
	return Ce(i), g(() => {
		De(r), Ne(i);
	}), () => {
		let a = S(e);
		return x(() => a.length == 0 ? (De(r), Me(i, n)) : (Ne(i), ie(r, a, t)));
	};
}, p = (e, t) => {
	let n = [
		[],
		[],
		[],
		[]
	];
	return we(n), g(() => {
		De(n);
	}), () => {
		let r = S(e);
		return x(() => ie(n, r, t));
	};
}, m = (e, t) => {
	let n = [
		[],
		[],
		[]
	];
	return Te(n), g(() => {
		Ee(n);
	}), () => {
		let r = S(e);
		return x(() => ae(n, r, t));
	};
}, h = (e, t) => {
	let n = [
		null,
		[],
		null,
		0,
		0
	];
	return b(n, e, t), n;
}, g = (e) => {
	var t = [e];
	return O >= 0 ? T[O][7].push(t) : k >= 0 && E[k][5].push(t), e;
}, ee = (e) => {
	var t = A == 0 && !M;
	t && (A = A + 1 | 0);
	try {
		for (var n = 0; n < e.length; n = n + 1 | 0) je(e[n] | 0);
	} catch (e) {
		throw t && (A = A - 1 | 0), e;
	}
	t && (A = A - 1 | 0, j > 0 ? ge() : he());
}, te = (e, t) => {
	var n = D, r = O, i = k;
	D = -1, e && e[0] != 0 ? e[0] == 2 ? (O = e[1], k = -1) : (O = -1, k = e[1]) : (O = -1, k = -1), j = j + 1 | 0;
	var a = !1;
	try {
		var o = t();
		return a = !0, o;
	} finally {
		j = j - 1 | 0, D = n, O = r, k = i, a && he();
	}
}, _ = (e, t = null, n = !1) => {
	var r = E.length;
	Fe.length > 0 && (r = Fe.pop());
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
	a[3] = [], a[4] = [], a[5] = [], a[6] = null, a[7] = !1, r == E.length ? E.push(a) : E[r] = a, n && (i == 1 ? E[t][4].push(r) : i == 2 && T[t][6].push(r));
	var o = D, s = k, c = O;
	D = -1, k = r, O = -1, j = j + 1 | 0, t = () => {
		E[r] == a && xe(r);
	};
	var l = !1;
	try {
		var u = e(t);
		return l = !0, u;
	} finally {
		j = j - 1 | 0, D = o, O = c, k = s, l && he();
	}
}, ne = (e, t, n) => {
	var r = T.length;
	if (Pe.length > 0 && (r = Pe.pop()), O >= 0) var i = O, a = 2, o;
	else k >= 0 ? (i = k, a = 1) : (a = 0, i = -1);
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
	], o[4] = [], o[5] = [], o[6] = [], o[7] = [], o[8] = null, o[9] = t, o[10] = !1, o[11] = !1, o[12] = 0, o[13] = [], r == T.length ? T.push(o) : T[r] = o, O >= 0 ? T[O][5].push(r) : k >= 0 && E[k][3].push(r), n && (j > 0 || M) ? je(r) : y(o), r;
}, re = (e, t, n) => {
	let r = [
		null,
		[],
		null,
		0,
		0
	];
	return b(r, e, n), ne(() => {
		C(r, t(r[0]));
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
		i >= 0 ? (r.add(i), C(e[3][i], o), d.push(e[1][i]), f.push(e[2][i]), p.push(e[3][i])) : (h = [
			null,
			[],
			null,
			0,
			0
		], b(h, o, u), g = [() => {}], d.push(_(((e, t, n, r) => (i) => (r[0] = i, e(t, n)))(n, m, h, g), null, !1)), f.push(g), p.push(h)), l.push(m);
	}
	for (t = 0; t < e[0].length; t += 1) !r.has(t) && e[2][t][0]();
	return e[0] = l, e[1] = d, e[2] = f, e[3] = p, e[1];
}
function ae(e, t, n) {
	var r = e[2].length;
	t.length < r && (r = t.length);
	for (var i = 0, a, o; i < r; i += 1) e[2][i][0] != t[i] && C(e[2][i], t[i]);
	for (var s = e[2].length; s < t.length; s = s + 1 | 0) i = t[s], a = [
		null,
		[],
		null,
		0,
		0
	], b(a, i, (e, t) => e == t), o = [() => {}], i = _(((e, t, n) => (r) => (n[0] = r, e(t, s)))(n, a, o), null, !1), e[2].push(a), e[1].push(o), e[0].push(i);
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
	if (!(T[e][11] || e == O)) {
		for (n = 0; n < T[e][13].length; n += 1) t = T[e][13][n] | 0, (fe(t) || de(t)) && le(t);
		pe(e) && y(T[e]);
	}
}, ue = (e, t, n) => {
	if (e == t) return !0;
	if (n >= T.length) return !1;
	for (var r = 0; r < T[e][13].length; r += 1) if (ue(T[e][13][r] | 0, t, n + 1 | 0)) return !0;
	return !1;
}, de = (e) => {
	for (var t = 0; t < T[e][13].length; t += 1) if (fe(T[e][13][t] | 0)) return !0;
	return !1;
}, fe = (e) => {
	for (var t = 0; t < P.length; t += 1) if ((P[t] | 0) == e) return !0;
	return !1;
}, pe = (e) => {
	for (var t = 0; t < P.length; t += 1) if ((P[t] | 0) == e) return ve(t), !0;
	return !1;
}, v = (e, t) => {
	for (var n = e.length - 1; n >= 0; --n) if ((e[n] | 0) == t) {
		e[n] = e[e.length - 1] | 0, e.pop();
		return;
	}
}, me = (e, t) => {
	var n = T[t][1];
	t = T[t][2];
	for (var r = 0, i; i = n != 0 && r <= (T.length + E.length | 0), i;) {
		if (n == 2) {
			if (t == e) return !0;
			n = T[t][1], t = T[t][2];
		} else n = E[t][1], t = E[t][2];
		r = r + 1 | 0;
	}
	return !1;
}, he = () => {
	if (!(M || A > 0 || j > 0)) {
		M = !0;
		var t = 0;
		try {
			for (; P.length > 0;) {
				if (t = t + 1 | 0, t > 1e3) throw e;
				y(T[_e()]);
			}
			M = !1;
		} catch (e) {
			throw P = [], M = !1, e;
		}
	}
}, ge = () => {
	if (!(M || A > 0)) {
		M = !0;
		var t = 0;
		try {
			for (;;) {
				for (var n = -1, r = 0; r < P.length; r = r + 1 | 0) {
					var i = P[r] | 0;
					i != O && T[i][9] && (n < 0 || T[i][12] > T[P[n] | 0][12]) && (n = r);
				}
				if (n < 0) {
					M = !1;
					return;
				}
				if (t = t + 1 | 0, t > 1e3) throw e;
				r = ve(n), y(T[r]);
			}
		} catch (e) {
			throw P = [], M = !1, e;
		}
	}
}, _e = () => {
	for (var e = -1, t = -1, n = -1, r = 0, i, a, o, s, c; r < P.length; r += 1) i = P[r] | 0, T[i][9] ? (e < 0 || T[i][12] > T[P[e] | 0][12]) && (e = r) : !T[i][10] && t < 0 ? t = r : n < 0 ? n = r : (o = P[n] | 0, a = T[i], s = T[o], c = a[1] == s[1] && a[2] == s[2], (c && a[12] > s[12] || !c && (me(i, o) || !me(o, i) && a[12] > s[12])) && (n = r));
	return t >= 0 && (e = t), ve(e < 0 ? n : e);
}, ve = (e) => {
	for (var t = P[e] | 0; e < P.length - 1; e = e + 1 | 0) P[e] = P[e + 1 | 0] | 0;
	return P.pop(), t;
};
function ye(e) {
	for (; e[4].length > 0;) {
		var t = e[4][e[4].length - 1];
		e[4].pop(), oe(t, e[0]);
	}
}
function be(e) {
	for (; e[6].length > 0;) {
		var t = e[6][e[6].length - 1] | 0;
		e[6].pop(), xe(t);
	}
	for (; e[5].length > 0;) t = e[5][e[5].length - 1] | 0, e[5].pop(), Se(t);
	for (; e[7].length > 0;) t = e[7][e[7].length - 1], e[7].pop(), t[0]();
}
var xe = (e) => {
	var t = E[e];
	if (!t[7]) {
		for (; t[4].length > 0;) {
			var n = t[4][t[4].length - 1] | 0;
			t[4].pop(), xe(n);
		}
		for (; t[3].length > 0;) n = t[3][t[3].length - 1] | 0, t[3].pop(), Se(n);
		for (; t[5].length > 0;) n = t[5][t[5].length - 1], t[5].pop(), n[0]();
		c(t), (n = t[6]) && (n[0] = 0, t[6] = null), t[7] = !0, Fe.push(e);
	}
}, Se = (e) => {
	var t = T[e];
	if (!t[11]) {
		be(t), ye(t), pe(e), l(t), t[3] = () => {}, t[13] = [];
		var n = t[8];
		n && (n[0] = 0, t[8] = null), t[11] = !0, Pe.push(e);
	}
};
function y(e) {
	if (!e[11]) {
		be(e), ye(e);
		var t = D, n = O, r = k;
		D = e[0], O = e[0], k = -1, e[12] = 0, e[13] = [], j = j + 1 | 0, e[3](), e[10] = !0, j = j - 1 | 0, D = t, O = n, k = r, he();
	}
}
function Ce(e) {
	e[0] = [], e[1] = () => {};
}
function we(e) {
	e[0] = [], e[1] = [], e[2] = [], e[3] = [];
}
function Te(e) {
	e[0] = [], e[1] = [], e[2] = [];
}
function Ee(e) {
	for (var t = 0; t < e[1].length; t += 1) e[1][t][0]();
	e[0] = [], e[1] = [], e[2] = [];
}
function De(e) {
	for (var t = 0; t < e[2].length; t += 1) e[2][t][0]();
	e[0] = [], e[1] = [], e[2] = [], e[3] = [];
}
function b(e, t, n) {
	e[0] = t, e[1] = [], e[2] = n, e[3] = 0, e[4] = -1;
}
function Oe(e, t) {
	return C(e, t(e[0]));
}
var ke = (e, t, n = !1) => {
	var r = N;
	r && (n = !0), N = n;
	try {
		return C(e, t);
	} finally {
		N = r;
	}
}, Ae = (e, t, n = !1) => {
	var r = N;
	r && (n = !0), N = n;
	try {
		return Oe(e, t);
	} finally {
		N = r;
	}
}, x = (e) => {
	var t = D;
	D = -1;
	try {
		return e();
	} finally {
		D = t;
	}
};
function S(e) {
	var t;
	if (D < 0 && e[4] < 0) return e[0];
	if (D >= 0 && e[4] >= 0 && e[4] == O) throw "Circular memo dependency detected.";
	if (e[4] >= 0 && e[4] != O ? (t = O >= 0 && !T[O][9] && !T[O][10], t = !t) : t = !1, t && le(e[4]), D >= 0) {
		t = D;
		var n = e[3] + 1 | 0;
		n > T[t][12] && (T[t][12] = n), e[4] >= 0 && ce(T[t], e[4]), se(e, t) && T[t][4].push(e);
	}
	return e[0];
}
var je = (e) => {
	if (O >= 0 && T[O][9] && !N && ue(O, e, 0) && !me(e, O)) throw "Reactive dependency cycle detected.";
	!T[e][11] && !fe(e) && P.push(e);
};
function C(e, t) {
	return O >= 0 && T[O][9] && (e[3] = T[O][12], e[4] = O), e[2](e[0], t) ? t : (e[0] = t, ee(e[1]), t);
}
var w = (e, t) => {
	if (e == 1) {
		var n = E[t][6];
		return n || (n = [
			e,
			t,
			null
		], E[t][6] = n, n[2] = w(E[t][1], E[t][2]), n);
	}
	return e == 2 ? (n = T[t][8]) ? n : (n = [
		e,
		t,
		null
	], T[t][8] = n, n[2] = w(T[t][1], T[t][2]), n) : null;
};
function Me(e, t) {
	return e[0].length == 0 && (e[0] = [_((n) => (e[1] = n, t()), null, !1)]), e[0];
}
function Ne(e) {
	e[0].length > 0 && e[1](), e[0] = [];
}
var T = [], E = [], Pe = [], Fe = [], D = -1, O = -1, k = -1, A = 0, j = 0, M = !1, N = !1, P = [], Ie = (e, t) => e === t, Le = Symbol("solid-proxy"), Re = Symbol("solid-track"), F = {
	context: void 0,
	registry: void 0,
	effects: void 0,
	done: !1,
	getContextId() {
		return ze(this.context.count);
	},
	getNextContextId() {
		return ze(this.context.count++);
	}
};
function ze(e) {
	let t = String(e), n = t.length - 1;
	return `${F.context.id}${n ? String.fromCharCode(96 + n) : ""}${t}`;
}
var I = Symbol("solidlil-signal"), Be = !1, L, R, Ve, He, Ue, We = /* @__PURE__ */ new WeakMap();
function Ge(e) {
	return e?.equals === !1 ? () => !1 : e?.equals ?? Ie;
}
function z(e, t) {
	let n = () => {
		if (L?.[2].has(e)) return L[2].get(e);
		if (L && t) {
			if (L[1].has(e)) return L[1].get(e);
			let n = t();
			return L[1].set(e, n), n;
		}
		return s(e);
	};
	return n[I] = e, n;
}
function Ke(e, t) {
	if (L) {
		let n = L[2].has(e) ? L[2].get(e) : s(e), r = typeof t == "function" ? t(n) : t;
		return L[2].set(e, r), L[1].clear(), r;
	}
	return typeof t == "function" ? Ae(e, t) : ke(e, t);
}
function qe(e, t) {
	return ke(e[I], t, !0);
}
function Je(e, t) {
	return Ae(e[I], t, !0);
}
function Ye(e) {
	return e instanceof Error ? e : Error(typeof e == "string" ? e : "Unknown error", { cause: e });
}
function B(e) {
	return e && e.owner === void 0 && (e.owner = B(t(e))), e;
}
function V(e, n = r()) {
	let i = Ye(e), a = n;
	for (; a;) {
		let e = We.get(a);
		if (e?.length) {
			for (let n of e) try {
				n(i);
			} catch (e) {
				return V(e, t(a));
			}
			return;
		}
		a = t(a);
	}
	throw i;
}
function Xe(e) {
	let t = Ze(e);
	return (...e) => {
		try {
			return t(...e);
		} catch (e) {
			return V(e);
		}
	};
}
function Ze(e) {
	if (!R) return e;
	let t = h(0, () => !1), n = R.factory(e, () => Ae(t, (e) => e + 1));
	return g(() => n.dispose()), (...e) => (s(t), n.track(...e));
}
function Qe(e, t = Ie) {
	let n = e[I];
	return n === void 0 && (n = U(e, void 0, { equals: t })[I]), n;
}
function $e(e) {
	for (; typeof e == "function" && !e.length;) e = e();
	if (!Array.isArray(e)) return e;
	let t = [];
	for (let n of e) {
		let e = $e(n);
		Array.isArray(e) ? t.push(...e) : t.push(e);
	}
	return t;
}
function et() {
	Be = !0;
}
function H(e, t) {
	let n = h(e, Ge(t));
	return [z(n), (e) => Ke(n, e)];
}
function U(e, t, n) {
	let r = Xe(e), i = Ge(n), a = !1, o = re(t, r, (e, t) => a ? i(e, t) : (a = !0, !1));
	return z(o, () => r(s(o)));
}
function tt(e, t, n) {
	let r = t, i = Xe(e);
	o(() => {
		r = i(r);
	});
}
function W(e, t, n) {
	let r = t, i = Xe(e);
	a(() => {
		r = i(r);
	});
}
function nt(e, t) {
	let n = !e.length, r = (t) => {
		try {
			return n ? e() : e(() => K(t));
		} catch (e) {
			return V(e);
		}
	};
	return n ? _(r, null) : t === void 0 ? i(r) : _(r, t);
}
function rt() {
	return B(r());
}
function it(e, t) {
	return te(e, () => {
		try {
			return t();
		} catch (e) {
			return V(e);
		}
	});
}
function at(e, t) {
	return _((n) => {
		let i = r();
		We.set(i, [t]);
		try {
			return e();
		} catch (e) {
			return V(e, i);
		}
	}, r(), !0);
}
function ot(e, t, n) {
	return _((i) => {
		let a = B(r());
		return a.context = { [e.id]: t }, n();
	}, r(), !0);
}
function st(e, t) {
	let n = {
		id: Symbol("context"),
		defaultValue: e,
		Provider(e) {
			return t?.name, ot(n, e.value, () => lt(() => e.children));
		}
	};
	return n;
}
function ct(e) {
	let t = B(r());
	for (; t;) {
		let n = t.context?.[e.id];
		if (n !== void 0) return n;
		t = t.owner;
	}
	return e.defaultValue;
}
function lt(e) {
	let t = U(e), n = U(() => $e(t()));
	return n.toArray = () => {
		let e = n();
		return Array.isArray(e) ? e : e == null ? [] : [e];
	}, n;
}
function ut(e, t) {
	if (Be && F.context) {
		let n = F.context;
		F.context = {
			...n,
			id: F.getNextContextId(),
			count: 0
		};
		let r = K(() => e(t || {}));
		return F.context = n, r;
	}
	return K(() => e(t || {}));
}
var dt = typeof Proxy == "function", G = () => !0, ft = {
	get(e, t, n) {
		return t === Le ? n : e.get(t);
	},
	has(e, t) {
		return t === Le || e.has(t);
	},
	set: G,
	deleteProperty: G,
	getOwnPropertyDescriptor(e, t) {
		return {
			configurable: !0,
			enumerable: !0,
			get() {
				return e.get(t);
			},
			set: G,
			deleteProperty: G
		};
	},
	ownKeys(e) {
		return e.keys();
	}
};
function pt(e) {
	return (e = typeof e == "function" ? e() : e) ? e : {};
}
function mt() {
	for (let e = 0; e < this.length; e += 1) {
		let t = this[e]();
		if (t !== void 0) return t;
	}
}
function ht(...e) {
	let t = !1;
	for (let n = 0; n < e.length; n += 1) {
		let r = e[n];
		t ||= !!r && Le in r, typeof r == "function" && (t = !0, e[n] = U(r));
	}
	if (dt && t) return new Proxy({
		get(t) {
			for (let n = e.length - 1; n >= 0; --n) {
				let r = pt(e[n])[t];
				if (r !== void 0) return r;
			}
		},
		has(t) {
			for (let n = e.length - 1; n >= 0; --n) if (t in pt(e[n])) return !0;
			return !1;
		},
		keys() {
			let t = [];
			for (let n of e) t.push(...Object.keys(pt(n)));
			return [...new Set(t)];
		}
	}, ft);
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
				get: mt.bind(n[t] = [o.get.bind(i)])
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
function gt(e, ...t) {
	let n = t.length;
	if (dt && Le in e) {
		let r = n > 1 ? t.flat() : t[0], i = t.map((t) => new Proxy({
			get: (n) => t.includes(n) ? e[n] : void 0,
			has: (n) => t.includes(n) && n in e,
			keys: () => t.filter((t) => t in e)
		}, ft));
		return i.push(new Proxy({
			get: (t) => r.includes(t) ? void 0 : e[t],
			has: (t) => !r.includes(t) && t in e,
			keys: () => Object.keys(e).filter((e) => !r.includes(e))
		}, ft)), i;
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
function _t(e, t, n = {}) {
	let r = Qe(() => {
		let t = e() || [];
		return t[Re], t;
	}, () => !1), i = (e, n) => t(e, z(n));
	return n.fallback ? f(r, i, n.fallback) : p(r, i);
}
function vt(e, t, n = {}) {
	let r = Qe(() => {
		let t = e() || [];
		return t[Re], t;
	}, () => !1), i = (e, n) => t(z(e), n);
	return n.fallback ? d(r, i, n.fallback) : m(r, i);
}
function yt(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return U(_t(() => e.each, e.children, t));
}
function bt(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return U(vt(() => e.each, e.children, t));
}
function xt(e) {
	let t = U(() => e.when), n = e.keyed ? t : U(t, void 0, { equals: (e, t) => !e == !t });
	return U(() => {
		let r = n();
		if (!r) return e.fallback;
		let i = e.children;
		return typeof i != "function" || !i.length ? i : K(() => i(e.keyed ? r : () => {
			if (!K(n)) throw Error("Stale read from <Show>.");
			return t();
		}));
	});
}
function St(e) {
	return e;
}
function Ct(e) {
	let t = lt(() => e.children), r = U(() => {
		let e = t(), n = Array.isArray(e) ? e : [e], r = () => void 0;
		for (let e = 0; e < n.length; e += 1) {
			let t = n[e], i = r, a = U(() => i() ? void 0 : t.when), o = t.keyed ? a : U(a, void 0, { equals: (e, t) => !e == !t });
			r = () => i() || (o() ? [
				e,
				a,
				t
			] : void 0);
		}
		return r;
	});
	return U(() => {
		let t = r()();
		if (!t) return e.fallback;
		let [i, a, o] = t, s = o.children;
		return typeof s != "function" || !s.length ? s : K(() => s(o.keyed ? a() : () => {
			if (K(r)()?.[0] !== i && !n()) throw Error("Stale read from <Match>.");
			return a();
		}));
	});
}
function wt(e) {
	let [t, n] = H(void 0);
	Ue ||= /* @__PURE__ */ new Set(), Ue.add(n), q(() => Ue.delete(n));
	let r = (t) => {
		let r = e.fallback;
		return typeof r == "function" && r.length ? K(() => r(t, () => n(void 0))) : r;
	};
	return U(() => {
		let i = t();
		if (i !== void 0) return r(i);
		let a, o = at(() => e.children, (e) => {
			a = e, n(e);
		});
		return a === void 0 ? o : r(a);
	});
}
function Tt() {
	return Ve ||= st();
}
function Et() {
	return He ||= st();
}
var Dt = (e, t) => e.showContent === t.showContent && e.showFallback === t.showFallback;
function Ot(e) {
	let [t] = H(() => ({ inFallback: !1 })), n = ct(Et()), r = [], [i] = H(0), a = !0, o = n ? n.register(U(() => t()().inFallback)) : null, s = U((t) => {
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
	return qe(t, s), ot(Et(), { register(e) {
		let t = r.length;
		return r.push(e), a || Je(i, (e) => e + 1), U(() => s()[t] ?? {
			showContent: !0,
			showFallback: !0
		}, void 0, { equals: Dt });
	} }, () => {
		let t = e.children;
		return a = !1, Je(i, (e) => e + 1), t;
	});
}
function kt(e) {
	let [t] = H(!1), n = 0, i = {
		effects: [],
		inFallback: t,
		resolved: !1,
		increment() {
			++n === 1 && qe(t, !0);
		},
		decrement() {
			--n === 0 && qe(t, !1);
		}
	}, a = ct(Et())?.register(i.inFallback), o = r(), s, c;
	return q(() => s?.()), ot(Tt(), i, () => {
		let t = U(() => e.children);
		return U((n) => {
			let r = a ? a() : {
				showContent: !0,
				showFallback: !0
			};
			if (!i.inFallback() && r.showContent) return i.resolved = !0, s?.(), s = void 0, t();
			if (r.showFallback) return s ? c : nt((t) => (s = t, c = e.fallback), o);
		});
	});
}
function K(e) {
	return x(R ? () => R.untrack(e) : e);
}
var q = g, At = /*#__PURE__*/ new Set("className value readOnly noValidate formNoValidate isMap noModule playsInline adAuctionHeaders allowFullscreen browsingTopics defaultChecked defaultMuted defaultSelected disablePictureInPicture disableRemotePlayback preservesPitch shadowRootClonable shadowRootCustomElementRegistry shadowRootDelegatesFocus shadowRootSerializable sharedStorageWritable allowfullscreen async alpha autofocus autoplay checked controls default disabled formnovalidate hidden indeterminate inert ismap loop multiple muted nomodule novalidate open playsinline readonly required reversed seamless selected adauctionheaders browsingtopics credentialless defaultchecked defaultmuted defaultselected defer disablepictureinpicture disableremoteplayback preservespitch shadowrootclonable shadowrootcustomelementregistry shadowrootdelegatesfocus shadowrootserializable sharedstoragewritable".split(" ")), jt = /*#__PURE__*/ new Set("innerHTML textContent innerText children".split(" ")), Mt = /*#__PURE__*/ Object.assign(Object.create(null), {
	className: "class",
	htmlFor: "for"
}), Nt = /*#__PURE__*/ Object.assign(Object.create(null), {
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
function Pt(e, t) {
	let n = Nt[e];
	return typeof n == "object" ? n[t] ? n.$ : void 0 : n;
}
var Ft = /*#__PURE__*/ new Set("beforeinput click dblclick contextmenu focusin focusout input keydown keyup mousedown mousemove mouseout mouseover mouseup pointerdown pointermove pointerout pointerover pointerup touchend touchmove touchstart".split(" ")), It = /*#__PURE__*/ new Set("altGlyph altGlyphDef altGlyphItem animate animateColor animateMotion animateTransform circle clipPath color-profile cursor defs desc ellipse feBlend feColorMatrix feComponentTransfer feComposite feConvolveMatrix feDiffuseLighting feDisplacementMap feDistantLight feDropShadow feFlood feFuncA feFuncB feFuncG feFuncR feGaussianBlur feImage feMerge feMergeNode feMorphology feOffset fePointLight feSpecularLighting feSpotLight feTile feTurbulence filter font font-face font-face-format font-face-name font-face-src font-face-uri foreignObject g glyph glyphRef hkern image line linearGradient marker mask metadata missing-glyph mpath path pattern polygon polyline radialGradient rect set stop svg switch symbol text textPath tref tspan use view vkern".split(" ")), Lt = {
	xlink: "http://www.w3.org/1999/xlink",
	xml: "http://www.w3.org/XML/1998/namespace"
}, Rt = /*#__PURE__*/ new Set("html base head link meta style title body address article aside footer header main nav section blockquote dd div dl dt figcaption figure hr li ol p pre ul a abbr b bdi bdo br cite code data dfn em i kbd mark q rp rt ruby s samp small span strong sub sup time u var wbr area audio img map track video embed iframe object param picture portal source svg math canvas noscript script del ins caption col colgroup table tbody td tfoot th thead tr button datalist fieldset form input label legend meter optgroup option output progress select textarea details dialog menu summary slot template acronym applet basefont bgsound big blink center content dir font frame frameset hgroup image keygen marquee menuitem nobr noembed noframes plaintext rb rtc shadow spacer strike tt xmp h1 h2 h3 h4 h5 h6 webview isindex listing multicol nextid noindex search".split(" ")), zt = (e) => U(() => e());
function Bt(e, t, n) {
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
var J = "_$DX_DELEGATE";
function Vt(e, t, n, r = {}) {
	let i;
	return nt((r) => {
		i = r, t === document ? e() : nn(t, e(), t.firstChild ? null : void 0, n);
	}, r.owner), () => {
		i(), t.textContent = "";
	};
}
function Ht(e, t, n, r) {
	let i, a = () => {
		let t = r ? document.createElementNS("http://www.w3.org/1998/Math/MathML", "template") : document.createElement("template");
		return t.innerHTML = e, n ? t.content.firstChild.firstChild : r ? t.firstChild : t.content.firstChild;
	}, o = t ? () => K(() => document.importNode(i ||= a(), !0)) : () => (i ||= a()).cloneNode(!0);
	return o.cloneNode = o, o;
}
function Ut(e, t = window.document) {
	let n = t[J] || (t[J] = /* @__PURE__ */ new Set());
	for (let r = 0, i = e.length; r < i; r++) {
		let i = e[r];
		n.has(i) || (n.add(i), t.addEventListener(i, pn));
	}
}
function Wt(e = window.document) {
	if (e[J]) {
		for (let t of e[J].keys()) e.removeEventListener(t, pn);
		delete e[J];
	}
}
function Gt(e, t, n) {
	X(e) || (e[t] = n);
}
function Y(e, t, n) {
	X(e) || (n == null ? e.removeAttribute(t) : e.setAttribute(t, n));
}
function Kt(e, t, n, r) {
	X(e) || (r == null ? e.removeAttributeNS(t, n) : e.setAttributeNS(t, n, r));
}
function qt(e, t, n) {
	X(e) || (n ? e.setAttribute(t, "") : e.removeAttribute(t));
}
function Jt(e, t) {
	X(e) || (t == null ? e.removeAttribute("class") : e.className = t);
}
function Yt(e, t, n, r) {
	if (r) Array.isArray(n) ? (e[`$$${t}`] = n[0], e[`$$${t}Data`] = n[1]) : e[`$$${t}`] = n;
	else if (Array.isArray(n)) {
		let r = n[0];
		e.addEventListener(t, n[0] = (t) => r.call(e, n[1], t));
	} else e.addEventListener(t, n, typeof n != "function" && n);
}
function Xt(e, t, n = {}) {
	let r = Object.keys(t || {}), i = Object.keys(n), a, o;
	for (a = 0, o = i.length; a < o; a++) {
		let r = i[a];
		!r || r === "undefined" || t[r] || (dn(e, r, !1), delete n[r]);
	}
	for (a = 0, o = r.length; a < o; a++) {
		let i = r[a], o = !!t[i];
		!i || i === "undefined" || n[i] === o || !o || (dn(e, i, !0), n[i] = o);
	}
	return n;
}
function Zt(e, t, n) {
	if (!t) return n ? Y(e, "style") : t;
	let r = e.style;
	if (typeof t == "string") return r.cssText = t;
	typeof n == "string" && (r.cssText = n = void 0), n ||= {};
	let i, a;
	for (a in n) t[a] ?? r.removeProperty(a), delete n[a];
	for (a in t) i = t[a], i !== n[a] && (r.setProperty(a, i), n[a] = i);
	return n;
}
function Qt(e, t, n) {
	n == null ? e.style.removeProperty(t) : e.style.setProperty(t, n);
}
function $t(e, t = {}, n, r) {
	let i = {};
	return r || W(() => i.children = Z(e, t.children, i.children)), W(() => typeof t.ref == "function" && tn(t.ref, e)), W(() => rn(e, t, n, !0, i, !0)), i;
}
function en(e, t) {
	let n = e[t];
	return Object.defineProperty(e, t, {
		get() {
			return n();
		},
		enumerable: !0
	}), e;
}
function tn(e, t, n) {
	return K(() => e(t, n));
}
function nn(e, t, n, r) {
	if (n !== void 0 && !r && (r = []), typeof t != "function") return Z(e, t, r, n);
	W((r) => Z(e, t(), r, n), r);
}
function rn(e, t, n, r, i = {}, a = !1) {
	t ||= {};
	for (let r in i) if (!(r in t)) {
		if (r === "children") continue;
		i[r] = fn(e, r, null, i[r], n, a, t);
	}
	for (let o in t) {
		if (o === "children") {
			r || Z(e, t.children);
			continue;
		}
		let s = t[o];
		i[o] = fn(e, o, s, i[o], n, a, t);
	}
}
function an(e, t, n = {}) {
	if (globalThis._$HY.done) return Vt(e, t, [...t.childNodes], n);
	F.completed = globalThis._$HY.completed, F.events = globalThis._$HY.events, F.load = (e) => globalThis._$HY.r[e], F.has = (e) => e in globalThis._$HY.r, F.gather = (e) => gn(t, e), F.registry = /* @__PURE__ */ new Map(), F.context = {
		id: n.renderId || "",
		count: 0
	};
	try {
		return gn(t, n.renderId), Vt(e, t, [...t.childNodes], n);
	} finally {
		F.context = null;
	}
}
function on(e) {
	let t, n;
	return !X() || !(t = F.registry.get(n = _n())) ? e() : (F.completed && F.completed.add(t), F.registry.delete(n), t);
}
function sn(e, t) {
	for (; e && e.localName !== t;) e = e.nextSibling;
	return e;
}
function cn(e) {
	let t = e, n = 0, r = [];
	if (X(e)) for (; t;) {
		if (t.nodeType === 8) {
			let e = t.nodeValue;
			if (e === "$") n++;
			else if (e === "/") {
				if (n === 0) return [t, r];
				n--;
			}
		}
		r.push(t), t = t.nextSibling;
	}
	return [t, r];
}
function ln() {
	F.events && !F.events.queued && (queueMicrotask(() => {
		let { completed: e, events: t } = F;
		if (t) {
			for (t.queued = !1; t.length;) {
				let [n, r] = t[0];
				if (!e.has(n)) return;
				t.shift(), pn(r);
			}
			F.done && (F.events = _$HY.events = null, F.completed = _$HY.completed = null);
		}
	}), F.events.queued = !0);
}
function X(e) {
	return !!F.context && !F.done && (!e || e.isConnected);
}
function un(e) {
	return e.toLowerCase().replace(/-([a-z])/g, (e, t) => t.toUpperCase());
}
function dn(e, t, n) {
	let r = t.trim().split(/\s+/);
	for (let t = 0, i = r.length; t < i; t++) e.classList.toggle(r[t], n);
}
function fn(e, t, n, r, i, a, o) {
	let s, c, l, u, d;
	if (t === "style") return Zt(e, n, r);
	if (t === "classList") return Xt(e, n, r);
	if (n === r) return r;
	if (t === "ref") a || n(e);
	else if (t.slice(0, 3) === "on:") {
		let i = t.slice(3);
		r && e.removeEventListener(i, r, typeof r != "function" && r), n && e.addEventListener(i, n, typeof n != "function" && n);
	} else if (t.slice(0, 10) === "oncapture:") {
		let i = t.slice(10);
		r && e.removeEventListener(i, r, !0), n && e.addEventListener(i, n, !0);
	} else if (t.slice(0, 2) === "on") {
		let i = t.slice(2).toLowerCase(), a = Ft.has(i);
		if (!a && r) {
			let t = Array.isArray(r) ? r[0] : r;
			e.removeEventListener(i, t);
		}
		(a || n) && (Yt(e, i, n, a), a && Ut([i]));
	} else if (t.slice(0, 5) === "attr:") Y(e, t.slice(5), n);
	else if (t.slice(0, 5) === "bool:") qt(e, t.slice(5), n);
	else if ((d = t.slice(0, 5) === "prop:") || (l = jt.has(t)) || !i && ((u = Pt(t, e.tagName)) || (c = At.has(t))) || (s = e.nodeName.includes("-") || "is" in o)) {
		if (d) t = t.slice(5), c = !0;
		else if (X(e)) return n;
		t === "class" || t === "className" ? Jt(e, n) : s && !c && !l ? e[un(t)] = n : e[u || t] = n;
	} else {
		let r = i && t.indexOf(":") > -1 && Lt[t.split(":")[0]];
		r ? Kt(e, r, t, n) : Y(e, Mt[t] || t, n);
	}
	return n;
}
function pn(e) {
	if (F.registry && F.events && F.events.find(([t, n]) => n === e)) return;
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
	}), F.registry && !F.done && (F.done = _$HY.done = !0), e.composedPath) {
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
function Z(e, t, n, r, i) {
	let a = X(e);
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
			i && i.nodeType === 3 ? i.data !== t && (i.data = t) : i = document.createTextNode(t), n = Q(e, n, r, i);
		} else n = n !== "" && typeof n == "string" ? e.firstChild.data = t : e.textContent = t;
	} else if (t == null || o === "boolean") {
		if (a) return n;
		n = Q(e, n, r);
	} else if (o === "function") return W(() => {
		let i = t();
		for (; typeof i == "function";) i = i();
		n = Z(e, i, n, r);
	}), () => n;
	else if (Array.isArray(t)) {
		let o = [], c = n && Array.isArray(n);
		if (mn(o, t, n, i)) return W(() => n = Z(e, o, n, r, !0)), () => n;
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
			if (n = Q(e, n, r), s) return n;
		} else c ? n.length === 0 ? hn(e, o, r) : Bt(e, n, o) : (n && Q(e), hn(e, o));
		n = o;
	} else if (t.nodeType) {
		if (a && t.parentNode) return n = s ? [t] : t;
		if (Array.isArray(n)) {
			if (s) return n = Q(e, n, r, t);
			Q(e, n, null, t);
		} else n == null || n === "" || !e.firstChild ? e.appendChild(t) : e.replaceChild(t, e.firstChild);
		n = t;
	}
	return n;
}
function mn(e, t, n, r) {
	let i = !1;
	for (let a = 0, o = t.length; a < o; a++) {
		let o = t[a], s = n && n[e.length], c;
		if (o != null && o !== !0 && o !== !1) {
			if ((c = typeof o) == "object" && o.nodeType) e.push(o);
			else if (Array.isArray(o)) i = mn(e, o, s) || i;
			else if (c === "function") {
				if (r) {
					for (; typeof o == "function";) o = o();
					i = mn(e, Array.isArray(o) ? o : [o], Array.isArray(s) ? s : [s]) || i;
				} else e.push(o), i = !0;
			} else {
				let t = String(o);
				s && s.nodeType === 3 && s.data === t ? e.push(s) : e.push(document.createTextNode(t));
			}
		}
	}
	return i;
}
function hn(e, t, n) {
	for (let r = 0, i = t.length; r < i; r++) e.insertBefore(t[r], n);
}
function Q(e, t, n, r) {
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
function gn(e, t) {
	let n = e.querySelectorAll("*[data-hk]");
	for (let e = 0; e < n.length; e++) {
		let r = n[e], i = r.getAttribute("data-hk");
		(!t || i.startsWith(t)) && !F.registry.has(i) && F.registry.set(i, r);
	}
}
function _n() {
	return F.getNextContextId();
}
function vn(e) {
	return F.context ? void 0 : e.children;
}
function yn(e) {
	return e.children;
}
var $ = () => void 0, bn = Symbol();
function xn(e, t) {
	!F.context && (e.innerHTML = t);
}
function Sn(e) {
	let t = /* @__PURE__ */ Error(`${e.name} is not supported in the browser, returning undefined`);
	console.error(t);
}
function Cn(e, t) {
	Sn(Cn);
}
function wn(e, t) {
	Sn(wn);
}
function Tn(e, t) {
	Sn(Tn);
}
function En(e, ...t) {}
function Dn(e, t, n, r) {}
function On(e) {}
function kn(e) {}
function An(e, t) {}
function jn() {}
function Mn(e) {}
function Nn(e) {}
function Pn(e, t, n) {}
var Fn = !1, In = !1, Ln = "http://www.w3.org/2000/svg";
function Rn(e, t, n) {
	return t ? document.createElementNS(Ln, e) : document.createElement(e, { is: n });
}
var zn = (...e) => (et(), an(...e));
function Bn(e) {
	let { useShadow: t } = e, n = document.createTextNode(""), r = () => e.mount || document.body, i = rt(), a, o = !!F.context;
	return tt(() => {
		o && (rt().user = o = !1), a ||= it(i, () => U(() => e.children));
		let s = r();
		if (s instanceof HTMLHeadElement) {
			let [e, t] = H(!1);
			nt((t) => nn(s, () => e() ? t() : a(), null)), q(() => t(!0));
		} else {
			let r = Rn(e.isSVG ? "g" : "div", e.isSVG), i = t && r.attachShadow ? r.attachShadow({ mode: "open" }) : r;
			Object.defineProperty(r, "_$host", {
				get() {
					return n.parentNode;
				},
				configurable: !0
			}), nn(i, a), s.appendChild(r), e.ref && e.ref(r), q(() => s.removeChild(r));
		}
	}, void 0, { render: !o }), n;
}
function Vn(e, t) {
	let n = U(e);
	return U(() => {
		let e = n();
		switch (typeof e) {
			case "function": return K(() => e(t));
			case "string":
				let n = It.has(e), r = F.context ? on() : Rn(e, n, K(() => t.is));
				return $t(r, t, n), r;
		}
	});
}
function Hn(e) {
	let [, t] = gt(e, ["component"]);
	return Vn(() => e.component, t);
}
//#endregion
export { Mt as Aliases, $ as Assets, $ as HydrationScript, $ as generateHydrationScript, $ as getAssets, $ as getRequestEvent, $ as useAssets, jt as ChildProperties, Rt as DOMElements, Ft as DelegatedEvents, Hn as Dynamic, wt as ErrorBoundary, yt as For, yn as Hydration, bt as Index, St as Match, vn as NoHydration, Bn as Portal, At as Properties, bn as RequestContext, It as SVGElements, Lt as SVGNamespace, xt as Show, kt as Suspense, Ot as SuspenseList, Ct as Switch, Yt as addEventListener, rn as assign, Xt as classList, Jt as className, Wt as clearDelegatedEvents, ut as createComponent, Vn as createDynamic, Ut as delegateEvents, en as dynamicProperty, W as effect, Nn as escape, _n as getHydrationKey, on as getNextElement, cn as getNextMarker, sn as getNextMatch, rt as getOwner, Pt as getPropAlias, zn as hydrate, xn as innerHTML, nn as insert, In as isDev, Fn as isServer, zt as memo, ht as mergeProps, Vt as render, Tn as renderToStream, Cn as renderToString, wn as renderToStringAsync, Mn as resolveSSRNode, ln as runHydrationEvents, Y as setAttribute, Kt as setAttributeNS, qt as setBoolAttribute, Gt as setProperty, Qt as setStyleProperty, $t as spread, En as ssr, An as ssrAttribute, On as ssrClassList, Dn as ssrElement, jn as ssrHydrationKey, Pn as ssrSpread, kn as ssrStyle, Zt as style, Ht as template, K as untrack, tn as use };
