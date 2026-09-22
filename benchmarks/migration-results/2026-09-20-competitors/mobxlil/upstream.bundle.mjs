// ../mobxlil/node_modules/mobx/dist/mobx.mjs
var niceErrors = {
  0: `Invalid value for configuration 'enforceActions', expected 'never', 'always' or 'observed'`,
  1(annotationType, key) {
    return `Cannot apply '${annotationType}' to '${key.toString()}': Field not found.`;
  },
  /*
  2(prop) {
      return `invalid decorator for '${prop.toString()}'`
  },
  3(prop) {
      return `Cannot decorate '${prop.toString()}': action can only be used on properties with a function value.`
  },
  4(prop) {
      return `Cannot decorate '${prop.toString()}': computed can only be used on getter properties.`
  },
  */
  5: "'keys()' can only be used on observable objects, arrays, sets and maps",
  6: "'values()' can only be used on observable objects, arrays, sets and maps",
  7: "'entries()' can only be used on observable objects, arrays and maps",
  8: "'set()' can only be used on observable objects, arrays and maps",
  9: "'remove()' can only be used on observable objects, arrays and maps",
  10: "'has()' can only be used on observable objects, arrays and maps",
  11: "'get()' can only be used on observable objects, arrays and maps",
  12: `Invalid annotation`,
  13: `Dynamic observable objects cannot be frozen. If you're passing observables to 3rd party component/function that calls Object.freeze, pass copy instead: toJS(observable)`,
  14: "Intercept handlers should return nothing or a change object",
  15: `Observable arrays cannot be frozen. If you're passing observables to 3rd party component/function that calls Object.freeze, pass copy instead: toJS(observable)`,
  16: `Modification exception: the internal structure of an observable array was changed.`,
  19(other) {
    return "Cannot initialize from classes that inherit from Map: " + other.constructor.name;
  },
  20(other) {
    return "Cannot initialize map from " + other;
  },
  21(dataStructure) {
    return `Cannot convert to map from '${dataStructure}'`;
  },
  23: "It is not possible to get index atoms from arrays",
  24(thing) {
    return "Cannot obtain administration from " + thing;
  },
  25(property, name) {
    return `the entry '${property}' does not exist in the observable map '${name}'`;
  },
  26: "please specify a property",
  27(property, name) {
    return `no observable property '${property.toString()}' found on the observable object '${name}'`;
  },
  28(thing) {
    return "Cannot obtain atom from " + thing;
  },
  29: "Expecting some object",
  30: "invalid action stack. did you forget to finish an action?",
  31: "missing option for computed: get",
  32(name, derivation) {
    return `Cycle detected in computation ${name}: ${derivation}`;
  },
  33(name) {
    return `The setter of computed value '${name}' is trying to update itself. Did you intend to update an _observable_ value, instead of the computed property?`;
  },
  34(name) {
    return `[ComputedValue '${name}'] It is not possible to assign a new value to a computed value.`;
  },
  35: "There are multiple, different versions of MobX active. Make sure MobX is loaded only once or use `configure({ isolateGlobalState: true })`",
  36: "isolateGlobalState should be called before MobX is running any reactions",
  37(method) {
    return `[mobx] \`observableArray.${method}()\` mutates the array in-place, which is not allowed inside a derivation. Use \`array.slice().${method}()\` instead`;
  },
  38: "'ownKeys()' can only be used on observable objects",
  39: "'defineProperty()' can only be used on observable objects",
  40(length) {
    return "Out of range: " + length;
  },
  41(other) {
    return "Cannot initialize set from " + other;
  },
  42(key) {
    return `Invalid index: '${key}'`;
  },
  43(annotationType, name, kind) {
    return `Cannot apply '${annotationType}' to '${name}' (kind: ${kind}):
'${annotationType}' can only be used on properties with a function value.`;
  },
  44(annotationType) {
    return `'${annotationType}' can only be used with 'makeObservable'`;
  }
};
var errors = true ? niceErrors : {};
function die(error, ...args) {
  if (true) {
    let e = typeof error === "string" ? error : errors[error];
    if (typeof e === "function") e = e.apply(null, args);
    throw new Error(`[MobX] ${e}`);
  }
  throw new Error(`[MobX] minified error nr: ${error}${args.length ? " " + args.map(String).join(",") : ""}. See mobx.js.org/errors`);
}
var assign = Object.assign;
var getDescriptor = Object.getOwnPropertyDescriptor;
var defineProperty = Object.defineProperty;
var objectPrototype = Object.prototype;
var EMPTY_ARRAY = [];
Object.freeze(EMPTY_ARRAY);
var EMPTY_OBJECT = {};
Object.freeze(EMPTY_OBJECT);
var plainObjectString = /* @__PURE__ */ Object.toString();
function getNextId() {
  return ++globalState.mobxGuid;
}
function once(func) {
  let invoked = false;
  return function() {
    if (invoked) {
      return;
    }
    invoked = true;
    return func.apply(this, arguments);
  };
}
var noop = () => {
};
function isFunction(fn) {
  return typeof fn === "function";
}
function isStringish(value) {
  const t = typeof value;
  switch (t) {
    case "string":
    case "symbol":
    case "number":
      return true;
  }
  return false;
}
function isObject(value) {
  return value !== null && typeof value === "object";
}
function isPlainObject(value) {
  if (!isObject(value)) {
    return false;
  }
  const proto = Object.getPrototypeOf(value);
  if (proto == null) {
    return true;
  }
  const protoConstructor = hasProp(proto, "constructor") && proto.constructor;
  return typeof protoConstructor === "function" && protoConstructor.toString() === plainObjectString;
}
function isGenerator(obj) {
  const constructor = obj == null ? void 0 : obj.constructor;
  if (!constructor) {
    return false;
  }
  if ("GeneratorFunction" === constructor.name || "GeneratorFunction" === constructor.displayName) {
    return true;
  }
  return false;
}
function addHiddenProp(object, propName, value) {
  defineProperty(object, propName, {
    enumerable: false,
    writable: true,
    configurable: true,
    value
  });
}
function addHiddenFinalProp(object, propName, value) {
  defineProperty(object, propName, {
    enumerable: false,
    writable: false,
    configurable: true,
    value
  });
}
function createInstanceofPredicate(name, theClass) {
  const propName = "isMobX" + name;
  theClass.prototype[propName] = true;
  return function(x) {
    return isObject(x) && x[propName] === true;
  };
}
function isES6Map(thing) {
  return thing != null && Object.prototype.toString.call(thing) === "[object Map]";
}
function isPlainES6Map(thing) {
  const mapProto = Object.getPrototypeOf(thing);
  const objectProto = Object.getPrototypeOf(mapProto);
  const nullProto = Object.getPrototypeOf(objectProto);
  return nullProto === null;
}
function isES6Set(thing) {
  return thing != null && Object.prototype.toString.call(thing) === "[object Set]";
}
function getPlainObjectKeys(object) {
  const keys2 = Object.keys(object);
  const symbols = Object.getOwnPropertySymbols(object);
  if (!symbols.length) {
    return keys2;
  }
  return [...keys2, ...symbols.filter((s) => objectPrototype.propertyIsEnumerable.call(object, s))];
}
var ownKeys = Reflect.ownKeys;
function stringifyKey(key) {
  if (typeof key === "string") {
    return key;
  }
  if (typeof key === "symbol") {
    return key.toString();
  }
  return new String(key).toString();
}
function toPrimitive(value) {
  return value === null ? null : typeof value === "object" ? "" + value : value;
}
function hasProp(target, prop) {
  return objectPrototype.hasOwnProperty.call(target, prop);
}
var getOwnPropertyDescriptors = Object.getOwnPropertyDescriptors;
function getFlag(flags, mask) {
  return !!(flags & mask);
}
function setFlag(flags, mask, newValue) {
  if (newValue) {
    flags |= mask;
  } else {
    flags &= ~mask;
  }
  return flags;
}
function assert20223DecoratorType(context, types) {
  if (!types.includes(context.kind)) {
    die(`The decorator applied to '${String(context.name)}' cannot be used on a ${context.kind} element`);
  }
}
var $mobx = /* @__PURE__ */ Symbol("mobx administration");
var Atom = class {
  /**
   * Create a new atom. For debugging purposes it is recommended to give it a name.
   * The onBecomeObserved and onBecomeUnobserved callbacks can be used for resource management.
   */
  constructor(name_ = true ? "Atom@" + getNextId() : "Atom") {
    this.name_ = void 0;
    this.flags_ = 0;
    this.observers_ = /* @__PURE__ */ new Set();
    this.lastAccessedBy_ = 0;
    this.lowestObserverState_ = -1;
    this.onBOL = void 0;
    this.onBUOL = void 0;
    this.name_ = name_;
  }
  // for effective unobserving. BaseAtom has true, for extra optimization, so its onBecomeUnobserved never gets called, because it's not needed
  get isBeingObserved() {
    return getFlag(
      this.flags_,
      1
      /* AtomFlags.isBeingObserved */
    );
  }
  set isBeingObserved(newValue) {
    this.flags_ = setFlag(this.flags_, 1, newValue);
  }
  get isPendingUnobservation() {
    return getFlag(
      this.flags_,
      2
      /* AtomFlags.isPendingUnobservation */
    );
  }
  set isPendingUnobservation(newValue) {
    this.flags_ = setFlag(this.flags_, 2, newValue);
  }
  get diffValue() {
    return getFlag(
      this.flags_,
      4
      /* AtomFlags.diffValue */
    ) ? 1 : 0;
  }
  set diffValue(newValue) {
    this.flags_ = setFlag(this.flags_, 4, newValue === 1 ? true : false);
  }
  onBO() {
    if (this.onBOL) {
      this.onBOL.forEach((listener) => listener());
    }
  }
  onBUO() {
    if (this.onBUOL) {
      this.onBUOL.forEach((listener) => listener());
    }
  }
  /**
   * Invoke this method to notify mobx that your atom has been used somehow.
   * Returns true if there is currently a reactive context.
   */
  reportObserved() {
    return reportObserved(this);
  }
  /**
   * Invoke this method _after_ this method has changed to signal mobx that all its observers should invalidate.
   */
  reportChanged() {
    startBatch();
    propagateChanged(this);
    endBatch();
  }
  toString() {
    return this.name_;
  }
};
var isAtom = /* @__PURE__ */ createInstanceofPredicate("Atom", Atom);
function createAtom(name, onBecomeObservedHandler = noop, onBecomeUnobservedHandler = noop) {
  const atom = new Atom(name);
  if (onBecomeObservedHandler !== noop) {
    atom.onBOL = /* @__PURE__ */ new Set([onBecomeObservedHandler]);
  }
  if (onBecomeUnobservedHandler !== noop) {
    atom.onBUOL = /* @__PURE__ */ new Set([onBecomeUnobservedHandler]);
  }
  return atom;
}
function compareIdentity(a, b) {
  return a === b;
}
function compareStructural(a, b) {
  return deepEqual(a, b);
}
function compareShallow(a, b) {
  return deepEqual(a, b, 1);
}
var compareDefault = Object.is;
function deepEnhancer(v, _, name) {
  if (isObservable(v)) {
    return v;
  }
  if (Array.isArray(v)) {
    return observable.array(v, {
      name
    });
  }
  if (isPlainObject(v)) {
    return observable.object(v, void 0, {
      name
    });
  }
  if (isES6Map(v)) {
    return observable.map(v, {
      name
    });
  }
  if (isES6Set(v)) {
    return observable.set(v, {
      name
    });
  }
  if (typeof v === "function" && !isAction(v) && !isFlow(v)) {
    if (isGenerator(v)) {
      return flow(v);
    } else {
      return autoAction(name, v);
    }
  }
  return v;
}
function shallowEnhancer(v, _, name) {
  if (v === void 0 || v === null) {
    return v;
  }
  if (isObservableObject(v) || isObservableArray(v) || isObservableMap(v) || isObservableSet(v)) {
    return v;
  }
  if (Array.isArray(v)) {
    return observable.array(v, {
      name,
      deep: false
    });
  }
  if (isPlainObject(v)) {
    return observable.object(v, void 0, {
      name,
      deep: false
    });
  }
  if (isES6Map(v)) {
    return observable.map(v, {
      name,
      deep: false
    });
  }
  if (isES6Set(v)) {
    return observable.set(v, {
      name,
      deep: false
    });
  }
  if (true) {
    die("The shallow modifier / decorator can only used in combination with arrays, objects, maps and sets");
  }
}
function referenceEnhancer(newValue) {
  return newValue;
}
function refStructEnhancer(v, oldValue) {
  if (isObservable(v)) {
    die(`observable.struct should not be used with observable values`);
  }
  if (deepEqual(v, oldValue)) {
    return oldValue;
  }
  return v;
}
var OVERRIDE = "override";
var override = {
  annotationType_: OVERRIDE,
  make_: make_$6,
  extend_: extend_$5
};
function isOverride(annotation) {
  return annotation.annotationType_ === OVERRIDE;
}
function make_$6(adm, key) {
  if (adm.isPlainObject_) {
    die(`Cannot apply '${this.annotationType_}' to '${adm.name_}.${key.toString()}':
'${this.annotationType_}' cannot be used on plain objects.`);
  }
  if (!hasProp(adm.appliedAnnotations_, key)) {
    die(`'${adm.name_}.${key.toString()}' is annotated with '${this.annotationType_}', but no such annotated member was found on prototype.`);
  }
  return 0;
}
function extend_$5(adm, key, descriptor, proxyTrap) {
  die(44, this.annotationType_);
}
function createActionAnnotation(name, options) {
  return {
    annotationType_: name,
    options_: options,
    make_: make_$5,
    extend_: extend_$4
  };
}
function make_$5(adm, key, descriptor, source) {
  var _this$options_;
  if ((_this$options_ = this.options_) != null && _this$options_.bound) {
    return this.extend_(adm, key, descriptor, false) === null ? 0 : 1;
  }
  if (source === adm.target_) {
    return this.extend_(adm, key, descriptor, false) === null ? 0 : 2;
  }
  if (isAction(descriptor.value)) {
    return 1;
  }
  const actionDescriptor = createActionDescriptor(adm, this, key, descriptor, false);
  defineProperty(source, key, actionDescriptor);
  return 2;
}
function extend_$4(adm, key, descriptor, proxyTrap) {
  const actionDescriptor = createActionDescriptor(adm, this, key, descriptor);
  return adm.defineProperty_(key, actionDescriptor, proxyTrap);
}
function decorateAction20223_(annotation, mthd, context) {
  if (true) {
    assert20223DecoratorType(context, ["method", "field"]);
  }
  const {
    kind,
    name,
    addInitializer
  } = context;
  const ann = annotation;
  const _createAction = (m) => {
    var _ann$options_$name, _ann$options_, _ann$options_$autoAct, _ann$options_2;
    return createAction((_ann$options_$name = (_ann$options_ = ann.options_) == null ? void 0 : _ann$options_.name) != null ? _ann$options_$name : name.toString(), m, (_ann$options_$autoAct = (_ann$options_2 = ann.options_) == null ? void 0 : _ann$options_2.autoAction) != null ? _ann$options_$autoAct : false);
  };
  if (kind == "field") {
    return function(initMthd) {
      var _ann$options_3;
      let mthd2 = initMthd;
      if (!isAction(mthd2)) {
        mthd2 = _createAction(mthd2);
      }
      if ((_ann$options_3 = ann.options_) != null && _ann$options_3.bound) {
        mthd2 = mthd2.bind(this);
        mthd2.isMobxAction = true;
      }
      return mthd2;
    };
  }
  if (kind == "method") {
    var _ann$options_4;
    if (!isAction(mthd)) {
      mthd = _createAction(mthd);
    }
    if ((_ann$options_4 = ann.options_) != null && _ann$options_4.bound) {
      addInitializer(function() {
        const self = this;
        const bound = self[name].bind(self);
        bound.isMobxAction = true;
        self[name] = bound;
      });
    }
    return mthd;
  }
  die(43, ann.annotationType_, String(name), kind);
}
function assertActionDescriptor(adm, {
  annotationType_
}, key, {
  value
}) {
  if (!isFunction(value)) {
    die(`Cannot apply '${annotationType_}' to '${adm.name_}.${key.toString()}':
'${annotationType_}' can only be used on properties with a function value.`);
  }
}
function createActionDescriptor(adm, annotation, key, descriptor, safeDescriptors = globalState.safeDescriptors) {
  var _annotation$options_, _annotation$options_$, _annotation$options_2, _annotation$options_$2, _annotation$options_3, _annotation$options_4, _adm$proxy_2;
  assertActionDescriptor(adm, annotation, key, descriptor);
  let {
    value
  } = descriptor;
  if ((_annotation$options_ = annotation.options_) != null && _annotation$options_.bound) {
    var _adm$proxy_;
    value = value.bind((_adm$proxy_ = adm.proxy_) != null ? _adm$proxy_ : adm.target_);
  }
  return {
    value: createAction(
      (_annotation$options_$ = (_annotation$options_2 = annotation.options_) == null ? void 0 : _annotation$options_2.name) != null ? _annotation$options_$ : key.toString(),
      value,
      (_annotation$options_$2 = (_annotation$options_3 = annotation.options_) == null ? void 0 : _annotation$options_3.autoAction) != null ? _annotation$options_$2 : false,
      // https://github.com/mobxjs/mobx/discussions/3140
      (_annotation$options_4 = annotation.options_) != null && _annotation$options_4.bound ? (_adm$proxy_2 = adm.proxy_) != null ? _adm$proxy_2 : adm.target_ : void 0
    ),
    // Non-configurable for classes
    // prevents accidental field redefinition in subclass
    configurable: safeDescriptors ? adm.isPlainObject_ : true,
    // https://github.com/mobxjs/mobx/pull/2641#issuecomment-737292058
    enumerable: false,
    // Non-obsevable, therefore non-writable
    // Also prevents rewriting in subclass constructor
    writable: safeDescriptors ? false : true
  };
}
function createFlowAnnotation(name, options) {
  return {
    annotationType_: name,
    options_: options,
    make_: make_$4,
    extend_: extend_$3
  };
}
function make_$4(adm, key, descriptor, source) {
  var _this$options_;
  if (source === adm.target_) {
    return this.extend_(adm, key, descriptor, false) === null ? 0 : 2;
  }
  if ((_this$options_ = this.options_) != null && _this$options_.bound && (!hasProp(adm.target_, key) || !isFlow(adm.target_[key]))) {
    if (this.extend_(adm, key, descriptor, false) === null) {
      return 0;
    }
  }
  if (isFlow(descriptor.value)) {
    return 1;
  }
  const flowDescriptor = createFlowDescriptor(adm, this, key, descriptor, false, false);
  defineProperty(source, key, flowDescriptor);
  return 2;
}
function extend_$3(adm, key, descriptor, proxyTrap) {
  var _this$options_2;
  const flowDescriptor = createFlowDescriptor(adm, this, key, descriptor, (_this$options_2 = this.options_) == null ? void 0 : _this$options_2.bound);
  return adm.defineProperty_(key, flowDescriptor, proxyTrap);
}
function decorateFlow20223_(annotation, mthd, context) {
  var _annotation$options_;
  if (true) {
    assert20223DecoratorType(context, ["method"]);
  }
  const {
    name,
    addInitializer
  } = context;
  if (!isFlow(mthd)) {
    mthd = flow(mthd);
  }
  if ((_annotation$options_ = annotation.options_) != null && _annotation$options_.bound) {
    addInitializer(function() {
      const self = this;
      const bound = self[name].bind(self);
      bound.isMobXFlow = true;
      self[name] = bound;
    });
  }
  return mthd;
}
function assertFlowDescriptor(adm, {
  annotationType_
}, key, {
  value
}) {
  if (!isFunction(value)) {
    die(`Cannot apply '${annotationType_}' to '${adm.name_}.${key.toString()}':
'${annotationType_}' can only be used on properties with a generator function value.`);
  }
}
function createFlowDescriptor(adm, annotation, key, descriptor, bound, safeDescriptors = globalState.safeDescriptors) {
  assertFlowDescriptor(adm, annotation, key, descriptor);
  let {
    value
  } = descriptor;
  if (!isFlow(value)) {
    value = flow(value);
  }
  if (bound) {
    var _adm$proxy_;
    value = value.bind((_adm$proxy_ = adm.proxy_) != null ? _adm$proxy_ : adm.target_);
    value.isMobXFlow = true;
  }
  return {
    value,
    // Non-configurable for classes
    // prevents accidental field redefinition in subclass
    configurable: safeDescriptors ? adm.isPlainObject_ : true,
    // https://github.com/mobxjs/mobx/pull/2641#issuecomment-737292058
    enumerable: false,
    // Non-obsevable, therefore non-writable
    // Also prevents rewriting in subclass constructor
    writable: safeDescriptors ? false : true
  };
}
function createComputedAnnotation(name, options) {
  return {
    annotationType_: name,
    options_: options,
    make_: make_$3,
    extend_: extend_$2
  };
}
function make_$3(adm, key, descriptor) {
  return this.extend_(adm, key, descriptor, false) === null ? 0 : 1;
}
function extend_$2(adm, key, descriptor, proxyTrap) {
  assertComputedDescriptor(adm, this, key, descriptor);
  return adm.defineComputedProperty_(key, assign({}, this.options_, {
    get: descriptor.get,
    set: descriptor.set
  }), proxyTrap);
}
function decorateComputed20223_(annotation, get2, context) {
  if (true) {
    assert20223DecoratorType(context, ["getter"]);
  }
  const ann = annotation;
  const {
    name: key,
    addInitializer
  } = context;
  let computedValues;
  function createComputedValue(target, adm) {
    const options = assign({}, ann.options_, {
      get: get2,
      context: target
    });
    options.name || (options.name = true ? `${adm.name_}.${key.toString()}` : `ObservableObject.${key.toString()}`);
    return new ComputedValue(options);
  }
  addInitializer(function() {
    var _adm$lazyComputedKeys;
    const adm = asObservableObject(this)[$mobx];
    const target = this;
    const observable2 = adm.values_.get(key);
    if (observable2 instanceof ComputedValue && observable2.derivation !== get2) {
      adm.values_.delete(key);
    }
    ((_adm$lazyComputedKeys = adm.lazyComputedKeys_) != null ? _adm$lazyComputedKeys : adm.lazyComputedKeys_ = /* @__PURE__ */ new Map()).set(key, () => createComputedValue(target, adm));
  });
  return function() {
    const adm = this[$mobx];
    const observable2 = adm.values_.get(key);
    if (observable2 instanceof ComputedValue && observable2.derivation !== get2) {
      var _computedValues;
      let computed3 = (_computedValues = computedValues) == null ? void 0 : _computedValues.get(this);
      if (!computed3) {
        var _computedValues2;
        computed3 = createComputedValue(this, adm);
        ((_computedValues2 = computedValues) != null ? _computedValues2 : computedValues = /* @__PURE__ */ new WeakMap()).set(this, computed3);
      }
      return computed3.get();
    }
    return adm.getObservablePropValue_(key);
  };
}
function assertComputedDescriptor(adm, {
  annotationType_
}, key, {
  get: get2
}) {
  if (!get2) {
    die(`Cannot apply '${annotationType_}' to '${adm.name_}.${key.toString()}':
'${annotationType_}' can only be used on getter(+setter) properties.`);
  }
}
function createObservableAnnotation(name, options) {
  return {
    annotationType_: name,
    options_: options,
    make_: make_$2,
    extend_: extend_$1
  };
}
function make_$2(adm, key, descriptor) {
  return this.extend_(adm, key, descriptor, false) === null ? 0 : 1;
}
function extend_$1(adm, key, descriptor, proxyTrap) {
  var _this$options_$enhanc, _this$options_;
  assertObservableDescriptor(adm, this, key, descriptor);
  return adm.defineObservableProperty_(key, descriptor.value, (_this$options_$enhanc = (_this$options_ = this.options_) == null ? void 0 : _this$options_.enhancer_) != null ? _this$options_$enhanc : deepEnhancer, proxyTrap);
}
function decorateObservable20223_(annotation, desc, context) {
  if (true) {
    if (context.kind === "field") {
      throw die(`Please use \`@observable accessor ${String(context.name)}\` instead of \`@observable ${String(context.name)}\``);
    }
    assert20223DecoratorType(context, ["accessor"]);
  }
  const ann = annotation;
  const {
    kind,
    name
  } = context;
  if (kind !== "accessor") {
    return;
  }
  function registerLazy(target, value) {
    var _adm$lazyObservableKe;
    const adm = asObservableObject(target)[$mobx];
    ((_adm$lazyObservableKe = adm.lazyObservableKeys_) != null ? _adm$lazyObservableKe : adm.lazyObservableKeys_ = /* @__PURE__ */ new Map()).set(name, () => {
      var _ann$options_$enhance, _ann$options_;
      return new ObservableValue(value, (_ann$options_$enhance = (_ann$options_ = ann.options_) == null ? void 0 : _ann$options_.enhancer_) != null ? _ann$options_$enhance : deepEnhancer, true ? `${adm.name_}.${name.toString()}` : `ObservableObject.${name.toString()}`, false);
    });
    return adm;
  }
  return {
    get() {
      var _this$$mobx;
      const adm = (_this$$mobx = this[$mobx]) != null ? _this$$mobx : registerLazy(this, desc.get.call(this));
      return adm.getObservablePropValue_(name);
    },
    set(value) {
      var _this$$mobx2;
      const adm = (_this$$mobx2 = this[$mobx]) != null ? _this$$mobx2 : registerLazy(this, value);
      return adm.setObservablePropValue_(name, value);
    },
    init(value) {
      registerLazy(this, value);
      return value;
    }
  };
}
function assertObservableDescriptor(adm, {
  annotationType_
}, key, descriptor) {
  if (!("value" in descriptor)) {
    die(`Cannot apply '${annotationType_}' to '${adm.name_}.${key.toString()}':
'${annotationType_}' cannot be used on getter/setter properties`);
  }
}
var AUTO = "true";
var autoAnnotation = /* @__PURE__ */ createAutoAnnotation();
function createAutoAnnotation(options) {
  return {
    annotationType_: AUTO,
    options_: options,
    make_: make_$1,
    extend_
  };
}
function make_$1(adm, key, descriptor, source) {
  var _this$options_3, _this$options_4;
  if (descriptor.get) {
    return computed.make_(adm, key, descriptor, source);
  }
  if (descriptor.set) {
    const set2 = isAction(descriptor.set) ? descriptor.set : createAction(key.toString(), descriptor.set);
    if (source === adm.target_) {
      return adm.defineProperty_(key, {
        configurable: globalState.safeDescriptors ? adm.isPlainObject_ : true,
        set: set2
      }) === null ? 0 : 2;
    }
    defineProperty(source, key, {
      configurable: true,
      set: set2
    });
    return 2;
  }
  if (source !== adm.target_ && typeof descriptor.value === "function") {
    var _this$options_2;
    if (isGenerator(descriptor.value)) {
      var _this$options_;
      const flowAnnotation2 = (_this$options_ = this.options_) != null && _this$options_.autoBind ? flowBound : flow;
      return flowAnnotation2.make_(adm, key, descriptor, source);
    }
    const actionAnnotation2 = (_this$options_2 = this.options_) != null && _this$options_2.autoBind ? autoActionBound : autoAction;
    return actionAnnotation2.make_(adm, key, descriptor, source);
  }
  let observableAnnotation2 = ((_this$options_3 = this.options_) == null ? void 0 : _this$options_3.deep) === false ? observableRef : observable;
  if (typeof descriptor.value === "function" && (_this$options_4 = this.options_) != null && _this$options_4.autoBind) {
    var _adm$proxy_;
    descriptor.value = descriptor.value.bind((_adm$proxy_ = adm.proxy_) != null ? _adm$proxy_ : adm.target_);
  }
  return observableAnnotation2.make_(adm, key, descriptor, source);
}
function extend_(adm, key, descriptor, proxyTrap) {
  var _this$options_5, _this$options_6;
  if (descriptor.get) {
    return computed.extend_(adm, key, descriptor, proxyTrap);
  }
  if (descriptor.set) {
    return adm.defineProperty_(key, {
      configurable: globalState.safeDescriptors ? adm.isPlainObject_ : true,
      set: createAction(key.toString(), descriptor.set)
    }, proxyTrap);
  }
  if (typeof descriptor.value === "function" && (_this$options_5 = this.options_) != null && _this$options_5.autoBind) {
    var _adm$proxy_2;
    descriptor.value = descriptor.value.bind((_adm$proxy_2 = adm.proxy_) != null ? _adm$proxy_2 : adm.target_);
  }
  let observableAnnotation2 = ((_this$options_6 = this.options_) == null ? void 0 : _this$options_6.deep) === false ? observableRef : observable;
  return observableAnnotation2.extend_(adm, key, descriptor, proxyTrap);
}
function createDecoratorAnnotation(annotation, decorate) {
  return assign(function decoratorAnnotation(value, context) {
    if (context && typeof context.kind === "string") {
      return decorate(annotation, value, context);
    }
    if (true) {
      die(`Invalid arguments for \`${annotation.annotationType_}\``);
    }
    return void 0;
  }, annotation);
}
var OBSERVABLE = "observable";
var OBSERVABLE_REF = "observable.ref";
var OBSERVABLE_SHALLOW = "observable.shallow";
var OBSERVABLE_STRUCT = "observable.struct";
var defaultCreateObservableOptions = {
  deep: true,
  name: void 0,
  defaultDecorator: void 0
};
Object.freeze(defaultCreateObservableOptions);
function asCreateObservableOptions(thing) {
  return thing || defaultCreateObservableOptions;
}
var observableAnnotation = /* @__PURE__ */ createObservableAnnotation(OBSERVABLE);
var observableRefAnnotation = /* @__PURE__ */ createObservableAnnotation(OBSERVABLE_REF, {
  enhancer_: referenceEnhancer
});
var observableShallowAnnotation = /* @__PURE__ */ createObservableAnnotation(OBSERVABLE_SHALLOW, {
  enhancer_: shallowEnhancer
});
var observableStructAnnotation = /* @__PURE__ */ createObservableAnnotation(OBSERVABLE_STRUCT, {
  enhancer_: refStructEnhancer
});
function createObservableDecoratorAnnotation(annotation) {
  return createDecoratorAnnotation(annotation, decorateObservable20223_);
}
function getEnhancerFromOptions(options) {
  return options.deep === true ? deepEnhancer : options.deep === false ? referenceEnhancer : getEnhancerFromAnnotation(options.defaultDecorator);
}
function getAnnotationFromOptions(options) {
  var _options$defaultDecor;
  return options ? (_options$defaultDecor = options.defaultDecorator) != null ? _options$defaultDecor : createAutoAnnotation(options) : void 0;
}
function getEnhancerFromAnnotation(annotation) {
  var _annotation$options_$, _annotation$options_;
  return !annotation ? deepEnhancer : (_annotation$options_$ = (_annotation$options_ = annotation.options_) == null ? void 0 : _annotation$options_.enhancer_) != null ? _annotation$options_$ : deepEnhancer;
}
function createObservable(v, arg2, arg3) {
  if (arg2 && typeof arg2.kind === "string") {
    return decorateObservable20223_(observableAnnotation, v, arg2);
  }
  if (isObservable(v)) {
    return v;
  }
  if (isPlainObject(v)) {
    return observable.object(v, arg2, arg3);
  }
  if (Array.isArray(v)) {
    return observable.array(v, arg2);
  }
  if (isES6Map(v)) {
    return observable.map(v, arg2);
  }
  if (isES6Set(v)) {
    return observable.set(v, arg2);
  }
  if (typeof v === "object" && v !== null) {
    return v;
  }
  return observable.box(v, arg2);
}
var observableFactories = {
  box(value, options) {
    const o = asCreateObservableOptions(options);
    return new ObservableValue(value, getEnhancerFromOptions(o), o.name, true, o.equals);
  },
  array(initialValues, options) {
    const o = asCreateObservableOptions(options);
    return createObservableArray(initialValues, getEnhancerFromOptions(o), o.name);
  },
  map(initialValues, options) {
    const o = asCreateObservableOptions(options);
    return new ObservableMap(initialValues, getEnhancerFromOptions(o), o.name);
  },
  set(initialValues, options) {
    const o = asCreateObservableOptions(options);
    return new ObservableSet(initialValues, getEnhancerFromOptions(o), o.name);
  },
  object(props, annotations, options) {
    return initObservable(() => extendObservable(asDynamicObservableObject({}, options), props, annotations));
  }
};
var observableRef = /* @__PURE__ */ createObservableDecoratorAnnotation(observableRefAnnotation);
var observableShallow = /* @__PURE__ */ createObservableDecoratorAnnotation(observableShallowAnnotation);
var observableDeep = /* @__PURE__ */ createObservableDecoratorAnnotation(observableAnnotation);
var observableStruct = /* @__PURE__ */ createObservableDecoratorAnnotation(observableStructAnnotation);
var observable = /* @__PURE__ */ assign(createObservable, observableAnnotation, observableFactories);
var COMPUTED = "computed";
var COMPUTED_STRUCT = "computed.struct";
function createComputedDecoratorAnnotation(annotation) {
  return createDecoratorAnnotation(annotation, decorateComputed20223_);
}
var computedAnnotation = /* @__PURE__ */ createComputedAnnotation(COMPUTED);
var computedStructAnnotation = /* @__PURE__ */ createComputedAnnotation(COMPUTED_STRUCT, {
  equals: compareStructural
});
var computedStruct = /* @__PURE__ */ createComputedDecoratorAnnotation(computedStructAnnotation);
var computed = function computed2(arg1, arg2) {
  if (arg2 && typeof arg2.kind === "string") {
    return decorateComputed20223_(computedAnnotation, arg1, arg2);
  }
  if (isPlainObject(arg1)) {
    return createComputedDecoratorAnnotation(createComputedAnnotation(COMPUTED, arg1));
  }
  if (true) {
    if (!isFunction(arg1)) {
      die("First argument to `computed` should be an expression.");
    }
    if (isFunction(arg2)) {
      die("A setter as second argument is no longer supported, use `{ set: fn }` option instead");
    }
  }
  const opts = isPlainObject(arg2) ? arg2 : {};
  opts.get = arg1;
  opts.name || (opts.name = arg1.name || "");
  return new ComputedValue(opts);
};
assign(computed, computedAnnotation);
var _getDescriptor$config;
var _getDescriptor;
var currentActionId = 0;
var nextActionId = 1;
var isFunctionNameConfigurable = (_getDescriptor$config = (_getDescriptor = /* @__PURE__ */ getDescriptor(() => {
}, "name")) == null ? void 0 : _getDescriptor.configurable) != null ? _getDescriptor$config : false;
var tmpNameDescriptor = {
  value: "action",
  configurable: true,
  writable: false,
  enumerable: false
};
function createAction(actionName, fn, autoAction2 = false, ref) {
  if (true) {
    if (!isFunction(fn)) {
      die("`action` can only be invoked on functions");
    }
    if (typeof actionName !== "string" || !actionName) {
      die(`actions should have valid names, got: '${actionName}'`);
    }
  }
  function res() {
    return executeAction(actionName, autoAction2, fn, ref || this, arguments);
  }
  res.isMobxAction = true;
  res.toString = () => fn.toString();
  if (isFunctionNameConfigurable) {
    tmpNameDescriptor.value = actionName;
    defineProperty(res, "name", tmpNameDescriptor);
  }
  return res;
}
function executeAction(actionName, canRunAsDerivation, fn, scope, args) {
  const runInfo = _startAction(actionName, canRunAsDerivation, scope, args);
  try {
    return fn.apply(scope, args);
  } catch (err) {
    runInfo.error_ = err;
    throw err;
  } finally {
    _endAction(runInfo);
  }
}
function _startAction(actionName, canRunAsDerivation, scope, args) {
  const notifySpy_ = isSpyEnabled() && !!actionName;
  let startTime_ = 0;
  if (notifySpy_) {
    startTime_ = Date.now();
    const flattenedArgs = args ? Array.from(args) : EMPTY_ARRAY;
    spyReportStart({
      type: ACTION,
      name: actionName,
      object: scope,
      arguments: flattenedArgs
    });
  }
  const prevDerivation_ = globalState.trackingDerivation;
  const runAsAction = !canRunAsDerivation || !prevDerivation_;
  startBatch();
  let prevAllowStateChanges_ = globalState.allowStateChanges;
  if (runAsAction) {
    untrackedStart();
    if (true) {
      prevAllowStateChanges_ = allowStateChangesStart(true);
    }
  }
  const prevAllowStateReads_ = globalState.allowStateReads;
  if (true) {
    allowStateReadsStart(true);
  }
  const runInfo = {
    runAsAction_: runAsAction,
    prevDerivation_,
    prevAllowStateChanges_,
    prevAllowStateReads_,
    notifySpy_,
    startTime_,
    actionId_: nextActionId++,
    parentActionId_: currentActionId
  };
  currentActionId = runInfo.actionId_;
  return runInfo;
}
function _endAction(runInfo) {
  if (currentActionId !== runInfo.actionId_) {
    die(30);
  }
  currentActionId = runInfo.parentActionId_;
  if (runInfo.error_ !== void 0) {
    globalState.suppressReactionErrors = true;
  }
  if (true) {
    allowStateChangesEnd(runInfo.prevAllowStateChanges_);
    allowStateReadsEnd(runInfo.prevAllowStateReads_);
  }
  endBatch();
  if (runInfo.runAsAction_) {
    untrackedEnd(runInfo.prevDerivation_);
  }
  if (runInfo.notifySpy_) {
    spyReportEnd({
      time: Date.now() - runInfo.startTime_
    });
  }
  globalState.suppressReactionErrors = false;
}
function allowStateChanges(allowStateChanges2, func) {
  const prev = allowStateChangesStart(allowStateChanges2);
  try {
    return func();
  } finally {
    allowStateChangesEnd(prev);
  }
}
function allowStateChangesStart(allowStateChanges2) {
  const prev = globalState.allowStateChanges;
  globalState.allowStateChanges = allowStateChanges2;
  return prev;
}
function allowStateChangesEnd(prev) {
  globalState.allowStateChanges = prev;
}
var CREATE = "create";
var ObservableValue = class extends Atom {
  constructor(value, enhancer_, name_ = true ? "ObservableValue@" + getNextId() : "ObservableValue", notifySpy = true, equals_ = compareDefault) {
    super(name_);
    this.enhancer_ = void 0;
    this.name_ = void 0;
    this.equals_ = void 0;
    this.hasUnreportedChange_ = false;
    this.interceptors_ = void 0;
    this.changeListeners_ = void 0;
    this.value_ = void 0;
    this.dehancer = void 0;
    this.enhancer_ = enhancer_;
    this.name_ = name_;
    this.equals_ = equals_;
    this.value_ = enhancer_(value, void 0, name_);
    if (notifySpy && isSpyEnabled()) {
      var _this$value_;
      spyReport({
        type: CREATE,
        object: this,
        observableKind: "value",
        debugObjectName: this.name_,
        newValue: "" + ((_this$value_ = this.value_) == null ? void 0 : _this$value_.toString())
      });
    }
  }
  dehanceValue(value) {
    if (this.dehancer !== void 0) {
      return this.dehancer(value);
    }
    return value;
  }
  set(newValue) {
    const oldValue = this.value_;
    newValue = this.prepareNewValue_(newValue);
    if (newValue !== globalState.UNCHANGED) {
      const notifySpy = isSpyEnabled();
      if (notifySpy) {
        spyReportStart({
          type: UPDATE,
          object: this,
          observableKind: "value",
          debugObjectName: this.name_,
          newValue,
          oldValue
        });
      }
      this.setNewValue_(newValue);
      if (notifySpy) {
        spyReportEnd();
      }
    }
  }
  prepareNewValue_(newValue) {
    checkIfStateModificationsAreAllowed(this);
    if (hasInterceptors(this)) {
      const change = interceptChange(this, {
        object: this,
        type: UPDATE,
        newValue
      });
      if (!change) {
        return globalState.UNCHANGED;
      }
      newValue = change.newValue;
    }
    newValue = this.enhancer_(newValue, this.value_, this.name_);
    return this.equals_(this.value_, newValue) ? globalState.UNCHANGED : newValue;
  }
  setNewValue_(newValue) {
    const oldValue = this.value_;
    this.value_ = newValue;
    this.reportChanged();
    if (hasListeners(this)) {
      notifyListeners(this, {
        type: UPDATE,
        object: this,
        newValue,
        oldValue
      });
    }
  }
  get() {
    this.reportObserved();
    return this.dehanceValue(this.value_);
  }
  raw() {
    return this.value_;
  }
  toJSON() {
    return this.get();
  }
  toString() {
    return `${this.name_}[${this.value_}]`;
  }
  valueOf() {
    return toPrimitive(this.get());
  }
  [Symbol.toPrimitive]() {
    return this.valueOf();
  }
};
var isObservableValue = /* @__PURE__ */ createInstanceofPredicate("ObservableValue", ObservableValue);
var ComputedValue = class {
  /**
   * Create a new computed value based on a function expression.
   *
   * The `name` property is for debug purposes only.
   *
   * The `equals` property specifies the comparer function used to determine if a newly produced
   * value differs from the previous value. Structural comparison can be convenient if you always
   * produce a new aggregated object and don't want to notify observers if it is structurally the same.
   * This is useful for working with vectors, mouse coordinates etc.
   */
  constructor(options) {
    this.dependenciesState_ = -1;
    this.observing_ = [];
    this.newObserving_ = null;
    this.observers_ = /* @__PURE__ */ new Set();
    this.runId_ = 0;
    this.lastAccessedBy_ = 0;
    this.lowestObserverState_ = 0;
    this.unboundDepsCount_ = 0;
    this.value_ = new CaughtException(null);
    this.name_ = void 0;
    this.triggeredBy_ = void 0;
    this.flags_ = 0;
    this.derivation = void 0;
    this.setter_ = void 0;
    this.scope_ = void 0;
    this.equals_ = void 0;
    this.requiresReaction_ = void 0;
    this.keepAlive_ = void 0;
    this.onBOL = void 0;
    this.onBUOL = void 0;
    if (!options.get) {
      die(31);
    }
    this.derivation = options.get;
    this.name_ = options.name || (true ? "ComputedValue@" + getNextId() : "ComputedValue");
    if (options.set) {
      this.setter_ = createAction(true ? this.name_ + "-setter" : "ComputedValue-setter", options.set);
    }
    this.equals_ = options.equals || compareDefault;
    this.scope_ = options.context;
    this.requiresReaction_ = options.requiresReaction;
    this.keepAlive_ = !!options.keepAlive;
  }
  onBecomeStale_() {
    propagateMaybeChanged(this);
  }
  onBO() {
    if (this.onBOL) {
      this.onBOL.forEach((listener) => listener());
    }
  }
  onBUO() {
    if (this.onBUOL) {
      this.onBUOL.forEach((listener) => listener());
    }
  }
  // to check for cycles
  get isComputing() {
    return getFlag(
      this.flags_,
      1
      /* ComputedValueFlags.isComputing */
    );
  }
  set isComputing(newValue) {
    this.flags_ = setFlag(this.flags_, 1, newValue);
  }
  get isRunningSetter() {
    return getFlag(
      this.flags_,
      2
      /* ComputedValueFlags.isRunningSetter */
    );
  }
  set isRunningSetter(newValue) {
    this.flags_ = setFlag(this.flags_, 2, newValue);
  }
  get isBeingObserved() {
    return getFlag(
      this.flags_,
      4
      /* ComputedValueFlags.isBeingObserved */
    );
  }
  set isBeingObserved(newValue) {
    this.flags_ = setFlag(this.flags_, 4, newValue);
  }
  get isPendingUnobservation() {
    return getFlag(
      this.flags_,
      8
      /* ComputedValueFlags.isPendingUnobservation */
    );
  }
  set isPendingUnobservation(newValue) {
    this.flags_ = setFlag(this.flags_, 8, newValue);
  }
  get diffValue() {
    return getFlag(
      this.flags_,
      16
      /* ComputedValueFlags.diffValue */
    ) ? 1 : 0;
  }
  set diffValue(newValue) {
    this.flags_ = setFlag(this.flags_, 16, newValue === 1 ? true : false);
  }
  /**
   * Returns the current value of this computed value.
   * Will evaluate its computation first if needed.
   */
  get() {
    if (this.isComputing) {
      die(32, this.name_, this.derivation);
    }
    if (globalState.inBatch === 0 && // !globalState.trackingDerivatpion &&
    this.observers_.size === 0 && !this.keepAlive_) {
      if (shouldCompute(this)) {
        this.warnAboutUntrackedRead_();
        startBatch();
        this.value_ = this.computeValue_(false);
        endBatch();
      }
    } else {
      reportObserved(this);
      if (shouldCompute(this)) {
        let prevTrackingContext = globalState.trackingContext;
        if (this.keepAlive_ && !prevTrackingContext) {
          globalState.trackingContext = this;
        }
        if (this.trackAndCompute()) {
          propagateChangeConfirmed(this);
        }
        globalState.trackingContext = prevTrackingContext;
      }
    }
    const result = this.value_;
    if (isCaughtException(result)) {
      throw result.cause;
    }
    return result;
  }
  set(value) {
    if (this.setter_) {
      if (this.isRunningSetter) {
        die(33, this.name_);
      }
      this.isRunningSetter = true;
      try {
        this.setter_.call(this.scope_, value);
      } finally {
        this.isRunningSetter = false;
      }
    } else {
      die(34, this.name_);
    }
  }
  trackAndCompute() {
    const oldValue = this.value_;
    const wasSuspended = (
      /* see #1208 */
      this.dependenciesState_ === -1
    );
    const newValue = this.computeValue_(true);
    const changed = wasSuspended || isCaughtException(oldValue) || isCaughtException(newValue) || !this.equals_(oldValue, newValue);
    if (changed) {
      this.value_ = newValue;
      if (isSpyEnabled()) {
        spyReport({
          observableKind: "computed",
          debugObjectName: this.name_,
          object: this.scope_,
          type: "update",
          oldValue,
          newValue
        });
      }
    }
    return changed;
  }
  computeValue_(track) {
    this.isComputing = true;
    const prev = true ? allowStateChangesStart(false) : false;
    let res;
    if (track) {
      res = trackDerivedFunction(this, this.derivation, this.scope_);
    } else {
      if (globalState.disableErrorBoundaries === true) {
        res = this.derivation.call(this.scope_);
      } else {
        try {
          res = this.derivation.call(this.scope_);
        } catch (e) {
          res = new CaughtException(e);
        }
      }
    }
    if (true) {
      allowStateChangesEnd(prev);
    }
    this.isComputing = false;
    return res;
  }
  suspend_() {
    if (!this.keepAlive_) {
      clearObserving(this);
      this.value_ = void 0;
    }
  }
  warnAboutUntrackedRead_() {
    if (false) {
      return;
    }
    if (typeof this.requiresReaction_ === "boolean" ? this.requiresReaction_ : globalState.computedRequiresReaction) {
      console.warn(`[mobx] Computed value '${this.name_}' is being read outside a reactive context. Doing a full recompute.`);
    }
  }
  toString() {
    return `${this.name_}[${this.derivation.toString()}]`;
  }
  valueOf() {
    return toPrimitive(this.get());
  }
  [Symbol.toPrimitive]() {
    return this.valueOf();
  }
};
var isComputedValue = /* @__PURE__ */ createInstanceofPredicate("ComputedValue", ComputedValue);
var CaughtException = class {
  constructor(cause) {
    this.cause = void 0;
    this.cause = cause;
  }
};
function isCaughtException(e) {
  return e instanceof CaughtException;
}
function shouldCompute(derivation) {
  switch (derivation.dependenciesState_) {
    case 0:
      return false;
    case -1:
    case 2:
      return true;
    case 1: {
      const prevAllowStateReads = true ? allowStateReadsStart(true) : true;
      const prevUntracked = untrackedStart();
      const obs = derivation.observing_, l = obs.length;
      for (let i = 0; i < l; i++) {
        const obj = obs[i];
        if (isComputedValue(obj)) {
          if (globalState.disableErrorBoundaries) {
            obj.get();
          } else {
            try {
              obj.get();
            } catch (e) {
              untrackedEnd(prevUntracked);
              if (true) {
                allowStateReadsEnd(prevAllowStateReads);
              }
              return true;
            }
          }
          if (derivation.dependenciesState_ === 2) {
            untrackedEnd(prevUntracked);
            if (true) {
              allowStateReadsEnd(prevAllowStateReads);
            }
            return true;
          }
        }
      }
      changeDependenciesStateTo0(derivation);
      untrackedEnd(prevUntracked);
      if (true) {
        allowStateReadsEnd(prevAllowStateReads);
      }
      return false;
    }
  }
}
function isComputingDerivation() {
  return globalState.trackingDerivation !== null;
}
function checkIfStateModificationsAreAllowed(atom) {
  if (false) {
    return;
  }
  const hasObservers2 = atom.observers_.size > 0;
  if (!globalState.allowStateChanges && (hasObservers2 || globalState.enforceActions === "always")) {
    console.warn("[MobX] " + (globalState.enforceActions ? "Since strict-mode is enabled, changing (observed) observable values without using an action is not allowed. Tried to modify: " : "Side effects like changing state are not allowed at this point. Are you trying to modify state from, for example, a computed value or the render function of a React component? You can wrap side effects in 'runInAction' (or decorate functions with 'action') if needed. Tried to modify: ") + atom.name_);
  }
}
function checkIfStateReadsAreAllowed(observable2) {
  if (!globalState.allowStateReads && globalState.observableRequiresReaction) {
    console.warn(`[mobx] Observable '${observable2.name_}' being read outside a reactive context.`);
  }
}
function trackDerivedFunction(derivation, f, context) {
  const prevAllowStateReads = true ? allowStateReadsStart(true) : true;
  changeDependenciesStateTo0(derivation);
  derivation.newObserving_ = new Array(
    // Reserve constant space for initial dependencies, dynamic space otherwise.
    // See https://github.com/mobxjs/mobx/pull/3833
    derivation.runId_ === 0 ? 100 : derivation.observing_.length
  );
  derivation.unboundDepsCount_ = 0;
  derivation.runId_ = ++globalState.runId;
  const prevTracking = globalState.trackingDerivation;
  globalState.trackingDerivation = derivation;
  globalState.inBatch++;
  let result;
  if (globalState.disableErrorBoundaries === true) {
    result = f.call(context);
  } else {
    try {
      result = f.call(context);
    } catch (e) {
      result = new CaughtException(e);
    }
  }
  globalState.inBatch--;
  globalState.trackingDerivation = prevTracking;
  bindDependencies(derivation);
  warnAboutDerivationWithoutDependencies(derivation);
  if (true) {
    allowStateReadsEnd(prevAllowStateReads);
  }
  return result;
}
function warnAboutDerivationWithoutDependencies(derivation) {
  if (false) {
    return;
  }
  if (derivation.observing_.length !== 0) {
    return;
  }
  if (typeof derivation.requiresObservable_ === "boolean" ? derivation.requiresObservable_ : globalState.reactionRequiresObservable) {
    console.warn(`[mobx] Derivation '${derivation.name_}' is created/updated without reading any observable value.`);
  }
}
function bindDependencies(derivation) {
  const prevObserving = derivation.observing_;
  const observing = derivation.observing_ = derivation.newObserving_;
  let lowestNewObservingDerivationState = 0;
  let i0 = 0, l = derivation.unboundDepsCount_;
  for (let i = 0; i < l; i++) {
    const dep = observing[i];
    if (dep.diffValue === 0) {
      dep.diffValue = 1;
      if (i0 !== i) {
        observing[i0] = dep;
      }
      i0++;
    }
    if (dep.dependenciesState_ > lowestNewObservingDerivationState) {
      lowestNewObservingDerivationState = dep.dependenciesState_;
    }
  }
  observing.length = i0;
  derivation.newObserving_ = null;
  l = prevObserving.length;
  while (l--) {
    const dep = prevObserving[l];
    if (dep.diffValue === 0) {
      removeObserver(dep, derivation);
    }
    dep.diffValue = 0;
  }
  while (i0--) {
    const dep = observing[i0];
    if (dep.diffValue === 1) {
      dep.diffValue = 0;
      addObserver(dep, derivation);
    }
  }
  if (lowestNewObservingDerivationState !== 0) {
    derivation.dependenciesState_ = lowestNewObservingDerivationState;
    derivation.onBecomeStale_();
  }
}
function clearObserving(derivation) {
  const obs = derivation.observing_;
  derivation.observing_ = [];
  let i = obs.length;
  while (i--) {
    removeObserver(obs[i], derivation);
  }
  derivation.dependenciesState_ = -1;
}
function untracked(action2) {
  const prev = untrackedStart();
  try {
    return action2();
  } finally {
    untrackedEnd(prev);
  }
}
function untrackedStart() {
  const prev = globalState.trackingDerivation;
  globalState.trackingDerivation = null;
  return prev;
}
function untrackedEnd(prev) {
  globalState.trackingDerivation = prev;
}
function allowStateReadsStart(allowStateReads) {
  const prev = globalState.allowStateReads;
  globalState.allowStateReads = allowStateReads;
  return prev;
}
function allowStateReadsEnd(prev) {
  globalState.allowStateReads = prev;
}
function changeDependenciesStateTo0(derivation) {
  if (derivation.dependenciesState_ === 0) {
    return;
  }
  derivation.dependenciesState_ = 0;
  const obs = derivation.observing_;
  let i = obs.length;
  while (i--) {
    obs[i].lowestObserverState_ = 0;
  }
}
var MOBX_GLOBALS_VERSION = 7;
var persistentKeys = ["mobxGuid", "spyListeners", "enforceActions", "computedRequiresReaction", "reactionRequiresObservable", "observableRequiresReaction", "allowStateReads", "disableErrorBoundaries", "runId", "UNCHANGED"];
var MobXGlobals = class {
  constructor() {
    this.version = MOBX_GLOBALS_VERSION;
    this.UNCHANGED = {};
    this.trackingDerivation = null;
    this.trackingContext = null;
    this.runId = 0;
    this.mobxGuid = 0;
    this.inBatch = 0;
    this.pendingUnobservations = [];
    this.pendingReactions = [];
    this.isRunningReactions = false;
    this.allowStateChanges = false;
    this.allowStateReads = true;
    this.enforceActions = true;
    this.spyListeners = [];
    this.globalReactionErrorHandlers = [];
    this.computedRequiresReaction = false;
    this.reactionRequiresObservable = false;
    this.observableRequiresReaction = false;
    this.disableErrorBoundaries = false;
    this.suppressReactionErrors = false;
    this.safeDescriptors = true;
  }
};
var canMergeGlobalState = true;
var isolateCalled = false;
var globalState = /* @__PURE__ */ (function() {
  let global = globalThis;
  if (global.__mobxInstanceCount > 0 && !global.__mobxGlobals) {
    canMergeGlobalState = false;
  }
  if (global.__mobxGlobals && global.__mobxGlobals.version !== MOBX_GLOBALS_VERSION) {
    canMergeGlobalState = false;
  }
  if (!canMergeGlobalState) {
    setTimeout(() => {
      if (!isolateCalled) {
        die(35);
      }
    }, 1);
    return new MobXGlobals();
  } else if (global.__mobxGlobals) {
    global.__mobxInstanceCount += 1;
    if (!global.__mobxGlobals.UNCHANGED) {
      global.__mobxGlobals.UNCHANGED = {};
    }
    return global.__mobxGlobals;
  } else {
    global.__mobxInstanceCount = 1;
    return global.__mobxGlobals = /* @__PURE__ */ new MobXGlobals();
  }
})();
function isolateGlobalState() {
  if (globalState.pendingReactions.length || globalState.inBatch || globalState.isRunningReactions) {
    die(36);
  }
  isolateCalled = true;
  if (canMergeGlobalState) {
    let global = globalThis;
    if (--global.__mobxInstanceCount === 0) {
      global.__mobxGlobals = void 0;
    }
    globalState = new MobXGlobals();
  }
}
function getGlobalState() {
  return globalState;
}
function resetGlobalState() {
  const defaultGlobals = new MobXGlobals();
  for (let key in defaultGlobals) {
    if (persistentKeys.indexOf(key) === -1) {
      globalState[key] = defaultGlobals[key];
    }
  }
  globalState.allowStateChanges = !globalState.enforceActions;
}
function hasObservers(observable2) {
  return observable2.observers_ && observable2.observers_.size > 0;
}
function getObservers(observable2) {
  return observable2.observers_;
}
function addObserver(observable2, node) {
  observable2.observers_.add(node);
  if (observable2.lowestObserverState_ > node.dependenciesState_) {
    observable2.lowestObserverState_ = node.dependenciesState_;
  }
}
function removeObserver(observable2, node) {
  observable2.observers_.delete(node);
  if (observable2.observers_.size === 0) {
    queueForUnobservation(observable2);
  }
}
function queueForUnobservation(observable2) {
  if (observable2.isPendingUnobservation === false) {
    observable2.isPendingUnobservation = true;
    globalState.pendingUnobservations.push(observable2);
  }
}
function startBatch() {
  globalState.inBatch++;
}
function endBatch() {
  if (--globalState.inBatch === 0) {
    runReactions();
    const list = globalState.pendingUnobservations;
    for (let i = 0; i < list.length; i++) {
      const observable2 = list[i];
      observable2.isPendingUnobservation = false;
      if (observable2.observers_.size === 0) {
        if (observable2.isBeingObserved) {
          observable2.isBeingObserved = false;
          observable2.onBUO();
        }
        if (observable2 instanceof ComputedValue) {
          observable2.suspend_();
        }
      }
    }
    globalState.pendingUnobservations = [];
  }
}
function reportObserved(observable2) {
  checkIfStateReadsAreAllowed(observable2);
  const derivation = globalState.trackingDerivation;
  if (derivation !== null) {
    if (derivation.runId_ !== observable2.lastAccessedBy_) {
      observable2.lastAccessedBy_ = derivation.runId_;
      derivation.newObserving_[derivation.unboundDepsCount_++] = observable2;
      if (!observable2.isBeingObserved && globalState.trackingContext) {
        observable2.isBeingObserved = true;
        observable2.onBO();
      }
    }
    return observable2.isBeingObserved;
  } else if (observable2.observers_.size === 0 && globalState.inBatch > 0) {
    queueForUnobservation(observable2);
  }
  return false;
}
function propagateChanged(observable2) {
  if (observable2.lowestObserverState_ === 2) {
    return;
  }
  observable2.lowestObserverState_ = 2;
  observable2.observers_.forEach((d) => {
    if (d.dependenciesState_ === 0) {
      d.onBecomeStale_();
    }
    d.dependenciesState_ = 2;
  });
}
function propagateChangeConfirmed(observable2) {
  if (observable2.lowestObserverState_ === 2) {
    return;
  }
  observable2.lowestObserverState_ = 2;
  observable2.observers_.forEach((d) => {
    if (d.dependenciesState_ === 1) {
      d.dependenciesState_ = 2;
    } else if (d.dependenciesState_ === 0) {
      observable2.lowestObserverState_ = 0;
    }
  });
}
function propagateMaybeChanged(observable2) {
  if (observable2.lowestObserverState_ !== 0) {
    return;
  }
  observable2.lowestObserverState_ = 1;
  observable2.observers_.forEach((d) => {
    if (d.dependenciesState_ === 0) {
      d.dependenciesState_ = 1;
      d.onBecomeStale_();
    }
  });
}
var Reaction = class {
  constructor(name_ = true ? "Reaction@" + getNextId() : "Reaction", onInvalidate_, errorHandler_, requiresObservable_) {
    this.name_ = void 0;
    this.onInvalidate_ = void 0;
    this.errorHandler_ = void 0;
    this.requiresObservable_ = void 0;
    this.observing_ = [];
    this.newObserving_ = [];
    this.dependenciesState_ = -1;
    this.runId_ = 0;
    this.unboundDepsCount_ = 0;
    this.flags_ = 0;
    this.name_ = name_;
    this.onInvalidate_ = onInvalidate_;
    this.errorHandler_ = errorHandler_;
    this.requiresObservable_ = requiresObservable_;
  }
  get isDisposed() {
    return getFlag(
      this.flags_,
      1
      /* ReactionFlags.isDisposed */
    );
  }
  set isDisposed(newValue) {
    this.flags_ = setFlag(this.flags_, 1, newValue);
  }
  get isScheduled() {
    return getFlag(
      this.flags_,
      2
      /* ReactionFlags.isScheduled */
    );
  }
  set isScheduled(newValue) {
    this.flags_ = setFlag(this.flags_, 2, newValue);
  }
  get isTrackPending() {
    return getFlag(
      this.flags_,
      4
      /* ReactionFlags.isTrackPending */
    );
  }
  set isTrackPending(newValue) {
    this.flags_ = setFlag(this.flags_, 4, newValue);
  }
  get isRunning() {
    return getFlag(
      this.flags_,
      8
      /* ReactionFlags.isRunning */
    );
  }
  set isRunning(newValue) {
    this.flags_ = setFlag(this.flags_, 8, newValue);
  }
  get diffValue() {
    return getFlag(
      this.flags_,
      16
      /* ReactionFlags.diffValue */
    ) ? 1 : 0;
  }
  set diffValue(newValue) {
    this.flags_ = setFlag(this.flags_, 16, newValue === 1 ? true : false);
  }
  onBecomeStale_() {
    this.schedule_();
  }
  schedule_() {
    if (!this.isScheduled) {
      this.isScheduled = true;
      globalState.pendingReactions.push(this);
      runReactions();
    }
  }
  /**
   * internal, use schedule() if you intend to kick off a reaction
   */
  runReaction_() {
    if (!this.isDisposed) {
      startBatch();
      this.isScheduled = false;
      const prev = globalState.trackingContext;
      globalState.trackingContext = this;
      if (shouldCompute(this)) {
        this.isTrackPending = true;
        try {
          this.onInvalidate_();
          if (this.isTrackPending && isSpyEnabled()) {
            spyReport({
              name: this.name_,
              type: "scheduled-reaction"
            });
          }
        } catch (e) {
          this.reportExceptionInDerivation_(e);
        }
      }
      globalState.trackingContext = prev;
      endBatch();
    }
  }
  track(fn) {
    if (this.isDisposed) {
      return;
    }
    startBatch();
    const notify = isSpyEnabled();
    let startTime;
    if (notify) {
      startTime = Date.now();
      spyReportStart({
        name: this.name_,
        type: "reaction"
      });
    }
    this.isRunning = true;
    const prevReaction = globalState.trackingContext;
    globalState.trackingContext = this;
    const result = trackDerivedFunction(this, fn, void 0);
    globalState.trackingContext = prevReaction;
    this.isRunning = false;
    this.isTrackPending = false;
    if (this.isDisposed) {
      clearObserving(this);
    }
    if (isCaughtException(result)) {
      this.reportExceptionInDerivation_(result.cause);
    }
    if (notify) {
      spyReportEnd({
        time: Date.now() - startTime
      });
    }
    endBatch();
  }
  reportExceptionInDerivation_(error) {
    if (this.errorHandler_) {
      this.errorHandler_(error, this);
      return;
    }
    if (globalState.disableErrorBoundaries) {
      throw error;
    }
    const message = true ? `[mobx] Encountered an uncaught exception that was thrown by a reaction or observer component, in: '${this}'` : `[mobx] uncaught error in '${this}'`;
    if (!globalState.suppressReactionErrors) {
      console.error(message, error);
    } else if (true) {
      console.warn(`[mobx] (error in reaction '${this.name_}' suppressed, fix error of causing action below)`);
    }
    if (isSpyEnabled()) {
      spyReport({
        type: "error",
        name: this.name_,
        message,
        error: "" + error
      });
    }
    globalState.globalReactionErrorHandlers.forEach((f) => f(error, this));
  }
  dispose() {
    if (!this.isDisposed) {
      this.isDisposed = true;
      if (!this.isRunning) {
        startBatch();
        clearObserving(this);
        endBatch();
      }
    }
  }
  getDisposer_(abortSignal) {
    const dispose = () => {
      this.dispose();
      abortSignal == null || abortSignal.removeEventListener == null || abortSignal.removeEventListener("abort", dispose);
    };
    abortSignal == null || abortSignal.addEventListener == null || abortSignal.addEventListener("abort", dispose);
    dispose[$mobx] = this;
    if ("dispose" in Symbol && typeof Symbol.dispose === "symbol") {
      dispose[Symbol.dispose] = dispose;
    }
    return dispose;
  }
  toString() {
    return `Reaction[${this.name_}]`;
  }
};
function onReactionError(handler) {
  globalState.globalReactionErrorHandlers.push(handler);
  return () => {
    const idx = globalState.globalReactionErrorHandlers.indexOf(handler);
    if (idx >= 0) {
      globalState.globalReactionErrorHandlers.splice(idx, 1);
    }
  };
}
var MAX_REACTION_ITERATIONS = 100;
var reactionScheduler = (f) => f();
function runReactions() {
  if (globalState.inBatch > 0 || globalState.isRunningReactions) {
    return;
  }
  reactionScheduler(runReactionsHelper);
}
function runReactionsHelper() {
  globalState.isRunningReactions = true;
  const allReactions = globalState.pendingReactions;
  let iterations = 0;
  while (allReactions.length > 0) {
    if (++iterations === MAX_REACTION_ITERATIONS) {
      console.error(true ? `Reaction doesn't converge to a stable state after ${MAX_REACTION_ITERATIONS} iterations. Probably there is a cycle in the reactive function: ${allReactions[0]}` : `[mobx] cycle in reaction: ${allReactions[0]}`);
      allReactions.splice(0);
    }
    let remainingReactions = allReactions.splice(0);
    for (let i = 0, l = remainingReactions.length; i < l; i++) {
      remainingReactions[i].runReaction_();
    }
  }
  globalState.isRunningReactions = false;
}
var isReaction = /* @__PURE__ */ createInstanceofPredicate("Reaction", Reaction);
function setReactionScheduler(fn) {
  const baseScheduler = reactionScheduler;
  reactionScheduler = (f) => fn(() => baseScheduler(f));
}
function isSpyEnabled() {
  return !!globalState.spyListeners.length;
}
function spyReport(event) {
  if (false) {
    return;
  }
  if (!globalState.spyListeners.length) {
    return;
  }
  const listeners = globalState.spyListeners;
  for (let i = 0, l = listeners.length; i < l; i++) {
    listeners[i](event);
  }
}
function spyReportStart(event) {
  if (false) {
    return;
  }
  const change = assign({}, event, {
    spyReportStart: true
  });
  spyReport(change);
}
var END_EVENT = {
  type: "report-end",
  spyReportEnd: true
};
function spyReportEnd(change) {
  if (false) {
    return;
  }
  if (change) {
    spyReport(assign({}, change, {
      type: "report-end",
      spyReportEnd: true
    }));
  } else {
    spyReport(END_EVENT);
  }
}
function spy(listener) {
  if (false) {
    console.warn(`[mobx.spy] Is a no-op in production builds`);
    return function() {
    };
  } else {
    globalState.spyListeners.push(listener);
    return once(() => {
      globalState.spyListeners = globalState.spyListeners.filter((l) => l !== listener);
    });
  }
}
var ACTION = "action";
var ACTION_BOUND = "action.bound";
var AUTOACTION = "autoAction";
var AUTOACTION_BOUND = "autoAction.bound";
var DEFAULT_ACTION_NAME = "<unnamed action>";
var actionAnnotation = /* @__PURE__ */ createActionAnnotation(ACTION);
var actionBoundAnnotation = /* @__PURE__ */ createActionAnnotation(ACTION_BOUND, {
  bound: true
});
var autoActionAnnotation = /* @__PURE__ */ createActionAnnotation(AUTOACTION, {
  autoAction: true
});
var autoActionBoundAnnotation = /* @__PURE__ */ createActionAnnotation(AUTOACTION_BOUND, {
  autoAction: true,
  bound: true
});
function createActionDecoratorAnnotation(annotation) {
  return createDecoratorAnnotation(annotation, decorateAction20223_);
}
function createActionFactory(autoAction2) {
  const res = function action2(arg1, arg2) {
    if (arg2 && typeof arg2.kind === "string") {
      return decorateAction20223_(autoAction2 ? autoActionAnnotation : actionAnnotation, arg1, arg2);
    }
    if (isFunction(arg1)) {
      return createAction(arg1.name || DEFAULT_ACTION_NAME, arg1, autoAction2);
    }
    if (isFunction(arg2)) {
      return createAction(arg1, arg2, autoAction2);
    }
    if (isStringish(arg1)) {
      return createActionDecoratorAnnotation(createActionAnnotation(autoAction2 ? AUTOACTION : ACTION, {
        name: arg1,
        autoAction: autoAction2
      }));
    }
    if (true) {
      die("Invalid arguments for `action`");
    }
  };
  return res;
}
var action = /* @__PURE__ */ createActionFactory(false);
assign(action, actionAnnotation);
var autoAction = /* @__PURE__ */ createActionFactory(true);
assign(autoAction, autoActionAnnotation);
var actionBound = /* @__PURE__ */ createActionDecoratorAnnotation(actionBoundAnnotation);
var autoActionBound = /* @__PURE__ */ createActionDecoratorAnnotation(autoActionBoundAnnotation);
function runInAction(fn) {
  return executeAction(fn.name || DEFAULT_ACTION_NAME, false, fn, this, void 0);
}
function isAction(thing) {
  return isFunction(thing) && thing.isMobxAction === true;
}
function autorun(view, opts = EMPTY_OBJECT) {
  var _opts$name, _opts$signal;
  if (true) {
    if (!isFunction(view)) {
      die("Autorun expects a function as first argument");
    }
    if (isAction(view)) {
      die("Autorun does not accept actions since actions are untrackable");
    }
  }
  const name = (_opts$name = opts == null ? void 0 : opts.name) != null ? _opts$name : true ? view.name || "Autorun@" + getNextId() : "Autorun";
  const runSync = !opts.scheduler && !opts.delay;
  let reaction2;
  if (runSync) {
    reaction2 = new Reaction(name, function() {
      this.track(reactionRunner);
    }, opts.onError, opts.requiresObservable);
  } else {
    const scheduler = createSchedulerFromOptions(opts);
    let isScheduled = false;
    reaction2 = new Reaction(name, () => {
      if (!isScheduled) {
        isScheduled = true;
        scheduler(() => {
          isScheduled = false;
          if (!reaction2.isDisposed) {
            reaction2.track(reactionRunner);
          }
        });
      }
    }, opts.onError, opts.requiresObservable);
  }
  function reactionRunner() {
    view(reaction2);
  }
  if (!(opts != null && (_opts$signal = opts.signal) != null && _opts$signal.aborted)) {
    reaction2.schedule_();
  }
  return reaction2.getDisposer_(opts == null ? void 0 : opts.signal);
}
var run = (f) => f();
function createSchedulerFromOptions(opts) {
  return opts.scheduler ? opts.scheduler : opts.delay ? (f) => setTimeout(f, opts.delay) : run;
}
function reaction(expression, effect, opts = EMPTY_OBJECT) {
  var _opts$name2, _opts$signal2;
  if (true) {
    if (!isFunction(expression) || !isFunction(effect)) {
      die("First and second argument to reaction should be functions");
    }
    if (!isPlainObject(opts)) {
      die("Third argument of reactions should be an object");
    }
  }
  const name = (_opts$name2 = opts.name) != null ? _opts$name2 : true ? "Reaction@" + getNextId() : "Reaction";
  const effectAction = action(name, opts.onError ? wrapErrorHandler(opts.onError, effect) : effect);
  const runSync = !opts.scheduler && !opts.delay;
  const scheduler = createSchedulerFromOptions(opts);
  let firstTime = true;
  let isScheduled = false;
  let value;
  const equals = opts.equals || compareDefault;
  const r = new Reaction(name, () => {
    if (firstTime || runSync) {
      reactionRunner();
    } else if (!isScheduled) {
      isScheduled = true;
      scheduler(reactionRunner);
    }
  }, opts.onError, opts.requiresObservable);
  function reactionRunner() {
    isScheduled = false;
    if (r.isDisposed) {
      return;
    }
    let changed = false;
    const oldValue = value;
    r.track(() => {
      const nextValue = allowStateChanges(false, () => expression(r));
      changed = firstTime || !equals(value, nextValue);
      value = nextValue;
    });
    if (firstTime && opts.fireImmediately) {
      effectAction(value, oldValue, r);
    } else if (!firstTime && changed) {
      effectAction(value, oldValue, r);
    }
    firstTime = false;
  }
  if (!(opts != null && (_opts$signal2 = opts.signal) != null && _opts$signal2.aborted)) {
    r.schedule_();
  }
  return r.getDisposer_(opts == null ? void 0 : opts.signal);
}
function wrapErrorHandler(errorHandler, baseFn) {
  return function() {
    try {
      return baseFn.apply(this, arguments);
    } catch (e) {
      errorHandler.call(this, e);
    }
  };
}
var ON_BECOME_OBSERVED = "onBO";
var ON_BECOME_UNOBSERVED = "onBUO";
function onBecomeObserved(thing, arg2, arg3) {
  return interceptHook(ON_BECOME_OBSERVED, thing, arg2, arg3);
}
function onBecomeUnobserved(thing, arg2, arg3) {
  return interceptHook(ON_BECOME_UNOBSERVED, thing, arg2, arg3);
}
function interceptHook(hook, thing, arg2, arg3) {
  const atom = typeof arg3 === "function" ? getAtom(thing, arg2) : getAtom(thing);
  const cb = isFunction(arg3) ? arg3 : arg2;
  const listenersKey = `${hook}L`;
  if (atom[listenersKey]) {
    atom[listenersKey].add(cb);
  } else {
    atom[listenersKey] = /* @__PURE__ */ new Set([cb]);
  }
  return function() {
    const hookListeners = atom[listenersKey];
    if (hookListeners) {
      hookListeners.delete(cb);
      if (hookListeners.size === 0) {
        delete atom[listenersKey];
      }
    }
  };
}
var ALWAYS = "always";
var OBSERVED = "observed";
function configure(options) {
  if (options.isolateGlobalState === true) {
    isolateGlobalState();
  }
  const {
    enforceActions
  } = options;
  if (enforceActions !== void 0) {
    const ea = enforceActions === ALWAYS ? ALWAYS : enforceActions === OBSERVED;
    globalState.enforceActions = ea;
    globalState.allowStateChanges = ea === true || ea === ALWAYS ? false : true;
  }
  ["computedRequiresReaction", "reactionRequiresObservable", "observableRequiresReaction", "disableErrorBoundaries", "safeDescriptors"].forEach((key) => {
    if (key in options) {
      globalState[key] = !!options[key];
    }
  });
  globalState.allowStateReads = !globalState.observableRequiresReaction;
  if (globalState.disableErrorBoundaries === true) {
    console.warn("WARNING: Debug feature only. MobX will NOT recover from errors when `disableErrorBoundaries` is enabled.");
  }
  if (options.reactionScheduler) {
    setReactionScheduler(options.reactionScheduler);
  }
}
function extendObservable(target, properties, annotations, options) {
  if (true) {
    if (arguments.length > 4) {
      die("'extendObservable' expected 2-4 arguments");
    }
    if (typeof target !== "object") {
      die("'extendObservable' expects an object as first argument");
    }
    if (isObservableMap(target)) {
      die("'extendObservable' should not be used on maps, use map.merge instead");
    }
    if (!isPlainObject(properties)) {
      die(`'extendObservable' only accepts plain objects as second argument`);
    }
    if (isObservable(properties) || isObservable(annotations)) {
      die(`Extending an object with another observable (object) is not supported`);
    }
  }
  const descriptors = getOwnPropertyDescriptors(properties);
  initObservable(() => {
    const adm = asObservableObject(target, options)[$mobx];
    ownKeys(descriptors).forEach((key) => {
      adm.extend_(
        key,
        descriptors[key],
        // must pass "undefined" for { key: undefined }
        !annotations ? true : key in annotations ? annotations[key] : true
      );
    });
  });
  return target;
}
function getDependencyTree(thing, property) {
  return nodeToDependencyTree(getAtom(thing, property));
}
function nodeToDependencyTree(node) {
  const result = {
    name: node.name_
  };
  if (node.observing_ && node.observing_.length > 0) {
    result.dependencies = unique(node.observing_).map(nodeToDependencyTree);
  }
  return result;
}
function getObserverTree(thing, property) {
  return nodeToObserverTree(getAtom(thing, property));
}
function nodeToObserverTree(node) {
  const result = {
    name: node.name_
  };
  if (hasObservers(node)) {
    result.observers = Array.from(getObservers(node), nodeToObserverTree);
  }
  return result;
}
function unique(list) {
  return Array.from(new Set(list));
}
var generatorId = 0;
var FlowCancellationError = class extends Error {
  constructor() {
    super("FLOW_CANCELLED");
    Object.setPrototypeOf(this, new.target.prototype);
    this.name = "FlowCancellationError";
  }
  toString() {
    return `Error: ${this.message}`;
  }
};
function isFlowCancellationError(error) {
  return error instanceof FlowCancellationError;
}
function createFlowDecoratorAnnotation(annotation) {
  return createDecoratorAnnotation(annotation, decorateFlow20223_);
}
var flowAnnotation = /* @__PURE__ */ createFlowAnnotation("flow");
var flowBoundAnnotation = /* @__PURE__ */ createFlowAnnotation("flow.bound", {
  bound: true
});
var flow = /* @__PURE__ */ assign(function flow2(arg1, arg2) {
  if (arg2 && typeof arg2.kind === "string") {
    return decorateFlow20223_(flowAnnotation, arg1, arg2);
  }
  if (arguments.length !== 1) {
    die(`Flow expects single argument with generator function`);
  }
  const generator = arg1;
  const name = generator.name || (true ? "<unnamed flow>" : "flow");
  const res = function res2() {
    const ctx = this;
    const args = arguments;
    const runId = true ? ++generatorId : 0;
    const gen = action(true ? `${name} - runid: ${runId} - init` : name, generator).apply(ctx, args);
    let rejector;
    let pendingPromise = void 0;
    const promise = new Promise(function(resolve, reject) {
      let stepId = 0;
      rejector = reject;
      function onFulfilled(res3) {
        pendingPromise = void 0;
        let ret;
        try {
          ret = action(true ? `${name} - runid: ${runId} - yield ${stepId++}` : name, gen.next).call(gen, res3);
        } catch (e) {
          return reject(e);
        }
        next(ret);
      }
      function onRejected(err) {
        pendingPromise = void 0;
        let ret;
        try {
          ret = action(true ? `${name} - runid: ${runId} - yield ${stepId++}` : name, gen.throw).call(gen, err);
        } catch (e) {
          return reject(e);
        }
        next(ret);
      }
      function next(ret) {
        if (isFunction(ret == null ? void 0 : ret.then)) {
          ret.then(next, reject);
          return;
        }
        if (ret.done) {
          return resolve(ret.value);
        }
        pendingPromise = Promise.resolve(ret.value);
        return pendingPromise.then(onFulfilled, onRejected);
      }
      onFulfilled(void 0);
    });
    const cancelActionName = true ? `${name} - runid: ${runId} - cancel` : name;
    promise.cancel = action(cancelActionName, function() {
      try {
        if (pendingPromise) {
          cancelPromise(pendingPromise);
        }
        const res3 = gen.return(void 0);
        const yieldedPromise = Promise.resolve(res3.value);
        yieldedPromise.then(noop, noop);
        cancelPromise(yieldedPromise);
        rejector(new FlowCancellationError());
      } catch (e) {
        rejector(e);
      }
    });
    return promise;
  };
  res.isMobXFlow = true;
  return res;
}, flowAnnotation);
var flowBound = /* @__PURE__ */ createFlowDecoratorAnnotation(flowBoundAnnotation);
function cancelPromise(promise) {
  if (isFunction(promise.cancel)) {
    promise.cancel();
  }
}
function flowResult(result) {
  return result;
}
function isFlow(fn) {
  return (fn == null ? void 0 : fn.isMobXFlow) === true;
}
function interceptReads(thing, propOrHandler, handler) {
  let target;
  if (isObservableMap(thing) || isObservableArray(thing) || isObservableValue(thing)) {
    target = getAdministration(thing);
  } else if (isObservableObject(thing)) {
    if (!isStringish(propOrHandler)) {
      return die(`InterceptReads can only be used with a specific property, not with an object in general`);
    }
    target = getAdministration(thing, propOrHandler);
  } else if (true) {
    return die(`Expected observable map, object or array as first array`);
  }
  if (target.dehancer !== void 0) {
    return die(`An intercept reader was already established`);
  }
  target.dehancer = typeof propOrHandler === "function" ? propOrHandler : handler;
  return () => {
    target.dehancer = void 0;
  };
}
function intercept(thing, propOrHandler, handler) {
  if (isFunction(handler)) {
    return interceptProperty(thing, propOrHandler, handler);
  } else {
    return interceptInterceptable(thing, propOrHandler);
  }
}
function interceptInterceptable(thing, handler) {
  return registerInterceptor(getAdministration(thing), handler);
}
function interceptProperty(thing, property, handler) {
  return registerInterceptor(getAdministration(thing, property), handler);
}
function _isComputed(value, property) {
  var _adm$lazyComputedKeys;
  if (property === void 0) {
    return isComputedValue(value);
  }
  if (isObservableObject(value) === false) {
    return false;
  }
  const adm = value[$mobx];
  if ((_adm$lazyComputedKeys = adm.lazyComputedKeys_) != null && _adm$lazyComputedKeys.has(property)) {
    return true;
  }
  if (!adm.values_.has(property)) {
    return false;
  }
  const atom = getAtom(value, property);
  return isComputedValue(atom);
}
function isComputed(value) {
  if (arguments.length > 1) {
    return die(`isComputed expects only 1 argument. Use isComputedProp to inspect the observability of a property`);
  }
  return _isComputed(value);
}
function isComputedProp(value, propName) {
  if (!isStringish(propName)) {
    return die(`isComputed expected a property name as second argument`);
  }
  return _isComputed(value, propName);
}
function _isObservable(value, property) {
  if (!value) {
    return false;
  }
  if (property !== void 0) {
    if (isObservableMap(value) || isObservableArray(value)) {
      return die("isObservable(object, propertyName) is not supported for arrays and maps. Use map.has or array.length instead.");
    }
    if (isObservableObject(value)) {
      var _adm$lazyComputedKeys, _adm$lazyObservableKe;
      const adm = value[$mobx];
      return adm.values_.has(property) || !!((_adm$lazyComputedKeys = adm.lazyComputedKeys_) != null && _adm$lazyComputedKeys.has(property)) || !!((_adm$lazyObservableKe = adm.lazyObservableKeys_) != null && _adm$lazyObservableKe.has(property));
    }
    return false;
  }
  return isObservableObject(value) || !!value[$mobx] || isAtom(value) || isReaction(value) || isComputedValue(value);
}
function isObservable(value) {
  if (arguments.length !== 1) {
    die(`isObservable expects only 1 argument. Use isObservableProp to inspect the observability of a property`);
  }
  return _isObservable(value);
}
function isObservableProp(value, propName) {
  if (!isStringish(propName)) {
    return die(`expected a property name as second argument`);
  }
  return _isObservable(value, propName);
}
function keys(obj) {
  if (isObservableObject(obj)) {
    return obj[$mobx].keys_();
  }
  if (isObservableMap(obj) || isObservableSet(obj)) {
    return Array.from(obj.keys());
  }
  if (isObservableArray(obj)) {
    return obj.map((_, index) => index);
  }
  die(5);
}
function values(obj) {
  if (isObservableObject(obj)) {
    return keys(obj).map((key) => obj[key]);
  }
  if (isObservableMap(obj)) {
    return keys(obj).map((key) => obj.get(key));
  }
  if (isObservableSet(obj)) {
    return Array.from(obj.values());
  }
  if (isObservableArray(obj)) {
    return obj.slice();
  }
  die(6);
}
function entries(obj) {
  if (isObservableObject(obj)) {
    return keys(obj).map((key) => [key, obj[key]]);
  }
  if (isObservableMap(obj)) {
    return keys(obj).map((key) => [key, obj.get(key)]);
  }
  if (isObservableSet(obj)) {
    return Array.from(obj.entries());
  }
  if (isObservableArray(obj)) {
    return obj.map((key, index) => [index, key]);
  }
  die(7);
}
function set(obj, key, value) {
  if (arguments.length === 2 && !isObservableSet(obj)) {
    startBatch();
    const values2 = key;
    try {
      for (let _key in values2) {
        set(obj, _key, values2[_key]);
      }
    } finally {
      endBatch();
    }
    return;
  }
  if (isObservableObject(obj)) {
    obj[$mobx].set_(key, value);
  } else if (isObservableMap(obj)) {
    obj.set(key, value);
  } else if (isObservableSet(obj)) {
    obj.add(key);
  } else if (isObservableArray(obj)) {
    if (typeof key !== "number") {
      key = parseInt(key, 10);
    }
    if (key < 0) {
      die(42, key);
    }
    startBatch();
    if (key >= obj.length) {
      obj.length = key + 1;
    }
    obj[key] = value;
    endBatch();
  } else {
    die(8);
  }
}
function remove(obj, key) {
  if (isObservableObject(obj)) {
    obj[$mobx].delete_(key);
  } else if (isObservableMap(obj)) {
    obj.delete(key);
  } else if (isObservableSet(obj)) {
    obj.delete(key);
  } else if (isObservableArray(obj)) {
    if (typeof key !== "number") {
      key = parseInt(key, 10);
    }
    obj.splice(key, 1);
  } else {
    die(9);
  }
}
function has(obj, key) {
  if (isObservableObject(obj)) {
    return obj[$mobx].has_(key);
  } else if (isObservableMap(obj)) {
    return obj.has(key);
  } else if (isObservableSet(obj)) {
    return obj.has(key);
  } else if (isObservableArray(obj)) {
    return key >= 0 && key < obj.length;
  }
  die(10);
}
function get(obj, key) {
  if (!has(obj, key)) {
    return void 0;
  }
  if (isObservableObject(obj)) {
    return obj[$mobx].get_(key);
  } else if (isObservableMap(obj)) {
    return obj.get(key);
  } else if (isObservableArray(obj)) {
    return obj[key];
  }
  die(11);
}
function apiDefineProperty(obj, key, descriptor) {
  if (isObservableObject(obj)) {
    return obj[$mobx].defineProperty_(key, descriptor);
  }
  die(39);
}
function apiOwnKeys(obj) {
  if (isObservableObject(obj)) {
    return obj[$mobx].ownKeys_();
  }
  die(38);
}
function observe(thing, propOrCb, cbOrFire, fireImmediately) {
  if (isFunction(cbOrFire)) {
    return observeObservableProperty(thing, propOrCb, cbOrFire, fireImmediately);
  } else {
    return observeObservable(thing, propOrCb, cbOrFire);
  }
}
function observeObservable(thing, listener, fireImmediately) {
  const adm = getAdministration(thing);
  if (isObservableArray(thing)) {
    if (fireImmediately) {
      listener({
        observableKind: "array",
        object: adm.proxy_,
        debugObjectName: adm.atom_.name_,
        type: "splice",
        index: 0,
        added: adm.values_.slice(),
        addedCount: adm.values_.length,
        removed: [],
        removedCount: 0
      });
    }
  } else if (isObservableMap(thing)) {
    if (fireImmediately === true) {
      die("`observe` doesn't support fireImmediately=true in combination with maps.");
    }
  } else if (isObservableSet(thing)) {
    if (fireImmediately === true) {
      die("`observe` doesn't support fireImmediately=true in combination with sets.");
    }
  } else if (isObservableObject(thing)) {
    if (fireImmediately === true) {
      die("`observe` doesn't support the fire immediately property for observable objects.");
    }
  } else {
    return observeValue(adm, listener, fireImmediately);
  }
  return registerListener(adm, listener);
}
function observeObservableProperty(thing, property, listener, fireImmediately) {
  return observeValue(getAdministration(thing, property), listener, fireImmediately);
}
function observeValue(adm, listener, fireImmediately) {
  if (isComputedValue(adm)) {
    let firstTime = true;
    let prevValue = void 0;
    return autorun(() => {
      const newValue = adm.get();
      if (!firstTime || fireImmediately) {
        const prevU = untrackedStart();
        listener({
          observableKind: "computed",
          debugObjectName: adm.name_,
          type: UPDATE,
          object: adm,
          newValue,
          oldValue: prevValue
        });
        untrackedEnd(prevU);
      }
      firstTime = false;
      prevValue = newValue;
    });
  }
  if (fireImmediately) {
    listener({
      observableKind: "value",
      debugObjectName: adm.name_,
      object: adm,
      type: UPDATE,
      newValue: adm.value_,
      oldValue: void 0
    });
  }
  return registerListener(adm, listener);
}
function cache(map, key, value) {
  map.set(key, value);
  return value;
}
function toJSHelper(source, __alreadySeen) {
  if (source == null || typeof source !== "object" || source instanceof Date || !isObservable(source)) {
    return source;
  }
  if (isObservableValue(source) || isComputedValue(source)) {
    return toJSHelper(source.get(), __alreadySeen);
  }
  if (__alreadySeen.has(source)) {
    return __alreadySeen.get(source);
  }
  if (isObservableArray(source)) {
    const res = cache(__alreadySeen, source, new Array(source.length));
    source.forEach((value, idx) => {
      res[idx] = toJSHelper(value, __alreadySeen);
    });
    return res;
  }
  if (isObservableSet(source)) {
    const res = cache(__alreadySeen, source, /* @__PURE__ */ new Set());
    source.forEach((value) => {
      res.add(toJSHelper(value, __alreadySeen));
    });
    return res;
  }
  if (isObservableMap(source)) {
    const res = cache(__alreadySeen, source, /* @__PURE__ */ new Map());
    source.forEach((value, key) => {
      res.set(key, toJSHelper(value, __alreadySeen));
    });
    return res;
  } else {
    const res = cache(__alreadySeen, source, {});
    apiOwnKeys(source).forEach((key) => {
      if (objectPrototype.propertyIsEnumerable.call(source, key)) {
        res[key] = toJSHelper(source[key], __alreadySeen);
      }
    });
    return res;
  }
}
function toJS(source, options) {
  if (options) {
    die("toJS no longer supports options");
  }
  return toJSHelper(source, /* @__PURE__ */ new Map());
}
function transaction(action2, thisArg = void 0) {
  startBatch();
  try {
    return action2.apply(thisArg);
  } finally {
    endBatch();
  }
}
function when(predicate, arg1, arg2) {
  if (arguments.length === 1 || arg1 && typeof arg1 === "object") {
    return whenPromise(predicate, arg1);
  }
  return _when(predicate, arg1, arg2 || {});
}
function _when(predicate, effect, opts) {
  let timeoutHandle;
  if (typeof opts.timeout === "number") {
    const error = new Error("WHEN_TIMEOUT");
    timeoutHandle = setTimeout(() => {
      if (!disposer[$mobx].isDisposed) {
        disposer();
        if (opts.onError) {
          opts.onError(error);
        } else {
          throw error;
        }
      }
    }, opts.timeout);
  }
  opts.name = true ? opts.name || "When@" + getNextId() : "When";
  const effectAction = createAction(true ? opts.name + "-effect" : "When-effect", effect);
  var disposer = autorun((r) => {
    let cond = allowStateChanges(false, predicate);
    if (cond) {
      r.dispose();
      if (timeoutHandle) {
        clearTimeout(timeoutHandle);
      }
      effectAction();
    }
  }, opts);
  return disposer;
}
function whenPromise(predicate, opts) {
  var _opts$signal;
  if (opts && opts.onError) {
    return die(`the options 'onError' and 'promise' cannot be combined`);
  }
  if (opts != null && (_opts$signal = opts.signal) != null && _opts$signal.aborted) {
    return assign(Promise.reject(new Error("WHEN_ABORTED")), {
      cancel: () => null
    });
  }
  let cancel;
  let abort;
  const res = new Promise((resolve, reject) => {
    var _opts$signal2;
    let disposer = _when(predicate, resolve, assign({}, opts, {
      onError: reject
    }));
    cancel = () => {
      disposer();
      reject(new Error("WHEN_CANCELLED"));
    };
    abort = () => {
      disposer();
      reject(new Error("WHEN_ABORTED"));
    };
    opts == null || (_opts$signal2 = opts.signal) == null || _opts$signal2.addEventListener == null || _opts$signal2.addEventListener("abort", abort);
  }).finally(() => {
    var _opts$signal3;
    return opts == null || (_opts$signal3 = opts.signal) == null || _opts$signal3.removeEventListener == null ? void 0 : _opts$signal3.removeEventListener("abort", abort);
  });
  res.cancel = cancel;
  return res;
}
function getAdm(target) {
  return target[$mobx];
}
var objectProxyTraps = {
  has(target, name) {
    return getAdm(target).has_(name);
  },
  get(target, name) {
    return getAdm(target).get_(name);
  },
  set(target, name, value) {
    var _getAdm$set_;
    if (!isStringish(name)) {
      return false;
    }
    return (_getAdm$set_ = getAdm(target).set_(name, value, true)) != null ? _getAdm$set_ : true;
  },
  deleteProperty(target, name) {
    var _getAdm$delete_;
    if (!isStringish(name)) {
      return false;
    }
    return (_getAdm$delete_ = getAdm(target).delete_(name, true)) != null ? _getAdm$delete_ : true;
  },
  defineProperty(target, name, descriptor) {
    var _getAdm$definePropert;
    return (_getAdm$definePropert = getAdm(target).defineProperty_(name, descriptor)) != null ? _getAdm$definePropert : true;
  },
  ownKeys(target) {
    return getAdm(target).ownKeys_();
  },
  preventExtensions(target) {
    die(13);
  }
};
function asDynamicObservableObject(target, options) {
  var _target$$mobx, _target$$mobx$proxy_;
  target = asObservableObject(target, options);
  return (_target$$mobx$proxy_ = (_target$$mobx = target[$mobx]).proxy_) != null ? _target$$mobx$proxy_ : _target$$mobx.proxy_ = new Proxy(target, objectProxyTraps);
}
function hasInterceptors(interceptable) {
  return interceptable.interceptors_ !== void 0 && interceptable.interceptors_.length > 0;
}
function registerInterceptor(interceptable, handler) {
  const interceptors = interceptable.interceptors_ || (interceptable.interceptors_ = []);
  interceptors.push(handler);
  return once(() => {
    const idx = interceptors.indexOf(handler);
    if (idx !== -1) {
      interceptors.splice(idx, 1);
    }
  });
}
function interceptChange(interceptable, change) {
  const prevU = untrackedStart();
  try {
    const interceptors = [...interceptable.interceptors_ || []];
    for (let i = 0, l = interceptors.length; i < l; i++) {
      change = interceptors[i](change);
      if (change && !change.type) {
        die(14);
      }
      if (!change) {
        break;
      }
    }
    return change;
  } finally {
    untrackedEnd(prevU);
  }
}
function hasListeners(listenable) {
  return listenable.changeListeners_ !== void 0 && listenable.changeListeners_.length > 0;
}
function registerListener(listenable, handler) {
  const listeners = listenable.changeListeners_ || (listenable.changeListeners_ = []);
  listeners.push(handler);
  return once(() => {
    const idx = listeners.indexOf(handler);
    if (idx !== -1) {
      listeners.splice(idx, 1);
    }
  });
}
function notifyListeners(listenable, change) {
  const prevU = untrackedStart();
  let listeners = listenable.changeListeners_;
  if (!listeners) {
    return;
  }
  listeners = listeners.slice();
  for (let i = 0, l = listeners.length; i < l; i++) {
    listeners[i](change);
  }
  untrackedEnd(prevU);
}
function makeObservable(target, annotations, options) {
  initObservable(() => {
    const adm = asObservableObject(target, options)[$mobx];
    ownKeys(annotations).forEach((key) => make_(adm, key, annotations[key]));
  });
  return target;
}
var keysSymbol = /* @__PURE__ */ Symbol("mobx-keys");
function makeAutoObservable(target, overrides, options) {
  if (true) {
    if (!isPlainObject(target) && !isPlainObject(Object.getPrototypeOf(target))) {
      die(`'makeAutoObservable' can only be used for classes that don't have a superclass`);
    }
    if (isObservableObject(target)) {
      die(`makeAutoObservable can only be used on objects not already made observable`);
    }
  }
  if (isPlainObject(target)) {
    return extendObservable(target, target, overrides, options);
  }
  initObservable(() => {
    const adm = asObservableObject(target, options)[$mobx];
    if (!target[keysSymbol]) {
      const proto = Object.getPrototypeOf(target);
      const keys2 = /* @__PURE__ */ new Set([...ownKeys(target), ...ownKeys(proto)]);
      keys2.delete("constructor");
      keys2.delete($mobx);
      addHiddenProp(proto, keysSymbol, keys2);
    }
    target[keysSymbol].forEach((key) => make_(
      adm,
      key,
      // must pass "undefined" for { key: undefined }
      !overrides ? true : key in overrides ? overrides[key] : true
    ));
  });
  return target;
}
function make_(adm, key, annotation) {
  if (annotation === true) {
    annotation = adm.defaultAnnotation_;
  }
  if (annotation === false) {
    return;
  }
  assertAnnotable(adm, annotation, key);
  if (!(key in adm.target_)) {
    die(1, annotation.annotationType_, `${adm.name_}.${key.toString()}`);
  }
  let source = adm.target_;
  while (source && source !== objectPrototype) {
    const descriptor = getDescriptor(source, key);
    if (descriptor) {
      const outcome = annotation.make_(adm, key, descriptor, source);
      if (outcome === 0) {
        return;
      }
      if (outcome === 1) {
        break;
      }
    }
    source = Object.getPrototypeOf(source);
  }
  recordAnnotationApplied(adm, annotation, key);
}
var SPLICE = "splice";
var UPDATE = "update";
var MAX_SPLICE_SIZE = 1e4;
var arrayTraps = {
  get(target, name) {
    const adm = target[$mobx];
    if (name === $mobx) {
      return adm;
    }
    if (name === "length") {
      return adm.getArrayLength_();
    }
    if (typeof name === "string" && !isNaN(name)) {
      return adm.get_(parseInt(name));
    }
    if (hasProp(arrayExtensions, name)) {
      return arrayExtensions[name];
    }
    return target[name];
  },
  set(target, name, value) {
    const adm = target[$mobx];
    if (name === "length") {
      adm.setArrayLength_(value);
    }
    if (typeof name === "symbol" || isNaN(name)) {
      target[name] = value;
    } else {
      adm.set_(parseInt(name), value);
    }
    return true;
  },
  preventExtensions() {
    die(15);
  }
};
var ObservableArrayAdministration = class {
  constructor(name = true ? "ObservableArray@" + getNextId() : "ObservableArray", enhancer, owned_) {
    this.owned_ = void 0;
    this.atom_ = void 0;
    this.values_ = [];
    this.interceptors_ = void 0;
    this.changeListeners_ = void 0;
    this.enhancer_ = void 0;
    this.dehancer = void 0;
    this.proxy_ = void 0;
    this.lastKnownLength_ = 0;
    this.owned_ = owned_;
    this.atom_ = new Atom(name);
    this.enhancer_ = (newV, oldV) => enhancer(newV, oldV, true ? name + "[..]" : "ObservableArray[..]");
  }
  dehanceValue_(value) {
    if (this.dehancer !== void 0) {
      return this.dehancer(value);
    }
    return value;
  }
  dehanceValues_(values2) {
    if (this.dehancer !== void 0 && values2.length > 0) {
      return values2.map(this.dehancer);
    }
    return values2;
  }
  getArrayLength_() {
    this.atom_.reportObserved();
    return this.values_.length;
  }
  setArrayLength_(newLength) {
    if (typeof newLength !== "number" || isNaN(newLength) || newLength < 0) {
      die(40, newLength);
    }
    let currentLength = this.values_.length;
    if (newLength === currentLength) {
      return;
    } else if (newLength > currentLength) {
      const newItems = Array.from({
        length: newLength - currentLength
      });
      this.spliceWithArray_(currentLength, 0, newItems);
    } else {
      this.spliceWithArray_(newLength, currentLength - newLength);
    }
  }
  updateArrayLength_(oldLength, delta) {
    if (oldLength !== this.lastKnownLength_) {
      die(16);
    }
    this.lastKnownLength_ += delta;
  }
  spliceWithArray_(index, deleteCount, newItems) {
    checkIfStateModificationsAreAllowed(this.atom_);
    const length = this.values_.length;
    if (index === void 0) {
      index = 0;
    } else if (index > length) {
      index = length;
    } else if (index < 0) {
      index = Math.max(0, length + index);
    }
    if (arguments.length === 1) {
      deleteCount = length - index;
    } else if (deleteCount === void 0 || deleteCount === null) {
      deleteCount = 0;
    } else {
      deleteCount = Math.max(0, Math.min(deleteCount, length - index));
    }
    if (newItems === void 0) {
      newItems = EMPTY_ARRAY;
    }
    if (hasInterceptors(this)) {
      const change = interceptChange(this, {
        object: this.proxy_,
        type: SPLICE,
        index,
        removedCount: deleteCount,
        added: newItems
      });
      if (!change) {
        return EMPTY_ARRAY;
      }
      deleteCount = change.removedCount;
      newItems = change.added;
    }
    newItems = newItems.length === 0 ? newItems : newItems.map((v) => this.enhancer_(v, void 0));
    if (true) {
      const lengthDelta = newItems.length - deleteCount;
      this.updateArrayLength_(length, lengthDelta);
    }
    const res = this.spliceItemsIntoValues_(index, deleteCount, newItems);
    if (deleteCount !== 0 || newItems.length !== 0) {
      this.notifyArraySplice_(index, newItems, res);
    }
    return this.dehanceValues_(res);
  }
  spliceItemsIntoValues_(index, deleteCount, newItems) {
    if (newItems.length < MAX_SPLICE_SIZE) {
      return this.values_.splice(index, deleteCount, ...newItems);
    } else {
      const res = this.values_.slice(index, index + deleteCount);
      let oldItems = this.values_.slice(index + deleteCount);
      this.values_.length += newItems.length - deleteCount;
      for (let i = 0; i < newItems.length; i++) {
        this.values_[index + i] = newItems[i];
      }
      for (let i = 0; i < oldItems.length; i++) {
        this.values_[index + newItems.length + i] = oldItems[i];
      }
      return res;
    }
  }
  notifyArrayChildUpdate_(index, newValue, oldValue) {
    const notifySpy = !this.owned_ && isSpyEnabled();
    const notify = hasListeners(this);
    const change = notify || notifySpy ? {
      observableKind: "array",
      object: this.proxy_,
      type: UPDATE,
      debugObjectName: this.atom_.name_,
      index,
      newValue,
      oldValue
    } : null;
    if (notifySpy) {
      spyReportStart(change);
    }
    this.atom_.reportChanged();
    if (notify) {
      notifyListeners(this, change);
    }
    if (notifySpy) {
      spyReportEnd();
    }
  }
  notifyArraySplice_(index, added, removed) {
    const notifySpy = !this.owned_ && isSpyEnabled();
    const notify = hasListeners(this);
    const change = notify || notifySpy ? {
      observableKind: "array",
      object: this.proxy_,
      debugObjectName: this.atom_.name_,
      type: SPLICE,
      index,
      removed,
      added,
      removedCount: removed.length,
      addedCount: added.length
    } : null;
    if (notifySpy) {
      spyReportStart(change);
    }
    this.atom_.reportChanged();
    if (notify) {
      notifyListeners(this, change);
    }
    if (notifySpy) {
      spyReportEnd();
    }
  }
  get_(index) {
    this.atom_.reportObserved();
    return this.dehanceValue_(this.values_[index]);
  }
  set_(index, newValue) {
    const values2 = this.values_;
    if (index < values2.length) {
      checkIfStateModificationsAreAllowed(this.atom_);
      const oldValue = values2[index];
      if (hasInterceptors(this)) {
        const change = interceptChange(this, {
          type: UPDATE,
          object: this.proxy_,
          // since "this" is the real array we need to pass its proxy
          index,
          newValue
        });
        if (!change) {
          return;
        }
        newValue = change.newValue;
      }
      newValue = this.enhancer_(newValue, oldValue);
      const changed = newValue !== oldValue;
      if (changed) {
        values2[index] = newValue;
        this.notifyArrayChildUpdate_(index, newValue, oldValue);
      }
    } else {
      const newItems = Array.from({
        length: index + 1 - values2.length
      });
      newItems[newItems.length - 1] = newValue;
      this.spliceWithArray_(values2.length, 0, newItems);
    }
  }
};
function createObservableArray(initialValues, enhancer, name = true ? "ObservableArray@" + getNextId() : "ObservableArray", owned = false) {
  return initObservable(() => {
    const adm = new ObservableArrayAdministration(name, enhancer, owned);
    addHiddenFinalProp(adm.values_, $mobx, adm);
    const proxy = new Proxy(adm.values_, arrayTraps);
    adm.proxy_ = proxy;
    if (initialValues && initialValues.length) {
      adm.spliceWithArray_(0, 0, initialValues);
    }
    return proxy;
  });
}
var arrayExtensions = {
  clear() {
    return this.splice(0);
  },
  replace(newItems) {
    const adm = this[$mobx];
    return adm.spliceWithArray_(0, adm.values_.length, newItems);
  },
  // Used by JSON.stringify
  toJSON() {
    return this.slice();
  },
  /*
   * functions that do alter the internal structure of the array, (based on lib.es6.d.ts)
   * since these functions alter the inner structure of the array, the have side effects.
   * Because the have side effects, they should not be used in computed function,
   * and for that reason the do not call dependencyState.notifyObserved
   */
  splice(index, deleteCount, ...newItems) {
    const adm = this[$mobx];
    switch (arguments.length) {
      case 0:
        return [];
      case 1:
        return adm.spliceWithArray_(index);
      case 2:
        return adm.spliceWithArray_(index, deleteCount);
    }
    return adm.spliceWithArray_(index, deleteCount, newItems);
  },
  spliceWithArray(index, deleteCount, newItems) {
    return this[$mobx].spliceWithArray_(index, deleteCount, newItems);
  },
  push(...items) {
    const adm = this[$mobx];
    adm.spliceWithArray_(adm.values_.length, 0, items);
    return adm.values_.length;
  },
  pop() {
    return this.splice(Math.max(this[$mobx].values_.length - 1, 0), 1)[0];
  },
  shift() {
    return this.splice(0, 1)[0];
  },
  unshift(...items) {
    const adm = this[$mobx];
    adm.spliceWithArray_(0, 0, items);
    return adm.values_.length;
  },
  reverse() {
    if (globalState.trackingDerivation) {
      die(37, "reverse");
    }
    this.replace(this.slice().reverse());
    return this;
  },
  sort() {
    if (globalState.trackingDerivation) {
      die(37, "sort");
    }
    const copy = this.slice();
    copy.sort.apply(copy, arguments);
    this.replace(copy);
    return this;
  },
  remove(value) {
    const adm = this[$mobx];
    const idx = adm.dehanceValues_(adm.values_).indexOf(value);
    if (idx > -1) {
      this.splice(idx, 1);
      return true;
    }
    return false;
  }
};
addArrayExtension("at", simpleFunc);
addArrayExtension("concat", simpleFunc);
addArrayExtension("flat", simpleFunc);
addArrayExtension("includes", simpleFunc);
addArrayExtension("indexOf", simpleFunc);
addArrayExtension("join", simpleFunc);
addArrayExtension("lastIndexOf", simpleFunc);
addArrayExtension("slice", simpleFunc);
addArrayExtension("toString", simpleFunc);
addArrayExtension("toLocaleString", simpleFunc);
addArrayExtension("toSorted", simpleFunc);
addArrayExtension("toSpliced", simpleFunc);
addArrayExtension("with", simpleFunc);
addArrayExtension("every", mapLikeFunc);
addArrayExtension("filter", mapLikeFunc);
addArrayExtension("find", mapLikeFunc);
addArrayExtension("findIndex", mapLikeFunc);
addArrayExtension("findLast", mapLikeFunc);
addArrayExtension("findLastIndex", mapLikeFunc);
addArrayExtension("flatMap", mapLikeFunc);
addArrayExtension("forEach", mapLikeFunc);
addArrayExtension("map", mapLikeFunc);
addArrayExtension("some", mapLikeFunc);
addArrayExtension("toReversed", mapLikeFunc);
addArrayExtension("reduce", reduceLikeFunc);
addArrayExtension("reduceRight", reduceLikeFunc);
function addArrayExtension(funcName, funcFactory) {
  if (typeof Array.prototype[funcName] === "function") {
    arrayExtensions[funcName] = funcFactory(funcName);
  }
}
function simpleFunc(funcName) {
  return function() {
    const adm = this[$mobx];
    adm.atom_.reportObserved();
    const dehancedValues = adm.dehanceValues_(adm.values_);
    return dehancedValues[funcName].apply(dehancedValues, arguments);
  };
}
function mapLikeFunc(funcName) {
  return function(callback, thisArg) {
    const adm = this[$mobx];
    adm.atom_.reportObserved();
    const dehancedValues = adm.dehanceValues_(adm.values_);
    return dehancedValues[funcName]((element, index) => {
      return callback.call(thisArg, element, index, this);
    });
  };
}
function reduceLikeFunc(funcName) {
  return function() {
    const adm = this[$mobx];
    adm.atom_.reportObserved();
    const dehancedValues = adm.dehanceValues_(adm.values_);
    const callback = arguments[0];
    arguments[0] = (accumulator, currentValue, index) => {
      return callback(accumulator, currentValue, index, this);
    };
    return dehancedValues[funcName].apply(dehancedValues, arguments);
  };
}
var isObservableArrayAdministration = /* @__PURE__ */ createInstanceofPredicate("ObservableArrayAdministration", ObservableArrayAdministration);
function isObservableArray(thing) {
  return isObject(thing) && isObservableArrayAdministration(thing[$mobx]);
}
var ObservableMapMarker = {};
var ADD = "add";
var DELETE = "delete";
var ObservableMap = class {
  constructor(initialData, enhancer_ = deepEnhancer, name_ = true ? "ObservableMap@" + getNextId() : "ObservableMap") {
    this.enhancer_ = void 0;
    this.name_ = void 0;
    this[$mobx] = ObservableMapMarker;
    this.data_ = void 0;
    this.hasMap_ = void 0;
    this.keysAtom_ = void 0;
    this.interceptors_ = void 0;
    this.changeListeners_ = void 0;
    this.dehancer = void 0;
    this.enhancer_ = enhancer_;
    this.name_ = name_;
    initObservable(() => {
      this.keysAtom_ = createAtom(true ? `${this.name_}.keys()` : "ObservableMap.keys()");
      this.data_ = /* @__PURE__ */ new Map();
      this.hasMap_ = /* @__PURE__ */ new Map();
      if (initialData) {
        this.merge(initialData);
      }
    });
  }
  has_(key) {
    return this.data_.has(key);
  }
  has(key) {
    if (!globalState.trackingDerivation) {
      return this.has_(key);
    }
    let entry = this.hasMap_.get(key);
    if (!entry) {
      const newEntry = entry = new ObservableValue(this.has_(key), referenceEnhancer, true ? `${this.name_}.${stringifyKey(key)}?` : "ObservableMap.key?", false);
      this.hasMap_.set(key, newEntry);
      newEntry.onBUOL = /* @__PURE__ */ new Set([() => this.hasMap_.delete(key)]);
    }
    return entry.get();
  }
  set(key, value) {
    const hasKey = this.has_(key);
    if (hasInterceptors(this)) {
      const change = interceptChange(this, {
        type: hasKey ? UPDATE : ADD,
        object: this,
        newValue: value,
        name: key
      });
      if (!change) {
        return this;
      }
      value = change.newValue;
    }
    if (hasKey) {
      this.updateValue_(key, value);
    } else {
      this.addValue_(key, value);
    }
    return this;
  }
  delete(key) {
    checkIfStateModificationsAreAllowed(this.keysAtom_);
    if (hasInterceptors(this)) {
      const change = interceptChange(this, {
        type: DELETE,
        object: this,
        name: key
      });
      if (!change) {
        return false;
      }
    }
    if (this.has_(key)) {
      const notifySpy = isSpyEnabled();
      const notify = hasListeners(this);
      const change = notify || notifySpy ? {
        observableKind: "map",
        debugObjectName: this.name_,
        type: DELETE,
        object: this,
        oldValue: this.data_.get(key).value_,
        name: key
      } : null;
      if (notifySpy) {
        spyReportStart(change);
      }
      transaction(() => {
        var _this$hasMap_$get;
        this.keysAtom_.reportChanged();
        (_this$hasMap_$get = this.hasMap_.get(key)) == null || _this$hasMap_$get.setNewValue_(false);
        const observable2 = this.data_.get(key);
        observable2.setNewValue_(void 0);
        this.data_.delete(key);
      });
      if (notify) {
        notifyListeners(this, change);
      }
      if (notifySpy) {
        spyReportEnd();
      }
      return true;
    }
    return false;
  }
  updateValue_(key, newValue) {
    const observable2 = this.data_.get(key);
    newValue = observable2.prepareNewValue_(newValue);
    if (newValue !== globalState.UNCHANGED) {
      const notifySpy = isSpyEnabled();
      const notify = hasListeners(this);
      const change = notify || notifySpy ? {
        observableKind: "map",
        debugObjectName: this.name_,
        type: UPDATE,
        object: this,
        oldValue: observable2.value_,
        name: key,
        newValue
      } : null;
      if (notifySpy) {
        spyReportStart(change);
      }
      observable2.setNewValue_(newValue);
      if (notify) {
        notifyListeners(this, change);
      }
      if (notifySpy) {
        spyReportEnd();
      }
    }
  }
  addValue_(key, newValue) {
    checkIfStateModificationsAreAllowed(this.keysAtom_);
    transaction(() => {
      var _this$hasMap_$get2;
      const observable2 = new ObservableValue(newValue, this.enhancer_, true ? `${this.name_}.${stringifyKey(key)}` : "ObservableMap.key", false);
      this.data_.set(key, observable2);
      newValue = observable2.value_;
      (_this$hasMap_$get2 = this.hasMap_.get(key)) == null || _this$hasMap_$get2.setNewValue_(true);
      this.keysAtom_.reportChanged();
    });
    const notifySpy = isSpyEnabled();
    const notify = hasListeners(this);
    const change = notify || notifySpy ? {
      observableKind: "map",
      debugObjectName: this.name_,
      type: ADD,
      object: this,
      name: key,
      newValue
    } : null;
    if (notifySpy) {
      spyReportStart(change);
    }
    if (notify) {
      notifyListeners(this, change);
    }
    if (notifySpy) {
      spyReportEnd();
    }
  }
  get(key) {
    if (this.has(key)) {
      return this.dehanceValue_(this.data_.get(key).get());
    }
    return this.dehanceValue_(void 0);
  }
  getOrInsert(key, value) {
    if (!this.has(key)) {
      this.set(key, value);
    }
    return this.get(key);
  }
  getOrInsertComputed(key, callback) {
    if (!this.has(key)) {
      this.set(key, callback(key));
    }
    return this.get(key);
  }
  dehanceValue_(value) {
    if (this.dehancer !== void 0) {
      return this.dehancer(value);
    }
    return value;
  }
  keys() {
    this.keysAtom_.reportObserved();
    return this.data_.keys();
  }
  values() {
    const self = this;
    const keys2 = this.keys();
    return makeIterableForMap({
      next() {
        const {
          done,
          value
        } = keys2.next();
        return {
          done,
          value: done ? void 0 : self.get(value)
        };
      }
    });
  }
  entries() {
    const self = this;
    const keys2 = this.keys();
    return makeIterableForMap({
      next() {
        const {
          done,
          value
        } = keys2.next();
        return {
          done,
          value: done ? void 0 : [value, self.get(value)]
        };
      }
    });
  }
  [Symbol.iterator]() {
    return this.entries();
  }
  forEach(callback, thisArg) {
    for (const [key, value] of this) {
      callback.call(thisArg, value, key, this);
    }
  }
  /** Merge another object into this object, returns this. */
  merge(other) {
    if (isObservableMap(other)) {
      other = new Map(other);
    }
    transaction(() => {
      if (isPlainObject(other)) {
        getPlainObjectKeys(other).forEach((key) => this.set(key, other[key]));
      } else if (Array.isArray(other)) {
        other.forEach(([key, value]) => this.set(key, value));
      } else if (isES6Map(other)) {
        if (!isPlainES6Map(other)) {
          die(19, other);
        }
        other.forEach((value, key) => this.set(key, value));
      } else if (other !== null && other !== void 0) {
        die(20, other);
      }
    });
    return this;
  }
  clear() {
    transaction(() => {
      untracked(() => {
        for (const key of this.keys()) {
          this.delete(key);
        }
      });
    });
  }
  replace(values2) {
    transaction(() => {
      const replacementMap = convertToMap(values2);
      const orderedData = /* @__PURE__ */ new Map();
      let keysReportChangedCalled = false;
      for (const key of this.data_.keys()) {
        if (!replacementMap.has(key)) {
          const deleted = this.delete(key);
          if (deleted) {
            keysReportChangedCalled = true;
          } else {
            const value = this.data_.get(key);
            orderedData.set(key, value);
          }
        }
      }
      for (const [key, value] of replacementMap.entries()) {
        const keyExisted = this.data_.has(key);
        this.set(key, value);
        if (this.data_.has(key)) {
          const _value = this.data_.get(key);
          orderedData.set(key, _value);
          if (!keyExisted) {
            keysReportChangedCalled = true;
          }
        }
      }
      if (!keysReportChangedCalled) {
        if (this.data_.size !== orderedData.size) {
          this.keysAtom_.reportChanged();
        } else {
          const iter1 = this.data_.keys();
          const iter2 = orderedData.keys();
          let next1 = iter1.next();
          let next2 = iter2.next();
          while (!next1.done) {
            if (next1.value !== next2.value) {
              this.keysAtom_.reportChanged();
              break;
            }
            next1 = iter1.next();
            next2 = iter2.next();
          }
        }
      }
      this.data_ = orderedData;
    });
    return this;
  }
  get size() {
    this.keysAtom_.reportObserved();
    return this.data_.size;
  }
  toString() {
    return "[object ObservableMap]";
  }
  toJSON() {
    return Array.from(this);
  }
  get [Symbol.toStringTag]() {
    return "Map";
  }
};
var isObservableMap = /* @__PURE__ */ createInstanceofPredicate("ObservableMap", ObservableMap);
function makeIterableForMap(iterator) {
  iterator[Symbol.toStringTag] = "MapIterator";
  return makeIterable(iterator);
}
function convertToMap(dataStructure) {
  if (isES6Map(dataStructure) || isObservableMap(dataStructure)) {
    return dataStructure;
  } else if (Array.isArray(dataStructure)) {
    return new Map(dataStructure);
  } else if (isPlainObject(dataStructure)) {
    const map = /* @__PURE__ */ new Map();
    for (const key in dataStructure) {
      map.set(key, dataStructure[key]);
    }
    return map;
  } else {
    return die(21, dataStructure);
  }
}
var ObservableSetMarker = {};
var ObservableSet = class {
  constructor(initialData, enhancer = deepEnhancer, name_ = true ? "ObservableSet@" + getNextId() : "ObservableSet") {
    this.name_ = void 0;
    this[$mobx] = ObservableSetMarker;
    this.data_ = /* @__PURE__ */ new Set();
    this.atom_ = void 0;
    this.changeListeners_ = void 0;
    this.interceptors_ = void 0;
    this.dehancer = void 0;
    this.enhancer_ = void 0;
    this.name_ = name_;
    this.enhancer_ = (newV, oldV) => enhancer(newV, oldV, name_);
    initObservable(() => {
      this.atom_ = createAtom(this.name_);
      if (initialData) {
        this.replace(initialData);
      }
    });
  }
  dehanceValue_(value) {
    if (this.dehancer !== void 0) {
      return this.dehancer(value);
    }
    return value;
  }
  clear() {
    transaction(() => {
      untracked(() => {
        for (const value of this.data_.values()) {
          this.delete(value);
        }
      });
    });
  }
  forEach(callbackFn, thisArg) {
    for (const value of this) {
      callbackFn.call(thisArg, value, value, this);
    }
  }
  get size() {
    this.atom_.reportObserved();
    return this.data_.size;
  }
  add(value) {
    checkIfStateModificationsAreAllowed(this.atom_);
    if (hasInterceptors(this)) {
      const change = interceptChange(this, {
        type: ADD,
        object: this,
        newValue: value
      });
      if (!change) {
        return this;
      }
      value = change.newValue;
    }
    if (!this.has(value)) {
      transaction(() => {
        this.data_.add(this.enhancer_(value, void 0));
        this.atom_.reportChanged();
      });
      const notifySpy = isSpyEnabled();
      const notify = hasListeners(this);
      const change = notify || notifySpy ? {
        observableKind: "set",
        debugObjectName: this.name_,
        type: ADD,
        object: this,
        newValue: value
      } : null;
      if (notifySpy && true) {
        spyReportStart(change);
      }
      if (notify) {
        notifyListeners(this, change);
      }
      if (notifySpy && true) {
        spyReportEnd();
      }
    }
    return this;
  }
  delete(value) {
    if (hasInterceptors(this)) {
      const change = interceptChange(this, {
        type: DELETE,
        object: this,
        oldValue: value
      });
      if (!change) {
        return false;
      }
    }
    if (this.has(value)) {
      const notifySpy = isSpyEnabled();
      const notify = hasListeners(this);
      const change = notify || notifySpy ? {
        observableKind: "set",
        debugObjectName: this.name_,
        type: DELETE,
        object: this,
        oldValue: value
      } : null;
      if (notifySpy && true) {
        spyReportStart(change);
      }
      transaction(() => {
        this.atom_.reportChanged();
        this.data_.delete(value);
      });
      if (notify) {
        notifyListeners(this, change);
      }
      if (notifySpy && true) {
        spyReportEnd();
      }
      return true;
    }
    return false;
  }
  has(value) {
    this.atom_.reportObserved();
    return this.data_.has(this.dehanceValue_(value));
  }
  entries() {
    const values2 = this.values();
    return makeIterableForSet({
      next() {
        const {
          value,
          done
        } = values2.next();
        return !done ? {
          value: [value, value],
          done
        } : {
          value: void 0,
          done
        };
      }
    });
  }
  keys() {
    return this.values();
  }
  values() {
    this.atom_.reportObserved();
    const self = this;
    const values2 = this.data_.values();
    return makeIterableForSet({
      next() {
        const {
          value,
          done
        } = values2.next();
        return !done ? {
          value: self.dehanceValue_(value),
          done
        } : {
          value: void 0,
          done
        };
      }
    });
  }
  intersection(otherSet) {
    if (isES6Set(otherSet) && !isObservableSet(otherSet)) {
      return otherSet.intersection(this);
    } else {
      const dehancedSet = new Set(this);
      return dehancedSet.intersection(otherSet);
    }
  }
  union(otherSet) {
    if (isES6Set(otherSet) && !isObservableSet(otherSet)) {
      return otherSet.union(this);
    } else {
      const dehancedSet = new Set(this);
      return dehancedSet.union(otherSet);
    }
  }
  difference(otherSet) {
    return new Set(this).difference(otherSet);
  }
  symmetricDifference(otherSet) {
    if (isES6Set(otherSet) && !isObservableSet(otherSet)) {
      return otherSet.symmetricDifference(this);
    } else {
      const dehancedSet = new Set(this);
      return dehancedSet.symmetricDifference(otherSet);
    }
  }
  isSubsetOf(otherSet) {
    return new Set(this).isSubsetOf(otherSet);
  }
  isSupersetOf(otherSet) {
    return new Set(this).isSupersetOf(otherSet);
  }
  isDisjointFrom(otherSet) {
    if (isES6Set(otherSet) && !isObservableSet(otherSet)) {
      return otherSet.isDisjointFrom(this);
    } else {
      const dehancedSet = new Set(this);
      return dehancedSet.isDisjointFrom(otherSet);
    }
  }
  replace(other) {
    if (isObservableSet(other)) {
      other = new Set(other);
    }
    transaction(() => {
      if (Array.isArray(other)) {
        this.clear();
        other.forEach((value) => this.add(value));
      } else if (isES6Set(other)) {
        this.clear();
        other.forEach((value) => this.add(value));
      } else if (other !== null && other !== void 0) {
        die(41, other);
      }
    });
    return this;
  }
  toJSON() {
    return Array.from(this);
  }
  toString() {
    return "[object ObservableSet]";
  }
  [Symbol.iterator]() {
    return this.values();
  }
  get [Symbol.toStringTag]() {
    return "Set";
  }
};
var isObservableSet = /* @__PURE__ */ createInstanceofPredicate("ObservableSet", ObservableSet);
function makeIterableForSet(iterator) {
  iterator[Symbol.toStringTag] = "SetIterator";
  return makeIterable(iterator);
}
var descriptorCache = /* @__PURE__ */ Object.create(null);
var REMOVE = "remove";
var ObservableObjectAdministration = class {
  constructor(target_, values_ = /* @__PURE__ */ new Map(), name_, defaultAnnotation_ = autoAnnotation) {
    this.target_ = void 0;
    this.values_ = void 0;
    this.name_ = void 0;
    this.defaultAnnotation_ = void 0;
    this.keysAtom_ = void 0;
    this.changeListeners_ = void 0;
    this.interceptors_ = void 0;
    this.proxy_ = void 0;
    this.isPlainObject_ = void 0;
    this.appliedAnnotations_ = void 0;
    this.pendingKeys_ = void 0;
    this.lazyComputedKeys_ = void 0;
    this.lazyObservableKeys_ = void 0;
    this.target_ = target_;
    this.values_ = values_;
    this.name_ = name_;
    this.defaultAnnotation_ = defaultAnnotation_;
    this.keysAtom_ = new Atom(true ? `${this.name_}.keys` : "ObservableObject.keys");
    this.isPlainObject_ = isPlainObject(this.target_);
    if (!isAnnotation(this.defaultAnnotation_)) {
      die(`defaultAnnotation must be valid annotation`);
    }
    if (true) {
      this.appliedAnnotations_ = {};
    }
  }
  getObservablePropValue_(key) {
    var _ref, _this$values_$get;
    const observable2 = (_ref = (_this$values_$get = this.values_.get(key)) != null ? _this$values_$get : this.materializeLazyComputed_(key)) != null ? _ref : this.materializeLazyObservable_(key);
    return observable2.get();
  }
  materializeLazyComputed_(key) {
    var _this$lazyComputedKey;
    const factory = (_this$lazyComputedKey = this.lazyComputedKeys_) == null ? void 0 : _this$lazyComputedKey.get(key);
    if (!factory) {
      return void 0;
    }
    this.lazyComputedKeys_.delete(key);
    if (this.lazyComputedKeys_.size === 0) {
      this.lazyComputedKeys_ = void 0;
    }
    const computed3 = factory();
    this.values_.set(key, computed3);
    return computed3;
  }
  materializeLazyObservable_(key) {
    var _this$lazyObservableK;
    const factory = (_this$lazyObservableK = this.lazyObservableKeys_) == null ? void 0 : _this$lazyObservableK.get(key);
    if (!factory) {
      return void 0;
    }
    this.lazyObservableKeys_.delete(key);
    if (this.lazyObservableKeys_.size === 0) {
      this.lazyObservableKeys_ = void 0;
    }
    const observable2 = factory();
    this.values_.set(key, observable2);
    return observable2;
  }
  setObservablePropValue_(key, newValue) {
    var _ref2, _this$values_$get2;
    const observable2 = (_ref2 = (_this$values_$get2 = this.values_.get(key)) != null ? _this$values_$get2 : this.materializeLazyComputed_(key)) != null ? _ref2 : this.materializeLazyObservable_(key);
    if (observable2 instanceof ComputedValue) {
      observable2.set(newValue);
      return true;
    }
    if (hasInterceptors(this)) {
      const change = interceptChange(this, {
        type: UPDATE,
        object: this.proxy_ || this.target_,
        name: key,
        newValue
      });
      if (!change) {
        return null;
      }
      newValue = change.newValue;
    }
    newValue = observable2.prepareNewValue_(newValue);
    if (newValue !== globalState.UNCHANGED) {
      const notify = hasListeners(this);
      const notifySpy = isSpyEnabled();
      const change = notify || notifySpy ? {
        type: UPDATE,
        observableKind: "object",
        debugObjectName: this.name_,
        object: this.proxy_ || this.target_,
        oldValue: observable2.value_,
        name: key,
        newValue
      } : null;
      if (notifySpy) {
        spyReportStart(change);
      }
      observable2.setNewValue_(newValue);
      if (notify) {
        notifyListeners(this, change);
      }
      if (notifySpy) {
        spyReportEnd();
      }
    }
    return true;
  }
  get_(key) {
    if (globalState.trackingDerivation && !hasProp(this.target_, key)) {
      this.has_(key);
    }
    return this.target_[key];
  }
  /**
   * @param {PropertyKey} key
   * @param {any} value
   * @param {Annotation|boolean} annotation true - use default annotation, false - copy as is
   * @param {boolean} proxyTrap whether it's called from proxy trap
   * @returns {boolean|null} true on success, false on failure (proxyTrap + non-configurable), null when cancelled by interceptor
   */
  set_(key, value, proxyTrap = false) {
    if (hasProp(this.target_, key)) {
      if (this.values_.has(key)) {
        return this.setObservablePropValue_(key, value);
      } else if (proxyTrap) {
        return Reflect.set(this.target_, key, value);
      } else {
        this.target_[key] = value;
        return true;
      }
    } else {
      return this.extend_(key, {
        value,
        enumerable: true,
        writable: true,
        configurable: true
      }, this.defaultAnnotation_, proxyTrap);
    }
  }
  // Trap for "in"
  has_(key) {
    if (!globalState.trackingDerivation) {
      return key in this.target_;
    }
    this.pendingKeys_ || (this.pendingKeys_ = /* @__PURE__ */ new Map());
    let entry = this.pendingKeys_.get(key);
    if (!entry) {
      entry = new ObservableValue(key in this.target_, referenceEnhancer, true ? `${this.name_}.${stringifyKey(key)}?` : "ObservableObject.key?", false);
      this.pendingKeys_.set(key, entry);
    }
    return entry.get();
  }
  /**
   * @param {PropertyKey} key
   * @param {PropertyDescriptor} descriptor
   * @param {Annotation|boolean} annotation true - use default annotation, false - copy as is
   * @param {boolean} proxyTrap whether it's called from proxy trap
   * @returns {boolean|null} true on success, false on failure (proxyTrap + non-configurable), null when cancelled by interceptor
   */
  extend_(key, descriptor, annotation, proxyTrap = false) {
    if (annotation === true) {
      annotation = this.defaultAnnotation_;
    }
    if (annotation === false) {
      return this.defineProperty_(key, descriptor, proxyTrap);
    }
    assertAnnotable(this, annotation, key);
    const outcome = annotation.extend_(this, key, descriptor, proxyTrap);
    if (outcome) {
      recordAnnotationApplied(this, annotation, key);
    }
    return outcome;
  }
  /**
   * @param {PropertyKey} key
   * @param {PropertyDescriptor} descriptor
   * @param {boolean} proxyTrap whether it's called from proxy trap
   * @returns {boolean|null} true on success, false on failure (proxyTrap + non-configurable), null when cancelled by interceptor
   */
  defineProperty_(key, descriptor, proxyTrap = false) {
    checkIfStateModificationsAreAllowed(this.keysAtom_);
    try {
      startBatch();
      const deleteOutcome = this.delete_(key);
      if (!deleteOutcome) {
        return deleteOutcome;
      }
      if (hasInterceptors(this)) {
        const change = interceptChange(this, {
          object: this.proxy_ || this.target_,
          name: key,
          type: ADD,
          newValue: descriptor.value
        });
        if (!change) {
          return null;
        }
        const {
          newValue
        } = change;
        if (descriptor.value !== newValue) {
          descriptor = assign({}, descriptor, {
            value: newValue
          });
        }
      }
      if (proxyTrap) {
        if (!Reflect.defineProperty(this.target_, key, descriptor)) {
          return false;
        }
      } else {
        defineProperty(this.target_, key, descriptor);
      }
      this.notifyPropertyAddition_(key, descriptor.value);
    } finally {
      endBatch();
    }
    return true;
  }
  // If original descriptor becomes relevant, move this to annotation directly
  defineObservableProperty_(key, value, enhancer, proxyTrap = false) {
    checkIfStateModificationsAreAllowed(this.keysAtom_);
    try {
      startBatch();
      const deleteOutcome = this.delete_(key);
      if (!deleteOutcome) {
        return deleteOutcome;
      }
      if (hasInterceptors(this)) {
        const change = interceptChange(this, {
          object: this.proxy_ || this.target_,
          name: key,
          type: ADD,
          newValue: value
        });
        if (!change) {
          return null;
        }
        value = change.newValue;
      }
      const cachedDescriptor = getCachedObservablePropDescriptor(key);
      const descriptor = {
        configurable: globalState.safeDescriptors ? this.isPlainObject_ : true,
        enumerable: true,
        get: cachedDescriptor.get,
        set: cachedDescriptor.set
      };
      if (proxyTrap) {
        if (!Reflect.defineProperty(this.target_, key, descriptor)) {
          return false;
        }
      } else {
        defineProperty(this.target_, key, descriptor);
      }
      const observable2 = new ObservableValue(value, enhancer, true ? `${this.name_}.${key.toString()}` : "ObservableObject.key", false);
      this.values_.set(key, observable2);
      this.notifyPropertyAddition_(key, observable2.value_);
    } finally {
      endBatch();
    }
    return true;
  }
  // If original descriptor becomes relevant, move this to annotation directly
  defineComputedProperty_(key, options, proxyTrap = false) {
    checkIfStateModificationsAreAllowed(this.keysAtom_);
    try {
      startBatch();
      const deleteOutcome = this.delete_(key);
      if (!deleteOutcome) {
        return deleteOutcome;
      }
      if (hasInterceptors(this)) {
        const change = interceptChange(this, {
          object: this.proxy_ || this.target_,
          name: key,
          type: ADD,
          newValue: void 0
        });
        if (!change) {
          return null;
        }
      }
      options.name || (options.name = true ? `${this.name_}.${key.toString()}` : "ObservableObject.key");
      options.context = this.proxy_ || this.target_;
      const cachedDescriptor = getCachedObservablePropDescriptor(key);
      const descriptor = {
        configurable: globalState.safeDescriptors ? this.isPlainObject_ : true,
        enumerable: false,
        get: cachedDescriptor.get,
        set: cachedDescriptor.set
      };
      if (proxyTrap) {
        if (!Reflect.defineProperty(this.target_, key, descriptor)) {
          return false;
        }
      } else {
        defineProperty(this.target_, key, descriptor);
      }
      this.values_.set(key, new ComputedValue(options));
      this.notifyPropertyAddition_(key, void 0);
    } finally {
      endBatch();
    }
    return true;
  }
  /**
   * @param {PropertyKey} key
   * @param {PropertyDescriptor} descriptor
   * @param {boolean} proxyTrap whether it's called from proxy trap
   * @returns {boolean|null} true on success, false on failure (proxyTrap + non-configurable), null when cancelled by interceptor
   */
  delete_(key, proxyTrap = false) {
    checkIfStateModificationsAreAllowed(this.keysAtom_);
    if (!hasProp(this.target_, key)) {
      return true;
    }
    if (hasInterceptors(this)) {
      const change = interceptChange(this, {
        object: this.proxy_ || this.target_,
        name: key,
        type: REMOVE
      });
      if (!change) {
        return null;
      }
    }
    try {
      var _this$pendingKeys_;
      startBatch();
      const notify = hasListeners(this);
      const notifySpy = isSpyEnabled();
      const observable2 = this.values_.get(key);
      let value = void 0;
      if (!observable2 && (notify || notifySpy)) {
        var _getDescriptor2;
        value = (_getDescriptor2 = getDescriptor(this.target_, key)) == null ? void 0 : _getDescriptor2.value;
      }
      if (proxyTrap) {
        if (!Reflect.deleteProperty(this.target_, key)) {
          return false;
        }
      } else {
        delete this.target_[key];
      }
      if (true) {
        delete this.appliedAnnotations_[key];
      }
      if (observable2) {
        this.values_.delete(key);
        if (observable2 instanceof ObservableValue) {
          value = observable2.value_;
        }
        propagateChanged(observable2);
      }
      this.keysAtom_.reportChanged();
      (_this$pendingKeys_ = this.pendingKeys_) == null || (_this$pendingKeys_ = _this$pendingKeys_.get(key)) == null || _this$pendingKeys_.set(key in this.target_);
      if (notify || notifySpy) {
        const change = {
          type: REMOVE,
          observableKind: "object",
          object: this.proxy_ || this.target_,
          debugObjectName: this.name_,
          oldValue: value,
          name: key
        };
        if (notifySpy) {
          spyReportStart(change);
        }
        if (notify) {
          notifyListeners(this, change);
        }
        if (notifySpy) {
          spyReportEnd();
        }
      }
    } finally {
      endBatch();
    }
    return true;
  }
  notifyPropertyAddition_(key, value) {
    var _this$pendingKeys_2;
    const notify = hasListeners(this);
    const notifySpy = isSpyEnabled();
    if (notify || notifySpy) {
      const change = notify || notifySpy ? {
        type: ADD,
        observableKind: "object",
        debugObjectName: this.name_,
        object: this.proxy_ || this.target_,
        name: key,
        newValue: value
      } : null;
      if (notifySpy) {
        spyReportStart(change);
      }
      if (notify) {
        notifyListeners(this, change);
      }
      if (notifySpy) {
        spyReportEnd();
      }
    }
    (_this$pendingKeys_2 = this.pendingKeys_) == null || (_this$pendingKeys_2 = _this$pendingKeys_2.get(key)) == null || _this$pendingKeys_2.set(true);
    this.keysAtom_.reportChanged();
  }
  ownKeys_() {
    this.keysAtom_.reportObserved();
    return ownKeys(this.target_);
  }
  keys_() {
    this.keysAtom_.reportObserved();
    return Object.keys(this.target_);
  }
};
function asObservableObject(target, options) {
  var _options$name;
  if (options && isObservableObject(target)) {
    die(`Options can't be provided for already observable objects.`);
  }
  if (hasProp(target, $mobx)) {
    if (!(getAdministration(target) instanceof ObservableObjectAdministration)) {
      die(`Cannot convert '${getDebugName(target)}' into observable object:
The target is already observable of different type.
Extending builtins is not supported.`);
    }
    return target;
  }
  if (!Object.isExtensible(target)) {
    die("Cannot make the designated object observable; it is not extensible");
  }
  const name = (_options$name = options == null ? void 0 : options.name) != null ? _options$name : true ? `${isPlainObject(target) ? "ObservableObject" : target.constructor.name}@${getNextId()}` : "ObservableObject";
  const adm = new ObservableObjectAdministration(target, /* @__PURE__ */ new Map(), String(name), getAnnotationFromOptions(options));
  addHiddenProp(target, $mobx, adm);
  return target;
}
var isObservableObjectAdministration = /* @__PURE__ */ createInstanceofPredicate("ObservableObjectAdministration", ObservableObjectAdministration);
function getCachedObservablePropDescriptor(key) {
  return descriptorCache[key] || (descriptorCache[key] = {
    get() {
      return this[$mobx].getObservablePropValue_(key);
    },
    set(value) {
      return this[$mobx].setObservablePropValue_(key, value);
    }
  });
}
function isObservableObject(thing) {
  if (isObject(thing)) {
    return isObservableObjectAdministration(thing[$mobx]);
  }
  return false;
}
function recordAnnotationApplied(adm, annotation, key) {
  if (true) {
    adm.appliedAnnotations_[key] = annotation;
  }
}
function assertAnnotable(adm, annotation, key) {
  if (!isAnnotation(annotation)) {
    die(`Cannot annotate '${adm.name_}.${key.toString()}': Invalid annotation.`);
  }
  if (!isOverride(annotation) && hasProp(adm.appliedAnnotations_, key)) {
    const fieldName = `${adm.name_}.${key.toString()}`;
    const currentAnnotationType = adm.appliedAnnotations_[key].annotationType_;
    const requestedAnnotationType = annotation.annotationType_;
    die(`Cannot apply '${requestedAnnotationType}' to '${fieldName}':
The field is already annotated with '${currentAnnotationType}'.
Re-annotating fields is not allowed.
Use 'override' annotation for methods overridden by subclass.`);
  }
}
function getAtom(thing, property) {
  if (typeof thing === "object" && thing !== null) {
    if (isObservableArray(thing)) {
      if (property !== void 0) {
        die(23);
      }
      return thing[$mobx].atom_;
    }
    if (isObservableSet(thing)) {
      return thing.atom_;
    }
    if (isObservableMap(thing)) {
      if (property === void 0) {
        return thing.keysAtom_;
      }
      const observable2 = thing.data_.get(property) || thing.hasMap_.get(property);
      if (!observable2) {
        die(25, property, getDebugName(thing));
      }
      return observable2;
    }
    if (isObservableObject(thing)) {
      var _ref, _adm$values_$get;
      if (!property) {
        return die(26);
      }
      const adm = thing[$mobx];
      const observable2 = (_ref = (_adm$values_$get = adm.values_.get(property)) != null ? _adm$values_$get : adm.materializeLazyComputed_(property)) != null ? _ref : adm.materializeLazyObservable_(property);
      if (!observable2) {
        die(27, property, getDebugName(thing));
      }
      return observable2;
    }
    if (isAtom(thing) || isComputedValue(thing) || isReaction(thing)) {
      return thing;
    }
  } else if (isFunction(thing)) {
    if (isReaction(thing[$mobx])) {
      return thing[$mobx];
    }
  }
  die(28);
}
function getAdministration(thing, property) {
  if (!thing) {
    die(29);
  }
  if (property !== void 0) {
    return getAdministration(getAtom(thing, property));
  }
  if (isAtom(thing) || isComputedValue(thing) || isReaction(thing)) {
    return thing;
  }
  if (isObservableMap(thing) || isObservableSet(thing)) {
    return thing;
  }
  if (thing[$mobx]) {
    return thing[$mobx];
  }
  die(24, thing);
}
function getDebugName(thing, property) {
  let named;
  if (property !== void 0) {
    named = getAtom(thing, property);
  } else if (isAction(thing)) {
    return thing.name;
  } else if (isObservableObject(thing) || isObservableMap(thing) || isObservableSet(thing)) {
    named = getAdministration(thing);
  } else {
    named = getAtom(thing);
  }
  return named.name_;
}
function initObservable(cb) {
  const derivation = untrackedStart();
  const allowStateChanges2 = true ? allowStateChangesStart(true) : true;
  startBatch();
  try {
    return cb();
  } finally {
    endBatch();
    if (true) {
      allowStateChangesEnd(allowStateChanges2);
    }
    untrackedEnd(derivation);
  }
}
var toString = objectPrototype.toString;
function deepEqual(a, b, depth = -1) {
  return eq(a, b, depth);
}
function eq(a, b, depth, aStack, bStack) {
  if (a === b) {
    return a !== 0 || 1 / a === 1 / b;
  }
  if (a == null || b == null) {
    return false;
  }
  if (a !== a) {
    return b !== b;
  }
  const type = typeof a;
  if (type !== "function" && type !== "object" && typeof b != "object") {
    return false;
  }
  const className = toString.call(a);
  if (className !== toString.call(b)) {
    return false;
  }
  switch (className) {
    // Strings, numbers, regular expressions, dates, and booleans are compared by value.
    case "[object RegExp]":
    // RegExps are coerced to strings for comparison (Note: '' + /a/i === '/a/i')
    case "[object String]":
      return "" + a === "" + b;
    case "[object Number]":
      if (+a !== +a) {
        return +b !== +b;
      }
      return +a === 0 ? 1 / +a === 1 / b : +a === +b;
    case "[object Date]":
    case "[object Boolean]":
      return +a === +b;
    case "[object Symbol]":
      return typeof Symbol !== "undefined" && Symbol.valueOf.call(a) === Symbol.valueOf.call(b);
    case "[object Map]":
    case "[object Set]":
      if (depth >= 0) {
        depth++;
      }
      break;
  }
  a = unwrap(a);
  b = unwrap(b);
  const areArrays = className === "[object Array]";
  if (!areArrays) {
    if (typeof a != "object" || typeof b != "object") {
      return false;
    }
    const aCtor = a.constructor, bCtor = b.constructor;
    if (aCtor !== bCtor && !(isFunction(aCtor) && aCtor instanceof aCtor && isFunction(bCtor) && bCtor instanceof bCtor) && "constructor" in a && "constructor" in b) {
      return false;
    }
  }
  if (depth === 0) {
    return false;
  } else if (depth < 0) {
    depth = -1;
  }
  aStack = aStack || [];
  bStack = bStack || [];
  let length = aStack.length;
  while (length--) {
    if (aStack[length] === a) {
      return bStack[length] === b;
    }
  }
  aStack.push(a);
  bStack.push(b);
  if (areArrays) {
    length = a.length;
    if (length !== b.length) {
      return false;
    }
    while (length--) {
      if (!eq(a[length], b[length], depth - 1, aStack, bStack)) {
        return false;
      }
    }
  } else {
    const keys2 = Object.keys(a);
    const _length = keys2.length;
    if (Object.keys(b).length !== _length) {
      return false;
    }
    for (let i = 0; i < _length; i++) {
      const key = keys2[i];
      if (!(hasProp(b, key) && eq(a[key], b[key], depth - 1, aStack, bStack))) {
        return false;
      }
    }
  }
  aStack.pop();
  bStack.pop();
  return true;
}
function unwrap(a) {
  if (isObservableArray(a)) {
    return a.slice();
  }
  if (isES6Map(a) || isObservableMap(a)) {
    return Array.from(a.entries());
  }
  if (isES6Set(a) || isObservableSet(a)) {
    return Array.from(a.entries());
  }
  return a;
}
var _globalThis$Iterator;
var maybeIteratorPrototype = ((_globalThis$Iterator = globalThis.Iterator) == null ? void 0 : _globalThis$Iterator.prototype) || {};
function makeIterable(iterator) {
  iterator[Symbol.iterator] = getSelf;
  return assign(Object.create(maybeIteratorPrototype), iterator);
}
function getSelf() {
  return this;
}
function isAnnotation(thing) {
  return (
    // Can be function
    thing instanceof Object && typeof thing.annotationType_ === "string" && isFunction(thing.make_) && isFunction(thing.extend_)
  );
}
if (true) {
  const g = globalThis;
  ["Symbol", "Map", "Set", "Proxy"].forEach((m) => {
    if (typeof g[m] === "undefined") {
      die(`MobX requires global '${m}' to be available or polyfilled`);
    }
  });
}
if (typeof __MOBX_DEVTOOLS_GLOBAL_HOOK__ === "object") {
  __MOBX_DEVTOOLS_GLOBAL_HOOK__.injectMobx({
    spy,
    extras: {
      getDebugName
    },
    $mobx
  });
}
export {
  $mobx,
  FlowCancellationError,
  ObservableMap,
  ObservableSet,
  Reaction,
  allowStateChanges as _allowStateChanges,
  runInAction as _allowStateChangesInsideComputed,
  allowStateReadsEnd as _allowStateReadsEnd,
  allowStateReadsStart as _allowStateReadsStart,
  autoAction as _autoAction,
  autoActionBound as _autoActionBound,
  _endAction,
  getAdministration as _getAdministration,
  getGlobalState as _getGlobalState,
  interceptReads as _interceptReads,
  isComputingDerivation as _isComputingDerivation,
  resetGlobalState as _resetGlobalState,
  _startAction,
  action,
  actionBound,
  autorun,
  compareDefault,
  compareIdentity,
  compareShallow,
  compareStructural,
  computed,
  computedStruct,
  configure,
  createAtom,
  apiDefineProperty as defineProperty,
  entries,
  extendObservable,
  flow,
  flowBound,
  flowResult,
  get,
  getAtom,
  getDebugName,
  getDependencyTree,
  getObserverTree,
  has,
  intercept,
  isAction,
  isObservableValue as isBoxedObservable,
  isComputed,
  isComputedProp,
  isFlow,
  isFlowCancellationError,
  isObservable,
  isObservableArray,
  isObservableMap,
  isObservableObject,
  isObservableProp,
  isObservableSet,
  keys,
  makeAutoObservable,
  makeObservable,
  observable,
  observableDeep,
  observableRef,
  observableShallow,
  observableStruct,
  observe,
  onBecomeObserved,
  onBecomeUnobserved,
  onReactionError,
  override,
  apiOwnKeys as ownKeys,
  reaction,
  remove,
  runInAction,
  set,
  spy,
  toJS,
  transaction,
  untracked,
  values,
  when
};
