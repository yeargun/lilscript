// Snapshot the decoder's initialized provider objects. Its generated enum is an
// ESM `var` export, while these adapter bindings are immutable by contract.
import {
  DecodingMode as Mode, EntityDecoder as Decoder, decodeHTML as html,
  decodeHTMLAttribute as attribute, htmlDecodeTree as tree,
} from '../../../node_modules/entities/dist/decode.js';
export const DecodingMode = Mode;
export const EntityDecoder = Decoder;
export const decodeHTML = html;
export const decodeHTMLAttribute = attribute;
export const htmlDecodeTree = tree;
