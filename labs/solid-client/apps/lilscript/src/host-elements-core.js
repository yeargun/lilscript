const svgNamespace = "http://www.w3.org/2000/svg";
const mathNamespace = "http://www.w3.org/1998/Math/MathML";
const svgElements = new Set([
  "altGlyph",
  "altGlyphDef",
  "altGlyphItem",
  "animate",
  "animateColor",
  "animateMotion",
  "animateTransform",
  "circle",
  "clipPath",
  "color-profile",
  "cursor",
  "defs",
  "desc",
  "ellipse",
  "feBlend",
  "feColorMatrix",
  "feComponentTransfer",
  "feComposite",
  "feConvolveMatrix",
  "feDiffuseLighting",
  "feDisplacementMap",
  "feDistantLight",
  "feDropShadow",
  "feFlood",
  "feFuncA",
  "feFuncB",
  "feFuncG",
  "feFuncR",
  "feGaussianBlur",
  "feImage",
  "feMerge",
  "feMergeNode",
  "feMorphology",
  "feOffset",
  "fePointLight",
  "feSpecularLighting",
  "feSpotLight",
  "feTile",
  "feTurbulence",
  "filter",
  "font",
  "font-face",
  "font-face-format",
  "font-face-name",
  "font-face-src",
  "font-face-uri",
  "foreignObject",
  "g",
  "glyph",
  "glyphRef",
  "hkern",
  "image",
  "line",
  "linearGradient",
  "marker",
  "mask",
  "metadata",
  "missing-glyph",
  "mpath",
  "path",
  "pattern",
  "polygon",
  "polyline",
  "radialGradient",
  "rect",
  "set",
  "stop",
  "svg",
  "switch",
  "symbol",
  "text",
  "textPath",
  "tref",
  "tspan",
  "use",
  "view",
  "vkern",
]);

export function installElementHost(scope, document, store) {
  scope.domCreateIntrinsicElement = (tag) =>
    store(
      svgElements.has(tag)
        ? document.createElementNS(svgNamespace, tag)
        : document.createElement(tag),
    );
  scope.domCreateSvgElement = (tag) =>
    store(document.createElementNS(svgNamespace, tag));
  scope.domCreateMathElement = (tag) =>
    store(document.createElementNS(mathNamespace, tag));
}

function prepareTemplateRoot(document, html) {
  const template = document.createElement("template");
  template.innerHTML = html;
  return template.content.firstChild;
}

export function installTemplateHost(scope, document, store, load) {
  scope.domPrepareTemplate = (html) => store(prepareTemplateRoot(document, html));
  scope.domPrepareSvgTemplate = (html) =>
    store(prepareTemplateRoot(document, `<svg>${html}</svg>`).firstChild);
  scope.domPrepareMathTemplate = (html) =>
    store(prepareTemplateRoot(document, `<math>${html}</math>`).firstChild);
  scope.domCloneNode = (node) => store(load(node).cloneNode(true));
  scope.domFirstChild = (node) => store(load(node).firstChild);
  scope.domNextSibling = (node) => store(load(node).nextSibling);
}
