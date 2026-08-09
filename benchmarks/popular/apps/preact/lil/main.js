import { createDocument } from "../shared/dom.js";
import { serializeElement } from "../shared/canon.js";
import { mountApp, bump } from "../../../build/preact-lilscript.js";

globalThis.document = createDocument();

const mount = mountApp();
const before = serializeElement(mount.firstChild);
bump();
const after = serializeElement(mount.firstChild);
console.log(`preact:${before}|${after}`);
