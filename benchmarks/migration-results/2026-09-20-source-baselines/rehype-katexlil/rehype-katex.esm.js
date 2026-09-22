/*! @itslil/rehype-katex 7.0.3 | LilScript reimplementation of rehype-katex | MIT */


// rehype-katex.raw.js
import { fromHtmlIsomorphic } from "hast-util-from-html-isomorphic";
import { toText } from "hast-util-to-text";
import { default as katex } from "katex";
import { SKIP, visitParents } from "unist-util-visit-parents";
var W = (q) => {
  var a = q;
  return a == null && (a = {}), (s, j) => {
    visitParents(s, "element", (n, h) => {
      var f, d = [], c = n.properties;
      c != null && Array.isArray(c.className) && (d = c.className), c = !!d.includes("language-math"), f = !!d.includes("math-display");
      var g = !!d.includes("math-inline");
      if (!(!c && !f && !g) && (g = h.length, d = void 0, g > 0 && (d = h[g - 1]), n.tagName == "code" && c && d != null && d.type == "element" && d.tagName == "pre" ? (c = void 0, g > 1 && (c = h[g - 2]), f = !0) : (c = d, d = n), !(c == null || !c))) {
        g = toText(d, { whitespace: "pre" }) + "";
        var hb;
        try {
          var ib = Object.assign({}, a);
          ib.displayMode = f, ib.throwOnError = !0;
          var jb = katex.renderToString(g, ib);
          hb = fromHtmlIsomorphic(jb, { fragment: !0 }).children;
        } catch (ub) {
          var G;
          G = h.slice(), Array.prototype.push.call(G, n);
          var lb = ub.name.toLowerCase();
          j.message("Could not render math with KaTeX", { ancestors: G, cause: ub, place: n.position, ruleId: lb, source: "rehype-katex" });
          try {
            var mb = Object.assign({}, a);
            mb.displayMode = f, mb.strict = "ignore", mb.throwOnError = !1;
            var nb = katex.renderToString(g, mb);
            hb = fromHtmlIsomorphic(nb, { fragment: !0 }).children;
          } catch {
            var ob = a.errorColor || "#cc0000", vb = "color:" + ob;
            hb = [{ type: "element", tagName: "span", properties: { className: ["katex-error"], style: vb, title: ub + "" }, children: [{ type: "text", value: g }] }];
          }
        }
        vb = c.children, n = [+vb.indexOf(d), 1], h = hb.length;
        for (var wb = 0; wb < h; wb++) n.push(hb[wb]);
        return vb.splice.apply(vb, n), SKIP;
      }
    });
  };
};
export {
  W as default
};
