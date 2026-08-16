//#region packages/solidlil/reactive.generated.js
var e = "Potential Infinite Loop Detected.";
function t(e, t, n, r) {
	e[0] = t, e[1] = n, e[2] = r, e[3] = [], e[4] = [], e[5] = [], e[6] = null, e[7] = !1;
}
function n(e, t, n) {
	e[0] = t, e[1] = [], e[2] = n, e[3] = 0, e[4] = -1;
}
function r(e, t) {
	for (var n = 0; n < e[1].length; n++) if ((e[1][n] | 0) == t) return !1;
	return e[1].push(t), !0;
}
function i(e, t) {
	for (var n = 0; n < e[1].length; n++) if ((e[1][n] | 0) == t) {
		e[1][n] = e[1][e[1].length - 1] | 0, e[1].pop();
		return;
	}
}
function a(e) {
	var t;
	if (M < 0 && e[4] < 0) return e[0];
	if (M >= 0 && e[4] >= 0 && e[4] == N) throw "Circular memo dependency detected.";
	if (e[4] >= 0 && e[4] != N ? (t = N >= 0 && !A[N][9] && !A[N][10], t = !t) : t = !1, t && _e(e[4]), M >= 0) {
		t = M;
		var n = e[3] + 1 | 0;
		n > A[t][12] && (A[t][12] = n), e[4] >= 0 && l(A[t], e[4]), r(e, t) && A[t][4].push(e);
	}
	return e[0];
}
function o(e, t) {
	return N >= 0 && A[N][9] && (e[3] = A[N][12], e[4] = N), e[2](e[0], t) ? t : (e[0] = t, be(e[1]), t);
}
function s(e, t) {
	return o(e, t(e[0]));
}
function c(e, t, n, r, i, a) {
	e[0] = t, e[1] = n, e[2] = r, e[3] = i, e[4] = [], e[5] = [], e[6] = [], e[7] = [], e[8] = null, e[9] = a, e[10] = !1, e[11] = !1, e[12] = 0, e[13] = [];
}
function l(e, t) {
	for (var n = 0; n < e[13].length; n++) if ((e[13][n] | 0) == t) return;
	e[13].push(t);
}
function u(e) {
	for (; e[6].length > 0;) {
		var t = e[6][e[6].length - 1] | 0;
		e[6].pop(), Ee(t);
	}
	for (; e[5].length > 0;) t = e[5][e[5].length - 1] | 0, e[5].pop(), Te(t);
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
		var t = M, n = N, r = P;
		M = e[0], N = e[0], P = -1, e[12] = 0, e[13] = [], I = I + 1 | 0, e[3](), e[10] = !0, I = I - 1 | 0, M = t, N = n, P = r, w();
	}
}
function p(e, t, n) {
	e[0] = t, e[1] = n, e[2] = [], e[3] = [], Me(() => {
		for (var t = a(e[0]), n = e[1], r = 0, i; r < e[2].length; r++) i = e[3][r], o(i, n(e[2][r], t));
	});
}
function m(e, t) {
	for (var r = e[1], i = 0; i < e[2].length; i++) if (r(e[2][i], t)) return a(e[3][i]);
	return r = r(t, e[0][0]), i = [
		null,
		[],
		null,
		0,
		0
	], n(i, r, oe), e[2].push(t), e[3].push(i), a(i);
}
function h(e) {
	e[0] = [], e[1] = [], e[2] = [], e[3] = [];
}
function g(e) {
	for (var t = 0; t < e[2].length; t++) e[2][t][0]();
	e[0] = [], e[1] = [], e[2] = [], e[3] = [];
}
function _(e, t, r) {
	var i = /* @__PURE__ */ new Set(), a = e[0].length;
	t.length < a && (a = t.length);
	for (var s = 0, c, l, u, d, f, p, m, h, g, _; c = s < a && e[0][s] == t[s], c;) s++;
	for (l = e[0].length - 1, u = t.length - 1; a = l >= s && u >= s && e[0][l] == t[u], a;) l--, u = u - 1 | 0;
	for (d = [], f = [], p = [], m = [], c = 0; c < t.length; c++) {
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
		], n(g, c, ae), _ = [() => {}], f.push(E(((e, t, n, r) => (i) => (r[0] = i, e(t, n)))(r, h, g, _), null, !1)), p.push(_), m.push(g)), d.push(h);
	}
	for (t = 0; t < e[0].length; t++) !i.has(t) && e[2][t][0]();
	return e[0] = d, e[1] = f, e[2] = p, e[3] = m, e[1];
}
function v(e) {
	e[0] = [], e[1] = [], e[2] = [];
}
function y(e) {
	for (var t = 0; t < e[1].length; t++) e[1][t][0]();
	e[0] = [], e[1] = [], e[2] = [];
}
function b(e, t, r) {
	var i = e[2].length;
	t.length < i && (i = t.length);
	for (var a = 0, s, c; a < i; a++) e[2][a][0] != t[a] && o(e[2][a], t[a]);
	for (var l = e[2].length; l < t.length; l = l + 1 | 0) a = t[l], s = [
		null,
		[],
		null,
		0,
		0
	], n(s, a, (e, t) => e == t), c = [() => {}], a = E(((e, t, n) => (r) => (n[0] = r, e(t, l)))(r, s, c), null, !1), e[2].push(s), e[1].push(c), e[0].push(a);
	for (; e[2].length > t.length;) e[1][e[2].length - 1][0](), e[1].pop(), e[2].pop(), e[0].pop();
	return e[0].slice();
}
function ee(e) {
	e[0] = [], e[1] = () => {};
}
function x(e, t) {
	return e[0].length == 0 && (e[0] = [E((n) => (e[1] = n, t()), null, !1)]), e[0];
}
function S(e) {
	e[0].length > 0 && e[1](), e[0] = [];
}
function te(e, t, r, i) {
	e[0] = t;
	let a = t[0], o = [
		null,
		[],
		null,
		0,
		0
	];
	n(o, a, r), e[1] = o, e[2] = t[0], e[3] = i, e[4] = null;
}
function ne(e) {
	e[2] = a(e[0]), !e[4] && (e[4] = xe((t) => {
		re(e);
	}, e[3]));
}
function re(e) {
	e[4] = null, o(e[1], e[2]);
}
function ie(e) {
	var t = e[4];
	t && (Ce(t), e[4] = null);
}
var ae = (e, t) => e == t, oe = (e, t) => e == t, se = (e) => {
	for (var t = 0; t < z.length; t++) if ((z[t] | 0) == e) return !0;
	return !1;
}, ce = (e) => {
	for (var t = z[e] | 0; e < z.length - 1; e = e + 1 | 0) z[e] = z[e + 1 | 0] | 0;
	return z.pop(), t;
}, le = (e) => {
	for (var t = 0; t < z.length; t++) if ((z[t] | 0) == e) return ce(t), !0;
	return !1;
}, ue = (e, t) => {
	for (var n = e.length - 1; n >= 0; n--) if ((e[n] | 0) == t) {
		e[n] = e[e.length - 1] | 0, e.pop();
		return;
	}
}, C = (e, t) => {
	if (e == 1) {
		var n = j[t][6];
		return n || (n = [
			e,
			t,
			null
		], j[t][6] = n, n[2] = C(j[t][1], j[t][2]), n);
	}
	return e == 2 ? (n = A[t][8]) ? n : (n = [
		e,
		t,
		null
	], A[t][8] = n, n[2] = C(A[t][1], A[t][2]), n) : null;
}, de = (e) => {
	e[1] == 1 ? ue(j[e[2]][3], e[0]) : e[1] == 2 && ue(A[e[2]][5], e[0]);
}, fe = (e) => {
	e[1] == 1 ? ue(j[e[2]][4], e[0]) : e[1] == 2 && ue(A[e[2]][6], e[0]);
}, pe = (e, t, n = 0) => {
	if (e == t) return !0;
	if (n >= A.length) return !1;
	for (var r = 0; r < A[e][13].length; r++) if (pe(A[e][13][r] | 0, t, n + 1 | 0)) return !0;
	return !1;
}, me = (e) => {
	if (N >= 0 && A[N][9] && !R && pe(N, e) && !ge(e, N)) throw "Reactive dependency cycle detected.";
	!A[e][11] && !se(e) && z.push(e);
}, he = (e) => {
	for (var t = 0; t < A[e][13].length; t++) if (se(A[e][13][t] | 0)) return !0;
	return !1;
}, ge = (e, t) => {
	var n = A[t][1];
	t = A[t][2];
	for (var r = 0, i; i = n != 0 && r <= (A.length + j.length | 0), i;) {
		if (n == 2) {
			if (t == e) return !0;
			n = A[t][1], t = A[t][2];
		} else n = j[t][1], t = j[t][2];
		r = r + 1 | 0;
	}
	return !1;
}, _e = (e) => {
	var t, n;
	if (!(A[e][11] || e == N)) {
		for (n = 0; n < A[e][13].length; n++) t = A[e][13][n] | 0, (se(t) || he(t)) && _e(t);
		le(e) && f(A[e]);
	}
}, ve = () => {
	for (var e = -1, t = -1, n = -1, r = 0, i, a, o, s, c; r < z.length; r++) i = z[r] | 0, A[i][9] ? (e < 0 || A[i][12] > A[z[e] | 0][12]) && (e = r) : !A[i][10] && t < 0 ? t = r : n < 0 ? n = r : (o = z[n] | 0, a = A[i], s = A[o], c = a[1] == s[1] && a[2] == s[2], (c && a[12] > s[12] || !c && (ge(i, o) || !ge(o, i) && a[12] > s[12])) && (n = r));
	return t >= 0 && (e = t), ce(e < 0 ? n : e);
}, w = () => {
	if (!(L || F > 0 || I > 0)) {
		L = !0;
		var t = 0;
		try {
			for (; z.length > 0;) {
				if (t = t + 1 | 0, t > 1e3) throw e;
				f(A[ve()]);
			}
			L = !1;
		} catch (e) {
			throw z = [], L = !1, e;
		}
	}
}, ye = () => {
	if (!(L || F > 0)) {
		L = !0;
		var t = 0;
		try {
			for (;;) {
				for (var n = -1, r = 0; r < z.length; r = r + 1 | 0) {
					var i = z[r] | 0;
					i != N && A[i][9] && (n < 0 || A[i][12] > A[z[n] | 0][12]) && (n = r);
				}
				if (n < 0) {
					L = !1;
					return;
				}
				if (t = t + 1 | 0, t > 1e3) throw e;
				r = ce(n), f(A[r]);
			}
		} catch (e) {
			throw z = [], L = !1, e;
		}
	}
}, be = (e) => {
	var t = F == 0 && !L;
	t && (F = F + 1 | 0);
	try {
		for (var n = 0; n < e.length; n = n + 1 | 0) me(e[n] | 0);
	} catch (e) {
		throw t && (F = F - 1 | 0), e;
	}
	t && (F = F - 1 | 0, I > 0 ? ye() : w());
}, xe = (e, t = 1073741823) => {
	var n = [
		$e,
		e,
		t,
		!1
	];
	for (n[3] = !1, $e = $e + 1 | 0, B.push(n), e = B.length - 1; t = e > 0 && B[e][2] < B[e - 1][2], t;) {
		t = e - 1;
		var r = B[t];
		B[t] = B[e], B[e] = r, e = t;
	}
	return et || (et = !0, tt(() => {
		et = !1, we();
	})), n;
}, Se = (e) => {
	tt = e;
}, Ce = (e) => {
	e[3] = !0;
}, we = () => {
	for (et = !1; B.length > 0;) {
		for (var e = B[0], t = 0; t < B.length - 1; t++) B[t] = B[t + 1];
		B.pop(), e[3] || (e[3] = !0, e[1](!1));
	}
}, T = (e, t, n) => {
	var r = A.length;
	if (Xe.length > 0 && (r = Xe.pop()), N >= 0) var i = N, a = 2, o;
	else P >= 0 ? (i = P, a = 1) : (a = 0, i = -1);
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
	], c(o, r, a, i, e, t), r == A.length ? A.push(o) : A[r] = o, N >= 0 ? A[N][5].push(r) : P >= 0 && j[P][3].push(r), n && (I > 0 || L) ? me(r) : f(o), r;
}, Te = (e) => {
	var t = A[e];
	if (!t[11]) {
		u(t), d(t), le(e), de(t), t[3] = () => {}, t[13] = [];
		var n = t[8];
		n && (n[0] = 0, t[8] = null), t[11] = !0, Xe.push(e);
	}
}, Ee = (e) => {
	var t = j[e];
	if (!t[7]) {
		for (; t[4].length > 0;) {
			var n = t[4][t[4].length - 1] | 0;
			t[4].pop(), Ee(n);
		}
		for (; t[3].length > 0;) n = t[3][t[3].length - 1] | 0, t[3].pop(), Te(n);
		for (; t[5].length > 0;) n = t[5][t[5].length - 1], t[5].pop(), n[0]();
		fe(t), (n = t[6]) && (n[0] = 0, t[6] = null), t[7] = !0, Ze.push(e);
	}
}, De = (e, t) => {
	let r = [
		null,
		[],
		null,
		0,
		0
	];
	return n(r, e, t), r;
}, Oe = (e) => a(e), ke = (e, t, n = !1) => {
	var r = R;
	r && (n = !0), R = n;
	try {
		return o(e, t);
	} finally {
		R = r;
	}
}, Ae = (e, t, n = !1) => {
	var r = R;
	r && (n = !0), R = n;
	try {
		return s(e, t);
	} finally {
		R = r;
	}
}, je = (e) => T(e, !1, !0), Me = (e) => T(e, !0, !1), Ne = (e) => T(e, !1, !1), Pe = (e) => {
	je(() => {
		let t = M;
		M = -1, e(), M = t;
	});
}, Fe = (e) => {
	let t = [
		0,
		!1,
		null
	];
	return t[0] = -1, t[1] = !1, t[2] = e, (e) => {
		t[0] >= 0 && (Te(t[0]), t[0] = -1), t[1] = !1, t[0] = T(() => {
			if (e(), t[1]) {
				var n = t[2], r = M;
				M = -1, n(), M = r, Te(t[0]), t[0] = -1;
			} else t[1] = !0;
		}, !1, !1);
	};
}, Ie = (e, t, r) => {
	let i = [
		null,
		[],
		null,
		0,
		0
	];
	return n(i, e, r), T(() => {
		o(i, t(i[0]));
	}, !0, !1), i;
}, Le = (e, t, n) => {
	let r = [
		null,
		null,
		null,
		0,
		null
	];
	return te(r, e, t, n), Me(() => {
		ne(r);
	}), O(() => {
		ie(r);
	}), r[1];
}, Re = (e, t) => {
	let n = [
		[],
		[],
		[],
		[]
	];
	return h(n), O(() => {
		g(n);
	}), () => {
		let r = a(e);
		return k(() => _(n, r, t));
	};
}, ze = (e, t, n) => {
	let r = [
		[],
		[],
		[],
		[]
	];
	h(r);
	let i = [[], null];
	return ee(i), O(() => {
		g(r), S(i);
	}), () => {
		let o = a(e);
		return k(() => o.length == 0 ? (g(r), x(i, n)) : (S(i), _(r, o, t)));
	};
}, Be = (e, t) => {
	let n = [
		[],
		[],
		[]
	];
	return v(n), O(() => {
		y(n);
	}), () => {
		let r = a(e);
		return k(() => b(n, r, t));
	};
}, Ve = (e, t, n) => {
	let r = [
		[],
		[],
		[]
	];
	v(r);
	let i = [[], null];
	return ee(i), O(() => {
		y(r), S(i);
	}), () => {
		let o = a(e);
		return k(() => o.length == 0 ? (y(r), x(i, n)) : (S(i), b(r, o, t)));
	};
}, E = (e, n = null, r = !1) => {
	var i = j.length;
	Ze.length > 0 && (i = Ze.pop());
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
	t(s, o, a, n), i == j.length ? j.push(s) : j[i] = s, r && (a == 1 ? j[n][4].push(i) : a == 2 && A[n][6].push(i));
	var c = M, l = P, u = N;
	M = -1, P = i, N = -1, I = I + 1 | 0, n = () => {
		j[i] == s && Ee(i);
	};
	var d = !1;
	try {
		var f = e(n);
		return d = !0, f;
	} finally {
		I = I - 1 | 0, M = c, N = u, P = l, d && w();
	}
}, He = (e) => E(e, D(), !1), Ue = () => {
	let e = Qe;
	return Qe = Qe + 1 | 0, "cl-" + e;
}, D = () => N >= 0 ? C(2, N) : P >= 0 ? C(1, P) : null, We = () => M >= 0 ? C(2, M) : null, Ge = (e, t) => {
	var n = M, r = N, i = P;
	M = -1, e && e[0] != 0 ? e[0] == 2 ? (N = e[1], P = -1) : (N = -1, P = e[1]) : (N = -1, P = -1), I = I + 1 | 0;
	var a = !1;
	try {
		var o = t();
		return a = !0, o;
	} finally {
		I = I - 1 | 0, M = n, N = r, P = i, a && w();
	}
}, Ke = (e) => e[2], qe = (e, t) => {
	let n = [
		null,
		null,
		[],
		[]
	];
	return p(n, e, t), n;
}, Je = (e, t) => m(e, t), O = (e) => {
	var t = [e];
	return N >= 0 ? A[N][7].push(t) : P >= 0 && j[P][5].push(t), e;
}, k = (e) => {
	var t = M;
	M = -1;
	try {
		return e();
	} finally {
		M = t;
	}
}, Ye = (e) => {
	F = F + 1 | 0;
	var t = !1;
	try {
		var n = e();
		return t = !0, n;
	} finally {
		F = F - 1 | 0, t && F == 0 && (I > 0 ? ye() : w());
	}
}, A = [], j = [], Xe = [], Ze = [], M = -1, N = -1, P = -1, F = 0, I = 0, L = !1, R = !1, z = [], Qe = 0, $e = 1, B = [], et = !1, tt = (e) => {}, V = (e, t) => e === t, H = Symbol("solid-proxy"), nt = Symbol("solid-track"), rt = Symbol("solid-dev-component"), it = void 0, U = {
	context: void 0,
	registry: void 0,
	effects: void 0,
	done: !1,
	getContextId() {
		return at(this.context.count);
	},
	getNextContextId() {
		return at(this.context.count++);
	}
};
function at(e) {
	let t = String(e), n = t.length - 1;
	return `${U.context.id}${n ? String.fromCharCode(96 + n) : ""}${t}`;
}
var W = Symbol("solidlil-signal"), ot = !1, st = !1, ct, G, lt, ut, K, dt, ft, pt, mt = /* @__PURE__ */ new WeakMap();
function ht() {
	st || (st = !0, Se(queueMicrotask));
}
function gt(e) {
	return e?.equals === !1 ? () => !1 : e?.equals ?? V;
}
function q(e, t) {
	let n = () => {
		if (G?.[2].has(e)) return G[2].get(e);
		if (G && t) {
			if (G[1].has(e)) return G[1].get(e);
			let n = t();
			return G[1].set(e, n), n;
		}
		return Oe(e);
	};
	return n[W] = e, n;
}
function _t(e, t) {
	if (G) {
		let n = G[2].has(e) ? G[2].get(e) : Oe(e), r = typeof t == "function" ? t(n) : t;
		return G[2].set(e, r), G[1].clear(), r;
	}
	return typeof t == "function" ? Ae(e, t) : ke(e, t);
}
function vt(e, t) {
	return ke(e[W], t, !0);
}
function yt(e, t) {
	return Ae(e[W], t, !0);
}
function bt(e) {
	return e instanceof Error ? e : Error(typeof e == "string" ? e : "Unknown error", { cause: e });
}
function xt(e) {
	return e && e.owner === void 0 && (e.owner = xt(Ke(e))), e;
}
function J(e, t = D()) {
	let n = bt(e), r = t;
	for (; r;) {
		let e = mt.get(r);
		if (e?.length) {
			for (let t of e) try {
				t(n);
			} catch (e) {
				return J(e, Ke(r));
			}
			return;
		}
		r = Ke(r);
	}
	throw n;
}
function St(e) {
	let t = Ct(e);
	return (...e) => {
		try {
			return t(...e);
		} catch (e) {
			return J(e);
		}
	};
}
function Ct(e) {
	if (!K) return e;
	let t = De(0, () => !1), n = K.factory(e, () => Ae(t, (e) => e + 1));
	return O(() => n.dispose()), (...e) => (Oe(t), n.track(...e));
}
function wt(e, t = V) {
	let n = e[W];
	return n === void 0 && (n = X(e, void 0, { equals: t })[W]), n;
}
function Tt(e) {
	for (; typeof e == "function" && !e.length;) e = e();
	if (!Array.isArray(e)) return e;
	let t = [];
	for (let n of e) {
		let e = Tt(n);
		Array.isArray(e) ? t.push(...e) : t.push(e);
	}
	return t;
}
function Et() {
	ot = !0;
}
function Dt(e = Tn) {
	ct = e;
}
function Ot(e, t = (e) => e()) {
	if (K) {
		let n = K;
		K = {
			factory(t, r) {
				let i = n.factory(t, r), a = e((e) => i.track(e), r);
				return {
					track: (e) => a.track(e),
					dispose() {
						a.dispose(), i.dispose();
					}
				};
			},
			untrack: (e) => n.untrack(() => t(e))
		};
	} else K = {
		factory: e,
		untrack: t
	};
}
function Y(e, t) {
	let n = De(e, gt(t));
	return [q(n), (e) => _t(n, e)];
}
function X(e, t, n) {
	let r = St(e), i = gt(n), a = !1, o = Ie(t, r, (e, t) => a ? i(e, t) : (a = !0, !1));
	return q(o, () => r(Oe(o)));
}
function kt(e, t, n) {
	let r = t, i = St(e);
	je(() => {
		r = i(r);
	});
}
function At(e, t, n) {
	let r = t, i = St(e);
	Me(() => {
		r = i(r);
	});
}
function jt(e, t, n) {
	let r = t, i = St(e);
	Ne(() => {
		r = i(r);
	});
}
function Mt(e, t) {
	let n = !e.length, r = (t) => {
		try {
			return n ? e() : e(() => Q(t));
		} catch (e) {
			return J(e);
		}
	};
	return n ? E(r, null) : t === void 0 ? He(r) : E(r, t);
}
function Nt() {
	return xt(D());
}
function Pt() {
	return xt(We());
}
function Ft(e, t) {
	return Ge(e, () => {
		try {
			return t();
		} catch (e) {
			return J(e);
		}
	});
}
function It(e, t = V) {
	let n = qe(wt(e, t), t);
	return (e) => Je(n, e);
}
function Lt(e, t) {
	return Fe(e);
}
function Rt(e, t, n) {
	let r = Array.isArray(e) ? e : null, i, a = n && n.defer;
	return (n) => {
		let o = r ? r.map((e) => e()) : e();
		if (a) return a = !1, n;
		let s = Q(() => t(o, i, n));
		return i = o, s;
	};
}
function zt(e) {
	let t = D();
	Pe(() => {
		try {
			e();
		} catch (e) {
			J(e, t);
		}
	});
}
function Bt(e) {
	let t = D();
	if (!t) return;
	let n = mt.get(t);
	n ? n.push(e) : mt.set(t, [e]);
}
function Vt(e, t) {
	return E((n) => {
		let r = D();
		mt.set(r, [t]);
		try {
			return e();
		} catch (e) {
			return J(e, r);
		}
	}, D(), !0);
}
function Ht(e, t) {
	return ht(), q(Le(wt(e, gt(t)), gt(t), t?.timeoutMs ?? 1073741823));
}
function Ut(e, t, n) {
	let r = typeof t == "function", i = !r || e, a = r ? t : e, o = (r ? n : t) ?? {}, s = typeof i == "function" ? X(i) : null, [c, l] = (o.storage ?? Y)(o.initialValue), [u, d] = Y(void 0), [f, p] = Y(void 0, { equals: !1 }), [m, h] = Y("initialValue" in o ? "ready" : "unresolved"), g = /* @__PURE__ */ new Set(), _ = "initialValue" in o, v = null, y = 0, b = !1, ee = D();
	function x(e, t, n, r) {
		if (e !== y) return t;
		v = null, r !== void 0 && (_ = !0), Z(() => {
			n === void 0 && l(() => t), h(n === void 0 ? _ ? "ready" : "unresolved" : "errored"), d(n);
		});
		for (let e of g) e.decrement();
		return g.clear(), t;
	}
	function S(e = !0) {
		if (e !== !1 && b) return v;
		b = !1;
		let t = s ? s() : i, n = ++y;
		if (t == null || t === !1) return v = null, x(n, Q(c), void 0, void 0);
		let r;
		try {
			r = Q(() => a(t, {
				value: c(),
				refetching: e
			}));
		} catch (e) {
			return x(n, void 0, bt(e), t);
		}
		if (!r || typeof r.then != "function") return x(n, r, void 0, t);
		if (v = r, "v" in r && "s" in r) return r.s === 1 ? x(n, r.v, void 0, t) : x(n, void 0, bt(r.v), t);
		b = !0, queueMicrotask(() => {
			b = !1;
		}), Z(() => {
			h(_ ? "refreshing" : "pending"), d(void 0), p();
		});
		let l = r.then((e) => (o.onHydrated && !_ && queueMicrotask(() => o.onHydrated(t, { value: e })), x(n, e, void 0, t)), (e) => x(n, void 0, bt(e), t));
		v = l;
		let u = G ?? lt;
		if (u) {
			u[3].add(r);
			let e = () => {
				u[3].delete(r), yn(u);
			};
			r.then(e, e);
		}
		return l;
	}
	function te() {
		let e = c(), t = u();
		if (t !== void 0 && !v) throw t;
		if (We() && dt) {
			let e = Kt(dt);
			e && At(() => {
				f(), v && !g.has(e) && (e.increment(), g.add(e));
			});
		}
		return e;
	}
	return Object.defineProperties(te, {
		state: { get: () => m() },
		error: { get: () => u() },
		loading: { get() {
			let e = m();
			return e === "pending" || e === "refreshing";
		} },
		latest: { get() {
			if (!_) return te();
			let e = u();
			if (e !== void 0 && !v) throw e;
			return c();
		} }
	}), s ? At(() => {
		ee = D(), S(!1);
	}) : S(!1), [te, {
		refetch: (e) => Ft(ee, () => S(e)),
		mutate: l
	}];
}
function Wt(e, t, n) {
	return E((r) => {
		let i = xt(D());
		return i.context = { [e.id]: t }, n();
	}, D(), !0);
}
function Gt(e, t) {
	let n = {
		id: Symbol("context"),
		defaultValue: e,
		Provider(e) {
			return t?.name, Wt(n, e.value, () => qt(() => e.children));
		}
	};
	return n;
}
function Kt(e) {
	let t = xt(D());
	for (; t;) {
		let n = t.context?.[e.id];
		if (n !== void 0) return n;
		t = t.owner;
	}
	return e.defaultValue;
}
function qt(e) {
	let t = X(e), n = X(() => Tt(t()));
	return n.toArray = () => {
		let e = n();
		return Array.isArray(e) ? e : e == null ? [] : [e];
	}, n;
}
function Jt(e, t) {
	if (ot && U.context) {
		let n = U.context;
		U.context = {
			...n,
			id: U.getNextContextId(),
			count: 0
		};
		let r = Q(() => e(t || {}));
		return U.context = n, r;
	}
	return Q(() => e(t || {}));
}
var Yt = typeof Proxy == "function", Xt = () => !0, Zt = {
	get(e, t, n) {
		return t === H ? n : e.get(t);
	},
	has(e, t) {
		return t === H || e.has(t);
	},
	set: Xt,
	deleteProperty: Xt,
	getOwnPropertyDescriptor(e, t) {
		return {
			configurable: !0,
			enumerable: !0,
			get() {
				return e.get(t);
			},
			set: Xt,
			deleteProperty: Xt
		};
	},
	ownKeys(e) {
		return e.keys();
	}
};
function Qt(e) {
	return (e = typeof e == "function" ? e() : e) ? e : {};
}
function $t() {
	for (let e = 0; e < this.length; e += 1) {
		let t = this[e]();
		if (t !== void 0) return t;
	}
}
function en(...e) {
	let t = !1;
	for (let n = 0; n < e.length; n += 1) {
		let r = e[n];
		t ||= !!r && H in r, typeof r == "function" && (t = !0, e[n] = X(r));
	}
	if (Yt && t) return new Proxy({
		get(t) {
			for (let n = e.length - 1; n >= 0; --n) {
				let r = Qt(e[n])[t];
				if (r !== void 0) return r;
			}
		},
		has(t) {
			for (let n = e.length - 1; n >= 0; --n) if (t in Qt(e[n])) return !0;
			return !1;
		},
		keys() {
			let t = [];
			for (let n of e) t.push(...Object.keys(Qt(n)));
			return [...new Set(t)];
		}
	}, Zt);
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
				get: $t.bind(n[t] = [o.get.bind(i)])
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
function tn(e, ...t) {
	let n = t.length;
	if (Yt && H in e) {
		let r = n > 1 ? t.flat() : t[0], i = t.map((t) => new Proxy({
			get: (n) => t.includes(n) ? e[n] : void 0,
			has: (n) => t.includes(n) && n in e,
			keys: () => t.filter((t) => t in e)
		}, Zt));
		return i.push(new Proxy({
			get: (t) => r.includes(t) ? void 0 : e[t],
			has: (t) => !r.includes(t) && t in e,
			keys: () => Object.keys(e).filter((e) => !r.includes(e))
		}, Zt)), i;
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
function nn(e, t, n = {}) {
	let r = wt(() => {
		let t = e() || [];
		return t[nt], t;
	}, () => !1), i = (e, n) => t(e, q(n));
	return n.fallback ? ze(r, i, n.fallback) : Re(r, i);
}
function rn(e, t, n = {}) {
	let r = wt(() => {
		let t = e() || [];
		return t[nt], t;
	}, () => !1), i = (e, n) => t(q(e), n);
	return n.fallback ? Ve(r, i, n.fallback) : Be(r, i);
}
function an(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return X(nn(() => e.each, e.children, t));
}
function on(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return X(rn(() => e.each, e.children, t));
}
function sn(e) {
	let t, n, r = () => n ||= Promise.resolve(e()).then((e) => (t = () => e.default, e.default)), i = (e) => {
		if (!t) {
			let [e] = Ut(r);
			t = e;
		}
		return X(() => {
			let n = t();
			return n ? Q(() => n(e)) : "";
		});
	};
	return i.preload = r, i;
}
function cn(e) {
	let t = X(() => e.when), n = e.keyed ? t : X(t, void 0, { equals: (e, t) => !e == !t });
	return X(() => {
		let r = n();
		if (!r) return e.fallback;
		let i = e.children;
		return typeof i != "function" || !i.length ? i : Q(() => i(e.keyed ? r : () => {
			if (!Q(n)) throw Error("Stale read from <Show>.");
			return t();
		}));
	});
}
function ln(e) {
	return e;
}
function un(e) {
	let t = qt(() => e.children), n = X(() => {
		let e = t(), n = Array.isArray(e) ? e : [e], r = () => void 0;
		for (let e = 0; e < n.length; e += 1) {
			let t = n[e], i = r, a = X(() => i() ? void 0 : t.when), o = t.keyed ? a : X(a, void 0, { equals: (e, t) => !e == !t });
			r = () => i() || (o() ? [
				e,
				a,
				t
			] : void 0);
		}
		return r;
	});
	return X(() => {
		let t = n()();
		if (!t) return e.fallback;
		let [r, i, a] = t, o = a.children;
		return typeof o != "function" || !o.length ? o : Q(() => o(a.keyed ? i() : () => {
			if (Q(n)()?.[0] !== r && !We()) throw Error("Stale read from <Match>.");
			return i();
		}));
	});
}
function dn() {
	if (pt) for (let e of [...pt]) e();
}
function fn(e) {
	let [t, n] = Y(void 0);
	pt ||= /* @__PURE__ */ new Set(), pt.add(n), $(() => pt.delete(n));
	let r = (t) => {
		let r = e.fallback;
		return typeof r == "function" && r.length ? Q(() => r(t, () => n(void 0))) : r;
	};
	return X(() => {
		let i = t();
		if (i !== void 0) return r(i);
		let a, o = Vt(() => e.children, (e) => {
			a = e, n(e);
		});
		return a === void 0 ? o : r(a);
	});
}
function pn() {
	return dt ||= Gt();
}
function mn() {
	return ft ||= Gt();
}
var hn = (e, t) => e.showContent === t.showContent && e.showFallback === t.showFallback;
function gn(e) {
	let [t] = Y(() => ({ inFallback: !1 })), n = Kt(mn()), r = [], [i] = Y(0), a = !0, o = n ? n.register(X(() => t()().inFallback)) : null, s = X((t) => {
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
	return vt(t, s), Wt(mn(), { register(e) {
		let t = r.length;
		return r.push(e), a || yt(i, (e) => e + 1), X(() => s()[t] ?? {
			showContent: !0,
			showFallback: !0
		}, void 0, { equals: hn });
	} }, () => {
		let t = e.children;
		return a = !1, yt(i, (e) => e + 1), t;
	});
}
function _n(e) {
	let [t] = Y(!1), n = 0, r = {
		effects: [],
		inFallback: t,
		resolved: !1,
		increment() {
			++n === 1 && vt(t, !0);
		},
		decrement() {
			--n === 0 && vt(t, !1);
		}
	}, i = Kt(mn())?.register(r.inFallback), a = D(), o, s;
	return $(() => o?.()), Wt(pn(), r, () => {
		let t = X(() => e.children);
		return X((n) => {
			let c = i ? i() : {
				showContent: !0,
				showFallback: !0
			};
			if (!r.inFallback() && c.showContent) return r.resolved = !0, o?.(), o = void 0, t();
			if (c.showFallback) return o ? s : Mt((t) => (o = t, s = e.fallback), a);
		});
	});
}
function vn() {
	return ut ||= De(!1, V);
}
function yn(e) {
	e[5] || !e[4] || e[3].size || (e[5] = !0, e[6](!1), e[7]());
}
function bn(e) {
	if (G) return e(), G[0];
	let t, n, r = new Promise((e, r) => {
		t = e, n = r;
	});
	return Promise.resolve().then(() => {
		if (!ct && !dt) {
			try {
				Z(e), t();
			} catch (e) {
				n(e);
			}
			return;
		}
		let i = !ct && !!dt, a = vn(), o = (e) => ke(a, e, !0), s = [
			r,
			/* @__PURE__ */ new Map(),
			/* @__PURE__ */ new Map(),
			/* @__PURE__ */ new Set(),
			!1,
			!1,
			o,
			t,
			i
		];
		G = s;
		try {
			Z(e);
		} catch (e) {
			G = void 0, n(e);
			return;
		}
		G = void 0, o(!0);
		try {
			(ct ?? ((e) => e()))(() => {
				try {
					lt = s, Z(() => {
						for (let [e, t] of s[2]) ke(e, t);
					}), lt = void 0, s[4] = !0, s[8] && s[3].size === 0 ? queueMicrotask(() => queueMicrotask(() => yn(s))) : yn(s);
				} catch (e) {
					lt = void 0, s[5] = !0, o(!1), n(e);
				}
			});
		} catch (e) {
			s[5] = !0, o(!1), n(e);
		}
	}), r;
}
function xn() {
	return [q(vn()), bn];
}
function Sn(e) {
	return {
		subscribe(t) {
			if (!(t instanceof Object)) throw TypeError("Expected the observer to be an object.");
			let n = typeof t == "function" ? t : t.next?.bind(t);
			if (!n) return { unsubscribe() {} };
			let r = Mt((t) => (kt(() => {
				let t = e();
				Q(() => n(t));
			}), t));
			return D() && $(r), { unsubscribe: r };
		},
		[Symbol.observable || "@@observable"]() {
			return this;
		}
	};
}
function Cn(e, t = void 0) {
	let [n, r] = Y(t, { equals: !1 });
	if (e && typeof e.subscribe == "function") {
		let t = e.subscribe((e) => r(() => e));
		$(() => {
			typeof t == "function" ? t() : t?.unsubscribe?.();
		});
	} else $(e((e) => r(() => e)));
	return n;
}
var Z = Ye;
function Q(e) {
	return k(K ? () => K.untrack(e) : e);
}
var $ = O, wn = Ue;
function Tn(e, t) {
	return ht(), xe(e, typeof t == "number" ? t : t?.timeout ?? 1073741823);
}
var En = Ce;
//#endregion
export { rt as $DEVCOMP, H as $PROXY, nt as $TRACK, it as DEV, fn as ErrorBoundary, an as For, on as Index, ln as Match, cn as Show, _n as Suspense, gn as SuspenseList, un as Switch, Z as batch, En as cancelCallback, Vt as catchError, qt as children, Jt as createComponent, At as createComputed, Gt as createContext, Ht as createDeferred, kt as createEffect, X as createMemo, Lt as createReaction, jt as createRenderEffect, Ut as createResource, Mt as createRoot, It as createSelector, Y as createSignal, wn as createUniqueId, Ot as enableExternalSource, Et as enableHydration, Dt as enableScheduling, V as equalFn, Cn as from, Pt as getListener, Nt as getOwner, rn as indexArray, sn as lazy, nn as mapArray, en as mergeProps, Sn as observable, Rt as on, $ as onCleanup, Bt as onError, zt as onMount, Tn as requestCallback, dn as resetErrorBoundaries, Ft as runWithOwner, U as sharedConfig, tn as splitProps, bn as startTransition, Q as untrack, Kt as useContext, xn as useTransition };
