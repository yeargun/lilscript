/** @const */
var _$HY = {};

/** @type {boolean} */
_$HY.done;

/** @param {string} selector @return {number} */
function domQueryRoot(selector) {}

/** @param {string} tag @return {number} */
function domCreateElement(tag) {}

/** @param {string} tag @return {number} */
function domCreateIntrinsicElement(tag) {}

/** @param {string} tag @return {number} */
function domCreateSvgElement(tag) {}

/** @param {string} tag @return {number} */
function domCreateMathElement(tag) {}

/** @param {string} value @return {number} */
function domCreateText(value) {}

/** @return {number} */
function domCreateComment() {}

/** @return {number} */
function domCreateFragment() {}

/** @param {string} html @return {number} */
function domPrepareTemplate(html) {}

/** @param {string} html @return {number} */
function domPrepareSvgTemplate(html) {}

/** @param {string} html @return {number} */
function domPrepareMathTemplate(html) {}

/** @param {number} node @return {number} */
function domCloneNode(node) {}

/** @param {number} node @return {number} */
function domFirstChild(node) {}

/** @param {number} node @return {number} */
function domNextSibling(node) {}

/** @param {number} parent @return {!Array<number>} */
function domChildNodes(parent) {}

/** @param {number} node @return {void} */
function domReleaseNode(node) {}

/** @param {number} node @return {boolean} */
function domIsFragment(node) {}

/** @param {number} parent @param {number} child @return {void} */
function domAppendChild(parent, child) {}

/** @param {number} node @return {void} */
function domRemoveNode(node) {}

/** @param {number} parent @param {number} marker @param {!Array<number>} current @param {!Array<number>} next @return {void} */
function domReconcile(parent, marker, current, next) {}

/** @param {number} parent @param {number} marker @param {number} current @param {number} next @return {void} */
function domReconcileOne(parent, marker, current, next) {}

/** @param {number} node @param {string} value @return {void} */
function domSetText(node, value) {}

/** @param {number} node @param {string} name @param {string} value @return {void} */
function domSetAttribute(node, name, value) {}

/** @param {number} node @param {string} namespace @param {string} name @param {string} value @return {void} */
function domSetAttributeNS(node, namespace, name, value) {}

/** @param {number} node @param {string} name @param {boolean} value @return {void} */
function domSetBoolAttribute(node, name, value) {}

/** @param {number} node @param {string} name @param {string} value @return {void} */
function domSetStringProperty(node, name, value) {}

/** @param {number} node @param {string} name @param {boolean} value @return {void} */
function domSetBoolProperty(node, name, value) {}

/** @param {number} node @param {string} name @param {boolean} value @return {void} */
function domToggleClass(node, name, value) {}

/** @param {number} node @param {string} name @param {string} value @return {void} */
function domSetStyleProperty(node, name, value) {}

/** @param {number} node @param {?} props @param {?} previous @param {boolean} svg @return {?} */
function domSpread(node, props, previous, svg) {}

/** @param {number} node @param {string} event @param {function(): void} callback @return {number} */
function domAddEventListener(node, event, callback) {}

/** @param {number} listener @return {void} */
function domRemoveEventListener(listener) {}

/** @param {number} node @param {string} event @param {function(number): void} callback @return {number} */
function domAddDelegatedEvent(node, event, callback) {}

/** @param {number} node @param {string} event @param {function(): void} callback @return {number} */
function domAddDelegatedEventVoid(node, event, callback) {}

/** @param {number} node @param {function(): void} callback @return {number} */
function domAddDelegatedClickVoid(node, callback) {}

/** @param {number} node @param {function(): void} callback @return {void} */
function domSetDelegatedClickVoid(node, callback) {}

/** @param {number} listener @return {void} */
function domRemoveDelegatedClick(listener) {}

/** @param {number} listener @return {void} */
function domRemoveDelegatedEvent(listener) {}

/** @return {void} */
function domClearDelegatedEvents() {}

/** @param {number} event @return {number} */
function domEventTarget(event) {}

/** @param {number} event @return {number} */
function domEventCurrentTarget(event) {}

/** @param {number} event @return {string} */
function domEventType(event) {}

/** @param {number} event @return {boolean} */
function domEventDefaultPrevented(event) {}

/** @param {number} event @return {void} */
function domEventPreventDefault(event) {}

/** @param {number} event @return {void} */
function domEventStopPropagation(event) {}

/** @param {number} node @param {number} host @return {void} */
function domSetEventHost(node, host) {}

/** @param {number} node @return {boolean} */
function domIsHead(node) {}

/** @param {number} node @return {number} */
function domAttachShadow(node) {}

/** @param {number} node @return {void} */
function domClear(node) {}

/** @param {function(): void} callback */
function hostSchedule(callback) {}

/** @param {function(): void} callback @return {void} */
function registerBenchmarkDispose(callback) {}

/** @param {function(): void} callback @return {void} */
function registerLsxDispose(callback) {}

/**
 * @param {function(): number} ownerSlots
 * @param {function(): number} effectSlots
 * @param {function(): number} freeOwnerSlots
 * @param {function(): number} freeEffectSlots
 * @param {function(): number} pendingEffects
 * @return {void}
 */
function registerLsxDiagnostics(
  ownerSlots,
  effectSlots,
  freeOwnerSlots,
  freeEffectSlots,
  pendingEffects,
) {}

/** @type {function(!Event): void} */
Element.prototype.$$click;

/** @type {?} */
Object.prototype.observers;

/** @type {?} */
Object.prototype.tState;

/** @type {function(): void} */
Object.prototype.__disposeSolidBenchmark;

/** @type {function(): void} */
Object.prototype.__disposeLsx;

/** @type {function(): ?} */
Object.prototype.__lsxDiagnostics;

/** @type {?} */
Element.prototype._$host;
