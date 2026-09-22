/*! @itslil/rehype-katex 7.0.3 | LilScript reimplementation of rehype-katex | MIT */
// rehype-katex.closed.js
import { fromHtmlIsomorphic } from "hast-util-from-html-isomorphic";
import { toText } from "hast-util-to-text";
import { default as katex } from "katex";
import { SKIP, visitParents } from "unist-util-visit-parents";
var a = (c) => {
  var b = c;
  return b == null && (b = {}), (l, m) => {
    visitParents(l, "element", (u, v) => {
      var x = [], w = u.properties, y;
      w != null && Array.isArray(w.className) && (x = w.className), w = !!x.includes("language-math"), y = !!x.includes("math-display");
      var z = !!x.includes("math-inline");
      if (!(!w && !y && !z) && (z = v.length, x = void 0, z > 0 && (x = v[z - 1]), u.tagName == "code" && w && x != null && x.type == "element" && x.tagName == "pre" ? (w = void 0, z > 1 && (w = v[z - 2]), y = !0) : (w = x, x = u), !(w == null || !w))) {
        z = toText(x, { whitespace: "pre" }) + "";
        var yb;
        try {
          var zb = Object.assign({}, b);
          zb.displayMode = y, zb.throwOnError = !0;
          var Ab = katex.renderToString(z, zb);
          yb = fromHtmlIsomorphic(Ab, { fragment: !0 }).children;
        } catch (A) {
          var Z = void 0, Bb = v.slice();
          Array.prototype.push.call(Bb, u);
          var Cb = A.name.toLowerCase();
          v = Bb;
          var B = u.position;
          m.message("Could not render math with KaTeX", { ancestors: v, cause: A, place: B, ruleId: Cb, source: "rehype-katex" });
          try {
            var Db = Object.assign({}, b);
            Db.displayMode = y, Db.strict = "ignore", Db.throwOnError = !1;
            var Eb = katex.renderToString(z, Db);
            yb = fromHtmlIsomorphic(Eb, { fragment: !0 }).children;
          } catch {
            var Fb = b.errorColor;
            Fb = Fb || "#cc0000";
            var Lb = "color:" + Fb;
            yb = [{ type: "element", tagName: "span", properties: { className: ["katex-error"], style: Lb, title: A + "" }, children: [{ type: "text", value: z }] }];
          }
        }
        Lb = w.children, u = [+Lb.indexOf(x) | 0, 1], v = yb.length;
        for (var Mb = 0; Mb < v; )
          Array.prototype.push.call(u, yb[Mb]), Mb = Mb + 1;
        return Lb.splice.apply(Lb, u), SKIP;
      }
    });
  };
};
export {
  a as default
};
