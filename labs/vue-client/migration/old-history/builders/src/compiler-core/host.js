import { parse, parseExpression } from "@babel/parser";
import {
  DecodingMode,
  EntityDecoder,
  decodeHTML,
  decodeHTMLAttribute,
  htmlDecodeTree,
} from "entities/decode";
import { SourceMapGenerator } from "source-map-js";

// Foreign dependencies used by compiler-core. Template parsing and expression
// validation policy stay in LilScript; this adapter only exposes primitives.
export function hostDecodeEntities(source, attribute) {
  return attribute ? decodeHTMLAttribute(source) : decodeHTML(source);
}

export function hostDecodeEntity(source, offset, attribute) {
  const characters = [];
  const decoder = new EntityDecoder(htmlDecodeTree, codePoint => {
    characters.push(String.fromCodePoint(codePoint));
  });
  decoder.startEntity(
    attribute ? DecodingMode.Attribute : DecodingMode.Legacy,
  );
  let consumed = decoder.write(source, offset);
  if (consumed < 0) consumed = decoder.end();
  return { characters, consumed };
}

export function hostNewFunction(body) {
  new Function(body);
}

export function hostParseExpression(source, plugins, mode) {
  const options = {
    plugins: plugins ? [...plugins, "typescript"] : ["typescript"],
  };
  if (mode === 1) return parse(` ${source} `, options).program;
  if (mode === 2) return parseExpression(`(${source})=>{}`, options);
  return parseExpression(`(${source})`, options);
}

export function hostCreateSourceMap(filename, source) {
  const map = new SourceMapGenerator();
  map.setSourceContent(filename, source);
  return map;
}

export function hostAddSourceMapping(map, mapping) {
  map.addMapping(mapping);
}

export function hostSourceMapJson(map) {
  return map.toJSON();
}
