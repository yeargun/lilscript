//#region packages/solidlil/reactive.generated.js
var e = (e, t) => e == t, t = () => {
	if (!(q || G > 0 || K > 0)) {
		for (q = !0; J.length > 0;) k(B[L()]);
		q = !1;
	}
};
function n(e, t) {
	return I(e, t);
}
function r(e, t) {
	return I(e, t(e.value));
}
function i(e, t) {
	return r(e, t);
}
function a(e, t) {
	return _(e, t);
}
function o(e) {
	return P(e);
}
function ee(e) {
	return y(e, !1, !0);
}
function s(e) {
	return y(e, !1, !1);
}
function c(e) {
	return y(e, !0, !1);
}
function l(e) {
	for (var t = 0; t < e.unsubscribers.length; t = t + 1 | 0) D(e.unsubscribers[t]);
	e.unsubscribers = [];
}
var u = (e) => {
	for (var t = 0; t < J.length; t = t + 1 | 0) if ((J[t] | 0) == e) {
		for (; t < J.length - 1; t = t + 1 | 0) J[t] = J[t + 1 | 0] | 0;
		return J.pop(), !0;
	}
	return !1;
}, d = (e) => {
	for (var t = 0; t < J.length; t = t + 1 | 0) if ((J[t] | 0) == e) return !0;
	return !1;
}, f = (e) => {
	for (var t = 0; t < B[e].sourceEffectIds.length; t = t + 1 | 0) if (d(B[e].sourceEffectIds[t] | 0)) return !0;
	return !1;
}, p = (e) => {
	var t, n;
	if (!(B[e].disposed || e == U)) {
		for (n = 0; n < B[e].sourceEffectIds.length; n = n + 1 | 0) t = B[e].sourceEffectIds[n] | 0, (d(t) || f(t)) && p(t);
		u(e) && k(B[e]);
	}
};
function m(e, t) {
	for (var n = 0; n < e.sourceEffectIds.length; n = n + 1 | 0) if ((e.sourceEffectIds[n] | 0) == t) return;
	e.sourceEffectIds.push(t);
}
function h(e, t) {
	for (var n = 0; n < e.observerIds.length; n = n + 1 | 0) if ((e.observerIds[n] | 0) == t) return !1;
	return e.observerIds.push(t), !0;
}
function g(e, t) {
	for (var n = 0; n < e.observerIds.length; n = n + 1 | 0) if ((e.observerIds[n] | 0) == t) {
		e.observerIds[n] = e.observerIds[e.observerIds.length - 1] | 0, e.observerIds.pop();
		return;
	}
}
function _(t, n) {
	for (var r = t.equals, i = 0; i < t.keys.length; i = i + 1 | 0) if (r(t.keys[i], n)) return P(t.matches[i]);
	return r = r(n, t.source.value), i = {}, F(i, r, e), t.keys.push(n), t.matches.push(i), P(i);
}
function v(e, t, n) {
	e.source = t, e.equals = n, e.keys = [], e.matches = [], c(() => {
		for (var t = P(e.source), n = e.equals, r = 0, i; r < e.keys.length; r = r + 1 | 0) i = e.matches[r], I(i, n(e.keys[r], t));
	});
}
var y = (e, t, n) => {
	var r = B.length;
	if (U >= 0) var i = U, a = 2, o;
	else W >= 0 ? (i = W, a = 1) : (a = 0, i = -1);
	return o = {}, ne(o, r, a, i, e, t), B.push(o), U >= 0 ? B[U].childEffectIds.push(r) : W >= 0 && V[W].effectIds.push(r), n && (K > 0 || q) ? z(r) : k(o), r;
};
function b(e) {
	var t = {};
	return E(t, e), U >= 0 ? B[U].cleanups.push(t) : W >= 0 && V[W].cleanups.push(t), e;
}
function x(e) {
	return G = G + 1 | 0, e = e(), G = G - 1 | 0, G == 0 && t(), e;
}
function S(e, t) {
	let n = {};
	return v(n, e, t), n;
}
function C(e, t) {
	let n = {};
	return F(n, e, t), n;
}
function w(e, t, n) {
	let r = {};
	return F(r, e, n), y(() => {
		I(r, t(r.value));
	}, !0, !1), r;
}
function T(e) {
	let t = H;
	H = -1;
	let n = e();
	return H = t, n;
}
function E(e, t) {
	e.callback = t;
}
function D(e) {
	e.callback();
}
var O = (e) => {
	e = B[e], !e.disposed && (M(e), l(e), e.disposed = !0);
};
function k(e) {
	if (!e.disposed) {
		M(e), l(e);
		var n = H, r = U, i = W;
		H = e.id, U = e.id, W = -1, e.level = 0, e.sourceEffectIds = [], K = K + 1 | 0, e.callback(), K = K - 1 | 0, H = n, U = r, W = i, t();
	}
}
function te(e, n = null) {
	var r = V.length;
	if (n) {
		var i = n.kind;
		n = n.id;
	} else i = 0, n = -1;
	var a = {};
	return N(a, r, i, n), V.push(a), i == 1 ? V[n].ownerIds.push(r) : i == 2 && B[n].childOwnerIds.push(r), n = H, i = W, a = U, H = -1, W = r, U = -1, K = K + 1 | 0, e = e(() => {
		j(r);
	}), K = K - 1 | 0, H = n, U = a, W = i, t(), e;
}
function A(e) {
	var n = V.length;
	if (U >= 0) var r = U, i = 2, a;
	else W >= 0 ? (r = W, i = 1) : (i = 0, r = -1);
	return a = {}, N(a, n, i, r), V.push(a), i == 1 ? V[r].ownerIds.push(n) : i == 2 && B[r].childOwnerIds.push(n), r = H, i = W, a = U, H = -1, W = n, U = -1, K = K + 1 | 0, e = e(() => {
		j(n);
	}), K = K - 1 | 0, H = r, U = a, W = i, t(), e;
}
var j = (e) => {
	if (e = V[e], !e.disposed) {
		for (var t = e.ownerIds.length - 1; t >= 0; --t) j(e.ownerIds[t] | 0);
		for (e.ownerIds = [], t = e.effectIds.length - 1; t >= 0; --t) O(e.effectIds[t] | 0);
		for (e.effectIds = [], t = 0; t < e.cleanups.length; t = t + 1 | 0) D(e.cleanups[t]);
		e.cleanups = [], e.disposed = !0;
	}
};
function M(e) {
	for (var t = e.childOwnerIds.length - 1; t >= 0; --t) j(e.childOwnerIds[t] | 0);
	for (e.childOwnerIds = [], t = e.childEffectIds.length - 1; t >= 0; --t) O(e.childEffectIds[t] | 0);
	for (e.childEffectIds = [], t = 0; t < e.cleanups.length; t = t + 1 | 0) D(e.cleanups[t]);
	e.cleanups = [];
}
function ne(e, t, n, r, i, a) {
	e.id = t, e.parentKind = n, e.parentId = r, e.callback = i, e.unsubscribers = [], e.childEffectIds = [], e.childOwnerIds = [], e.cleanups = [], e.memoComputation = a, e.disposed = !1, e.level = 0, e.sourceEffectIds = [];
}
function N(e, t, n, r) {
	e.id = t, e.parentKind = n, e.parentId = r, e.effectIds = [], e.ownerIds = [], e.cleanups = [], e.disposed = !1;
}
function P(e) {
	var t;
	if (e.producerEffectId >= 0 && e.producerEffectId != U && p(e.producerEffectId), H >= 0) {
		var n = H;
		if (t = e.level + 1 | 0, t > B[n].level && (B[n].level = t), e.producerEffectId >= 0 && m(B[n], e.producerEffectId), h(e, n)) {
			t = B[n].unsubscribers;
			var r = () => {
				g(e, n);
			}, i = {};
			E(i, r), t.push(i);
		}
	}
	return e.value;
}
function F(e, t, n) {
	e.value = t, e.observerIds = [], e.equals = n, e.level = 0, e.producerEffectId = -1;
}
function I(e, t) {
	return U >= 0 && B[U].memoComputation && (e.level = B[U].level, e.producerEffectId = U), e.equals(e.value, t) ? t : (e.value = t, R(e.observerIds), t);
}
var L = () => {
	for (var e = -1, t = -1, n = 0, r; n < J.length; n = n + 1 | 0) r = J[n] | 0, B[r].memoComputation ? (e < 0 || B[r].level > B[J[e] | 0].level) && (e = n) : t < 0 && (t = n);
	return e < 0 && (e = t), e = J[e] | 0, u(e), e;
}, R = (e) => {
	var n = G == 0 && !q;
	n && (G = G + 1 | 0);
	for (var r = 0; r < e.length; r = r + 1 | 0) z(e[r]);
	n && (G = G - 1 | 0, t());
}, z = (e) => {
	!B[e].disposed && !d(e) && J.push(e);
}, B = [], V = [], H = -1, U = -1, W = -1, G = 0, K = 0, q = !1, J = [], Y = (e, t) => e === t, X = /* @__PURE__ */ new WeakMap();
function Z(e) {
	return e?.equals === !1 ? () => !1 : e?.equals ?? Y;
}
function Q(e) {
	let t = () => o(e);
	return X.set(t, e), t;
}
function re(e, t) {
	let r = C(e, Z(t));
	return [Q(r), (e) => typeof e == "function" ? i(r, e) : n(r, e)];
}
function $(e, t, n) {
	return Q(w(t, e, Z(n)));
}
function ie(e, t) {
	let n = t;
	ee(() => {
		n = e(n);
	});
}
function ae(e, t) {
	let n = t;
	s(() => {
		n = e(n);
	});
}
function oe(e, t) {
	return t == null ? A(e) : te(e, t);
}
function se(e, t = Y) {
	let n = X.get(e);
	if (n === void 0) {
		let r = $(e, e(), { equals: t });
		n = X.get(r);
	}
	let r = S(n, t);
	return (e) => a(r, e);
}
var ce = x, le = T, ue = b;
//#endregion
export { ce as batch, ie as createEffect, $ as createMemo, ae as createRenderEffect, oe as createRoot, se as createSelector, re as createSignal, ue as onCleanup, le as untrack };
