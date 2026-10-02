// Source-owned primitive adapter; package providers stay explicit.
export function createHost(parse,parseExpression,DecodingMode,EntityDecoder,decodeHTML,decodeHTMLAttribute,htmlDecodeTree,SourceMapGenerator) {




// Foreign dependencies used by compiler-core. Template parsing and expression
// validation policy stay in LilScript; this adapter only exposes primitives.
function hostDecodeEntities(source, attribute) {
  return attribute ? decodeHTMLAttribute(source) : decodeHTML(source);
}

function hostDecodeEntity(source, offset, attribute) {
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

function hostNewFunction(body) {
  new Function(body);
}

function hostParseExpression(source, plugins, mode) {
  const options = {
    plugins: plugins ? [...plugins, "typescript"] : ["typescript"],
  };
  if (mode === 1) return parse(` ${source} `, options).program;
  if (mode === 2) return parseExpression(`(${source})=>{}`, options);
  return parseExpression(`(${source})`, options);
}

function hostCreateSourceMap(filename, source) {
  const map = new SourceMapGenerator();
  map.setSourceContent(filename, source);
  return map;
}

function hostAddSourceMapping(map, mapping) {
  map.addMapping(mapping);
}

function hostSourceMapJson(map) {
  return map.toJSON();
}

return {hostDecodeEntities,hostDecodeEntity,hostNewFunction,hostParseExpression,hostCreateSourceMap,hostAddSourceMapping,hostSourceMapJson};
}
