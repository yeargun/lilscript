import assert from "node:assert/strict";
import { describe, test } from "node:test";
import * as candidate from "./compiler-dom-upstream.candidate.mjs";
import * as oracle from "@vue/compiler-dom";

const { isValidHTMLNesting } = candidate;

describe("compiler-dom html nesting", () => {
  test("preserves helper reflection", () => {
    assert.deepStrictEqual(
      [isValidHTMLNesting.name, isValidHTMLNesting.length],
      ["isValidHTMLNesting", 2],
    );
    assert.deepStrictEqual(
      [candidate.DOMNodeTransforms[2].name, candidate.DOMNodeTransforms[2].length],
      [oracle.DOMNodeTransforms[2].name, oracle.DOMNodeTransforms[2].length],
    );
    assert.equal("isValidHTMLNesting" in oracle, false);
  });

  test("preserves every restricted-child category", () => {
    const cases = [
      ["head", "base", true],
      ["head", "div", false],
      ["optgroup", "option", true],
      ["optgroup", "div", false],
      ["select", "hr", true],
      ["select", "div", false],
      ["table", "caption", true],
      ["table", "tr", false],
      ["tr", "th", true],
      ["tr", "tr", false],
      ["colgroup", "col", true],
      ["colgroup", "span", false],
      ["tbody", "tr", true],
      ["tbody", "td", false],
      ["thead", "tr", true],
      ["thead", "td", false],
      ["tfoot", "tr", true],
      ["tfoot", "td", false],
    ];
    for (const [parent, child, expected] of cases) {
      assert.equal(isValidHTMLNesting(parent, child), expected, `${parent} > ${child}`);
    }

    for (const parent of ["script", "iframe", "option", "textarea", "style", "title"]) {
      assert.equal(isValidHTMLNesting(parent, "span"), false, parent);
    }
  });

  test("preserves every restricted-parent category", () => {
    const cases = [
      ["div", "html", false],
      ["html", "body", true],
      ["div", "body", false],
      ["html", "head", true],
      ["tr", "td", true],
      ["div", "td", false],
      ["table", "colgroup", true],
      ["table", "caption", true],
      ["table", "tbody", true],
      ["table", "tfoot", true],
      ["colgroup", "col", true],
      ["tr", "th", true],
      ["table", "thead", true],
      ["thead", "tr", true],
      ["table", "tr", false],
      ["dl", "dd", true],
      ["div", "dd", true],
      ["span", "dd", false],
      ["dl", "dt", true],
      ["div", "dt", true],
      ["span", "dt", false],
      ["figure", "figcaption", true],
      ["div", "figcaption", false],
      ["details", "summary", true],
      ["div", "summary", false],
      ["map", "area", true],
      ["div", "area", false],
    ];
    for (const [parent, child, expected] of cases) {
      assert.equal(isValidHTMLNesting(parent, child), expected, `${parent} > ${child}`);
    }
  });

  test("preserves known invalid child and parent sets", () => {
    const cases = [
      ["p", "address", false],
      ["p", "table", false],
      ["p", "span", true],
      ["svg", "div", false],
      ["svg", "img", false],
      ["svg", "g", true],
      ["a", "a", false],
      ["button", "button", false],
      ["dd", "dt", false],
      ["dt", "dd", false],
      ["form", "form", false],
      ["li", "li", false],
      ["h1", "h6", false],
      ["h6", "h1", false],
      ["h1", "div", true],
      ["section", "custom-element", true],
    ];
    for (const [parent, child, expected] of cases) {
      assert.equal(isValidHTMLNesting(parent, child), expected, `${parent} > ${child}`);
    }
  });

  test("template bypasses restrictions", () => {
    for (const child of ["html", "body", "tr", "td", "col", "figcaption"]) {
      assert.equal(isValidHTMLNesting("template", child), true, child);
    }
  });

  test("compile owns validation diagnostics while parse remains structural", () => {
    const source = "<p><div></div></p>";
    const summarize = implementation => {
      const warnings = [];
      const result = implementation.compile(source, {
        onWarn(error) {
          warnings.push({
            constructor: error.constructor.name,
            name: error.name,
            message: error.message,
            loc: error.loc,
            keys: Object.keys(error),
          });
        },
      });
      return { warnings, code: result.code };
    };
    assert.deepStrictEqual(summarize(candidate), summarize(oracle));

    for (const implementation of [candidate, oracle]) {
      const warnings = [];
      const ast = implementation.parse(source, { onWarn: warning => warnings.push(warning) });
      assert.equal(ast.children[0].tag, "p");
      assert.deepStrictEqual(warnings, []);
    }
  });

  test("traverses nested parents and emits one exact warning per invalid node", () => {
    const source = "<main><p><div/><div/></p><table><tr/></table></main>";
    const summarize = implementation => {
      const warnings = [];
      implementation.compile(source, {
        onWarn(error) {
          warnings.push({ message: error.message, loc: error.loc });
        },
      });
      return warnings;
    };

    const actual = summarize(candidate);
    assert.deepStrictEqual(actual, summarize(oracle));
    assert.equal(actual.length, 3);
    assert.deepStrictEqual(actual.map(warning => warning.loc.source), [
      "<div/>",
      "<div/>",
      "<tr/>",
    ]);
  });

  test("preserves warning receiver and node location identity", () => {
    for (const implementation of [candidate, oracle]) {
      const parent = implementation.parse("<p><div/></p>").children[0];
      const node = parent.children[0];
      let receiver;
      let warning;
      const context = {
        parent,
        onWarn(error) {
          receiver = this;
          warning = error;
        },
      };

      implementation.DOMNodeTransforms[2](node, context);
      assert.equal(receiver, context);
      assert.equal(warning.loc, node.loc);
      assert.equal(
        warning.message,
        "<div> cannot be child of <p>, according to HTML specifications. " +
          "This can cause hydration errors or potentially disrupt future functionality.",
      );
    }
  });

  test("validation ignores components and accepts the template exception", () => {
    for (const source of [
      "<Component><tr/></Component>",
      "<template><tr/></template>",
    ]) {
      for (const implementation of [candidate, oracle]) {
        const warnings = [];
        implementation.compile(source, { onWarn: warning => warnings.push(warning) });
        assert.deepStrictEqual(warnings, [], source);
      }
    }
  });

  test("continues validating descendants inside template and v-pre", () => {
    for (const source of [
      "<template><p><div/></p></template>",
      "<section v-pre><p><div/></p></section>",
    ]) {
      const summarize = implementation => {
        const warnings = [];
        implementation.compile(source, {
          onWarn(error) {
            warnings.push({ message: error.message, loc: error.loc });
          },
        });
        return warnings;
      };
      assert.deepStrictEqual(summarize(candidate), summarize(oracle), source);
    }
  });
});
