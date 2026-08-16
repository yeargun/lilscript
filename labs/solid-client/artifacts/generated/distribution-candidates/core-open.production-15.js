//#region packages/solidlil/reactive.generated.js
var e = "Potential Infinite Loop Detected.", t = (e) => e[2], n = () => M >= 0 ? k(2, M) : null, r = () => N >= 0 ? k(2, N) : P >= 0 ? k(1, P) : null, i = (e) => S(e, r(), !1), a = (e) => C(e, !0, !1), o = (e) => C(e, !1, !1), s = (e) => C(e, !1, !0), c = (e) => D(e), l = (e) => {
	$e = e;
}, u = (e) => {
	e[3] = !0;
}, d = (e) => {
	e[1] == 1 ? Se(j[e[2]][4], e[0]) : e[1] == 2 && Se(A[e[2]][6], e[0]);
}, f = (e) => {
	e[1] == 1 ? Se(j[e[2]][3], e[0]) : e[1] == 2 && Se(A[e[2]][5], e[0]);
}, p = (e, t) => fe(e, t), m = (e, t) => e == t, ee = (e, t) => e == t, h = () => {
	let e = Xe;
	return Xe = Xe + 1 | 0, "cl-" + e;
}, te = (e) => {
	s(() => {
		let t = M;
		M = -1, e(), M = t;
	});
}, g = (e, t, n) => {
	let r = [
		[],
		[],
		[]
	];
	Fe(r);
	let i = [[], null];
	return Ne(i), x(() => {
		Ie(r), We(i);
	}), () => {
		let a = D(e);
		return E(() => a.length == 0 ? (Ie(r), Ue(i, n)) : (We(i), de(r, a, t)));
	};
}, _ = (e, t, n) => {
	let r = [
		[],
		[],
		[],
		[]
	];
	Pe(r);
	let i = [[], null];
	return Ne(i), x(() => {
		Le(r), We(i);
	}), () => {
		let a = D(e);
		return E(() => a.length == 0 ? (Le(r), Ue(i, n)) : (We(i), ue(r, a, t)));
	};
}, v = (e, t) => {
	let n = [
		[],
		[],
		[],
		[]
	];
	return Pe(n), x(() => {
		Le(n);
	}), () => {
		let r = D(e);
		return E(() => ue(n, r, t));
	};
}, ne = (e, t) => {
	let n = [
		[],
		[],
		[]
	];
	return Fe(n), x(() => {
		Ie(n);
	}), () => {
		let r = D(e);
		return E(() => de(n, r, t));
	};
}, y = (e, t) => {
	let n = [
		null,
		[],
		null,
		0,
		0
	];
	return T(n, e, t), n;
}, re = (e, t) => {
	let n = [
		null,
		null,
		[],
		[]
	];
	return Re(n, e, t), n;
}, b = (e) => {
	let t = [
		0,
		!1,
		null
	];
	return t[0] = -1, t[1] = !1, t[2] = e, (e) => {
		t[0] >= 0 && (je(t[0]), t[0] = -1), t[1] = !1, t[0] = C(() => {
			if (e(), t[1]) {
				var n = t[2], r = M;
				M = -1, n(), M = r, je(t[0]), t[0] = -1;
			} else t[1] = !0;
		}, !1, !1);
	};
}, x = (e) => {
	var t = [e];
	return N >= 0 ? A[N][7].push(t) : P >= 0 && j[P][5].push(t), e;
}, ie = (e) => {
	var t = F == 0 && !L;
	t && (F = F + 1 | 0);
	try {
		for (var n = 0; n < e.length; n = n + 1 | 0) He(e[n] | 0);
	} catch (e) {
		throw t && (F = F - 1 | 0), e;
	}
	t && (F = F - 1 | 0, I > 0 ? we() : w());
}, ae = (e) => {
	F = F + 1 | 0;
	var t = !1;
	try {
		var n = e();
		return t = !0, n;
	} finally {
		F = F - 1 | 0, t && F == 0 && (I > 0 ? we() : w());
	}
}, oe = (e, t) => {
	var n = M, r = N, i = P;
	M = -1, e && e[0] != 0 ? e[0] == 2 ? (N = e[1], P = -1) : (N = -1, P = e[1]) : (N = -1, P = -1), I = I + 1 | 0;
	var a = !1;
	try {
		var o = t();
		return a = !0, o;
	} finally {
		I = I - 1 | 0, M = n, N = r, P = i, a && w();
	}
}, S = (e, t = null, n = !1) => {
	var r = j.length;
	Ye.length > 0 && (r = Ye.pop());
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
	a[3] = [], a[4] = [], a[5] = [], a[6] = null, a[7] = !1, r == j.length ? j.push(a) : j[r] = a, n && (i == 1 ? j[t][4].push(r) : i == 2 && A[t][6].push(r));
	var o = M, s = P, c = N;
	M = -1, P = r, N = -1, I = I + 1 | 0, t = () => {
		j[r] == a && Ae(r);
	};
	var l = !1;
	try {
		var u = e(t);
		return l = !0, u;
	} finally {
		I = I - 1 | 0, M = o, N = c, P = s, l && w();
	}
}, C = (e, t, n) => {
	var r = A.length;
	if (Je.length > 0 && (r = Je.pop()), N >= 0) var i = N, a = 2, o;
	else P >= 0 ? (i = P, a = 1) : (a = 0, i = -1);
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
	], o[4] = [], o[5] = [], o[6] = [], o[7] = [], o[8] = null, o[9] = t, o[10] = !1, o[11] = !1, o[12] = 0, o[13] = [], r == A.length ? A.push(o) : A[r] = o, N >= 0 ? A[N][5].push(r) : P >= 0 && j[P][3].push(r), n && (I > 0 || L) ? He(r) : Me(o), r;
}, se = (e, t, n) => {
	let r = [
		null,
		null,
		null,
		0,
		null
	];
	return le(r, e, t, n), a(() => {
		qe(r);
	}), x(() => {
		Ge(r);
	}), r[1];
}, ce = (e, t, n) => {
	let r = [
		null,
		[],
		null,
		0,
		0
	];
	return T(r, e, n), C(() => {
		O(r, t(r[0]));
	}, !0, !1), r;
};
function le(e, t, n, r) {
	e[0] = t;
	let i = t[0], a = [
		null,
		[],
		null,
		0,
		0
	];
	T(a, i, n), e[1] = a, e[2] = t[0], e[3] = r, e[4] = null;
}
function ue(e, t, n) {
	var r = /* @__PURE__ */ new Set(), i = e[0].length;
	t.length < i && (i = t.length);
	for (var a = 0, o, s, c, l, u, d, f, p, m, h; o = a < i && e[0][a] == t[a], o;) a += 1;
	for (s = e[0].length - 1, c = t.length - 1; i = s >= a && c >= a && e[0][s] == t[c], i;) --s, c = c - 1 | 0;
	for (l = [], u = [], d = [], f = [], o = 0; o < t.length; o += 1) {
		if (p = t[o], o < a) i = o;
		else if (o > c) i = (s + o | 0) - c | 0;
		else for (i = a;;) {
			if (i > s) {
				i = -1;
				break;
			}
			if (!r.has(i) && e[0][i] == p) break;
			i = i + 1 | 0;
		}
		i >= 0 ? (r.add(i), O(e[3][i], o), u.push(e[1][i]), d.push(e[2][i]), f.push(e[3][i])) : (m = [
			null,
			[],
			null,
			0,
			0
		], T(m, o, ee), h = [() => {}], u.push(S(((e, t, n, r) => (i) => (r[0] = i, e(t, n)))(n, p, m, h), null, !1)), d.push(h), f.push(m)), l.push(p);
	}
	for (t = 0; t < e[0].length; t += 1) !r.has(t) && e[2][t][0]();
	return e[0] = l, e[1] = u, e[2] = d, e[3] = f, e[1];
}
function de(e, t, n) {
	var r = e[2].length;
	t.length < r && (r = t.length);
	for (var i = 0, a, o; i < r; i += 1) e[2][i][0] != t[i] && O(e[2][i], t[i]);
	for (var s = e[2].length; s < t.length; s = s + 1 | 0) i = t[s], a = [
		null,
		[],
		null,
		0,
		0
	], T(a, i, (e, t) => e == t), o = [() => {}], i = S(((e, t, n) => (r) => (n[0] = r, e(t, s)))(n, a, o), null, !1), e[2].push(a), e[1].push(o), e[0].push(i);
	for (; e[2].length > t.length;) e[1][e[2].length - 1][0](), e[1].pop(), e[2].pop(), e[0].pop();
	return e[0].slice();
}
function fe(e, t) {
	for (var n = e[1], r = 0; r < e[2].length; r += 1) if (n(e[2][r], t)) return D(e[3][r]);
	return n = n(t, e[0][0]), r = [
		null,
		[],
		null,
		0,
		0
	], T(r, n, m), e[2].push(t), e[3].push(r), D(r);
}
function pe(e, t) {
	for (var n = 0; n < e[1].length; n += 1) if ((e[1][n] | 0) == t) {
		e[1][n] = e[1][e[1].length - 1] | 0, e[1].pop();
		return;
	}
}
function me(e, t) {
	for (var n = 0; n < e[1].length; n += 1) if ((e[1][n] | 0) == t) return !1;
	return e[1].push(t), !0;
}
function he(e, t) {
	for (var n = 0; n < e[13].length; n += 1) if ((e[13][n] | 0) == t) return;
	e[13].push(t);
}
var ge = (e) => {
	var t, n;
	if (!(A[e][11] || e == N)) {
		for (n = 0; n < A[e][13].length; n += 1) t = A[e][13][n] | 0, (ye(t) || ve(t)) && ge(t);
		be(e) && Me(A[e]);
	}
}, _e = (e, t, n) => {
	if (e == t) return !0;
	if (n >= A.length) return !1;
	for (var r = 0; r < A[e][13].length; r += 1) if (_e(A[e][13][r] | 0, t, n + 1 | 0)) return !0;
	return !1;
}, ve = (e) => {
	for (var t = 0; t < A[e][13].length; t += 1) if (ye(A[e][13][t] | 0)) return !0;
	return !1;
}, ye = (e) => {
	for (var t = 0; t < z.length; t += 1) if ((z[t] | 0) == e) return !0;
	return !1;
}, be = (e) => {
	for (var t = 0; t < z.length; t += 1) if ((z[t] | 0) == e) return Ee(t), !0;
	return !1;
}, xe = (e, t = 1073741823) => {
	var n = [
		Ze,
		e,
		t,
		!1
	];
	for (n[3] = !1, Ze = Ze + 1 | 0, B.push(n), e = B.length - 1; t = e > 0 && B[e][2] < B[e - 1][2], t;) t = B[e - 1], B[e - 1] = B[e], B[e] = t, --e;
	return Qe || (Qe = !0, $e(() => {
		Qe = !1, De();
	})), n;
}, Se = (e, t) => {
	for (var n = e.length - 1; n >= 0; --n) if ((e[n] | 0) == t) {
		e[n] = e[e.length - 1] | 0, e.pop();
		return;
	}
}, Ce = (e, t) => {
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
}, w = () => {
	if (!(L || F > 0 || I > 0)) {
		L = !0;
		var t = 0;
		try {
			for (; z.length > 0;) {
				if (t = t + 1 | 0, t > 1e3) throw e;
				Me(A[Te()]);
			}
			L = !1;
		} catch (e) {
			throw z = [], L = !1, e;
		}
	}
}, we = () => {
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
				r = Ee(n), Me(A[r]);
			}
		} catch (e) {
			throw z = [], L = !1, e;
		}
	}
}, Te = () => {
	for (var e = -1, t = -1, n = -1, r = 0, i, a, o, s, c; r < z.length; r += 1) i = z[r] | 0, A[i][9] ? (e < 0 || A[i][12] > A[z[e] | 0][12]) && (e = r) : !A[i][10] && t < 0 ? t = r : n < 0 ? n = r : (o = z[n] | 0, a = A[i], s = A[o], c = a[1] == s[1] && a[2] == s[2], (c && a[12] > s[12] || !c && (Ce(i, o) || !Ce(o, i) && a[12] > s[12])) && (n = r));
	return t >= 0 && (e = t), Ee(e < 0 ? n : e);
}, Ee = (e) => {
	for (var t = z[e] | 0; e < z.length - 1; e = e + 1 | 0) z[e] = z[e + 1 | 0] | 0;
	return z.pop(), t;
}, De = () => {
	for (Qe = !1; B.length > 0;) {
		for (var e = B[0], t = 0; t < B.length - 1; t += 1) B[t] = B[t + 1];
		B.pop(), e[3] || (e[3] = !0, e[1](!1));
	}
};
function Oe(e) {
	for (; e[4].length > 0;) {
		var t = e[4][e[4].length - 1];
		e[4].pop(), pe(t, e[0]);
	}
}
function ke(e) {
	for (; e[6].length > 0;) {
		var t = e[6][e[6].length - 1] | 0;
		e[6].pop(), Ae(t);
	}
	for (; e[5].length > 0;) t = e[5][e[5].length - 1] | 0, e[5].pop(), je(t);
	for (; e[7].length > 0;) t = e[7][e[7].length - 1], e[7].pop(), t[0]();
}
var Ae = (e) => {
	var t = j[e];
	if (!t[7]) {
		for (; t[4].length > 0;) {
			var n = t[4][t[4].length - 1] | 0;
			t[4].pop(), Ae(n);
		}
		for (; t[3].length > 0;) n = t[3][t[3].length - 1] | 0, t[3].pop(), je(n);
		for (; t[5].length > 0;) n = t[5][t[5].length - 1], t[5].pop(), n[0]();
		d(t), (n = t[6]) && (n[0] = 0, t[6] = null), t[7] = !0, Ye.push(e);
	}
}, je = (e) => {
	var t = A[e];
	if (!t[11]) {
		ke(t), Oe(t), be(e), f(t), t[3] = () => {}, t[13] = [];
		var n = t[8];
		n && (n[0] = 0, t[8] = null), t[11] = !0, Je.push(e);
	}
};
function Me(e) {
	if (!e[11]) {
		ke(e), Oe(e);
		var t = M, n = N, r = P;
		M = e[0], N = e[0], P = -1, e[12] = 0, e[13] = [], I = I + 1 | 0, e[3](), e[10] = !0, I = I - 1 | 0, M = t, N = n, P = r, w();
	}
}
function Ne(e) {
	e[0] = [], e[1] = () => {};
}
function Pe(e) {
	e[0] = [], e[1] = [], e[2] = [], e[3] = [];
}
function Fe(e) {
	e[0] = [], e[1] = [], e[2] = [];
}
function Ie(e) {
	for (var t = 0; t < e[1].length; t += 1) e[1][t][0]();
	e[0] = [], e[1] = [], e[2] = [];
}
function Le(e) {
	for (var t = 0; t < e[2].length; t += 1) e[2][t][0]();
	e[0] = [], e[1] = [], e[2] = [], e[3] = [];
}
function Re(e, t, n) {
	e[0] = t, e[1] = n, e[2] = [], e[3] = [], a(() => {
		for (var t = D(e[0]), n = e[1], r = 0, i; r < e[2].length; r += 1) i = e[3][r], O(i, n(e[2][r], t));
	});
}
function T(e, t, n) {
	e[0] = t, e[1] = [], e[2] = n, e[3] = 0, e[4] = -1;
}
function ze(e, t) {
	return O(e, t(e[0]));
}
var Be = (e, t, n = !1) => {
	var r = R;
	r && (n = !0), R = n;
	try {
		return O(e, t);
	} finally {
		R = r;
	}
}, Ve = (e, t, n = !1) => {
	var r = R;
	r && (n = !0), R = n;
	try {
		return ze(e, t);
	} finally {
		R = r;
	}
}, E = (e) => {
	var t = M;
	M = -1;
	try {
		return e();
	} finally {
		M = t;
	}
};
function D(e) {
	var t;
	if (M < 0 && e[4] < 0) return e[0];
	if (M >= 0 && e[4] >= 0 && e[4] == N) throw "Circular memo dependency detected.";
	if (e[4] >= 0 && e[4] != N ? (t = N >= 0 && !A[N][9] && !A[N][10], t = !t) : t = !1, t && ge(e[4]), M >= 0) {
		t = M;
		var n = e[3] + 1 | 0;
		n > A[t][12] && (A[t][12] = n), e[4] >= 0 && he(A[t], e[4]), me(e, t) && A[t][4].push(e);
	}
	return e[0];
}
var He = (e) => {
	if (N >= 0 && A[N][9] && !R && _e(N, e, 0) && !Ce(e, N)) throw "Reactive dependency cycle detected.";
	!A[e][11] && !ye(e) && z.push(e);
};
function O(e, t) {
	return N >= 0 && A[N][9] && (e[3] = A[N][12], e[4] = N), e[2](e[0], t) ? t : (e[0] = t, ie(e[1]), t);
}
var k = (e, t) => {
	if (e == 1) {
		var n = j[t][6];
		return n || (n = [
			e,
			t,
			null
		], j[t][6] = n, n[2] = k(j[t][1], j[t][2]), n);
	}
	return e == 2 ? (n = A[t][8]) ? n : (n = [
		e,
		t,
		null
	], A[t][8] = n, n[2] = k(A[t][1], A[t][2]), n) : null;
};
function Ue(e, t) {
	return e[0].length == 0 && (e[0] = [S((n) => (e[1] = n, t()), null, !1)]), e[0];
}
function We(e) {
	e[0].length > 0 && e[1](), e[0] = [];
}
function Ge(e) {
	var t = e[4];
	t && (u(t), e[4] = null);
}
function Ke(e) {
	e[4] = null, O(e[1], e[2]);
}
function qe(e) {
	e[2] = D(e[0]), !e[4] && (e[4] = xe((t) => {
		Ke(e);
	}, e[3]));
}
var A = [], j = [], Je = [], Ye = [], M = -1, N = -1, P = -1, F = 0, I = 0, L = !1, R = !1, z = [], Xe = 0, Ze = 1, B = [], Qe = !1, $e = (e) => {}, V = (e, t) => e === t, H = Symbol("solid-proxy"), et = Symbol("solid-track"), tt = Symbol("solid-dev-component"), nt = void 0, U = {
	context: void 0,
	registry: void 0,
	effects: void 0,
	done: !1,
	getContextId() {
		return rt(this.context.count);
	},
	getNextContextId() {
		return rt(this.context.count++);
	}
};
function rt(e) {
	let t = String(e), n = t.length - 1;
	return `${U.context.id}${n ? String.fromCharCode(96 + n) : ""}${t}`;
}
var W = Symbol("solidlil-signal"), it = !1, at = !1, ot, G, st, ct, K, lt, ut, dt, ft = /* @__PURE__ */ new WeakMap();
function pt() {
	at || (at = !0, l(queueMicrotask));
}
function mt(e) {
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
		return c(e);
	};
	return n[W] = e, n;
}
function ht(e, t) {
	if (G) {
		let n = G[2].has(e) ? G[2].get(e) : c(e), r = typeof t == "function" ? t(n) : t;
		return G[2].set(e, r), G[1].clear(), r;
	}
	return typeof t == "function" ? Ve(e, t) : Be(e, t);
}
function gt(e, t) {
	return Be(e[W], t, !0);
}
function _t(e, t) {
	return Ve(e[W], t, !0);
}
function vt(e) {
	return e instanceof Error ? e : Error(typeof e == "string" ? e : "Unknown error", { cause: e });
}
function yt(e) {
	return e && e.owner === void 0 && (e.owner = yt(t(e))), e;
}
function J(e, n = r()) {
	let i = vt(e), a = n;
	for (; a;) {
		let e = ft.get(a);
		if (e?.length) {
			for (let n of e) try {
				n(i);
			} catch (e) {
				return J(e, t(a));
			}
			return;
		}
		a = t(a);
	}
	throw i;
}
function bt(e) {
	let t = xt(e);
	return (...e) => {
		try {
			return t(...e);
		} catch (e) {
			return J(e);
		}
	};
}
function xt(e) {
	if (!K) return e;
	let t = y(0, () => !1), n = K.factory(e, () => Ve(t, (e) => e + 1));
	return x(() => n.dispose()), (...e) => (c(t), n.track(...e));
}
function St(e, t = V) {
	let n = e[W];
	return n === void 0 && (n = X(e, void 0, { equals: t })[W]), n;
}
function Ct(e) {
	for (; typeof e == "function" && !e.length;) e = e();
	if (!Array.isArray(e)) return e;
	let t = [];
	for (let n of e) {
		let e = Ct(n);
		Array.isArray(e) ? t.push(...e) : t.push(e);
	}
	return t;
}
function wt() {
	it = !0;
}
function Tt(e = Cn) {
	ot = e;
}
function Et(e, t = (e) => e()) {
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
	let n = y(e, mt(t));
	return [q(n), (e) => ht(n, e)];
}
function X(e, t, n) {
	let r = bt(e), i = mt(n), a = !1, o = ce(t, r, (e, t) => a ? i(e, t) : (a = !0, !1));
	return q(o, () => r(c(o)));
}
function Dt(e, t, n) {
	let r = t, i = bt(e);
	s(() => {
		r = i(r);
	});
}
function Ot(e, t, n) {
	let r = t, i = bt(e);
	a(() => {
		r = i(r);
	});
}
function kt(e, t, n) {
	let r = t, i = bt(e);
	o(() => {
		r = i(r);
	});
}
function At(e, t) {
	let n = !e.length, r = (t) => {
		try {
			return n ? e() : e(() => Q(t));
		} catch (e) {
			return J(e);
		}
	};
	return n ? S(r, null) : t === void 0 ? i(r) : S(r, t);
}
function jt() {
	return yt(r());
}
function Mt() {
	return yt(n());
}
function Nt(e, t) {
	return oe(e, () => {
		try {
			return t();
		} catch (e) {
			return J(e);
		}
	});
}
function Pt(e, t = V) {
	let n = re(St(e, t), t);
	return (e) => p(n, e);
}
function Ft(e, t) {
	return b(e);
}
function It(e, t, n) {
	let r = Array.isArray(e) ? e : null, i, a = n && n.defer;
	return (n) => {
		let o = r ? r.map((e) => e()) : e();
		if (a) return a = !1, n;
		let s = Q(() => t(o, i, n));
		return i = o, s;
	};
}
function Lt(e) {
	let t = r();
	te(() => {
		try {
			e();
		} catch (e) {
			J(e, t);
		}
	});
}
function Rt(e) {
	let t = r();
	if (!t) return;
	let n = ft.get(t);
	n ? n.push(e) : ft.set(t, [e]);
}
function zt(e, t) {
	return S((n) => {
		let i = r();
		ft.set(i, [t]);
		try {
			return e();
		} catch (e) {
			return J(e, i);
		}
	}, r(), !0);
}
function Bt(e, t) {
	return pt(), q(se(St(e, mt(t)), mt(t), t?.timeoutMs ?? 1073741823));
}
function Vt(e, t, i) {
	let a = typeof t == "function", o = !a || e, s = a ? t : e, c = (a ? i : t) ?? {}, l = typeof o == "function" ? X(o) : null, [u, d] = (c.storage ?? Y)(c.initialValue), [f, p] = Y(void 0), [m, ee] = Y(void 0, { equals: !1 }), [h, te] = Y("initialValue" in c ? "ready" : "unresolved"), g = /* @__PURE__ */ new Set(), _ = "initialValue" in c, v = null, ne = 0, y = !1, re = r();
	function b(e, t, n, r) {
		if (e !== ne) return t;
		v = null, r !== void 0 && (_ = !0), Z(() => {
			n === void 0 && d(() => t), te(n === void 0 ? _ ? "ready" : "unresolved" : "errored"), p(n);
		});
		for (let e of g) e.decrement();
		return g.clear(), t;
	}
	function x(e = !0) {
		if (e !== !1 && y) return v;
		y = !1;
		let t = l ? l() : o, n = ++ne;
		if (t == null || t === !1) return v = null, b(n, Q(u), void 0, void 0);
		let r;
		try {
			r = Q(() => s(t, {
				value: u(),
				refetching: e
			}));
		} catch (e) {
			return b(n, void 0, vt(e), t);
		}
		if (!r || typeof r.then != "function") return b(n, r, void 0, t);
		if (v = r, "v" in r && "s" in r) return r.s === 1 ? b(n, r.v, void 0, t) : b(n, void 0, vt(r.v), t);
		y = !0, queueMicrotask(() => {
			y = !1;
		}), Z(() => {
			te(_ ? "refreshing" : "pending"), p(void 0), ee();
		});
		let i = r.then((e) => (c.onHydrated && !_ && queueMicrotask(() => c.onHydrated(t, { value: e })), b(n, e, void 0, t)), (e) => b(n, void 0, vt(e), t));
		v = i;
		let a = G ?? st;
		if (a) {
			a[3].add(r);
			let e = () => {
				a[3].delete(r), _n(a);
			};
			r.then(e, e);
		}
		return i;
	}
	function ie() {
		let e = u(), t = f();
		if (t !== void 0 && !v) throw t;
		if (n() && lt) {
			let e = Wt(lt);
			e && Ot(() => {
				m(), v && !g.has(e) && (e.increment(), g.add(e));
			});
		}
		return e;
	}
	return Object.defineProperties(ie, {
		state: { get: () => h() },
		error: { get: () => f() },
		loading: { get() {
			let e = h();
			return e === "pending" || e === "refreshing";
		} },
		latest: { get() {
			if (!_) return ie();
			let e = f();
			if (e !== void 0 && !v) throw e;
			return u();
		} }
	}), l ? Ot(() => {
		re = r(), x(!1);
	}) : x(!1), [ie, {
		refetch: (e) => Nt(re, () => x(e)),
		mutate: d
	}];
}
function Ht(e, t, n) {
	return S((i) => {
		let a = yt(r());
		return a.context = { [e.id]: t }, n();
	}, r(), !0);
}
function Ut(e, t) {
	let n = {
		id: Symbol("context"),
		defaultValue: e,
		Provider(e) {
			return t?.name, Ht(n, e.value, () => Gt(() => e.children));
		}
	};
	return n;
}
function Wt(e) {
	let t = yt(r());
	for (; t;) {
		let n = t.context?.[e.id];
		if (n !== void 0) return n;
		t = t.owner;
	}
	return e.defaultValue;
}
function Gt(e) {
	let t = X(e), n = X(() => Ct(t()));
	return n.toArray = () => {
		let e = n();
		return Array.isArray(e) ? e : e == null ? [] : [e];
	}, n;
}
function Kt(e, t) {
	if (it && U.context) {
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
var qt = typeof Proxy == "function", Jt = () => !0, Yt = {
	get(e, t, n) {
		return t === H ? n : e.get(t);
	},
	has(e, t) {
		return t === H || e.has(t);
	},
	set: Jt,
	deleteProperty: Jt,
	getOwnPropertyDescriptor(e, t) {
		return {
			configurable: !0,
			enumerable: !0,
			get() {
				return e.get(t);
			},
			set: Jt,
			deleteProperty: Jt
		};
	},
	ownKeys(e) {
		return e.keys();
	}
};
function Xt(e) {
	return (e = typeof e == "function" ? e() : e) ? e : {};
}
function Zt() {
	for (let e = 0; e < this.length; e += 1) {
		let t = this[e]();
		if (t !== void 0) return t;
	}
}
function Qt(...e) {
	let t = !1;
	for (let n = 0; n < e.length; n += 1) {
		let r = e[n];
		t ||= !!r && H in r, typeof r == "function" && (t = !0, e[n] = X(r));
	}
	if (qt && t) return new Proxy({
		get(t) {
			for (let n = e.length - 1; n >= 0; --n) {
				let r = Xt(e[n])[t];
				if (r !== void 0) return r;
			}
		},
		has(t) {
			for (let n = e.length - 1; n >= 0; --n) if (t in Xt(e[n])) return !0;
			return !1;
		},
		keys() {
			let t = [];
			for (let n of e) t.push(...Object.keys(Xt(n)));
			return [...new Set(t)];
		}
	}, Yt);
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
				get: Zt.bind(n[t] = [o.get.bind(i)])
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
function $t(e, ...t) {
	let n = t.length;
	if (qt && H in e) {
		let r = n > 1 ? t.flat() : t[0], i = t.map((t) => new Proxy({
			get: (n) => t.includes(n) ? e[n] : void 0,
			has: (n) => t.includes(n) && n in e,
			keys: () => t.filter((t) => t in e)
		}, Yt));
		return i.push(new Proxy({
			get: (t) => r.includes(t) ? void 0 : e[t],
			has: (t) => !r.includes(t) && t in e,
			keys: () => Object.keys(e).filter((e) => !r.includes(e))
		}, Yt)), i;
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
function en(e, t, n = {}) {
	let r = St(() => {
		let t = e() || [];
		return t[et], t;
	}, () => !1), i = (e, n) => t(e, q(n));
	return n.fallback ? _(r, i, n.fallback) : v(r, i);
}
function tn(e, t, n = {}) {
	let r = St(() => {
		let t = e() || [];
		return t[et], t;
	}, () => !1), i = (e, n) => t(q(e), n);
	return n.fallback ? g(r, i, n.fallback) : ne(r, i);
}
function nn(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return X(en(() => e.each, e.children, t));
}
function rn(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return X(tn(() => e.each, e.children, t));
}
function an(e) {
	let t, n, r = () => n ||= Promise.resolve(e()).then((e) => (t = () => e.default, e.default)), i = (e) => {
		if (!t) {
			let [e] = Vt(r);
			t = e;
		}
		return X(() => {
			let n = t();
			return n ? Q(() => n(e)) : "";
		});
	};
	return i.preload = r, i;
}
function on(e) {
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
function sn(e) {
	return e;
}
function cn(e) {
	let t = Gt(() => e.children), r = X(() => {
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
		let t = r()();
		if (!t) return e.fallback;
		let [i, a, o] = t, s = o.children;
		return typeof s != "function" || !s.length ? s : Q(() => s(o.keyed ? a() : () => {
			if (Q(r)()?.[0] !== i && !n()) throw Error("Stale read from <Match>.");
			return a();
		}));
	});
}
function ln() {
	if (dt) for (let e of [...dt]) e();
}
function un(e) {
	let [t, n] = Y(void 0);
	dt ||= /* @__PURE__ */ new Set(), dt.add(n), $(() => dt.delete(n));
	let r = (t) => {
		let r = e.fallback;
		return typeof r == "function" && r.length ? Q(() => r(t, () => n(void 0))) : r;
	};
	return X(() => {
		let i = t();
		if (i !== void 0) return r(i);
		let a, o = zt(() => e.children, (e) => {
			a = e, n(e);
		});
		return a === void 0 ? o : r(a);
	});
}
function dn() {
	return lt ||= Ut();
}
function fn() {
	return ut ||= Ut();
}
var pn = (e, t) => e.showContent === t.showContent && e.showFallback === t.showFallback;
function mn(e) {
	let [t] = Y(() => ({ inFallback: !1 })), n = Wt(fn()), r = [], [i] = Y(0), a = !0, o = n ? n.register(X(() => t()().inFallback)) : null, s = X((t) => {
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
	return gt(t, s), Ht(fn(), { register(e) {
		let t = r.length;
		return r.push(e), a || _t(i, (e) => e + 1), X(() => s()[t] ?? {
			showContent: !0,
			showFallback: !0
		}, void 0, { equals: pn });
	} }, () => {
		let t = e.children;
		return a = !1, _t(i, (e) => e + 1), t;
	});
}
function hn(e) {
	let [t] = Y(!1), n = 0, i = {
		effects: [],
		inFallback: t,
		resolved: !1,
		increment() {
			++n === 1 && gt(t, !0);
		},
		decrement() {
			--n === 0 && gt(t, !1);
		}
	}, a = Wt(fn())?.register(i.inFallback), o = r(), s, c;
	return $(() => s?.()), Ht(dn(), i, () => {
		let t = X(() => e.children);
		return X((n) => {
			let r = a ? a() : {
				showContent: !0,
				showFallback: !0
			};
			if (!i.inFallback() && r.showContent) return i.resolved = !0, s?.(), s = void 0, t();
			if (r.showFallback) return s ? c : At((t) => (s = t, c = e.fallback), o);
		});
	});
}
function gn() {
	return ct ||= y(!1, V);
}
function _n(e) {
	e[5] || !e[4] || e[3].size || (e[5] = !0, e[6](!1), e[7]());
}
function vn(e) {
	if (G) return e(), G[0];
	let t, n, r = new Promise((e, r) => {
		t = e, n = r;
	});
	return Promise.resolve().then(() => {
		if (!ot && !lt) {
			try {
				Z(e), t();
			} catch (e) {
				n(e);
			}
			return;
		}
		let i = !ot && !!lt, a = gn(), o = (e) => Be(a, e, !0), s = [
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
			(ot ?? ((e) => e()))(() => {
				try {
					st = s, Z(() => {
						for (let [e, t] of s[2]) Be(e, t);
					}), st = void 0, s[4] = !0, s[8] && s[3].size === 0 ? queueMicrotask(() => queueMicrotask(() => _n(s))) : _n(s);
				} catch (e) {
					st = void 0, s[5] = !0, o(!1), n(e);
				}
			});
		} catch (e) {
			s[5] = !0, o(!1), n(e);
		}
	}), r;
}
function yn() {
	return [q(gn()), vn];
}
function bn(e) {
	return {
		subscribe(t) {
			if (!(t instanceof Object)) throw TypeError("Expected the observer to be an object.");
			let n = typeof t == "function" ? t : t.next?.bind(t);
			if (!n) return { unsubscribe() {} };
			let i = At((t) => (Dt(() => {
				let t = e();
				Q(() => n(t));
			}), t));
			return r() && $(i), { unsubscribe: i };
		},
		[Symbol.observable || "@@observable"]() {
			return this;
		}
	};
}
function xn(e, t = void 0) {
	let [n, r] = Y(t, { equals: !1 });
	if (e && typeof e.subscribe == "function") {
		let t = e.subscribe((e) => r(() => e));
		$(() => {
			typeof t == "function" ? t() : t?.unsubscribe?.();
		});
	} else $(e((e) => r(() => e)));
	return n;
}
var Z = ae;
function Q(e) {
	return E(K ? () => K.untrack(e) : e);
}
var $ = x, Sn = h;
function Cn(e, t) {
	return pt(), xe(e, typeof t == "number" ? t : t?.timeout ?? 1073741823);
}
var wn = u;
//#endregion
export { tt as $DEVCOMP, H as $PROXY, et as $TRACK, nt as DEV, un as ErrorBoundary, nn as For, rn as Index, sn as Match, on as Show, hn as Suspense, mn as SuspenseList, cn as Switch, Z as batch, wn as cancelCallback, zt as catchError, Gt as children, Kt as createComponent, Ot as createComputed, Ut as createContext, Bt as createDeferred, Dt as createEffect, X as createMemo, Ft as createReaction, kt as createRenderEffect, Vt as createResource, At as createRoot, Pt as createSelector, Y as createSignal, Sn as createUniqueId, Et as enableExternalSource, wt as enableHydration, Tt as enableScheduling, V as equalFn, xn as from, Mt as getListener, jt as getOwner, tn as indexArray, an as lazy, en as mapArray, Qt as mergeProps, bn as observable, It as on, $ as onCleanup, Rt as onError, Lt as onMount, Cn as requestCallback, ln as resetErrorBoundaries, Nt as runWithOwner, U as sharedConfig, $t as splitProps, vn as startTransition, Q as untrack, Wt as useContext, yn as useTransition };
