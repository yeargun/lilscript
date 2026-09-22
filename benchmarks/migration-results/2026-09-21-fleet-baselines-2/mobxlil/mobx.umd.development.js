"use strict";
var mobx = (() => {
  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, { get: all[name], enumerable: true });
  };
  var __copyProps = (to, from, except, desc) => {
    if (from && typeof from === "object" || typeof from === "function") {
      for (let key of __getOwnPropNames(from))
        if (!__hasOwnProp.call(to, key) && key !== except)
          __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
    }
    return to;
  };
  var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

  // dist/mobx.dev.esm.js
  var mobx_dev_esm_exports = {};
  __export(mobx_dev_esm_exports, {
    $mobx: () => $mobx,
    FlowCancellationError: () => FlowCancellationError,
    ObservableMap: () => ObservableMap,
    ObservableSet: () => ObservableSet,
    Reaction: () => Reaction,
    _allowStateChanges: () => _allowStateChanges,
    _allowStateChangesInsideComputed: () => _allowStateChangesInsideComputed,
    _allowStateReadsEnd: () => _allowStateReadsEnd,
    _allowStateReadsStart: () => _allowStateReadsStart,
    _autoAction: () => _autoAction,
    _autoActionBound: () => _autoActionBound,
    _endAction: () => _endAction,
    _getAdministration: () => _getAdministration,
    _getGlobalState: () => _getGlobalState,
    _interceptReads: () => _interceptReads,
    _isComputingDerivation: () => _isComputingDerivation,
    _resetGlobalState: () => _resetGlobalState,
    _startAction: () => _startAction,
    action: () => action,
    actionBound: () => actionBound,
    autorun: () => autorun,
    compareDefault: () => compareDefault,
    compareIdentity: () => compareIdentity,
    compareShallow: () => compareShallow,
    compareStructural: () => compareStructural,
    computed: () => computed,
    computedStruct: () => computedStruct,
    configure: () => configure,
    createAtom: () => createAtom,
    defineProperty: () => defineProperty,
    entries: () => entries,
    extendObservable: () => extendObservable,
    flow: () => flow,
    flowBound: () => flowBound,
    flowResult: () => flowResult,
    get: () => get,
    getAtom: () => getAtom,
    getDebugName: () => getDebugName,
    getDependencyTree: () => getDependencyTree,
    getObserverTree: () => getObserverTree,
    has: () => has,
    intercept: () => intercept,
    isAction: () => isAction,
    isBoxedObservable: () => isBoxedObservable,
    isComputed: () => isComputed,
    isComputedProp: () => isComputedProp,
    isFlow: () => isFlow,
    isFlowCancellationError: () => isFlowCancellationError,
    isObservable: () => isObservable,
    isObservableArray: () => isObservableArray,
    isObservableMap: () => isObservableMap,
    isObservableObject: () => isObservableObject,
    isObservableProp: () => isObservableProp,
    isObservableSet: () => isObservableSet,
    keys: () => keys,
    makeAutoObservable: () => makeAutoObservable,
    makeObservable: () => makeObservable,
    observable: () => observable,
    observableDeep: () => observableDeep,
    observableRef: () => observableRef,
    observableShallow: () => observableShallow,
    observableStruct: () => observableStruct,
    observe: () => observe,
    onBecomeObserved: () => onBecomeObserved,
    onBecomeUnobserved: () => onBecomeUnobserved,
    onReactionError: () => onReactionError,
    override: () => override,
    ownKeys: () => ownKeys,
    reaction: () => reaction,
    remove: () => remove,
    runInAction: () => runInAction,
    set: () => set,
    spy: () => spy,
    toJS: () => toJS,
    transaction: () => transaction,
    untracked: () => untracked,
    values: () => values,
    when: () => when
  });
  var $jsMethod1 = (e) => function(a2) {
    return e(this, a2);
  };
  var a = -1;
  function $m3$isObj(value) {
    return !(value == null) && "object" == typeof value;
  }
  function $m3$isStringish(value) {
    var v2 = typeof value;
    var v10, v15;
    return "string" == v2 || "symbol" == v2 || "number" == v2;
  }
  function $m3$protoIsPrototypeOf(ctor, value) {
    if (value == null) return false;
    return true === $m3$isPrototypeOfFn.call(ctor.prototype, value);
  }
  function $m3$arrayFrom(iterable) {
    return Array.from(iterable);
  }
  function $m3$defineProperty(target, key, descriptor) {
    $m3$objectCtor.defineProperty(target, key, descriptor);
  }
  function $m3$hasOwn(target, key) {
    return !!$m3$hasOwnPropertyFn.call(target, key);
  }
  function $m3$assign(target, source) {
    return $m3$objectCtor.assign(target, source);
  }
  function $m3$defineGetter(object, propName, getter) {
    $m3$defineProperty(object, propName, { enumerable: false, configurable: true, get: getter });
  }
  function $m3$createInstanceofPredicate(name, theClass) {
    let v4 = "isMobX" + name;
    theClass.prototype[v4] = true;
    return function(x) {
      return $m3$isObj(x) && true === x[v4];
    };
  }
  function $m3$throwErr(err) {
    throw err;
  }
  function $m3$throwTypeError(message) {
    throw new TypeError(message);
  }
  function $m3$isGenerator(obj) {
    if (obj == null) return false;
    var v6 = obj.constructor;
    if (!v6) return false;
    if ("GeneratorFunction" == v6.name + "") return true;
    if ("GeneratorFunction" == v6.displayName + "") return true;
    return false;
  }
  function $m3$isPlainObject(value) {
    if (!$m3$isObj(value)) return false;
    var v29 = $m3$objectCtor.getPrototypeOf(value);
    if (v29 == null) return true;
    var v30 = void 0;
    if ($m3$hasOwn(v29, "constructor")) {
      var v162 = v29.constructor;
      var v26 = v162, v24, v25;
    } else {
      v26 = v30;
    }
    "function" == typeof v26 ? (v24 = v26.toString() === $m3$plainObjectString, v25 = v24) : v25 = false;
    return v25;
  }
  function $m3$isES6Map(thing) {
    if (thing == null) return false;
    var v6 = $m3$objectCtor.prototype;
    return "[object Map]" == v6.toString.call(thing) + "";
  }
  function $m3$isES6Set(thing) {
    if (thing == null) return false;
    var v6 = $m3$objectCtor.prototype;
    return "[object Set]" == v6.toString.call(thing) + "";
  }
  function $m3$stringifyKey(key) {
    var v2 = typeof key;
    if ("string" == v2) return key + "";
    if ("symbol" == v2) return key.toString() + "";
    return new String(key) + "";
  }
  function $m3$toPrimitive(value) {
    if (null === value) return null;
    return "object" == typeof value ? "" + value : value;
  }
  function $m3$getPlainObjectKeys(object) {
    var v42 = $m3$objectCtor.keys(object);
    var v6 = $m3$objectCtor.getOwnPropertySymbols(object);
    if (0 == v6.length) return v42;
    var v49 = $m3$arraySliceFn.call(v42);
    var v53 = v6.length;
    var v39 = 0, v22, v25;
    while (v39 < v53) {
      v22 = v6[v39];
      v25 = $m3$objectCtor.prototype;
      true === v25.propertyIsEnumerable.call(object, v22) && v49.push(v22);
      v39 = v39 + 1;
    }
    return v49;
  }
  function $m3$makeIterable(iterator) {
    iterator[Symbol.iterator] = $m3$getSelf;
    if (!$m3$iteratorProtoReady) {
      $m3$iteratorProtoReady = true;
      var v21 = globalThis.Iterator;
      $m3$iteratorProtoCached = v21 ? v21.prototype : {};
    }
    var v23 = $m3$iteratorProtoCached;
    return $m3$assign($m3$objectCtor.create(v23), iterator);
  }
  function $m3$once(fn) {
    var invoked = false;
    return function() {
      if (invoked) return;
      invoked = true;
      return fn.apply(this, arguments);
    };
  }
  function $m4$niceError(error, args) {
    if ("string" == typeof error) return error + "";
    var v336 = +error | 0;
    if (0 == v336) return "Invalid value for configuration 'enforceActions', expected 'never', 'always' or 'observed'";
    if (1 == v336) {
      var v22 = "Cannot apply '" + args[0] + "' to '";
      return v22 + $m3$stringifyKey(args[1]) + "': Field not found.";
    }
    if (5 == v336) return "'keys()' can only be used on observable objects, arrays, sets and maps";
    if (6 == v336) return "'values()' can only be used on observable objects, arrays, sets and maps";
    if (7 == v336) return "'entries()' can only be used on observable objects, arrays and maps";
    if (8 == v336) return "'set()' can only be used on observable objects, arrays and maps";
    if (9 == v336) return "'remove()' can only be used on observable objects, arrays and maps";
    if (10 == v336) return "'has()' can only be used on observable objects, arrays and maps";
    if (11 == v336) return "'get()' can only be used on observable objects, arrays and maps";
    if (12 == v336) return "Invalid annotation";
    if (13 == v336) return "Dynamic observable objects cannot be frozen. If you're passing observables to 3rd party component/function that calls Object.freeze, pass copy instead: toJS(observable)";
    if (14 == v336) return "Intercept handlers should return nothing or a change object";
    if (15 == v336) return "Observable arrays cannot be frozen. If you're passing observables to 3rd party component/function that calls Object.freeze, pass copy instead: toJS(observable)";
    if (16 == v336) return "Modification exception: the internal structure of an observable array was changed.";
    if (19 == v336) {
      var v84 = args[0];
      var v86 = v84.constructor;
      return "Cannot initialize from classes that inherit from Map: " + v86.name;
    }
    if (20 == v336) return "Cannot initialize map from " + args[0];
    if (21 == v336) return "Cannot convert to map from '" + args[0] + "'";
    if (23 == v336) return "It is not possible to get index atoms from arrays";
    if (24 == v336) return "Cannot obtain administration from " + args[0];
    if (25 == v336) {
      var v134 = "the entry '" + args[0] + "' does not exist in the observable map '";
      return v134 + args[1] + "'";
    }
    if (26 == v336) return "please specify a property";
    if (27 == v336) {
      var v156 = "no observable property '" + $m3$stringifyKey(args[0]) + "' found on the observable object '";
      return v156 + args[1] + "'";
    }
    if (28 == v336) return "Cannot obtain atom from " + args[0];
    if (29 == v336) return "Expecting some object";
    if (30 == v336) return "invalid action stack. did you forget to finish an action?";
    if (31 == v336) return "missing option for computed: get";
    if (32 == v336) {
      var v195 = "Cycle detected in computation " + args[0] + ": ";
      return v195 + args[1];
    }
    if (33 == v336) return "The setter of computed value '" + args[0] + "' is trying to update itself. Did you intend to update an _observable_ value, instead of the computed property?";
    if (34 == v336) return "[ComputedValue '" + args[0] + "'] It is not possible to assign a new value to a computed value.";
    if (35 == v336) return "There are multiple, different versions of MobX active. Make sure MobX is loaded only once or use `configure({ isolateGlobalState: true })`";
    if (36 == v336) return "isolateGlobalState should be called before MobX is running any reactions";
    if (37 == v336) return "[mobx] `observableArray." + args[0] + "()` mutates the array in-place, which is not allowed inside a derivation. Use `array.slice()." + args[0] + "()` instead";
    if (38 == v336) return "'ownKeys()' can only be used on observable objects";
    if (39 == v336) return "'defineProperty()' can only be used on observable objects";
    if (40 == v336) return "Out of range: " + args[0];
    if (41 == v336) return "Cannot initialize set from " + args[0];
    if (42 == v336) return "Invalid index: '" + args[0] + "'";
    if (43 == v336) {
      var v296 = "Cannot apply '" + args[0] + "' to '";
      var v303 = v296 + args[1] + "' (kind: ";
      return v303 + args[2] + "):\n'" + args[0] + "' can only be used on properties with a function value.";
    }
    if (44 == v336) return "'" + args[0] + "' can only be used with 'makeObservable'";
    return error + "";
  }
  function $m4$dieRest(error, args) {
    $m3$throwErr(new Error("[MobX] " + $m4$niceError(error, args)));
    if (args.length > 0) {
      var v24 = " " + args.map(String).join(",");
      var v35 = v24;
    } else {
      v35 = "";
    }
    $m3$throwErr(new Error("[MobX] minified error nr: " + error + v35 + ". See mobx.js.org/errors"));
  }
  function $m4$die(error) {
    $m4$dieRest(error, []);
  }
  function $m4$die1(error, a$2) {
    let v2 = [];
    v2.push(a$2);
    $m4$dieRest(error, v2);
  }
  function $m7$isAtom(x) {
    return $m3$isObj(x) && true === x.isMobXAtom;
  }
  function $m7$isComputedValue(x) {
    return $m3$isObj(x) && true === x.isMobXComputedValue;
  }
  function $m7$isReaction(x) {
    return $m3$isObj(x) && true === x.isMobXReaction;
  }
  function $m7$isObservableMap(x) {
    return !!$m7$isObservableMapPred(x);
  }
  function $m7$isObservableSet(x) {
    return !!$m7$isObservableSetPred(x);
  }
  function $m7$isObservableArray(thing) {
    if (!$m3$isObj(thing)) return false;
    return !!$m7$isObservableArrayAdmPred(thing[$m2$$mobx]);
  }
  function $m7$isObservableObject(thing) {
    if (!$m3$isObj(thing)) return false;
    return !!$m7$isObservableObjectAdmPred(thing[$m2$$mobx]);
  }
  function $m7$isAction(thing) {
    return "function" == typeof thing && true === thing.isMobxAction;
  }
  function $m7$isFlow(fn) {
    if (fn == null) return false;
    var v6 = fn.isMobXFlow;
    return true === v6;
  }
  function $m8$initPersistentKeys() {
    $m8$persistentKeys.push("mobxGuid");
    $m8$persistentKeys.push("spyListeners");
    $m8$persistentKeys.push("enforceActions");
    $m8$persistentKeys.push("computedRequiresReaction");
    $m8$persistentKeys.push("reactionRequiresObservable");
    $m8$persistentKeys.push("observableRequiresReaction");
    $m8$persistentKeys.push("allowStateReads");
    $m8$persistentKeys.push("disableErrorBoundaries");
    $m8$persistentKeys.push("runId");
    $m8$persistentKeys.push("UNCHANGED");
  }
  function $m8$createMobXGlobals() {
    let v0 = {};
    v0.version = 7;
    v0.UNCHANGED = {};
    let v9 = null;
    v0.trackingDerivation = v9;
    v0.trackingContext = v9;
    v0.runId = 0;
    v0.mobxGuid = 0;
    v0.inBatch = 0;
    v0.pendingUnobservations = [];
    v0.pendingReactions = [];
    v0.isRunningReactions = false;
    v0.allowStateChanges = false;
    v0.allowStateReads = true;
    v0.enforceActions = true;
    v0.spyListeners = [];
    v0.globalReactionErrorHandlers = [];
    v0.computedRequiresReaction = false;
    v0.reactionRequiresObservable = false;
    v0.observableRequiresReaction = false;
    v0.disableErrorBoundaries = false;
    v0.suppressReactionErrors = false;
    v0.safeDescriptors = true;
    return v0;
  }
  function $m8$initGlobalState() {
    var v69 = globalThis;
    if (v69.__mobxInstanceCount) {
      var v112 = (+v69.__mobxInstanceCount | 0) > 0;
      var v12 = v112, v18, v19, v23, v32, v33, v64;
    } else {
      v12 = false;
    }
    v12 && !v69.__mobxGlobals && ($m8$canMergeGlobalState = false);
    v23 = v69.__mobxGlobals;
    v23 ? (v32 = 7 != (+v23.version | 0), v33 = v32) : v33 = false;
    v33 && ($m8$canMergeGlobalState = false);
    if (!$m8$canMergeGlobalState) return setTimeout(function() {
      $m8$isolateCalled || $m4$die(35);
    }, 1), $m8$createMobXGlobals();
    else {
      if (v23) {
        var v51 = (+v69.__mobxInstanceCount | 0) + 1 | 0;
        v69.__mobxInstanceCount = v51;
        v23.UNCHANGED || (v23.UNCHANGED = {});
        return v23;
      }
    }
    v69.__mobxInstanceCount = 1;
    v64 = $m8$createMobXGlobals();
    v69.__mobxGlobals = v64;
    return v64;
  }
  function $m8$getNextId() {
    let v5 = (+$m8$globalState.mobxGuid | 0) + 1 | 0;
    $m8$globalState.mobxGuid = v5;
    return v5;
  }
  function $m9$isSpyEnabled() {
    var v4 = $m8$globalState.spyListeners;
    var v7 = v4.length > 0;
    return v7;
  }
  function $m9$spyReport(event) {
    var v5 = $m8$globalState.spyListeners;
    var v28 = v5.length;
    if (0 == v28) return;
    var v24 = 0;
    while (v24 < v28) {
      v5[v24](event);
      v24 = v24 + 1;
    }
  }
  function $m9$spyReportStart(event) {
    $m9$spyReport($m3$objectCtor.assign({}, event, { spyReportStart: true }));
  }
  function $m9$spyReportEnd(change) {
    change ? $m9$spyReport($m3$objectCtor.assign({}, change, { type: "report-end", spyReportEnd: true })) : $m9$spyReport($m9$END_EVENT);
  }
  function $m10$hasObservers(observable$2) {
    if (observable$2.observers_) {
      var v8 = observable$2.observers_;
      var v112 = !!v8.size;
      var v12 = v112;
    } else {
      v12 = false;
    }
    return v12;
  }
  function $m10$queueForUnobservation(observable$2) {
    if (!observable$2.isPendingUnobservation) {
      observable$2.isPendingUnobservation = true;
      var v112 = $m8$globalState.pendingUnobservations;
      v112.push(observable$2);
    }
  }
  function $m10$removeObserver(observable$2, node) {
    var v4 = observable$2.observers_;
    v4.delete(node);
    var v9 = observable$2.observers_;
    v9.size || $m10$queueForUnobservation(observable$2);
  }
  function $m10$startBatch() {
    let v0 = $m8$globalState;
    v0.inBatch = (+$m8$globalState.inBatch | 0) + 1 | 0;
  }
  function $m10$endBatch() {
    var v5 = (+$m8$globalState.inBatch | 0) - 1 | 0;
    $m8$globalState.inBatch = v5;
    if (0 == v5) {
      $m10$runReactions();
      var v15 = $m8$globalState.pendingUnobservations;
      var v55 = 0, v23, v29;
      while (v55 < v15.length) {
        v23 = v15[v55];
        v23.isPendingUnobservation = false;
        v29 = v23.observers_;
        v29.size || (!v23.isBeingObserved || (v23.isBeingObserved = false, v23.onBUO()), $m7$isComputedValue(v23) && v23.suspend_());
        v55 = v55 + 1;
      }
      $m8$globalState.pendingUnobservations = [];
    }
  }
  function $m10$reportObserved(observable$2) {
    var v7 = !$m8$globalState.allowStateReads;
    v7 && $m8$globalState.observableRequiresReaction && console.warn("[mobx] Observable '" + observable$2.name_ + "' being read outside a reactive context.");
    var v26 = $m8$globalState.trackingDerivation;
    if (!(v26 == null)) {
      var v32 = v26.runId_;
      if (!(v32 === observable$2.lastAccessedBy_)) {
        observable$2.lastAccessedBy_ = v26.runId_;
        var v99 = +v26.unboundDepsCount_ | 0;
        v26.newObserving_[v99] = observable$2;
        v26.unboundDepsCount_ = v99 + 1 | 0;
        if (!observable$2.isBeingObserved) {
          var v66 = !!$m8$globalState.trackingContext;
          var v67 = v66, v13, v14, v802, v89, v90;
        } else {
          v67 = false;
        }
        v67 && (observable$2.isBeingObserved = true, observable$2.onBO());
      }
      return !!observable$2.isBeingObserved;
    } else {
      v802 = observable$2.observers_;
      !v802.size ? (v89 = !!$m8$globalState.inBatch, v90 = v89) : v90 = false;
      v90 && $m10$queueForUnobservation(observable$2);
    }
    return false;
  }
  function $m10$propagateChanged(observable$2) {
    var v32 = observable$2.lowestObserverState_;
    if (2 === v32) return;
    observable$2.lowestObserverState_ = 2;
    observable$2.observers_.forEach($m10$propagateChangedEach);
  }
  function $m10$propagateChangeConfirmed(observable$2) {
    var v32 = observable$2.lowestObserverState_;
    if (2 === v32) return;
    observable$2.lowestObserverState_ = 2;
    var v9 = $m10$confirmedObservable;
    $m10$confirmedObservable = observable$2;
    observable$2.observers_.forEach($m10$propagateChangeConfirmedEach);
    $m10$confirmedObservable = v9;
  }
  function $m10$reportChanged(observable$2) {
    $m10$startBatch();
    $m10$propagateChanged(observable$2);
    $m10$endBatch();
  }
  function $m10$propagateMaybeChanged(observable$2) {
    if (observable$2.lowestObserverState_) return;
    observable$2.lowestObserverState_ = 1;
    observable$2.observers_.forEach($m10$propagateMaybeChangedEach);
  }
  function $m10$runReactions() {
    if ((+$m8$globalState.inBatch | 0) > 0) {
      var v112 = true, v10;
    } else {
      v10 = !!$m8$globalState.isRunningReactions;
      v112 = v10;
    }
    if (v112) return;
    $m10$reactionScheduler($m10$runReactionsHelperFn);
  }
  function $m12$installFlagAccessor(proto, name, mask) {
    $m3$defineProperty(proto, name, { configurable: true, get: function() {
      return 0 != (+this.flags_ & mask);
    }, set: function(value) {
      var v25 = +this.flags_;
      value ? this.flags_ = v25 | mask : this.flags_ = v25 & (mask ^ a);
    } });
  }
  function $m12$installNumericFlagAccessor(proto, mask) {
    $m3$defineProperty(proto, "diffValue", { configurable: true, get: function() {
      if (0 != (+this.flags_ & mask)) return 1;
      return 0;
    }, set: function(value) {
      var v27 = +this.flags_;
      1 == (+value | 0) ? this.flags_ = v27 | mask : this.flags_ = v27 & (mask ^ a);
    } });
  }
  function $m12$onBOImpl(self) {
    !self.onBOL || self.onBOL.forEach(function(listener) {
      listener();
    });
  }
  function $m12$onBUOImpl(self) {
    !self.onBUOL || self.onBUOL.forEach(function(listener) {
      listener();
    });
  }
  function $m11$untrackedStart() {
    let v2 = $m8$globalState.trackingDerivation;
    $m8$globalState.trackingDerivation = null;
    return v2;
  }
  function $m11$untrackedEnd(prev) {
    $m8$globalState.trackingDerivation = prev;
  }
  function $m11$allowStateChangesStart(allow) {
    let v4 = !!$m8$globalState.allowStateChanges;
    $m8$globalState.allowStateChanges = allow;
    return v4;
  }
  function $m11$checkIfStateModificationsAreAllowed(atom) {
    var v5 = atom.observers_;
    var v8 = (+v5.size | 0) > 0;
    var v23, v19, v21, v22, v37, v39;
    !$m8$globalState.allowStateChanges && (v8 || "always" === $m8$globalState.enforceActions) && (v39 = $m8$globalState.enforceActions ? "Since strict-mode is enabled, changing (observed) observable values without using an action is not allowed. Tried to modify: " : "Side effects like changing state are not allowed at this point. Are you trying to modify state from, for example, a computed value or the render function of a React component? You can wrap side effects in 'runInAction' (or decorate functions with 'action') if needed. Tried to modify: ", v37 = "[MobX] " + v39 + atom.name_, console.warn(v37));
  }
  function $m11$isCaughtException(e) {
    return $m3$isObj(e) && true === e.isMobXCaughtException;
  }
  var $m11$shouldCompute;
  var $m11$trackDerivedFunction;
  (function() {
    function $m10$addObserver(observable$2, node) {
      var v4 = observable$2.observers_;
      v4.add(node);
      var v24 = +observable$2.lowestObserverState_ | 0;
      v24 > (+node.dependenciesState_ | 0) && (observable$2.lowestObserverState_ = node.dependenciesState_);
    }
    function $m11$changeDependenciesStateTo0(derivation) {
      if (0 == (+derivation.dependenciesState_ | 0)) return;
      derivation.dependenciesState_ = 0;
      var v12 = derivation.observing_;
      var v32 = v12.length;
      var v26 = v32, v20, v23;
      while (v26 > 0) {
        v20 = v26 - 1;
        v23 = v12[v20];
        v23.lowestObserverState_ = 0;
        v26 = v20;
      }
    }
    function $m11$warnAboutDerivationWithoutDependencies(derivation) {
      var v5 = derivation.observing_;
      if (0 != v5.length) return;
      if ("boolean" == typeof derivation.requiresObservable_) {
        var v17 = !!derivation.requiresObservable_;
        var v32 = v17, v21;
      } else {
        v21 = !!$m8$globalState.reactionRequiresObservable;
        v32 = v21;
      }
      v32 && console.warn("[mobx] Derivation '" + derivation.name_ + "' is created/updated without reading any observable value.");
    }
    function $m11$bindDependencies(derivation) {
      var v32 = derivation.observing_;
      var v6 = derivation.newObserving_;
      derivation.observing_ = v6;
      var v125 = +derivation.unboundDepsCount_ | 0;
      var v118 = 0, v121 = 0, v123 = 0, v22, v43, v51, v52, v71, v74, v92, v95, v117, v119, v1202, v122, v131, v136;
      while (v123 < v125) {
        v22 = v6[v123];
        0 == (+v22.diffValue | 0) ? (v22.diffValue = 1, v121 != v123 && (v6[v121] = v22), v1202 = v121 + 1 | 0) : v1202 = v121;
        v43 = v22.dependenciesState_;
        !(v43 === void 0) ? (v51 = (+v43 | 0) > v118, v52 = v51) : v52 = false;
        v52 ? (v136 = +v43 | 0, v117 = v136) : v117 = v118;
        v118 = v117;
        v121 = v1202;
        v123 = v123 + 1;
      }
      v6.length = v121;
      derivation.newObserving_ = null;
      v131 = v32.length;
      v122 = v131;
      while (v122 > 0) {
        v71 = v122 - 1;
        v74 = v32[v71];
        0 == (+v74.diffValue | 0) && $m10$removeObserver(v74, derivation);
        v74.diffValue = 0;
        v122 = v71;
      }
      v119 = v121;
      while (v119 > 0) {
        v92 = v119 - 1;
        v95 = v6[v92];
        1 == (+v95.diffValue | 0) && (v95.diffValue = 0, $m10$addObserver(v95, derivation));
        v119 = v92;
      }
      0 != v118 && (derivation.dependenciesState_ = v118, derivation.onBecomeStale_());
    }
    $m11$shouldCompute = function(derivation) {
      var v84 = +derivation.dependenciesState_ | 0;
      if (0 == v84) return false;
      if (v84 == a || 2 == v84) return true;
      if (1 == v84) {
        var v88 = !!$m8$globalState.allowStateReads;
        $m8$globalState.allowStateReads = true;
        var v81 = v88, v15, v162, v25, v28, v37, v42, v60, v82, v93;
        v25 = $m11$untrackedStart();
        v28 = derivation.observing_;
        v93 = v28.length;
        v82 = 0;
        while (v82 < v93) {
          v37 = v28[v82];
          if ($m7$isComputedValue(v37)) {
            v42 = $m8$globalState.disableErrorBoundaries;
            if (true === v42) {
              v37.get();
            } else {
              try {
                v37.get();
              } catch {
                $m11$untrackedEnd(v25);
                $m8$globalState.allowStateReads = v81;
                return true;
              }
            }
            v60 = derivation.dependenciesState_;
            if (2 === v60) return $m11$untrackedEnd(v25), $m8$globalState.allowStateReads = v81, true;
          }
          v82 = v82 + 1;
        }
        $m11$changeDependenciesStateTo0(derivation);
        $m11$untrackedEnd(v25);
        $m8$globalState.allowStateReads = v81;
        return false;
      }
      return false;
    };
    $m11$trackDerivedFunction = function(derivation, f, context) {
      var v93 = !!$m8$globalState.allowStateReads;
      $m8$globalState.allowStateReads = true;
      var v88 = v93, v18, v32, v41, v45, v56, v68, v89, v100;
      $m11$changeDependenciesStateTo0(derivation);
      0 != (+derivation.runId_ | 0) ? (v18 = derivation.observing_, v100 = v18.length, v89 = v100) : v89 = 100;
      derivation.newObserving_ = new Array(v89);
      derivation.unboundDepsCount_ = 0;
      v32 = (+$m8$globalState.runId | 0) + 1 | 0;
      $m8$globalState.runId = v32;
      derivation.runId_ = v32;
      v41 = $m8$globalState.trackingDerivation;
      $m8$globalState.trackingDerivation = derivation;
      v45 = $m8$globalState;
      v45.inBatch = (+$m8$globalState.inBatch | 0) + 1 | 0;
      var result;
      v56 = $m8$globalState.disableErrorBoundaries;
      if (true === v56) {
        result = f.call(context);
      } else {
        try {
          result = f.call(context);
        } catch (v65) {
          result = new $m11$CaughtException(v65);
        }
      }
      v68 = $m8$globalState;
      v68.inBatch = (+$m8$globalState.inBatch | 0) - 1 | 0;
      $m8$globalState.trackingDerivation = v41;
      $m11$bindDependencies(derivation);
      $m11$warnAboutDerivationWithoutDependencies(derivation);
      $m8$globalState.allowStateReads = v88;
      return result;
    };
  })();
  function $m11$clearObserving(derivation) {
    var v32 = derivation.observing_;
    derivation.observing_ = [];
    var v27 = v32.length;
    var v23 = v27, v14;
    while (v23 > 0) {
      v14 = v23 - 1;
      $m10$removeObserver(v32[v14], derivation);
      v23 = v14;
    }
    derivation.dependenciesState_ = a;
  }
  function $m14$createAction(actionName, fn, autoActionFlag, ref) {
    "function" == typeof fn || $m4$die("`action` can only be invoked on functions");
    (!("string" == typeof actionName) || "" == actionName) && $m4$die("actions should have valid names, got: '" + actionName + "'");
    var v29 = (0, function() {
      var v162 = ref == null ? this : ref;
      return $m14$executeAction(actionName, autoActionFlag, fn, v162, arguments);
    });
    v29.isMobxAction = true;
    v29.toString = function() {
      return fn.toString();
    };
    $m14$isFunctionNameConfigurable && ($m14$tmpNameDescriptor.value = actionName, $m3$defineProperty(v29, "name", $m14$tmpNameDescriptor));
    return v29;
  }
  function $m14$_startAction(actionName, canRunAsDerivation, scope, args) {
    var v6 = $m9$isSpyEnabled();
    var v112 = v6 && !!actionName;
    if (v112) {
      var v92 = Date.now();
      var v15 = $m2$EMPTY_ARRAY;
      if (args) {
        var v19 = $m3$arrayFrom(args);
        var v89 = v19, v10, v32, v37, v38, v43, v48, v52, v56, v60, v62, v88, v91;
      } else {
        v89 = v15;
      }
      $m9$spyReportStart({ type: "action", name: actionName, object: scope, arguments: v89 });
      v88 = v92;
    } else {
      v88 = 0;
    }
    var v30 = $m8$globalState;
    v32 = v30.trackingDerivation;
    v38 = !canRunAsDerivation || v32 == null;
    $m10$startBatch();
    v43 = !!$m8$globalState.allowStateChanges;
    v38 ? ($m11$untrackedStart(), v48 = $m11$allowStateChangesStart(true), v91 = v48) : v91 = v43;
    v52 = !!$m8$globalState.allowStateReads;
    $m8$globalState.allowStateReads;
    $m8$globalState.allowStateReads = true;
    v56 = $m14$nextActionId;
    $m14$nextActionId = $m14$nextActionId + 1 | 0;
    v60 = $m14$currentActionId;
    $m14$currentActionId = v56;
    v62 = {};
    v62.runAsAction_ = v38;
    v62.prevDerivation_ = v32;
    v62.prevAllowStateChanges_ = v91;
    v62.prevAllowStateReads_ = v52;
    v62.notifySpy_ = v112;
    v62.startTime_ = v88;
    v62.actionId_ = v56;
    v62.parentActionId_ = v60;
    return v62;
  }
  function $m14$_endAction(runInfo) {
    var v1 = $m14$currentActionId;
    v1 != (+runInfo.actionId_ | 0) && $m4$die(30);
    $m14$currentActionId = +runInfo.parentActionId_ | 0;
    runInfo.error_ === void 0 || ($m8$globalState.suppressReactionErrors = true);
    $m8$globalState.allowStateChanges = !!runInfo.prevAllowStateChanges_;
    $m8$globalState.allowStateReads = !!runInfo.prevAllowStateReads_;
    $m10$endBatch();
    !runInfo.runAsAction_ || $m11$untrackedEnd(runInfo.prevDerivation_);
    var v46 = !!runInfo.notifySpy_;
    if (v46) {
      var v72 = +Date.now() | 0;
      $m9$spyReportEnd({ time: v72 - (+runInfo.startTime_ | 0) | 0 });
    }
    $m8$globalState.suppressReactionErrors = false;
  }
  function $m14$executeAction(actionName, canRunAsDerivation, fn, scope, args) {
    var runInfo = $m14$_startAction(actionName, canRunAsDerivation, scope, args);
    try {
      return fn.apply(scope, args);
    } catch (v14) {
      runInfo.error_ = v14;
      throw v14;
    } finally {
      $m14$_endAction(runInfo);
    }
  }
  function $m17$hasListeners(listenable) {
    if (!(listenable.changeListeners_ === void 0)) {
      var v9 = listenable.changeListeners_;
      var v12 = v9.length > 0;
      var v13 = v12;
    } else {
      v13 = false;
    }
    return v13;
  }
  function $m17$registerListener(listenable, handler) {
    listenable.changeListeners_ === void 0 && (listenable.changeListeners_ = []);
    var v112 = listenable.changeListeners_;
    v112.push(handler);
    return $m3$once(function() {
      var v18 = +v112.indexOf(handler) | 0;
      v18 != a && v112.splice(v18, 1);
    });
  }
  function $m17$notifyListeners(listenable, change) {
    var v2 = $m11$untrackedStart();
    var v5 = listenable.changeListeners_;
    if (!v5) {
      $m11$untrackedEnd(v2);
      return;
    }
    var v32 = $m3$arraySliceFn.call(v5);
    var v36 = v32.length;
    var v30 = 0;
    while (v30 < v36) {
      v32[v30](change);
      v30 = v30 + 1;
    }
    $m11$untrackedEnd(v2);
  }
  function $m18$hasInterceptors(interceptable) {
    if (!(interceptable.interceptors_ === void 0)) {
      var v9 = interceptable.interceptors_;
      var v12 = v9.length > 0;
      var v13 = v12;
    } else {
      v13 = false;
    }
    return v13;
  }
  function $m18$registerInterceptor(interceptable, handler) {
    interceptable.interceptors_ === void 0 && (interceptable.interceptors_ = []);
    var v112 = interceptable.interceptors_;
    v112.push(handler);
    return $m3$once(function() {
      var v18 = +v112.indexOf(handler) | 0;
      v18 != a && v112.splice(v18, 1);
    });
  }
  function $m18$interceptChange(interceptable, change) {
    var prevU = $m11$untrackedStart();
    try {
      var interceptorsSrc = [];
      !interceptable.interceptors_ || (interceptorsSrc = interceptable.interceptors_);
      var interceptors = $m3$arraySliceFn.call(interceptorsSrc);
      var n = interceptors.length;
      var i = 0;
      while (i < n) {
        change = interceptors[i](change);
        if (change) {
          var v32 = !change.type;
          var v33 = v32;
        } else {
          v33 = false;
        }
        v33 && $m4$die(14);
        if (!change) break;
        i = i + 1 | 0;
      }
      return change;
    } finally {
      $m11$untrackedEnd(prevU);
    }
  }
  function $m20$isAnnotation(thing) {
    return ($m3$isObj(thing) || "function" == typeof thing) && "string" == typeof thing.annotationType_ && "function" == typeof thing.make_ && "function" == typeof thing.extend_;
  }
  function $m21$assert20223DecoratorType(context, types) {
    var v7 = context.kind;
    var v9 = !(true === types.includes(v7));
    if (v9) {
      var v18 = "The decorator applied to '" + $m3$stringifyKey(context.name) + "' cannot be used on a ";
      $m4$die(v18 + context.kind + " element");
    }
  }
  function $m22$createDecoratorAnnotation(annotation, decorate) {
    return $m3$assign(function(value, context) {
      if (context && "string" == typeof context.kind) return decorate(annotation, value, context);
      $m4$die("Invalid arguments for `" + annotation.annotationType_ + "`");
    }, annotation);
  }
  function $m23$initObservable(cb) {
    var derivation = $m11$untrackedStart();
    var allow = true;
    allow = $m11$allowStateChangesStart(true);
    $m10$startBatch();
    try {
      return cb();
    } finally {
      $m10$endBatch();
      $m8$globalState.allowStateChanges = allow;
      $m11$untrackedEnd(derivation);
    }
  }
  function $m25$isObservableValueCheck(value) {
    if (!value) return false;
    if ($m7$isObservableObject(value)) {
      var v12 = true, v112, v15, v162, v19, v20, v23, v24;
    } else {
      v112 = !!value[$m2$$mobx];
      v12 = v112;
    }
    return v12 || $m7$isAtom(value) || $m7$isReaction(value) || $m7$isComputedValue(value);
  }
  var $m26$eq = /* @__PURE__ */ (function() {
    function $m26$protoInstance(ctor, value) {
      if (!ctor) return false;
      var v8 = ctor.isPrototypeOf;
      if ("function" == typeof v8) return true === ctor.isPrototypeOf(value);
      if (true === "constructor" in value) {
        var v24 = value.constructor == ctor;
        var v35 = true === v24;
        var v26 = v35;
      } else {
        v26 = false;
      }
      return v26;
    }
    function $m26$unwrapEq(a$2) {
      if ($m7$isObservableArray(a$2)) return a$2.slice();
      if ($m3$isES6Map(a$2) || $m7$isObservableMap(a$2)) return $m3$arrayFrom(a$2.entries());
      return $m3$isES6Set(a$2) || $m7$isObservableSet(a$2) ? $m3$arrayFrom(a$2.entries()) : a$2;
    }
    return function(a$2, b, depth, aStack, bStack) {
      if (a$2 === b) {
        if (!(0 === a$2)) {
          var v22 = true, v162, v21, v28, v39, v47, v53, v57, v802, v81, v110, v125, v1262, v139, v152, v1532, v161, v163, v166, v177, v178, v182, v185, v196, v197, v201, v205, v206, v207, v208, v213, v218, v240, v270, v301, v308, v311, v317, v318, v329, v330, v331, v332, v333, v334, v335, v336, v338, v343, v346, v361, v372, v381, v389, v396, v400, v426, v428;
        } else {
          v162 = 1 / +a$2;
          v21 = v162 === 1 / +b;
          v22 = v21;
        }
        return v22;
      }
      if (a$2 == null || b == null) return false;
      if (!(a$2 === a$2)) return !(b === b);
      v39 = typeof a$2;
      if ("function" != v39 && "object" != v39 && "object" != typeof b) return false;
      v57 = $m3$objectCtor.prototype;
      v338 = v57.toString.call(a$2) + "";
      if (v338 != $m3$objectCtor.prototype.toString.call(b) + "") return false;
      if ("[object RegExp]" == v338 || "[object String]" == v338) return "" + a$2 == "" + b;
      if ("[object Number]" == v338) {
        v343 = Number(a$2);
        v346 = Number(b);
        if (!(true === $m3$objectIsFn(v343, v343))) return !(true === $m3$objectIsFn(v346, v346));
        if (0 === v343) return v110 = 1 / +v343, true === $m3$objectIsFn(v110, 1 / +v346);
        return v343 === v346;
      }
      if ("[object Date]" == v338 || "[object Boolean]" == v338) return v361 = Number(a$2), v361 === Number(b);
      if ("[object Symbol]" == v338) return v139 = Symbol.valueOf.call(a$2), v139 === Symbol.valueOf.call(b);
      "[object Map]" == v338 || "[object Set]" == v338 ? (v330 = depth >= 0 ? depth + 1 | 0 : depth, v331 = v330) : v331 = depth;
      v161 = $m26$unwrapEq(a$2);
      v163 = $m26$unwrapEq(b);
      v166 = "[object Array]" == v338;
      if (!v166) {
        if ("object" != typeof v161 || "object" != typeof v163) return false;
        v182 = v161.constructor;
        v185 = v163.constructor;
        !(v182 === v185) ? (v207 = !("function" == typeof v182 && $m26$protoInstance(v182, v182) && "function" == typeof v185 && $m26$protoInstance(v185, v185)), v208 = v207) : v208 = false;
        if (v208 && true === "constructor" in v161 && true === "constructor" in v163) return false;
      }
      if (0 == v331) return false;
      else {
        v329 = v331 < 0 ? a : v331;
      }
      aStack === void 0 ? (v332 = [], v333 = []) : (v332 = aStack, v333 = bStack);
      v381 = v332.length;
      v335 = v381;
      while (v335 > 0) {
        v240 = v335 - 1;
        if (v332[v240] === v161) return v333[v240] === v163;
        v335 = v240;
      }
      v332.push(v161);
      v333.push(v163);
      if (v166) {
        v389 = v161.length;
        if (v389 != v163.length) return false;
        v334 = v389;
        while (v334 > 0) {
          v270 = v334 - 1;
          if (!$m26$eq(v161[v270], v163[v270], v329 - 1 | 0, v332, v333)) return false;
          v334 = v270;
        }
      } else {
        v396 = $m3$objectCtor.keys(v161);
        v400 = v396.length;
        if ($m3$objectCtor.keys(v163).length != v400) return false;
        v336 = 0;
        while (v336 < v400) {
          v301 = v396[v336];
          $m3$hasOwn(v163, v301) ? (v308 = v161[v301], v311 = v163[v301], v317 = $m26$eq(v308, v311, v329 - 1 | 0, v332, v333), v318 = v317) : v318 = false;
          if (!v318) return false;
          v336 = v336 + 1;
        }
      }
      v332.pop();
      v333.pop();
      return true;
    };
  })();
  function $m27$getEnhancerFromOptions(options) {
    var v32 = options.deep;
    if (true === v32) return $m27$deepEnhancerFn;
    var v9 = options.deep;
    if (false === v9) return $m27$referenceEnhancerFn;
    if (options.defaultDecorator) {
      var v20 = options.defaultDecorator;
      var v23 = !!v20.options_;
      var v24 = v23, v28, v30, v33, v34, v37, v39;
    } else {
      v24 = false;
    }
    if (v24 && options.defaultDecorator.options_.enhancer_) return v37 = options.defaultDecorator, v39 = v37.options_, v39.enhancer_;
    return $m27$deepEnhancerFn;
  }
  function $m27$asCreateObservableOptions(thing) {
    return !!thing ? thing : $m27$defaultCreateObservableOptions;
  }
  function $m32$admProxyOrTarget(self) {
    if (self.proxy_) return self.proxy_;
    return self.target_;
  }
  function $m33$getCachedObservablePropDescriptor(key) {
    var v32 = $m33$descriptorCache[key];
    if (v32) return v32;
    var v7 = {};
    v7.get = function() {
      let v4 = this[$m2$$mobx];
      return v4.getObservablePropValue_(key);
    };
    v7.set = function(value) {
      let v5 = this[$m2$$mobx];
      return v5.setObservablePropValue_(key, value);
    };
    $m33$descriptorCache[key] = v7;
    return v7;
  }
  function $m33$recordAnnotationApplied(adm, annotation, key) {
    adm.appliedAnnotations_[key] = annotation;
  }
  function $m33$assertAnnotable(adm, annotation, key) {
    var v7 = !$m20$isAnnotation(annotation);
    v7 && $m4$die("Cannot annotate '" + adm.name_ + "." + $m3$stringifyKey(key) + "': Invalid annotation.");
    var v27 = !("override" == annotation.annotationType_ + "");
    if (v27 && $m3$hasOwn(adm.appliedAnnotations_, key)) {
      var v43 = "Cannot apply '" + annotation.annotationType_ + "' to '";
      var v55 = v43 + adm.name_ + "." + $m3$stringifyKey(key) + "':\nThe field is already annotated with '";
      $m4$die(v55 + adm.appliedAnnotations_[key].annotationType_ + "'.\nRe-annotating fields is not allowed.\nUse 'override' annotation for methods overridden by subclass.");
    }
  }
  function $m33$setObjectValue(self, key, value, proxyTrap) {
    if ($m3$hasOwn(self.target_, key)) {
      var v112 = self.values_;
      if (v112.has(key)) return self.setObservablePropValue_(key, value);
      if (proxyTrap) {
        var v22 = self.target_;
        return true === Reflect.set(v22, key, value);
      }
      self.target_[key] = value;
      return true;
    }
    return self.extend_(key, { value, enumerable: true, writable: true, configurable: true }, self.defaultAnnotation_, proxyTrap);
  }
  function $m33$getAnnotationFromOptions(options) {
    if (!options) return;
    if (!(options.defaultDecorator === void 0)) return options.defaultDecorator;
    if (options.autoBind || false === options.deep) {
      var v29 = {};
      v29.annotationType_ = "true";
      v29.options_ = options;
      v29.make_ = $m31$autoAnnotationMake;
      v29.extend_ = $m31$autoAnnotationExtend;
      return v29;
    }
  }
  var $m35$spliceWithArrayImpl = /* @__PURE__ */ (function() {
    function $m35$callSpliceItems(self, index, deleteCount, newItems) {
      var v6 = self.values_;
      var v121 = newItems.length;
      var v122 = v121;
      var v123 = v122;
      if (0 == deleteCount && index == v6.length) {
        var v116 = 0, v162, v17, v25, v34, v49, v64, v117, v118, v119, v135, v141, v145;
        while (v116 < v123) {
          v25 = newItems[v116];
          v6.push(v25);
          v116 = v116 + 1;
        }
        return $m2$EMPTY_ARRAY;
      }
      if (v123 < 1e4) {
        v34 = [];
        v34.push(index);
        v34.push(deleteCount);
        v117 = 0;
        while (v117 < newItems.length) {
          v49 = newItems[v117];
          v34.push(v49);
          v117 = v117 + 1;
        }
        return v6.splice.apply(v6, v34);
      }
      v64 = index + deleteCount | 0;
      v135 = $m3$arraySliceFn.call(v6, index, v64);
      v141 = $m3$arraySliceFn.call(v6, v64, v6.length);
      v145 = v6.length;
      v6.length = (v145 + newItems.length | 0) - deleteCount | 0;
      v119 = 0;
      while (v119 < newItems.length) {
        v6[index + v119] = newItems[v119];
        v119 = v119 + 1;
      }
      v118 = 0;
      while (v118 < v141.length) {
        v6[(index + newItems.length | 0) + v118] = v141[v118];
        v118 = v118 + 1;
      }
      return v135;
    }
    function $m35$callNotifySplice(self, index, added, removed) {
      var v10 = !self.owned_;
      var v14 = v10 && $m9$isSpyEnabled();
      var v162 = $m17$hasListeners(self);
      var v17 = null;
      var v21, v13, v27, v31, v33, v48, v68, v72;
      v162 || v14 ? (v27 = self.proxy_, v31 = self.atom_, v33 = v31.name_, v72 = removed.length, v48 = { observableKind: "array", object: v27, debugObjectName: v33, type: "splice", index, removed, added, removedCount: v72, addedCount: added.length }, v68 = v48) : v68 = v17;
      v14 && $m9$spyReportStart(v68);
      $m10$reportChanged(self.atom_);
      v162 && $m17$notifyListeners(self, v68);
      v14 && $m9$spyReportEnd(void 0);
    }
    return function(self, index, deleteCount, newItems) {
      $m11$checkIfStateModificationsAreAllowed(self.atom_);
      var v112 = self.values_;
      var v149 = v112.length;
      if (index > v149) {
        var v136 = v149, v22, v34, v44, v51, v55, v62, v72, v83, v88, v114, v122, v123, v135, v137, v1382, v139, v140, v141, v142, v143, v1442, v145, v158, v162;
      } else {
        index < 0 ? (v22 = v149 + index | 0, v1442 = v22 < 0 ? 0 : v22, v135 = v1442) : v135 = index;
        v136 = v135;
      }
      v139 = deleteCount < 0 ? 0 : deleteCount;
      v34 = v149 - v136 | 0;
      v1382 = v139 > v34 ? v34 : v139;
      newItems == null ? v143 = [] : (!Array.isArray(newItems) ? (v51 = Array.prototype, v55 = v51.slice.call(newItems), v142 = v55) : v142 = newItems, v143 = v142);
      if ($m18$hasInterceptors(self)) {
        v62 = self.proxy_;
        v72 = $m18$interceptChange(self, { object: v62, type: "splice", index: v136, removedCount: v1382, added: v143 });
        if (!v72) return $m2$EMPTY_ARRAY;
        v158 = +v72.removedCount | 0;
        v83 = v72.added;
        v137 = v158;
        v141 = v83;
      } else {
        v137 = v1382;
        v141 = v143;
      }
      if (0 != v141.length) {
        v88 = [];
        v162 = v141.length;
        v145 = 0;
        while (v145 < v162) {
          v88.push(self.enhancer_(v141[v145], void 0));
          v145 = v145 + 1;
        }
        v140 = v88;
      } else {
        v140 = v141;
      }
      v114 = $m35$callSpliceItems(self, v136, v137, v140);
      (0 != v137 || 0 != v140.length) && $m35$callNotifySplice(self, v136, v140, v114);
      return self.dehanceValues_(v114);
    };
  })();
  function $m35$callNotifyUpdate(self, index, newValue, oldValue) {
    var v10 = !self.owned_;
    var v14 = v10 && $m9$isSpyEnabled();
    var v162 = $m17$hasListeners(self);
    var v17 = null;
    var v21, v13, v27, v33, v35, v42, v62;
    v162 || v14 ? (v27 = self.proxy_, v33 = self.atom_, v35 = v33.name_, v42 = { observableKind: "array", object: v27, type: "update", debugObjectName: v35, index, newValue, oldValue }, v62 = v42) : v62 = v17;
    v14 && $m9$spyReportStart(v62);
    $m10$reportChanged(self.atom_);
    v162 && $m17$notifyListeners(self, v62);
    v14 && $m9$spyReportEnd(void 0);
  }
  function $m35$addSimpleArrayExt(funcName) {
    "function" == typeof Array.prototype[funcName] && ($m35$arrayExtensions[funcName] = function() {
      let v5 = this[$m2$$mobx];
      $m10$reportObserved(v5.atom_);
      let v24 = v5.dehanceValues_(v5.values_);
      return v24[funcName].apply(v24, arguments);
    });
  }
  function $m35$addMapLikeArrayExt(funcName) {
    "function" == typeof Array.prototype[funcName] && ($m35$arrayExtensions[funcName] = function(v20, v28) {
      var self = this;
      var v5$2 = self[$m2$$mobx];
      $m10$reportObserved(v5$2.atom_);
      var v37 = v5$2.dehanceValues_(v5$2.values_);
      var thisArg;
      arguments.length > 1 && (thisArg = v28);
      return v37[funcName](function(v9, v12) {
        let v6 = thisArg;
        return v20.call(v6, v9, v12, self);
      });
    });
  }
  function $m35$addReduceLikeArrayExt(funcName) {
    "function" == typeof Array.prototype[funcName] && ($m35$arrayExtensions[funcName] = function() {
      var self = this;
      let v5$2 = self[$m2$$mobx];
      $m10$reportObserved(v5$2.atom_);
      let v33 = v5$2.dehanceValues_(v5$2.values_), v20 = arguments[0];
      arguments[0] = function(v8, v112, v14) {
        return v20(v8, v112, v14, self);
      };
      return v33[funcName].apply(v33, arguments);
    });
  }
  function $m36$convertToMap(dataStructure) {
    if ($m3$isES6Map(dataStructure) || $m7$isObservableMap(dataStructure)) return dataStructure;
    if (Array.isArray(dataStructure)) return new Map(dataStructure);
    if ($m3$isPlainObject(dataStructure)) {
      var v45 = /* @__PURE__ */ new Map();
      var v48 = $m3$objectCtor.keys(dataStructure);
      var v41 = 0, v5, v6, v26, v31;
      while (v41 < v48.length) {
        v26 = v48[v41];
        v31 = dataStructure[v48[v41]];
        v45.set(v26, v31);
        v41 = v41 + 1;
      }
      return v45;
    }
    $m4$die1(21, dataStructure);
    return /* @__PURE__ */ new Map();
  }
  function $m36$dehanceMap(self, value) {
    return !(self.dehancer === void 0) ? self.dehancer(value) : value;
  }
  function $m37$dehanceMap(self, value) {
    return !(self.dehancer === void 0) ? self.dehancer(value) : value;
  }
  function $m37$addSetAlgebra(funcName) {
    let v32 = $m37$ObservableSet.prototype;
    v32[funcName] = function(other) {
      var v5 = this.atom_;
      $m10$reportObserved(v5);
      if (("intersection" == funcName || "union" == funcName || "symmetricDifference" == funcName || "isDisjointFrom" == funcName) && $m3$isES6Set(other) && !$m7$isObservableSet(other) && "function" == typeof other[funcName]) return other[funcName](this);
      return new Set(this)[funcName](other);
    };
  }
  function $m38$createObservableAnnotation(name, options) {
    let v2 = {};
    v2.annotationType_ = name;
    v2.options_ = options;
    v2.make_ = $m38$annotationOwnMake;
    v2.extend_ = $m38$observableAnnotationExtend;
    return v2;
  }
  function $m39$createComputedAnnotation(name, options) {
    let v2 = {};
    v2.annotationType_ = name;
    v2.options_ = options;
    v2.make_ = $m38$annotationOwnMake;
    v2.extend_ = $m39$computedAnnotationExtend;
    return v2;
  }
  function $m40$createActionDescriptor(adm, annotation, key, descriptor, safeDescriptors) {
    var v112 = !("function" == typeof descriptor.value);
    if (v112) {
      var v20 = "Cannot apply '" + annotation.annotationType_ + "' to '";
      $m4$die(v20 + adm.name_ + "." + $m3$stringifyKey(key) + "':\n'" + annotation.annotationType_ + "' can only be used on properties with a function value.");
    }
    var v43 = descriptor.value;
    if (annotation.options_) {
      var v51 = annotation.options_;
      var v54 = !!v51.bound;
      var v55 = v54, v61, v69, v72, v73, v76, v88, v91, v92, v102, v105, v106, v108, v115, v130, v131, v1322, v133, v134, v135, v143, v1442, v145;
    } else {
      v55 = false;
    }
    v55 ? (v143 = v43.bind($m32$admProxyOrTarget(adm)), v130 = v143) : v130 = v43;
    v61 = $m3$stringifyKey(key);
    annotation.options_ ? (v69 = annotation.options_, v72 = !!v69.name, v73 = v72) : v73 = false;
    v73 ? (v76 = annotation.options_, v1442 = v76.name + "", v131 = v1442) : v131 = v61;
    annotation.options_ ? (v88 = annotation.options_, v91 = !!v88.autoAction, v92 = v91) : v92 = false;
    v1322 = v92;
    v145 = void 0;
    annotation.options_ ? (v102 = annotation.options_, v105 = !!v102.bound, v106 = v105) : v106 = false;
    v106 ? (v108 = $m32$admProxyOrTarget(adm), v133 = v108) : v133 = v145;
    safeDescriptors ? (v115 = !!adm.isPlainObject_, v134 = v115, v135 = false) : (v134 = true, v135 = true);
    return { value: $m14$createAction(v131, v130, v1322, v133), configurable: v134, enumerable: false, writable: v135 };
  }
  function $m40$createActionAnnotation(name, options) {
    let v2 = {};
    v2.annotationType_ = name;
    v2.options_ = options;
    v2.make_ = $m40$actionAnnotationMake;
    v2.extend_ = $m40$actionAnnotationExtend;
    return v2;
  }
  function $m41$createFlowDescriptor(adm, descriptor, bound, safeDescriptors) {
    var v8 = descriptor.value;
    if (!$m7$isFlow(v8)) {
      var v15 = $m28$flow(v8);
      var v42 = v15, v30, v41, v43, v44, v47;
    } else {
      v42 = v8;
    }
    bound ? (v47 = v42.bind($m32$admProxyOrTarget(adm)), v47.isMobXFlow = true, v41 = v47) : v41 = v42;
    safeDescriptors ? (v30 = !!adm.isPlainObject_, v43 = v30, v44 = false) : (v43 = true, v44 = true);
    return { value: v41, configurable: v43, enumerable: false, writable: v44 };
  }
  function $m43$registerLazyObservable(target, annotation, keyName, value) {
    var v9 = $m33$asObservableObject(target)[$m2$$mobx];
    v9.lazyObservableKeys_ || (v9.lazyObservableKeys_ = /* @__PURE__ */ new Map());
    v9.lazyObservableKeys_.set(keyName, function() {
      var v5 = $m27$deepEnhancerFn;
      if (annotation.options_) {
        var v13 = annotation.options_;
        var v162 = !!v13.enhancer_;
        var v17 = v162, v20, v22, v36, v43;
      } else {
        v17 = false;
      }
      v17 ? (v20 = annotation.options_, v22 = v20.enhancer_, v43 = v22) : v43 = v5;
      $m3$stringifyKey(keyName);
      v36 = v9.name_ + "." + $m3$stringifyKey(keyName);
      return new $m29$ObservableValue(value, v43, v36, false);
    });
    return v9;
  }
  function $m43$createObservable(v, arg2, arg3) {
    if (arg2 && "string" == typeof arg2.kind) return $m43$decorateObservable20223_($m43$observableAnnotation, v, arg2);
    if ($m25$isObservableValueCheck(v)) return v;
    if ($m3$isPlainObject(v)) return $m28$observable.object.call($m28$observable, v, arg2, arg3);
    if (Array.isArray(v)) return $m28$observable.array.call($m28$observable, v, arg2);
    if ($m3$isES6Map(v)) return $m28$observable.map.call($m28$observable, v, arg2);
    if ($m3$isES6Set(v)) return $m28$observable.set.call($m28$observable, v, arg2);
    if ("object" == typeof v && !(v == null)) return v;
    return $m28$observable.box.call($m28$observable, v, arg2);
  }
  function $m48$createSchedulerFromOptions(opts) {
    if (opts.scheduler) return opts.scheduler;
    if (opts.delay) return function(callback) {
      let v6 = opts.delay;
      return setTimeout(callback, v6);
    };
    return function(callback) {
      return callback();
    };
  }
  function $m48$wrapErrorHandler(errorHandler, baseFn) {
    return !errorHandler ? baseFn : function() {
      try {
        return baseFn.apply(this, arguments);
      } catch (v8) {
        errorHandler.call(this, v8);
        return;
      }
    };
  }
  function $m48$whenEffect(predicate, effect, opts) {
    opts = opts || {};
    var disposer;
    var timeoutHandle;
    if ("number" == typeof opts.timeout) {
      var v70 = new Error("WHEN_TIMEOUT");
      var v19 = function() {
        var v6 = disposer[$m2$$mobx];
        if (!v6.isDisposed) {
          disposer();
          if (opts.onError) {
            opts.onError(v70);
          } else {
            var v27 = void 0;
            $m3$throwErr(v70);
          }
        }
      };
      var v22 = opts.timeout;
      timeoutHandle = setTimeout(v19, v22);
    }
    if (!opts.name) {
      var v30 = opts;
      v30.name = "When@" + $m8$getNextId();
    }
    var effectAction = $m14$createAction("When-effect", effect, false, void 0);
    effectAction = $m14$createAction(opts.name + "-effect", effect, false, void 0);
    disposer = $m48$autorun(function(r) {
      if ($m11$allowStateChanges(false, predicate)) {
        var v25 = void 0;
        r.dispose();
        !timeoutHandle || clearTimeout(timeoutHandle);
        effectAction();
      }
    }, opts);
    return disposer;
  }
  function $m48$whenPromise(predicate, opts) {
    var v5 = !!opts;
    v5 && opts.onError && $m4$die("the options 'onError' and 'promise' cannot be combined");
    if (opts) {
      var v21 = !!opts.signal;
      var v22 = v21, v112, v12, v26, v29, v30, v35, v41, v54, v61, v62, v66, v70, v77, v84, v93;
    } else {
      v22 = false;
    }
    if (v22 && opts.signal.aborted) return v35 = Promise.reject(new Error("WHEN_ABORTED")), v35.cancel = function() {
      return null;
    }, v35;
    v41 = {};
    v41.cancel = void 0;
    v41.abort = void 0;
    v54 = new Promise(function(v7, v10$2) {
      var v13 = $m3$assign({}, opts);
      v13.onError = v10$2;
      var v20$2 = $m48$whenEffect(predicate, v7, v13);
      v41.cancel = function() {
        v20$2();
        v10$2(new Error("WHEN_CANCELLED"));
      };
      v41.abort = function() {
        v20$2();
        v10$2(new Error("WHEN_ABORTED"));
      };
      if (opts) {
        var v39$2 = !!opts.signal;
        var v40 = v39$2, v44, v48, v61$2;
      } else {
        v40 = false;
      }
      v40 && "function" == typeof opts.signal.addEventListener && opts.signal.addEventListener("abort", v41.abort);
    });
    opts ? (v61 = !!opts.signal, v62 = v61) : v62 = false;
    v62 && "function" == typeof opts.signal.removeEventListener ? (v77 = v54.finally(function() {
      opts.signal.removeEventListener("abort", v41.abort);
    }), v84 = v77) : v84 = v54;
    v84.cancel = v41.cancel;
    return v84;
  }
  function $m50$makeAnnotate(adm, key, annotation) {
    if (true === annotation) {
      var v8 = adm.defaultAnnotation_;
      var v75 = v8, v25, v34, v38, v47, v48, v62, v76, v802, v84, v88;
    } else {
      v75 = annotation;
    }
    if (false === v75) return;
    $m33$assertAnnotable(adm, v75, key);
    true === key in adm.target_ || (v25 = v75.annotationType_, v34 = adm.name_ + "." + $m3$stringifyKey(key), v802 = [], v802.push(v25), v802.push(v34), $m4$dieRest(1, v802));
    v38 = adm.target_;
    v76 = v38;
    for (; ; ) {
      v76 ? (v47 = !(v76 === $m3$objectCtor.prototype), v48 = v47) : v48 = false;
      if (!v48) {
        break;
      }
      v84 = $m3$objectCtor.getOwnPropertyDescriptor(v76, key);
      if (v84) {
        v62 = v75.make_(adm, key, v84, v76);
        if (0 === v62) return;
        if (1 === v62) break;
      }
      v88 = $m3$objectCtor.getPrototypeOf(v76);
      v76 = v88;
    }
    $m33$recordAnnotationApplied(adm, v75, key);
  }
  function $m53$observeObservable(thing, listener, fireImmediately) {
    var v6 = $m52$getAdministration(thing);
    if ($m7$isObservableArray(thing)) {
      var v83 = void 0;
      if (fireImmediately) {
        var v12 = v6.values_;
        var v85 = $m3$arraySliceFn.call(v12);
        var v21 = v6.proxy_;
        var v25 = v6.atom_;
        var v27 = v25.name_;
        listener({ observableKind: "array", object: v21, debugObjectName: v27, type: "splice", index: 0, added: v85, addedCount: v85.length, removed: [], removedCount: 0 });
      }
      return $m17$registerListener(v6, listener);
    }
    if ($m7$isObservableMap(thing)) return fireImmediately && $m4$die("`observe` doesn't support fireImmediately=true in combination with maps."), $m17$registerListener(v6, listener);
    if ($m7$isObservableSet(thing)) return fireImmediately && $m4$die("`observe` doesn't support fireImmediately=true in combination with sets."), $m17$registerListener(v6, listener);
    if ($m7$isObservableObject(thing)) return fireImmediately && $m4$die("`observe` doesn't support the fire immediately property for observable objects."), $m17$registerListener(v6, listener);
    return $m53$observeValue(v6, listener, fireImmediately);
  }
  function $m53$observeValue(adm, listener, fireImmediately) {
    if ($m7$isComputedValue(adm)) {
      var firstTime = true;
      var prevValue;
      return $m48$autorun(function() {
        var v8 = adm.get;
        var v40 = adm.get();
        if (!firstTime || fireImmediately) {
          var v162 = $m11$untrackedStart();
          var v24 = adm.name_;
          listener({ observableKind: "computed", debugObjectName: v24, type: "update", object: adm, newValue: v40, oldValue: prevValue });
          var v41 = void 0;
          $m11$untrackedEnd(v162);
        }
        firstTime = false;
        prevValue = v40;
      });
    }
    if (fireImmediately) {
      var v25 = adm.name_;
      var v33 = adm.value_;
      listener({ observableKind: "value", debugObjectName: v25, object: adm, type: "update", newValue: v33, oldValue: void 0 });
    }
    return $m17$registerListener(adm, listener);
  }
  function $m55$interceptHook(listenersKey, args) {
    var atomNode;
    var v5 = args[1];
    var cb = v5;
    args.length > 2 && "function" == typeof args[2] ? (atomNode = $m52$getAtom(args[0], args[1]), cb = args[2]) : atomNode = $m52$getAtom(args[0]);
    if (atomNode[listenersKey]) {
      var v40 = atomNode[listenersKey];
      v40.add(cb);
    } else {
      var v43 = atomNode;
      v43[listenersKey] = /* @__PURE__ */ new Set();
      var v48 = atomNode[listenersKey];
      v48.add(cb);
    }
    return function() {
      var v6 = atomNode[listenersKey];
      !v6 || (v6.delete(cb), 0 == (+v6.size | 0) && delete atomNode[listenersKey]);
    };
  }
  function $m56$toJSHelper(source, alreadySeen) {
    if (source == null || "object" != typeof source || $m3$protoIsPrototypeOf(Date, source) || !$m25$isObservableValueCheck(source)) return source;
    if ($m7$isObservableValuePred(source) || $m7$isComputedValue(source)) return $m56$toJSHelper(source.get(), alreadySeen);
    if (alreadySeen.has(source)) return alreadySeen.get(source);
    if ($m7$isObservableArray(source)) {
      var v42 = [];
      alreadySeen.set(source, v42);
      var v166 = 0, v9, v13, v14, v18, v19, v25, v26, v75, v102, v1132, v116, v1262, v142, v154, v167, v168, v169, v186, v196, v202;
      while (v166 < source.length) {
        v42[v166] = $m56$toJSHelper(source[v166], alreadySeen);
        v166 = v166 + 1;
      }
      return v42;
    }
    if ($m7$isObservableSet(source)) {
      v186 = /* @__PURE__ */ new Set();
      alreadySeen.set(source, v186);
      v75 = $m3$arrayFrom(source.values());
      v167 = 0;
      while (v167 < v75.length) {
        v186.add($m56$toJSHelper(v75[v167], alreadySeen));
        v167 = v167 + 1;
      }
      return v186;
    }
    if ($m7$isObservableMap(source)) {
      v196 = /* @__PURE__ */ new Map();
      alreadySeen.set(source, v196);
      v102 = $m3$arrayFrom(source.entries());
      v168 = 0;
      while (v168 < v102.length) {
        v1132 = v102[v168][0];
        v116 = v102[v168];
        v196.set(v1132, $m56$toJSHelper(v116[1], alreadySeen));
        v168 = v168 + 1;
      }
      return v196;
    }
    v1262 = {};
    alreadySeen.set(source, v1262);
    v202 = $m51$ownKeysApi(source);
    v169 = 0;
    var v201 = void 0;
    while (v169 < v202.length) {
      v142 = $m3$objectCtor.prototype;
      true === v142.propertyIsEnumerable.call(source, v202[v169]) && (v154 = v202[v169], v1262[v154] = $m56$toJSHelper(source[v202[v169]], alreadySeen));
      v169 = v169 + 1;
    }
    return v1262;
  }
  function $m57$nodeToDependencyTree(node) {
    var v5 = { name: node.name_ };
    if (node.observing_) {
      var v13 = node.observing_;
      var v162 = v13.length > 0;
      var v17 = v162, v18, v23, v41;
    } else {
      v17 = false;
    }
    if (v17) {
      v18 = [];
      v41 = 0;
      for (; ; ) {
        v23 = node.observing_;
        if (v41 >= v23.length) {
          break;
        }
        v18.push($m57$nodeToDependencyTree(node.observing_[v41]));
        v41 = v41 + 1;
      }
      v5.dependencies = v18;
    }
    return v5;
  }
  function $m57$nodeToObserverTree(node) {
    var v5 = { name: node.name_ };
    if ($m10$hasObservers(node)) {
      var v10 = node.observers_;
      var v13 = $m3$arrayFrom(v10.values());
      var v14 = [];
      var v33 = 0;
      while (v33 < v13.length) {
        v14.push($m57$nodeToObserverTree(v13[v33]));
        v33 = v33 + 1;
      }
      v5.observers = v14;
    }
    return v5;
  }
  var $m3$objectCtor = Object;
  var v3 = $m3$objectCtor.prototype;
  var $m3$hasOwnPropertyFn = v3.hasOwnProperty;
  var $m3$objectIsFn = $m3$objectCtor.is;
  var v11 = Array.prototype;
  var $m3$arraySliceFn = v11.slice;
  var v16 = $m3$objectCtor.prototype;
  var $m3$isPrototypeOfFn = v16.isPrototypeOf;
  var $m3$plainObjectString = $m3$objectCtor.toString();
  var $m3$iteratorProtoCached = void 0;
  var $m3$iteratorProtoReady = false;
  var $m3$getSelf = (0, function() {
    return this;
  });
  var $m3$noop = function() {
  };
  var v1311 = [];
  $m3$objectCtor.freeze(v1311);
  var $m2$EMPTY_ARRAY = v1311;
  var v1315 = {};
  $m3$objectCtor.freeze(v1315);
  var $m2$EMPTY_OBJECT = v1315;
  var $m2$$mobx = Symbol("mobx administration");
  var $m6$defaultEquals = function(a$2, b) {
    return true === $m3$objectIsFn(a$2, b);
  };
  var $m7$isObservableValuePred = void 0;
  var $m7$isObservableMapPred = void 0;
  var $m7$isObservableSetPred = void 0;
  var $m7$isObservableArrayAdmPred = void 0;
  var $m7$isObservableObjectAdmPred = void 0;
  var $m8$canMergeGlobalState = true;
  var $m8$isolateCalled = false;
  var $m8$persistentKeys = [];
  $m8$initPersistentKeys();
  var $m8$globalState = $m8$initGlobalState();
  var $m8$isolateGlobalState = function() {
    var v32 = $m8$globalState.pendingReactions;
    if (0 != v32.length) {
      var v14 = true, v13, v19, v20, v31, v49;
    } else {
      v13 = 0 != (+$m8$globalState.inBatch | 0);
      v14 = v13;
    }
    (v14 || $m8$globalState.isRunningReactions) && $m4$die(36);
    $m8$isolateCalled = true;
    $m8$canMergeGlobalState && (v49 = globalThis, v31 = (+v49.__mobxInstanceCount | 0) - 1 | 0, v49.__mobxInstanceCount = v31, 0 == v31 && (v49.__mobxGlobals = void 0), $m8$globalState = $m8$createMobXGlobals());
  };
  var v78 = function() {
    return $m8$globalState;
  };
  var v80 = function() {
    var v1 = $m8$createMobXGlobals();
    var v38 = $m3$objectCtor.keys(v1);
    var v42 = v38.length;
    var v35 = 0, v12, v27;
    while (v35 < v42) {
      v12 = v38[v35];
      (+$m8$persistentKeys.indexOf(v12) | 0) == a && ($m8$globalState[v12] = v1[v12]);
      v35 = v35 + 1;
    }
    v27 = $m8$globalState;
    v27.allowStateChanges = !$m8$globalState.enforceActions;
  };
  var $m9$END_EVENT = { type: "report-end", spyReportEnd: true };
  var v87 = function(listener) {
    var v10 = $m8$globalState.spyListeners;
    v10.push(listener);
    return $m3$once(function() {
      var v2 = $m8$globalState;
      var v6 = $m8$globalState.spyListeners;
      var v112 = [];
      var v15 = v6.length;
      var v17 = 0, v22;
      while (v17 < v15) {
        v6[v17] === listener || (v22 = v6[v17], v112.push(v22));
        v17 = v17 + 1;
      }
      v2.spyListeners = v112;
    });
  };
  var $m10$propagateChangedEach = function(d) {
    d.dependenciesState_ || d.onBecomeStale_();
    d.dependenciesState_ = 2;
  };
  var $m10$propagateMaybeChangedEach = function(d) {
    d.dependenciesState_ || (d.dependenciesState_ = 1, d.onBecomeStale_());
  };
  var $m10$confirmedObservable = void 0;
  var $m10$propagateChangeConfirmedEach = function(d) {
    var v4 = d.dependenciesState_;
    1 === v4 ? d.dependenciesState_ = 2 : v4 || ($m10$confirmedObservable.lowestObserverState_ = 0);
  };
  var $m10$reactionScheduler = function(f) {
    return f();
  };
  var $m10$runReactionsHelperFn = function() {
    $m8$globalState.isRunningReactions = true;
    var v5 = $m8$globalState.pendingReactions;
    var v60 = 0, v13, v23, v50, v61, v72, v76;
    while (v5.length > 0) {
      v13 = v60 + 1 | 0;
      100 == v13 && (v23 = "Reaction doesn't converge to a stable state after 100 iterations. Probably there is a cycle in the reactive function: " + v5[0], console.error(v23), v5.splice(0, v5.length));
      v72 = v5.splice(0, v5.length);
      v76 = v72.length;
      v61 = 0;
      while (v61 < v76) {
        v50 = v72[v61];
        v50.runReaction_();
        v61 = v61 + 1;
      }
      v60 = v13;
    }
    $m8$globalState.isRunningReactions = false;
  };
  var v98 = function(actionFn) {
    var prev = $m11$untrackedStart();
    try {
      return actionFn();
    } finally {
      $m11$untrackedEnd(prev);
    }
  };
  var $m11$allowStateChanges = function(allow, callback) {
    var prev = $m11$allowStateChangesStart(!!allow);
    try {
      return callback();
    } finally {
      $m8$globalState.allowStateChanges = prev;
    }
  };
  var $m11$CaughtException = (0, function(cause) {
    this.cause = cause;
    return this;
  });
  $m3$defineProperty($m11$CaughtException, "name", { value: "CaughtException", configurable: true });
  var v113 = $m11$CaughtException.prototype;
  v113.isMobXCaughtException = true;
  var $m13$Atom = (0, function(nameArg) {
    if (!(nameArg === void 0)) {
      var v31 = nameArg + "";
      var v29 = v31, v112;
    } else {
      v112 = "Atom@" + $m8$getNextId();
      v29 = v112;
    }
    this.name_ = v29;
    this.observers_ = /* @__PURE__ */ new Set();
    this.lastAccessedBy_ = 0;
    this.lowestObserverState_ = a;
    this.flags_ = 0;
    return this;
  });
  var v120 = $m13$Atom.prototype;
  v120.onBO = function() {
    $m12$onBOImpl(this);
  };
  var v126 = $m13$Atom.prototype;
  v126.onBUO = function() {
    $m12$onBUOImpl(this);
  };
  var v132 = $m13$Atom.prototype;
  v132.reportObserved = function() {
    return $m10$reportObserved(this);
  };
  var v138 = $m13$Atom.prototype;
  v138.reportChanged = function() {
    $m10$reportChanged(this);
  };
  var v144 = $m13$Atom.prototype;
  v144.toString = function() {
    return this.name_;
  };
  $m3$createInstanceofPredicate("Atom", $m13$Atom);
  var v153 = $m13$Atom.prototype;
  $m12$installFlagAccessor(v153, "isBeingObserved", 1);
  $m12$installFlagAccessor(v153, "isPendingUnobservation", 2);
  $m12$installNumericFlagAccessor(v153, 4);
  var $m13$createAtom = function(nameArg, onObsArg, onUnobsArg) {
    if (!(nameArg === void 0)) {
      var v51 = new $m13$Atom(nameArg);
      var v46 = v51, v162, v20, v30, v42, v47, v48, v52;
    } else {
      v52 = new $m13$Atom();
      v46 = v52;
    }
    onObsArg === void 0 ? (v162 = $m3$noop, v47 = v162) : v47 = onObsArg;
    onUnobsArg === void 0 ? (v20 = $m3$noop, v48 = v20) : v48 = onUnobsArg;
    v47 === $m3$noop || (v46.onBOL = /* @__PURE__ */ new Set(), v30 = v46.onBOL, v30.add(v47));
    v48 === $m3$noop || (v46.onBUOL = /* @__PURE__ */ new Set(), v42 = v46.onBUOL, v42.add(v48));
    return v46;
  };
  var $m14$currentActionId = 0;
  var $m14$nextActionId = 1;
  var $m14$tmpNameDescriptor = { value: "action", configurable: true, writable: false, enumerable: false };
  var v1343 = $m3$objectCtor.getOwnPropertyDescriptor(function() {
  }, "name");
  if (!(v1343 == null)) {
    v179 = !!v1343.configurable;
    v180 = v179;
  } else {
    v180 = false;
  }
  var v179;
  var v180;
  var v185;
  var v191;
  var v197;
  var v203;
  var v209;
  var v215;
  var v221;
  var v227;
  var v236;
  var v239;
  var v244;
  var v250;
  var v256;
  var v262;
  var v268;
  var v274;
  var v280;
  var v286;
  var v292;
  var v298;
  var v304;
  var v310;
  var v320;
  var v323;
  var v325;
  var v327;
  var v352;
  var v354;
  var v370;
  var v373;
  var v377;
  var v382;
  var v388;
  var v394;
  var v400;
  var v406;
  var v412;
  var v418;
  var v424;
  var v430;
  var v438;
  var v464;
  var v470;
  var v476;
  var v482;
  var v488;
  var v494;
  var v500;
  var v506;
  var v512;
  var v518;
  var v524;
  var v530;
  var v536;
  var v542;
  var v548;
  var v604;
  var v610;
  var v616;
  var v622;
  var v628;
  var v634;
  var v640;
  var v754;
  var v760;
  var v766;
  var v772;
  var v778;
  var v784;
  var v790;
  var v796;
  var v802;
  var v808;
  var v814;
  var v820;
  var v826;
  var v832;
  var v838;
  var v844;
  var v850;
  var v856;
  var v862;
  var v869;
  var v876;
  var v890;
  var v896;
  var v902;
  var v908;
  var v914;
  var v920;
  var v926;
  var v932;
  var v938;
  var v944;
  var v950;
  var v956;
  var v963;
  var v970;
  var v1020;
  var v1025;
  var v1030;
  var v1060;
  var v1063;
  var v1066;
  var v1077;
  var v1084;
  var v1086;
  var v1096;
  var v1108;
  var v1119;
  var v1127;
  var v1130;
  var v1147;
  var v1152;
  var v1155;
  var v1168;
  var v1170;
  var v1174;
  var v1176;
  var v1178;
  var v1184;
  var v1186;
  var v1190;
  var v1192;
  var v1196;
  var v1200;
  var v1204;
  var v1210;
  var v1212;
  var v1214;
  var v1216;
  var v1218;
  var v1220;
  var v1222;
  var v1224;
  var v1226;
  var v1228;
  var v1230;
  var v1346;
  var v1373;
  var v1375;
  var v1385;
  var v1387;
  var v1389;
  var v1391;
  var v1404;
  var v1411;
  var $m14$isFunctionNameConfigurable = v180;
  var $m15$Reaction = (0, function(v162, v36, v44, v59) {
    if (arguments.length > 0 && !(v162 === void 0)) {
      var v94 = v162 + "";
      var v88 = v94, v12, v13, v21, v45, v46, v61, v62;
    } else {
      v21 = "Reaction@" + $m8$getNextId();
      v88 = v21;
    }
    this.name_ = v88;
    this.onInvalidate_ = void 0;
    arguments.length > 1 && (this.onInvalidate_ = v36);
    arguments.length > 2 ? (v45 = !!v44, v46 = v45) : v46 = false;
    v46 && (this.errorHandler_ = v44);
    arguments.length > 3 && !(v59 === void 0) && (this.requiresObservable_ = v59);
    this.observing_ = [];
    this.newObserving_ = null;
    this.dependenciesState_ = a;
    this.runId_ = 0;
    this.unboundDepsCount_ = 0;
    this.flags_ = 0;
    return this;
  });
  v185 = $m15$Reaction.prototype;
  v185.onBecomeStale_ = function() {
    this.schedule_();
  };
  v191 = $m15$Reaction.prototype;
  v191.schedule_ = function() {
    if (!this.isScheduled) {
      this.isScheduled = true;
      var v112 = $m8$globalState.pendingReactions;
      v112.push(this);
      $m10$runReactions();
    }
  };
  v197 = $m15$Reaction.prototype;
  v197.runReaction_ = function() {
    if (!this.isDisposed) {
      $m10$startBatch();
      this.isScheduled = false;
      var v12 = $m8$globalState.trackingContext;
      $m8$globalState.trackingContext = this;
      if ($m11$shouldCompute(this)) {
        this.isTrackPending = true;
        try {
          this.onInvalidate_();
          var v29 = !!this.isTrackPending;
          var v30 = v29, v32, v33, v37, v42;
          v30 && $m9$isSpyEnabled() && (v37 = this.name_, $m9$spyReport({ name: v37, type: "scheduled-reaction" }));
        } catch (v422) {
          this.reportExceptionInDerivation_(v422);
        }
      }
      $m8$globalState.trackingContext = v12;
      $m10$endBatch();
    }
  };
  v203 = $m15$Reaction.prototype;
  v203.track = function(fn) {
    if (this.isDisposed) return;
    $m10$startBatch();
    var v10 = $m9$isSpyEnabled();
    var v802 = void 0;
    if (v10) {
      var v81 = Date.now();
      var v21 = this.name_;
      $m9$spyReportStart({ name: v21, type: "reaction" });
      var v782 = v81, v31, v38, v86;
    } else {
      v782 = v802;
    }
    this.isRunning = true;
    v31 = $m8$globalState.trackingContext;
    $m8$globalState.trackingContext = this;
    v38 = $m11$trackDerivedFunction(this, fn, void 0);
    $m8$globalState.trackingContext = v31;
    this.isRunning = false;
    this.isTrackPending = false;
    !this.isDisposed || $m11$clearObserving(this);
    $m11$isCaughtException(v38) && this.reportExceptionInDerivation_(v38.cause);
    v10 && (v86 = +Date.now() | 0, $m9$spyReportEnd({ time: v86 - (+v782 | 0) | 0 }));
    $m10$endBatch();
  };
  v209 = $m15$Reaction.prototype;
  v209.reportExceptionInDerivation_ = function(error) {
    if (this.errorHandler_) {
      this.errorHandler_(error, this);
      return;
    }
    !$m8$globalState.disableErrorBoundaries || $m3$throwErr(error);
    "[mobx] uncaught error in '" + this;
    var v32 = "[mobx] Encountered an uncaught exception that was thrown by a reaction or observer component, in: '" + this + "'";
    !$m8$globalState.suppressReactionErrors ? console.error(v32, error) : console.warn("[mobx] (error in reaction '" + this.name_ + "' suppressed, fix error of causing action below)");
    var v53 = $m9$isSpyEnabled();
    if (v53) {
      var v60 = this.name_;
      $m9$spyReport({ type: "error", name: v60, message: v32, error: "" + error });
    }
    var v71 = $m8$globalState.globalReactionErrorHandlers;
    var v106 = v71.length;
    var v90 = 0;
    while (v90 < v106) {
      v71[v90](error, this);
      v90 = v90 + 1;
    }
  };
  v215 = $m15$Reaction.prototype;
  v215.dispose = function() {
    this.isDisposed || (this.isDisposed = true, this.isRunning || ($m10$startBatch(), $m11$clearObserving(this), $m10$endBatch()));
  };
  v221 = $m15$Reaction.prototype;
  v221.getDisposer_ = function(abortSignal) {
    var self = this;
    var dispose = function() {
      var v5 = self.dispose;
      self.dispose();
      !(abortSignal == null) && "function" == typeof abortSignal.removeEventListener && abortSignal.removeEventListener("abort", dispose);
    };
    !(abortSignal == null) && "function" == typeof abortSignal.addEventListener && abortSignal.addEventListener("abort", dispose);
    dispose[$m2$$mobx] = self;
    if (true === "dispose" in Symbol) {
      var v48 = "symbol" == typeof Symbol.dispose;
      var v31 = v48, v14$2, v32, v35, v41;
    } else {
      v31 = false;
    }
    v31 && (v32 = dispose, v35 = Symbol.dispose, v32[v35] = dispose);
    return dispose;
  };
  v227 = $m15$Reaction.prototype;
  v227.toString = function() {
    return "Reaction[" + this.name_ + "]";
  };
  $m3$createInstanceofPredicate("Reaction", $m15$Reaction);
  v236 = $m15$Reaction.prototype;
  $m12$installFlagAccessor(v236, "isDisposed", 1);
  $m12$installFlagAccessor(v236, "isScheduled", 2);
  $m12$installFlagAccessor(v236, "isTrackPending", 4);
  $m12$installFlagAccessor(v236, "isRunning", 8);
  $m12$installNumericFlagAccessor(v236, 16);
  v239 = function(handler) {
    let v4 = $m8$globalState.globalReactionErrorHandlers;
    v4.push(handler);
    return function() {
      var v42 = $m8$globalState.globalReactionErrorHandlers;
      var v20 = +v42.indexOf(handler) | 0;
      v20 >= 0 && $m8$globalState.globalReactionErrorHandlers.splice(v20, 1);
    };
  };
  var $m16$ComputedValue = (0, function(options) {
    options.get || $m4$die(31);
    this.derivation = options.get;
    if (options.name) {
      var v115 = options.name + "";
      var v1132 = v115, v26, v38, v872;
    } else {
      v26 = "ComputedValue@" + $m8$getNextId();
      v1132 = v26;
    }
    this.name_ = v1132;
    options.set ? (v38 = v1132 + "-setter", this.setter_ = $m14$createAction(v38, options.set, false, void 0)) : this.setter_ = void 0;
    this.equals_ = $m6$defaultEquals;
    !options.equals || (this.equals_ = options.equals);
    this.scope_ = options.context;
    this.requiresReaction_ = options.requiresReaction;
    this.keepAlive_ = !!options.keepAlive;
    this.dependenciesState_ = a;
    this.observing_ = [];
    v872 = null;
    this.newObserving_ = v872;
    this.observers_ = /* @__PURE__ */ new Set();
    this.runId_ = 0;
    this.lastAccessedBy_ = 0;
    this.lowestObserverState_ = 0;
    this.unboundDepsCount_ = 0;
    this.value_ = new $m11$CaughtException(v872);
    this.flags_ = 0;
    return this;
  });
  v244 = $m16$ComputedValue.prototype;
  v244.onBecomeStale_ = function() {
    $m10$propagateMaybeChanged(this);
  };
  v250 = $m16$ComputedValue.prototype;
  v250.onBO = function() {
    $m12$onBOImpl(this);
  };
  v256 = $m16$ComputedValue.prototype;
  v256.onBUO = function() {
    $m12$onBUOImpl(this);
  };
  v262 = $m16$ComputedValue.prototype;
  v262.computeValue_ = function(track) {
    this.isComputing = true;
    var v8 = $m11$allowStateChangesStart(false);
    var v49 = v8, v15, v22;
    var res;
    if (track) {
      v15 = this.derivation;
      res = $m11$trackDerivedFunction(this, v15, this.scope_);
    } else {
      v22 = $m8$globalState.disableErrorBoundaries;
      if (true === v22) {
        res = this.derivation.call(this.scope_);
      } else {
        try {
          res = this.derivation.call(this.scope_);
        } catch (v39) {
          res = new $m11$CaughtException(v39);
        }
      }
    }
    $m8$globalState.allowStateChanges = v49;
    this.isComputing = false;
    return res;
  };
  v268 = $m16$ComputedValue.prototype;
  v268.trackAndCompute = function() {
    var v32 = this.value_;
    var v9 = (+this.dependenciesState_ | 0) == a;
    var v65 = this.computeValue_(true);
    if (v9) {
      var v20 = true, v19, v23, v24, v34, v35, v42, v49, v53;
    } else {
      v19 = $m11$isCaughtException(v32);
      v20 = v19;
    }
    v35 = v20 || $m11$isCaughtException(v65) || !(true === this.equals_(v32, v65));
    v35 && (this.value_ = v65, v42 = $m9$isSpyEnabled(), v42 && (v49 = this.name_, v53 = this.scope_, $m9$spyReport({ observableKind: "computed", debugObjectName: v49, object: v53, type: "update", oldValue: v32, newValue: v65 })));
    return v35;
  };
  v274 = $m16$ComputedValue.prototype;
  v274.get = function() {
    if (this.isComputing) {
      var v8 = this.name_;
      var v112 = this.derivation;
      var v88 = [];
      v88.push(v8);
      v88.push(v112);
      $m4$dieRest(32, v88);
    }
    if (!$m8$globalState.inBatch) {
      var v21 = this.observers_;
      var v25 = !v21.size;
      var v26 = v25, v32, v33, v56, v64, v65, v802;
    } else {
      v26 = false;
    }
    if (v26 && !this.keepAlive_) {
      $m11$shouldCompute(this) && (this.warnAboutUntrackedRead_(), $m10$startBatch(), this.value_ = this.computeValue_(false), $m10$endBatch());
    } else {
      $m10$reportObserved(this);
      if ($m11$shouldCompute(this)) {
        var v54 = $m8$globalState;
        v56 = v54.trackingContext;
        this.keepAlive_ && !v56 && ($m8$globalState.trackingContext = this);
        !this.trackAndCompute() || $m10$propagateChangeConfirmed(this);
        $m8$globalState.trackingContext = v56;
      }
    }
    v802 = this.value_;
    $m11$isCaughtException(v802) && $m3$throwErr(v802.cause);
    return v802;
  };
  v280 = $m16$ComputedValue.prototype;
  v280.set = $jsMethod1(function(self, value) {
    if (self.setter_) {
      !self.isRunningSetter || $m4$die1(33, self.name_);
      self.isRunningSetter = true;
      try {
        self.setter_.call(self.scope_, value);
      } finally {
        self.isRunningSetter = false;
      }
    } else {
      $m4$die1(34, self.name_);
    }
  });
  v286 = $m16$ComputedValue.prototype;
  v286.suspend_ = function() {
    this.keepAlive_ || ($m11$clearObserving(this), this.value_ = void 0);
  };
  v292 = $m16$ComputedValue.prototype;
  v292.warnAboutUntrackedRead_ = function() {
    if ("boolean" == typeof this.requiresReaction_) {
      var v12 = !!this.requiresReaction_;
      var v28 = v12, v162;
    } else {
      v162 = !!$m8$globalState.computedRequiresReaction;
      v28 = v162;
    }
    v28 && console.warn("[mobx] Computed value '" + this.name_ + "' is being read outside a reactive context. Doing a full recompute.");
  };
  v298 = $m16$ComputedValue.prototype;
  v298.toString = function() {
    let v6 = this.name_ + "[", v9 = this.derivation;
    return v6 + v9.toString() + "]";
  };
  v304 = $m16$ComputedValue.prototype;
  v304.valueOf = function() {
    return $m3$toPrimitive(this.get());
  };
  v310 = $m16$ComputedValue.prototype;
  v1346 = Symbol.toPrimitive;
  v310[v1346] = function() {
    return this.valueOf();
  };
  $m3$createInstanceofPredicate("ComputedValue", $m16$ComputedValue);
  v320 = $m16$ComputedValue.prototype;
  $m12$installFlagAccessor(v320, "isComputing", 1);
  $m12$installFlagAccessor(v320, "isRunningSetter", 2);
  $m12$installFlagAccessor(v320, "isBeingObserved", 4);
  $m12$installFlagAccessor(v320, "isPendingUnobservation", 8);
  $m12$installNumericFlagAccessor(v320, 16);
  v323 = function(actionFn, thisArg) {
    $m10$startBatch();
    try {
      return actionFn.apply(thisArg);
    } finally {
      $m10$endBatch();
    }
  };
  v325 = function(value) {
    return $m25$isObservableValueCheck(value);
  };
  v327 = function(value, property) {
    var v7 = !$m3$isStringish(property);
    v7 && $m4$die("expected a property name as second argument");
    if (!$m7$isObservableObject(value)) return false;
    var v17 = value[$m2$$mobx];
    var v20 = v17.values_;
    if (v20.has(property)) return true;
    if (v17.lazyComputedKeys_) {
      var v31 = v17.lazyComputedKeys_;
      var v54 = !!v31.has(property);
      var v34 = v54, v43, v46, v57;
    } else {
      v34 = false;
    }
    if (v34) return true;
    v17.lazyObservableKeys_ ? (v43 = v17.lazyObservableKeys_, v57 = !!v43.has(property), v46 = v57) : v46 = false;
    if (v46) return true;
    return false;
  };
  var $m28$observable = void 0;
  var $m28$observableRef = void 0;
  var $m28$computed = void 0;
  var $m28$action = void 0;
  var $m28$autoAction = void 0;
  var $m28$autoActionBound = void 0;
  var $m28$flow = void 0;
  var $m28$flowBound = void 0;
  var $m28$autoAnnotation = void 0;
  var $m28$extendObservable = void 0;
  var $m27$deepEnhancerFn = void 0;
  var $m27$referenceEnhancerFn = void 0;
  $m27$referenceEnhancerFn = function(v) {
    return v;
  };
  $m27$deepEnhancerFn = function(v, _old, name) {
    if ($m25$isObservableValueCheck(v)) return v;
    if (Array.isArray(v)) {
      if (name) return $m28$observable.array.call($m28$observable, v, { name });
      return $m28$observable.array.call($m28$observable, v);
    }
    if ($m3$isPlainObject(v)) {
      if (name) return $m28$observable.object.call($m28$observable, v, void 0, { name });
      return $m28$observable.object.call($m28$observable, v);
    }
    if ($m3$isES6Map(v)) {
      if (name) return $m28$observable.map.call($m28$observable, v, { name });
      return $m28$observable.map.call($m28$observable, v);
    }
    if ($m3$isES6Set(v)) {
      if (name) return $m28$observable.set.call($m28$observable, v, { name });
      return $m28$observable.set.call($m28$observable, v);
    }
    if ("function" == typeof v && !$m7$isAction(v) && !$m7$isFlow(v)) {
      if ($m3$isGenerator(v)) return $m28$flow(v);
      return $m28$autoAction(name, v);
    }
    return v;
  };
  v352 = function(v, _old, name) {
    if (v == null) return v;
    if ($m7$isObservableObject(v) || $m7$isObservableArray(v) || $m7$isObservableMap(v) || $m7$isObservableSet(v)) return v;
    if (Array.isArray(v)) return $m28$observable.array.call($m28$observable, v, { name, deep: false });
    if ($m3$isPlainObject(v)) return $m28$observable.object.call($m28$observable, v, void 0, { name, deep: false });
    if ($m3$isES6Map(v)) return $m28$observable.map.call($m28$observable, v, { name, deep: false });
    if ($m3$isES6Set(v)) return $m28$observable.set.call($m28$observable, v, { name, deep: false });
    $m4$die("The shallow modifier / decorator can only used in combination with arrays, objects, maps and sets");
    return v;
  };
  v354 = function(v, oldValue) {
    var v6 = $m25$isObservableValueCheck(v);
    v6 && $m4$die("observable.struct should not be used with observable values");
    return $m26$eq(v, oldValue, a, void 0, void 0) ? oldValue : v;
  };
  var $m27$defaultCreateObservableOptions = { deep: true, name: void 0, defaultDecorator: void 0 };
  $m3$objectCtor.freeze($m27$defaultCreateObservableOptions);
  var $m29$ObservableValue = (0, function(v4, v7, v162, v35, v45) {
    if (arguments.length > 2 && !(v162 === void 0)) {
      var v110 = v162 + "";
      var v102 = v110, v18, v19, v27, v36, v37, v46, v47, v802, v81, v91, v103, v104;
    } else {
      v27 = "ObservableValue@" + $m8$getNextId();
      v102 = v27;
    }
    arguments.length > 3 ? (v36 = !!v35, v103 = v36) : v103 = true;
    v37 = $m6$defaultEquals;
    arguments.length > 4 ? (v46 = !!v45, v47 = v46) : v47 = false;
    v104 = v47 ? v45 : v37;
    $m13$Atom.call(this, v102);
    this.enhancer_ = v7;
    this.name_ = v102;
    this.equals_ = v104;
    this.hasUnreportedChange_ = false;
    this.value_ = v7(v4, void 0, v102);
    var v119 = void 0;
    v103 && $m9$isSpyEnabled() && (v91 = this.name_, $m9$spyReport({ type: "create", object: this, observableKind: "value", debugObjectName: v91, newValue: "" + this.value_ }));
    return this;
  });
  v370 = $m29$ObservableValue.prototype;
  v373 = $m13$Atom.prototype;
  $m3$objectCtor.setPrototypeOf(v370, v373);
  v377 = $m29$ObservableValue.prototype;
  v377.constructor = $m29$ObservableValue;
  v382 = $m29$ObservableValue.prototype;
  v382.prepareNewValue_ = function(newValue) {
    $m11$checkIfStateModificationsAreAllowed(this);
    if ($m18$hasInterceptors(this)) {
      var v15 = $m18$interceptChange(this, { object: this, type: "update", newValue });
      if (!v15) return $m8$globalState.UNCHANGED;
      var v24 = v15.newValue;
      var v51 = v24, v36;
    } else {
      v51 = newValue;
    }
    v36 = this.enhancer_(v51, this.value_, this.name_);
    return true === this.equals_(this.value_, v36) ? $m8$globalState.UNCHANGED : v36;
  };
  v388 = $m29$ObservableValue.prototype;
  v388.setNewValue_ = function(newValue) {
    var v4 = this.value_;
    this.value_ = newValue;
    $m10$reportChanged(this);
    $m17$hasListeners(this) && $m17$notifyListeners(this, { type: "update", object: this, newValue, oldValue: v4 });
  };
  v394 = $m29$ObservableValue.prototype;
  v394.set = function(newValue) {
    var v48 = this.prepareNewValue_(newValue);
    if (!(v48 === $m8$globalState.UNCHANGED)) {
      var v14 = $m9$isSpyEnabled();
      if (v14) {
        var v29 = this.name_;
        $m9$spyReportStart({ type: "update", object: this, observableKind: "value", debugObjectName: v29, newValue: v48, oldValue: this.value_ });
      }
      this.setNewValue_(v48);
      v14 && $m9$spyReportEnd(void 0);
    }
  };
  v400 = $m29$ObservableValue.prototype;
  v400.get = function() {
    $m10$reportObserved(this);
    if (this.dehancer === void 0) return this.value_;
    return this.dehancer(this.value_);
  };
  v406 = $m29$ObservableValue.prototype;
  v406.raw = function() {
    return this.value_;
  };
  v412 = $m29$ObservableValue.prototype;
  v412.toJSON = function() {
    return this.get();
  };
  v418 = $m29$ObservableValue.prototype;
  v418.toString = function() {
    let v6 = this.name_ + "[";
    return v6 + this.value_ + "]";
  };
  v424 = $m29$ObservableValue.prototype;
  v424.valueOf = function() {
    return $m3$toPrimitive(this.get());
  };
  v430 = $m29$ObservableValue.prototype;
  v1373 = Symbol.toPrimitive;
  v430[v1373] = function() {
    return this.valueOf();
  };
  $m7$isObservableValuePred = $m3$createInstanceofPredicate("ObservableValue", $m29$ObservableValue);
  v438 = {};
  v438.annotationType_ = "override";
  v438.make_ = function(adm, key) {
    var v8 = !!adm.isPlainObject_;
    if (v8) {
      var v17 = "Cannot apply '" + this.annotationType_ + "' to '";
      $m4$die(v17 + adm.name_ + "." + $m3$stringifyKey(key) + "':\n'" + this.annotationType_ + "' cannot be used on plain objects.");
    }
    var v45 = !$m3$hasOwn(adm.appliedAnnotations_, key);
    if (v45) {
      var v59 = "'" + adm.name_ + "." + $m3$stringifyKey(key) + "' is annotated with '";
      $m4$die(v59 + this.annotationType_ + "', but no such annotated member was found on prototype.");
    }
    return 0;
  };
  v438.extend_ = function() {
    $m4$die1(44, this.annotationType_);
    return false;
  };
  var $m31$autoAnnotationMake = (0, function(v4, v7, v10, v13) {
    if (v10.get) return $m28$computed.make_.call($m28$computed, v4, v7, v10, v13);
    if (v10.set) {
      var v33 = v10.set;
      if (!$m7$isAction(v33)) {
        var v42 = $m14$createAction($m3$stringifyKey(v7), v33, false, void 0);
        var v184 = v42, v83, v88, v96, v99, v100, v101, v111, v119, v122, v123, v124, v134, v142, v1442, v146, v147, v148, v157, v158, v162, v165, v166, v185, v186, v187, v192;
      } else {
        v184 = v33;
      }
      if (v13 === v4.target_) {
        if (v4.defineProperty_(v7, { configurable: true, set: v184 }) == null) return 0;
        return 2;
      }
      $m3$defineProperty(v13, v7, { configurable: true, set: v184 });
      return 2;
    }
    if (!(v13 === v4.target_) && "function" == typeof v10.value) {
      if ($m3$isGenerator(v10.value)) return v88 = $m28$flow, this.options_ ? (v96 = this.options_, v99 = !!v96.autoBind, v100 = v99) : v100 = false, v100 ? (v101 = $m28$flowBound, v185 = v101) : v185 = v88, v185.make_(v4, v7, v10, v13);
      v111 = $m28$autoAction;
      this.options_ ? (v119 = this.options_, v122 = !!v119.autoBind, v123 = v122) : v123 = false;
      v123 ? (v124 = $m28$autoActionBound, v186 = v124) : v186 = v111;
      return v186.make_(v4, v7, v10, v13);
    }
    v134 = $m28$observable;
    this.options_ ? (v142 = this.options_, v1442 = v142.deep, v146 = false === v1442, v147 = v146) : v147 = false;
    v147 ? (v148 = $m28$observableRef, v187 = v148) : v187 = v134;
    "function" == typeof v10.value ? (v157 = !!this.options_, v158 = v157) : v158 = false;
    if (v158 && this.options_.autoBind) {
      var v197 = v10.value.bind($m32$admProxyOrTarget(v4));
      v10.value = v197;
    }
    return v187.make_(v4, v7, v10, v13);
  });
  var $m31$autoAnnotationExtend = (0, function(v4, v7, v10, v13) {
    if (v10.get) return $m28$computed.extend_.call($m28$computed, v4, v7, v10, v13);
    if (v10.set) {
      var v33 = v4.defineProperty_;
      return v4.defineProperty_(v7, { configurable: true, set: $m14$createAction($m3$stringifyKey(v7), v10.set, false, void 0) }, v13);
    }
    if ("function" == typeof v10.value) {
      var v58 = !!this.options_;
      var v59 = v58, v63, v66, v67, v76, v84, v86, v88, v89, v90, v100;
    } else {
      v59 = false;
    }
    if (v59 && this.options_.autoBind) {
      var v106 = v10.value.bind($m32$admProxyOrTarget(v4));
      v10.value = v106;
    }
    v76 = $m28$observable;
    this.options_ ? (v84 = this.options_, v86 = v84.deep, v88 = false === v86, v89 = v88) : v89 = false;
    v89 ? (v90 = $m28$observableRef, v100 = v90) : v100 = v76;
    return v100.extend_(v4, v7, v10, v13);
  });
  v1375 = {};
  v1375.annotationType_ = "true";
  v1375.options_ = void 0;
  v1375.make_ = $m31$autoAnnotationMake;
  v1375.extend_ = $m31$autoAnnotationExtend;
  $m28$autoAnnotation = v1375;
  var $m33$descriptorCache = $m3$objectCtor.create(null);
  var $m33$ObservableObjectAdministration = (0, function(v6, v14, v29, v41) {
    this.target_ = v6;
    if (arguments.length > 1) {
      var v15 = !!v14;
      var v162 = v15, v42, v43, v56, v74;
    } else {
      v162 = false;
    }
    v162 ? this.values_ = v14 : this.values_ = /* @__PURE__ */ new Map();
    this.name_ = v29 + "";
    this.defaultAnnotation_ = $m28$autoAnnotation;
    arguments.length > 3 ? (v42 = !!v41, v43 = v42) : v43 = false;
    v43 && (this.defaultAnnotation_ = v41);
    v56 = this.name_ + ".keys";
    this.keysAtom_ = new $m13$Atom(v56);
    this.isPlainObject_ = $m3$isPlainObject(this.target_);
    v74 = !$m20$isAnnotation(this.defaultAnnotation_);
    v74 && $m4$die("defaultAnnotation must be valid annotation");
    this.appliedAnnotations_ = {};
    return this;
  });
  v464 = $m33$ObservableObjectAdministration.prototype;
  v464.materializeLazyComputed_ = function(key) {
    if (!this.lazyComputedKeys_) return;
    var v10 = this.lazyComputedKeys_;
    var v43 = v10.get(key);
    if (!v43) return;
    var v19 = this.lazyComputedKeys_;
    v19.delete(key);
    var v24 = this.lazyComputedKeys_;
    0 == (+v24.size | 0) && (this.lazyComputedKeys_ = void 0);
    var v54 = v43();
    var v36 = this.values_;
    v36.set(key, v54);
    return v54;
  };
  v470 = $m33$ObservableObjectAdministration.prototype;
  v470.materializeLazyObservable_ = function(key) {
    if (!this.lazyObservableKeys_) return;
    var v10 = this.lazyObservableKeys_;
    var v43 = v10.get(key);
    if (!v43) return;
    var v19 = this.lazyObservableKeys_;
    v19.delete(key);
    var v24 = this.lazyObservableKeys_;
    0 == (+v24.size | 0) && (this.lazyObservableKeys_ = void 0);
    var v54 = v43();
    var v36 = this.values_;
    v36.set(key, v54);
    return v54;
  };
  v476 = $m33$ObservableObjectAdministration.prototype;
  v476.getObservablePropValue_ = function(key) {
    var v4 = this.values_;
    var v31 = v4.get(key);
    if (!v31) {
      var v32 = this.materializeLazyComputed_(key);
      if (!v32) {
        var v34 = this.materializeLazyObservable_(key);
        var v28 = v34, v29;
      } else {
        v28 = v32;
      }
      v29 = v28;
    } else {
      v29 = v31;
    }
    return v29.get();
  };
  v482 = $m33$ObservableObjectAdministration.prototype;
  v482.setObservablePropValue_ = function(key, newValue) {
    var v5 = this.values_;
    var v122 = v5.get(key);
    if (!v122) {
      var v123 = this.materializeLazyComputed_(key);
      var v119 = v123, v48, v55, v67, v70, v72, v76, v84, v872, v91, v96, v117, v118, v1202, v124, v1262;
    } else {
      v119 = v122;
    }
    !v119 ? (v124 = this.materializeLazyObservable_(key), v118 = v124) : v118 = v119;
    if ($m7$isComputedValue(v118)) return v118.set(newValue), true;
    if ($m18$hasInterceptors(this)) {
      v48 = $m18$interceptChange(this, { type: "update", object: $m32$admProxyOrTarget(this), name: key, newValue });
      if (!v48) return null;
      v55 = v48.newValue;
      v117 = v55;
    } else {
      v117 = newValue;
    }
    v1262 = v118.prepareNewValue_(v117);
    v1262 === $m8$globalState.UNCHANGED || (v67 = $m17$hasListeners(this), v70 = $m9$isSpyEnabled(), v72 = null, v67 || v70 ? (v84 = this.name_, v872 = $m32$admProxyOrTarget(this), v91 = v118.value_, v96 = { type: "update", observableKind: "object", debugObjectName: v84, object: v872, oldValue: v91, name: key, newValue: v1262 }, v1202 = v96) : v1202 = v72, v70 && $m9$spyReportStart(v1202), v118.setNewValue_(v1262), v67 && $m17$notifyListeners(this, v1202), v70 && $m9$spyReportEnd(void 0));
    return true;
  };
  v488 = $m33$ObservableObjectAdministration.prototype;
  v488.get_ = function(key) {
    if ($m8$globalState.trackingDerivation) {
      var v12 = !$m3$hasOwn(this.target_, key);
      var v13 = v12;
    } else {
      v13 = false;
    }
    v13 && this.has_(key);
    return this.target_[key];
  };
  v494 = $m33$ObservableObjectAdministration.prototype;
  v494.set_ = function(key, value) {
    return $m33$setObjectValue(this, key, value, false);
  };
  v500 = $m33$ObservableObjectAdministration.prototype;
  v500.has_ = function(key) {
    if (!$m8$globalState.trackingDerivation) return true === key in this.target_;
    this.pendingKeys_ || (this.pendingKeys_ = /* @__PURE__ */ new Map());
    var v22 = this.pendingKeys_;
    var v69 = v22.get(key);
    if (!v69) {
      var v40 = this.name_ + "." + $m3$stringifyKey(key) + "?";
      var v41 = $m29$ObservableValue;
      var v50 = new v41(true === key in this.target_, $m27$referenceEnhancerFn, v40, false);
      var v53 = this.pendingKeys_;
      v53.set(key, v50);
      var v62 = v50;
    } else {
      v62 = v69;
    }
    return v62.get();
  };
  v506 = $m33$ObservableObjectAdministration.prototype;
  v506.extend_ = function(v4, v7, v10, v18) {
    var v57 = arguments.length > 3 && v18, v24, v48, v56;
    true === v10 ? (v24 = this.defaultAnnotation_, v56 = v24) : v56 = v10;
    if (false === v56) return this.defineProperty_(v4, v7, v57);
    $m33$assertAnnotable(this, v56, v4);
    v48 = v56.extend_(this, v4, v7, v57);
    !v48 || $m33$recordAnnotationApplied(this, v56, v4);
    return v48;
  };
  v512 = $m33$ObservableObjectAdministration.prototype;
  v512.notifyPropertyAddition_ = function(key, value) {
    var v4 = $m17$hasListeners(this);
    var v7 = $m9$isSpyEnabled();
    if (v4 || v7) {
      var v20 = this.name_;
      var v28 = { type: "add", observableKind: "object", debugObjectName: v20, object: $m32$admProxyOrTarget(this), name: key, newValue: value };
      v7 && $m9$spyReportStart(v28);
      v4 && $m17$notifyListeners(this, v28);
      v7 && $m9$spyReportEnd(void 0);
    }
    if (this.pendingKeys_) {
      var v50 = this.pendingKeys_;
      var v69 = v50.get(key);
      !v69 || v69.set(true);
    }
    $m10$reportChanged(this.keysAtom_);
  };
  v518 = $m33$ObservableObjectAdministration.prototype;
  v518.defineProperty_ = function(key, descriptor, proxyTrapArg) {
    var v5 = !!proxyTrapArg;
    $m11$checkIfStateModificationsAreAllowed(this.keysAtom_);
    try {
      $m10$startBatch();
      var deleteOutcome = this.delete_(key);
      if (!deleteOutcome) return deleteOutcome;
      if ($m18$hasInterceptors(this)) {
        var v27 = $m32$admProxyOrTarget(this);
        var change = $m18$interceptChange(this, { object: v27, name: key, type: "add", newValue: descriptor.value });
        if (!change) return null;
        var v44 = descriptor.value;
        if (!(v44 === change.newValue)) {
          descriptor = $m3$assign({}, descriptor);
          var v53 = descriptor;
          v53.value = change.newValue;
        }
      }
      if (v5) {
        var v61 = this.target_;
        if (!(true === Reflect.defineProperty(v61, key, descriptor))) return false;
      } else {
        $m3$defineProperty(this.target_, key, descriptor);
      }
      this.notifyPropertyAddition_(key, descriptor.value);
    } finally {
      $m10$endBatch();
    }
    return true;
  };
  v524 = $m33$ObservableObjectAdministration.prototype;
  v524.defineObservableProperty_ = function() {
    var v4 = arguments[0];
    var value = arguments[1];
    var v10 = arguments[2];
    if (arguments.length > 3) {
      var v19 = !!arguments[3];
      var v130 = v19, v69, v75, v84;
    } else {
      v130 = false;
    }
    $m11$checkIfStateModificationsAreAllowed(this.keysAtom_);
    try {
      $m10$startBatch();
      var deleteOutcome = this.delete_(v4);
      if (!deleteOutcome) return deleteOutcome;
      if ($m18$hasInterceptors(this)) {
        var change = $m18$interceptChange(this, { object: $m32$admProxyOrTarget(this), name: v4, type: "add", newValue: value });
        if (!change) return null;
        value = change.newValue;
      }
      var cachedDescriptor = $m33$getCachedObservablePropDescriptor(v4);
      var configurable = true;
      !$m8$globalState.safeDescriptors || (configurable = !!this.isPlainObject_);
      v69 = configurable;
      v75 = cachedDescriptor.get;
      var descriptor = { configurable: v69, enumerable: true, get: v75, set: cachedDescriptor.set };
      if (v130) {
        v84 = this.target_;
        if (!(true === Reflect.defineProperty(v84, v4, descriptor))) return false;
      } else {
        $m3$defineProperty(this.target_, v4, descriptor);
      }
      var obsName = "ObservableObject.key";
      obsName = this.name_ + "." + $m3$stringifyKey(v4);
      var observableNode = new $m29$ObservableValue(value, v10, obsName, false);
      this.values_.set(v4, observableNode);
      this.notifyPropertyAddition_(v4, observableNode.value_);
    } finally {
      $m10$endBatch();
    }
    return true;
  };
  v530 = $m33$ObservableObjectAdministration.prototype;
  v530.defineComputedProperty_ = function(key, options, proxyTrapArg) {
    var v5 = !!proxyTrapArg;
    $m11$checkIfStateModificationsAreAllowed(this.keysAtom_);
    try {
      $m10$startBatch();
      var deleteOutcome = this.delete_(key);
      if (!deleteOutcome) return deleteOutcome;
      if ($m18$hasInterceptors(this)) {
        var change = $m18$interceptChange(this, { object: $m32$admProxyOrTarget(this), name: key, type: "add", newValue: void 0 });
        if (!change) return null;
      }
      options.name || (options.name = this.name_ + "." + $m3$stringifyKey(key));
      options.context = $m32$admProxyOrTarget(this);
      var cachedDescriptor = $m33$getCachedObservablePropDescriptor(key);
      var configurable = true;
      !$m8$globalState.safeDescriptors || (configurable = !!this.isPlainObject_);
      var v76 = configurable;
      var v82 = cachedDescriptor.get;
      var descriptor = { configurable: v76, enumerable: false, get: v82, set: cachedDescriptor.set };
      if (v5) {
        var v91 = this.target_;
        if (!(true === Reflect.defineProperty(v91, key, descriptor))) return false;
      } else {
        $m3$defineProperty(this.target_, key, descriptor);
      }
      this.values_.set(key, new $m16$ComputedValue(options));
      this.notifyPropertyAddition_(key, void 0);
    } finally {
      $m10$endBatch();
    }
    return true;
  };
  v536 = $m33$ObservableObjectAdministration.prototype;
  v536.delete_ = function(key, proxyTrapArg) {
    var v4 = !!proxyTrapArg;
    $m11$checkIfStateModificationsAreAllowed(this.keysAtom_);
    if (!$m3$hasOwn(this.target_, key)) return true;
    if ($m18$hasInterceptors(this) && !$m18$interceptChange(this, { object: $m32$admProxyOrTarget(this), name: key, type: "remove" })) return null;
    try {
      $m10$startBatch();
      var notify = $m17$hasListeners(this);
      var v38 = $m9$isSpyEnabled();
      var v39 = v38, v42, v52, v53, v54, v57, v68, v75, v91, v92, v102, v122, v129, v130, v139, v140, v147, v151, v159, v160, v169, v170;
      var notifySpy = v39;
      v42 = this.values_;
      var v175 = v42.get(key);
      var observableNode = v175;
      var value;
      if (!observableNode && (notify || notifySpy)) {
        v57 = this.target_;
        var desc = $m3$objectCtor.getOwnPropertyDescriptor(v57, key);
        !desc || (value = desc.value);
      }
      if (v4) {
        v68 = this.target_;
        if (!(true === Reflect.deleteProperty(v68, key))) return false;
      } else {
        v75 = this.target_;
        true === Reflect.deleteProperty(v75, key) || $m3$throwTypeError("Cannot delete property '" + $m3$stringifyKey(key) + "'");
      }
      v91 = !!this.appliedAnnotations_;
      v92 = v91;
      v92 && delete this.appliedAnnotations_[key];
      !observableNode || (v102 = this.values_, v102.delete(key), !$m7$isObservableValuePred(observableNode) || (value = observableNode.value_), $m10$propagateChanged(observableNode));
      $m10$reportChanged(this.keysAtom_);
      if (this.pendingKeys_) {
        v122 = this.pendingKeys_;
        var pending = v122.get(key);
        !pending || (v129 = pending.set, v130 = pending, v129.call(v130, true === key in this.target_));
      }
      if (notify || notifySpy) {
        v147 = $m32$admProxyOrTarget(this);
        v151 = this.name_;
        var change$2 = { type: "remove", observableKind: "object", object: v147, debugObjectName: v151, oldValue: value, name: key };
        v159 = notifySpy;
        v160 = v159;
        v160 && $m9$spyReportStart(change$2);
        notify && $m17$notifyListeners(this, change$2);
        v169 = notifySpy;
        v170 = v169;
        v170 && $m9$spyReportEnd(void 0);
      }
    } finally {
      $m10$endBatch();
    }
    return true;
  };
  v542 = $m33$ObservableObjectAdministration.prototype;
  v542.ownKeys_ = function() {
    $m10$reportObserved(this.keysAtom_);
    let v7 = this.target_;
    return Reflect.ownKeys(v7);
  };
  v548 = $m33$ObservableObjectAdministration.prototype;
  v548.keys_ = function() {
    $m10$reportObserved(this.keysAtom_);
    let v7 = this.target_;
    return $m3$objectCtor.keys(v7);
  };
  $m7$isObservableObjectAdmPred = $m3$createInstanceofPredicate("ObservableObjectAdministration", $m33$ObservableObjectAdministration);
  var $m33$asObservableObject = function(target, options) {
    var v6 = !!options;
    v6 && $m7$isObservableObject(target) && $m4$die("Options can't be provided for already observable objects.");
    if ($m3$hasOwn(target, $m2$$mobx)) {
      var v21 = !$m7$isObservableObject(target);
      v21 && $m4$die("Cannot convert '" + target.name_ + "' into observable object:\nThe target is already observable of different type.\nExtending builtins is not supported.");
      return target;
    }
    var v37 = !(true === $m3$objectCtor.isExtensible(target));
    v37 && $m4$die("Cannot make the designated object observable; it is not extensible");
    if (options) {
      var v48 = !!options.name;
      var v49 = v48, v10, v112, v59, v62, v69, v76, v82, v84, v90;
    } else {
      v49 = false;
    }
    v49 ? (v90 = options.name + "", v84 = v90) : ($m3$isPlainObject(target) ? (v59 = "ObservableObject@" + $m8$getNextId(), v82 = v59) : (v62 = target.constructor, v69 = v62.name + "@" + $m8$getNextId(), v82 = v69), v84 = v82);
    v76 = new $m33$ObservableObjectAdministration(target, /* @__PURE__ */ new Map(), v84, $m33$getAnnotationFromOptions(options));
    $m3$defineProperty(target, $m2$$mobx, { enumerable: false, writable: true, configurable: true, value: v76 });
    return target;
  };
  var $m34$objectProxyTraps = {};
  $m34$objectProxyTraps.has = function(target, key) {
    let v5 = target[$m2$$mobx];
    return v5.has_.call(target[$m2$$mobx], key);
  };
  $m34$objectProxyTraps.get = function(target, key) {
    let v5 = target[$m2$$mobx];
    return v5.get_.call(target[$m2$$mobx], key);
  };
  $m34$objectProxyTraps.set = function(target, key, value) {
    if (!$m3$isStringish(key)) return false;
    var v14 = $m33$setObjectValue(target[$m2$$mobx], key, value, true);
    if (v14 == null) return true;
    return !!v14;
  };
  $m34$objectProxyTraps.deleteProperty = function(target, key) {
    if (!$m3$isStringish(key)) return false;
    var v9 = target[$m2$$mobx];
    var v17 = v9.delete_.call(target[$m2$$mobx], key, true);
    if (v17 == null) return true;
    return !!v17;
  };
  $m34$objectProxyTraps.defineProperty = function(target, key, descriptor) {
    var v6 = target[$m2$$mobx];
    var v20 = v6.defineProperty_.call(target[$m2$$mobx], key, descriptor);
    if (v20 == null) return true;
    return !!v20;
  };
  $m34$objectProxyTraps.ownKeys = function(target) {
    let v4 = target[$m2$$mobx];
    return v4.ownKeys_.call(target[$m2$$mobx]);
  };
  $m34$objectProxyTraps.preventExtensions = function(_t) {
    $m4$die(13);
    return false;
  };
  var $m35$arrayExtensions = {};
  var $m35$arrayTraps = {};
  $m35$arrayTraps.get = function(target, name) {
    var v5 = target[$m2$$mobx];
    if (name === $m2$$mobx) return v5;
    if ("length" === name) return v5.getArrayLength_();
    if ("string" == typeof name && !(true === isNaN(name))) return v5.get_(parseInt(name));
    if ($m3$hasOwn($m35$arrayExtensions, name)) return $m35$arrayExtensions[name];
    return target[name];
  };
  $m35$arrayTraps.set = function(target, name, value) {
    var v6 = target[$m2$$mobx];
    "length" === name && v6.setArrayLength_(value);
    "symbol" == typeof name || true === isNaN(name) ? target[name] = value : v6.set_(parseInt(name), value);
    return true;
  };
  $m35$arrayTraps.preventExtensions = function() {
    $m4$die(15);
    return false;
  };
  var $m35$ObservableArrayAdministration = (0, function(v16$2, v24$2, v36) {
    if (arguments.length > 0 && !(v16$2 === void 0)) {
      var v80$2 = v16$2 + "";
      var v74$2 = v80$2, v12, v13$2, v21$2;
    } else {
      v21$2 = "ObservableArray@" + $m8$getNextId();
      v74$2 = v21$2;
    }
    this.owned_ = false;
    arguments.length > 2 && (this.owned_ = !!v36);
    this.atom_ = new $m13$Atom(v74$2);
    this.values_ = [];
    this.interceptors_ = void 0;
    this.changeListeners_ = void 0;
    this.dehancer = void 0;
    this.proxy_ = void 0;
    this.lastKnownLength_ = 0;
    var enhName = "ObservableArray[..]";
    enhName = v74$2 + "[..]";
    this.enhancer_ = function(newV, oldV, _n) {
      return v24$2(newV, oldV, enhName);
    };
    return this;
  });
  v604 = $m35$ObservableArrayAdministration.prototype;
  v604.dehanceValue_ = function(value) {
    return !(this.dehancer === void 0) ? this.dehancer(value) : value;
  };
  v610 = $m35$ObservableArrayAdministration.prototype;
  v610.dehanceValues_ = function(values$2) {
    if (!(this.dehancer === void 0)) {
      var v112 = values$2.length > 0;
      var v12 = v112;
    } else {
      v12 = false;
    }
    return v12 ? values$2.map(this.dehancer) : values$2;
  };
  v616 = $m35$ObservableArrayAdministration.prototype;
  v616.getArrayLength_ = function() {
    $m10$reportObserved(this.atom_);
    let v7 = this.values_;
    return v7.length;
  };
  v622 = $m35$ObservableArrayAdministration.prototype;
  v622.setArrayLength_ = function(newLengthVal) {
    (!("number" == typeof newLengthVal) || true === Number.isNaN(newLengthVal) || (+newLengthVal | 0) < 0) && $m4$die1(40, newLengthVal);
    var v64 = +newLengthVal | 0;
    var v22 = this.values_;
    var v68 = v22.length;
    if (v64 == v68) return;
    if (v64 > v68) {
      var v35 = new Array(v64 - v68 | 0);
      this.spliceWithArray_(v68, 0, v35);
    } else {
      this.spliceWithArray_(v64, v68 - v64 | 0);
    }
  };
  v628 = $m35$ObservableArrayAdministration.prototype;
  v628.spliceWithArray_ = function(v20, v44, v59) {
    var v4 = this.values_;
    var v72 = v4.length;
    if (arguments.length > 0) {
      var v162 = !(v20 === void 0);
      var v17 = v162, v29, v39, v40, v46, v47, v65, v66, v67, v68, v79, v88, v95;
    } else {
      v17 = false;
    }
    v17 ? (v79 = +v20 | 0, v65 = v79) : v65 = 0;
    1 == arguments.length ? (v29 = v72 - v65 | 0, v67 = v29) : (arguments.length > 1 && !(v44 === void 0) && !(v44 == null) ? (v95 = +v44 | 0, v66 = v95) : v66 = 0, v67 = v66);
    v88 = void 0;
    v68 = arguments.length > 2 ? v59 : v88;
    return $m35$spliceWithArrayImpl(this, v65, v67, v68);
  };
  v634 = $m35$ObservableArrayAdministration.prototype;
  v634.get_ = function(index) {
    $m10$reportObserved(this.atom_);
    let v8 = this.dehanceValue_;
    return this.dehanceValue_(this.values_[index]);
  };
  v640 = $m35$ObservableArrayAdministration.prototype;
  v640.set_ = function(indexVal, newValue) {
    var v84 = +indexVal | 0;
    var v7 = this.values_;
    if (v84 < v7.length) {
      $m11$checkIfStateModificationsAreAllowed(this.atom_);
      var v19 = v7[v84];
      if ($m18$hasInterceptors(this)) {
        var v28 = this.proxy_;
        var v34 = $m18$interceptChange(this, { type: "update", object: v28, index: v84, newValue });
        if (!v34) return;
        var v41 = v34.newValue;
        var v82 = v41, v48, v64, v68, v89;
      } else {
        v82 = newValue;
      }
      v48 = this.enhancer_(v82, v19);
      v48 === v19 || (v7[v84] = v48, $m35$callNotifyUpdate(this, v84, v48, v19));
    } else {
      v89 = Array;
      v64 = v84 + 1 | 0;
      v68 = new v89(v64 - v7.length | 0);
      var v73 = v68.length - 1;
      v68[v73] = newValue;
      $m35$spliceWithArrayImpl(this, v7.length, 0, v68);
    }
  };
  var $m35$createObservableArray = function(v4, v7, v16$2, v35) {
    var name = "ObservableArray";
    name = arguments.length > 2 && !(v16$2 === void 0) ? v16$2 + "" : "ObservableArray@" + $m8$getNextId();
    var owned = false;
    arguments.length > 3 && (owned = !!v35);
    return $m23$initObservable(function() {
      var v9 = new $m35$ObservableArrayAdministration(name, v7, owned);
      var v12 = v9.values_;
      $m3$defineProperty(v12, $m2$$mobx, { enumerable: false, writable: false, configurable: true, value: v9 });
      var v18 = v9.values_;
      var v49 = new Proxy(v18, $m35$arrayTraps);
      v9.proxy_ = v49;
      if (v4) {
        var v30 = v4.length > 0;
        var v31 = v30, v34;
      } else {
        v31 = false;
      }
      v31 && (v34 = v9.spliceWithArray_, v9.spliceWithArray_(0, 0, v4));
      return v49;
    });
  };
  $m7$isObservableArrayAdmPred = $m3$createInstanceofPredicate("ObservableArrayAdministration", $m35$ObservableArrayAdministration);
  $m35$arrayExtensions.clear = function() {
    return this.splice(0);
  };
  $m35$arrayExtensions.replace = function(newItems) {
    let v4 = this[$m2$$mobx], v7 = v4.spliceWithArray_, v12 = v4.values_;
    return v4.spliceWithArray_(0, v12.length, newItems);
  };
  $m35$arrayExtensions.toJSON = function() {
    return this.slice();
  };
  $m35$arrayExtensions.splice = function(v20, v51) {
    var v4 = this[$m2$$mobx];
    if (0 == arguments.length) return [];
    if (1 == arguments.length) return v4.spliceWithArray_(v20);
    if (2 == arguments.length) return v4.spliceWithArray_(v20, v51);
    var v72 = $m3$arraySliceFn.call(arguments, 2, arguments.length);
    return v4.spliceWithArray_(v20, v51, v72);
  };
  $m35$arrayExtensions.spliceWithArray = function() {
    let v4 = this[$m2$$mobx];
    return v4.spliceWithArray_.apply(this[$m2$$mobx], arguments);
  };
  $m35$arrayExtensions.push = function() {
    let v4 = this[$m2$$mobx], v8 = v4.values_;
    $m35$spliceWithArrayImpl(v4, v8.length, 0, arguments);
    return v4.values_.length;
  };
  $m35$arrayExtensions.pop = function() {
    var v32 = this[$m2$$mobx];
    var v5 = v32.values_;
    var v8 = v5.length - 1 | 0;
    var v20 = v8 < 0 ? 0 : v8;
    return this.splice(v20, 1)[0];
  };
  $m35$arrayExtensions.shift = function() {
    return this.splice(0, 1)[0];
  };
  $m35$arrayExtensions.unshift = function() {
    let v4 = this[$m2$$mobx];
    $m35$spliceWithArrayImpl(v4, 0, 0, arguments);
    let v12 = v4.values_;
    return v12.length;
  };
  $m35$arrayExtensions.reverse = function() {
    !$m8$globalState.trackingDerivation || $m4$die1(37, "reverse");
    this.replace(this.slice().reverse());
    return this;
  };
  $m35$arrayExtensions.sort = function() {
    !$m8$globalState.trackingDerivation || $m4$die1(37, "sort");
    var v112 = this.slice();
    v112.sort.apply(v112, arguments);
    this.replace(v112);
    return this;
  };
  $m35$arrayExtensions.remove = function(value) {
    var v4 = this[$m2$$mobx];
    var v30 = +v4.dehanceValues_(v4.values_).indexOf(value) | 0;
    if (v30 > a) return this.splice(v30, 1), true;
    return false;
  };
  $m35$addSimpleArrayExt("at");
  $m35$addSimpleArrayExt("concat");
  $m35$addSimpleArrayExt("flat");
  $m35$addSimpleArrayExt("includes");
  $m35$addSimpleArrayExt("indexOf");
  $m35$addSimpleArrayExt("join");
  $m35$addSimpleArrayExt("lastIndexOf");
  $m35$addSimpleArrayExt("slice");
  $m35$addSimpleArrayExt("toString");
  $m35$addSimpleArrayExt("toLocaleString");
  $m35$addSimpleArrayExt("toSorted");
  $m35$addSimpleArrayExt("toSpliced");
  $m35$addSimpleArrayExt("with");
  $m35$addMapLikeArrayExt("every");
  $m35$addMapLikeArrayExt("filter");
  $m35$addMapLikeArrayExt("find");
  $m35$addMapLikeArrayExt("findIndex");
  $m35$addMapLikeArrayExt("findLast");
  $m35$addMapLikeArrayExt("findLastIndex");
  $m35$addMapLikeArrayExt("flatMap");
  $m35$addMapLikeArrayExt("forEach");
  $m35$addMapLikeArrayExt("map");
  $m35$addMapLikeArrayExt("some");
  $m35$addMapLikeArrayExt("toReversed");
  $m35$addReduceLikeArrayExt("reduce");
  $m35$addReduceLikeArrayExt("reduceRight");
  var $m36$ObservableMapMarker = {};
  var $m36$ObservableMap = (0, function(initialData, enhancerArg, nameArg) {
    var self = this;
    self[$m2$$mobx] = $m36$ObservableMapMarker;
    self.enhancer_ = $m27$deepEnhancerFn;
    !enhancerArg || (self.enhancer_ = enhancerArg);
    if (!(nameArg === void 0)) {
      var v46 = nameArg + "";
      var v44 = v46, v24$2;
    } else {
      v24$2 = "ObservableMap@" + $m8$getNextId();
      v44 = v24$2;
    }
    self.name_ = v44;
    self.interceptors_ = void 0;
    self.changeListeners_ = void 0;
    self.dehancer = void 0;
    $m23$initObservable(function() {
      var v10 = self.name_ + ".keys()";
      self.keysAtom_ = $m13$createAtom(v10);
      self.data_ = /* @__PURE__ */ new Map();
      self.hasMap_ = /* @__PURE__ */ new Map();
      !initialData || self.merge(initialData);
    });
    return self;
  });
  v754 = $m36$ObservableMap.prototype;
  v754.has_ = function(key) {
    let v4 = this.data_;
    return !!v4.has(key);
  };
  v760 = $m36$ObservableMap.prototype;
  v760.has = function(key) {
    var self = this;
    if (!$m8$globalState.trackingDerivation) return self.has_(key);
    var v15 = self.hasMap_;
    var v71 = v15.get(key);
    if (!v71) {
      var v33 = self.name_ + "." + $m3$stringifyKey(key) + "?";
      var v34 = $m29$ObservableValue;
      var v44 = new v34(self.has_(key), $m27$referenceEnhancerFn, v33, false);
      var v47 = self.hasMap_;
      v47.set(key, v44);
      v44.onBUOL = /* @__PURE__ */ new Set();
      v44.onBUOL.add(function() {
        let v5 = self.hasMap_;
        v5.delete(key);
      });
      var v67 = v44;
    } else {
      v67 = v71;
    }
    return v67.get();
  };
  v766 = $m36$ObservableMap.prototype;
  v766.set = function(key, value) {
    var v5 = this.data_;
    var v51 = !!v5.has(key);
    if ($m18$hasInterceptors(this)) {
      var v48 = v51 ? "update" : "add", v23, v30, v47;
      v23 = $m18$interceptChange(this, { type: v48, object: this, newValue: value, name: key });
      if (!v23) return this;
      v30 = v23.newValue;
      v47 = v30;
    } else {
      v47 = value;
    }
    v51 ? this.updateValue_(key, v47) : this.addValue_(key, v47);
    return this;
  };
  v772 = $m36$ObservableMap.prototype;
  v772.updateValue_ = function(key, newValue) {
    var v5 = this.data_;
    var v74 = v5.get(key);
    var v75 = v74.prepareNewValue_(newValue);
    if (!(v75 === $m8$globalState.UNCHANGED)) {
      var v22 = $m9$isSpyEnabled();
      var v25 = $m17$hasListeners(this);
      var v26 = null;
      var v30, v36, v44, v49, v72;
      v25 || v22 ? (v36 = this.name_, v44 = v74.value_, v49 = { observableKind: "map", debugObjectName: v36, type: "update", object: this, oldValue: v44, name: key, newValue: v75 }, v72 = v49) : v72 = v26;
      v22 && $m9$spyReportStart(v72);
      v74.setNewValue_(v75);
      v25 && $m17$notifyListeners(this, v72);
      v22 && $m9$spyReportEnd(void 0);
    }
  };
  v778 = $m36$ObservableMap.prototype;
  v778.addValue_ = function(key, newValue) {
    $m11$checkIfStateModificationsAreAllowed(this.keysAtom_);
    $m10$startBatch();
    try {
      var n = "ObservableMap.key";
      n = this.name_ + "." + $m3$stringifyKey(key);
      var observableNode = new $m29$ObservableValue(newValue, this.enhancer_, n, false);
      this.data_.set(key, observableNode);
      newValue = observableNode.value_;
      var v39 = this.hasMap_;
      var hasEntry = v39.get(key);
      !hasEntry || hasEntry.setNewValue_.call(hasEntry, true);
      $m10$reportChanged(this.keysAtom_);
    } finally {
      $m10$endBatch();
    }
    var v57 = $m9$isSpyEnabled();
    var v58 = v57, v60, v61, v65, v71, v802, v84, v94, v97;
    v60 = $m17$hasListeners(this);
    v61 = null;
    v60 || v58 ? (v71 = this.name_, v802 = { observableKind: "map", debugObjectName: v71, type: "add", object: this, name: key, newValue }, v97 = v802) : v97 = v61;
    v84 = v58;
    v84 && $m9$spyReportStart(v97);
    v60 && $m17$notifyListeners(this, v97);
    v94 = v58;
    v94 && $m9$spyReportEnd(void 0);
  };
  v784 = $m36$ObservableMap.prototype;
  v784.delete = function(key) {
    $m11$checkIfStateModificationsAreAllowed(this.keysAtom_);
    if ($m18$hasInterceptors(this) && !$m18$interceptChange(this, { type: "delete", object: this, name: key })) return false;
    var v24 = this.data_;
    if (v24.has(key)) {
      var v29 = $m9$isSpyEnabled();
      var v30 = v29, v32, v33, v37, v43, v51, v55, v58, v62, v72, v85, v96, v107, v111;
      v32 = $m17$hasListeners(this);
      v33 = null;
      v32 || v30 ? (v43 = this.name_, v51 = this.data_, v55 = v51.get(key).value_, v58 = { observableKind: "map", debugObjectName: v43, type: "delete", object: this, oldValue: v55, name: key }, v111 = v58) : v111 = v33;
      v62 = v30;
      v62 && $m9$spyReportStart(v111);
      $m10$startBatch();
      try {
        $m10$reportChanged(this.keysAtom_);
        v72 = this.hasMap_;
        var hasEntry = v72.get(key);
        !hasEntry || hasEntry.setNewValue_.call(hasEntry, false);
        v85 = this.data_;
        var observableNode = v85.get(key);
        observableNode.setNewValue_.call(observableNode, void 0);
        v96 = this.data_;
        v96.delete(key);
      } finally {
        $m10$endBatch();
      }
      v32 && $m17$notifyListeners(this, v111);
      v107 = v30;
      v107 && $m9$spyReportEnd(void 0);
      return true;
    }
    return false;
  };
  v790 = $m36$ObservableMap.prototype;
  v790.get = function(key) {
    if (this.has(key)) {
      var v112 = this.data_;
      var v26 = v112.get(key);
      return $m36$dehanceMap(this, v26.get());
    }
    return $m36$dehanceMap(this, void 0);
  };
  v796 = $m36$ObservableMap.prototype;
  v796.getOrInsert = function(key, value) {
    this.has(key) || this.set(key, value);
    return this.get(key);
  };
  v802 = $m36$ObservableMap.prototype;
  v802.getOrInsertComputed = function(key, compute) {
    this.has(key) || this.set(key, compute(key));
    return this.get(key);
  };
  v808 = $m36$ObservableMap.prototype;
  v808.keys = function() {
    $m10$reportObserved(this.keysAtom_);
    let v7 = this.data_;
    return v7.keys();
  };
  v814 = $m36$ObservableMap.prototype;
  v814.values = function() {
    var self = this;
    let v19 = self.keys(), v6$2 = {};
    v6$2.next = function() {
      var v5 = v19.next();
      if (v5.done) return { done: true, value: void 0 };
      return { done: false, value: self.get(v5.value) };
    };
    v6$2[Symbol.toStringTag] = "MapIterator";
    return $m3$makeIterable(v6$2);
  };
  v820 = $m36$ObservableMap.prototype;
  v820.entries = function() {
    var self = this;
    let v19 = self.keys(), v6$2 = {};
    v6$2.next = function() {
      var v5 = v19.next();
      if (v5.done) return { done: true, value: void 0 };
      var v15 = [];
      var b = v5.value;
      v15.push(b);
      v15.push(self.get(v5.value));
      return { done: false, value: v15 };
    };
    v6$2[Symbol.toStringTag] = "MapIterator";
    return $m3$makeIterable(v6$2);
  };
  v826 = $m36$ObservableMap.prototype;
  v826.forEach = function(callback, thisArg) {
    var v35 = this.entries();
    var v10 = v35.next();
    var v34 = v10, v22, v32;
    while (!v34.done) {
      v22 = v34.value[1];
      callback.call(thisArg, v22, v34.value[0], this);
      v32 = v35.next();
      v34 = v32;
    }
  };
  v832 = $m36$ObservableMap.prototype;
  v832.merge = function(other) {
    var self = this;
    if ($m7$isObservableMap(other)) {
      var v6$2 = new Map(other);
      var v80$2 = v6$2, v13$2, v19, v23, v36, v42, v48, v51;
    } else {
      v80$2 = other;
    }
    $m10$startBatch();
    try {
      if ($m3$isPlainObject(v80$2)) {
        var keys$2 = $m3$getPlainObjectKeys(v80$2);
        var i = 0;
        for (; ; ) {
          v13$2 = i;
          if (v13$2 >= keys$2.length) {
            break;
          }
          v19 = self.set;
          v23 = keys$2[i];
          self.set(v23, v80$2[keys$2[i]]);
          i = i + 1 | 0;
        }
      } else {
        if (Array.isArray(v80$2)) {
          var i$2 = 0;
          for (; ; ) {
            v36 = i$2;
            if (v36 >= v80$2.length) {
              break;
            }
            v42 = self.set;
            v48 = v80$2[i$2][0];
            v51 = v80$2[i$2];
            self.set(v48, v51[1]);
            i$2 = i$2 + 1 | 0;
          }
        } else {
          $m3$isES6Map(v80$2) ? ($m3$objectCtor.getPrototypeOf($m3$objectCtor.getPrototypeOf($m3$objectCtor.getPrototypeOf(v80$2))) == null || $m4$die1(19, v80$2), v80$2.forEach(function(v12, v9) {
            self.set(v9, v12);
          })) : v80$2 == null || $m4$die1(20, v80$2);
        }
      }
    } finally {
      $m10$endBatch();
    }
    return self;
  };
  v838 = $m36$ObservableMap.prototype;
  v838.clear = function() {
    $m10$startBatch();
    try {
      var prev = $m11$untrackedStart();
      try {
        var keys$2 = $m3$arrayFrom(this.keys());
        var i = 0;
        for (; ; ) {
          var v10 = i;
          if (v10 >= keys$2.length) {
            break;
          }
          this.delete(keys$2[i]);
          i = i + 1 | 0;
        }
      } finally {
        $m11$untrackedEnd(prev);
      }
    } finally {
      $m10$endBatch();
    }
  };
  v844 = $m36$ObservableMap.prototype;
  v844.replace = function(values$2) {
    $m10$startBatch();
    try {
      var replacementMap = $m36$convertToMap(values$2);
      var orderedData = /* @__PURE__ */ new Map();
      var keysReportChangedCalled = false;
      var v9 = this.data_;
      var existingKeys = $m3$arrayFrom(v9.keys());
      var i = 0;
      for (; ; ) {
        var v14 = i;
        if (v14 >= existingKeys.length) {
          break;
        }
        var key = existingKeys[i];
        if (!replacementMap.has(key)) if (this.delete(key)) {
          keysReportChangedCalled = true;
        } else {
          var v33 = orderedData;
          var v34 = key;
          v33.set(v34, this.data_.get(key));
        }
        i = i + 1 | 0;
      }
      var entries$2 = $m3$arrayFrom(replacementMap.entries());
      i = 0;
      for (; ; ) {
        var v49 = i;
        if (v49 >= entries$2.length) {
          break;
        }
        var key$2 = entries$2[i][0];
        var value = entries$2[i][1];
        var keyExisted = !!this.data_.has(key$2);
        this.set(key$2, value);
        if (this.data_.has(key$2)) {
          var v802 = orderedData;
          var v81 = key$2;
          v802.set(v81, this.data_.get(key$2));
          keyExisted || (keysReportChangedCalled = true);
        }
        i = i + 1 | 0;
      }
      if (!keysReportChangedCalled) {
        var v982 = this.data_;
        var v180 = +v982.size | 0;
        if (v180 != (+orderedData.size | 0)) {
          $m10$reportChanged(this.keysAtom_);
        } else {
          var v109 = this.data_;
          var iter1 = v109.keys();
          var iter2 = orderedData.keys();
          var next1 = iter1.next();
          var next2 = iter2.next();
          while (!next1.done) {
            var v128 = next1.value;
            if (!(v128 === next2.value)) {
              $m10$reportChanged(this.keysAtom_);
              break;
            }
            next1 = iter1.next();
            next2 = iter2.next();
          }
        }
      }
      this.data_ = orderedData;
    } finally {
      $m10$endBatch();
    }
    return this;
  };
  v850 = $m36$ObservableMap.prototype;
  v850.toJSON = function() {
    return $m3$arrayFrom(this);
  };
  v856 = $m36$ObservableMap.prototype;
  v856.toString = function() {
    return "[object ObservableMap]";
  };
  v862 = $m36$ObservableMap.prototype;
  v1385 = Symbol.iterator;
  v862[v1385] = function() {
    return this.entries();
  };
  v869 = $m36$ObservableMap.prototype;
  $m3$defineGetter(v869, "size", function() {
    $m10$reportObserved(this.keysAtom_);
    let v7 = this.data_;
    return v7.size;
  });
  v876 = $m36$ObservableMap.prototype;
  v1387 = Symbol.toStringTag;
  $m3$defineGetter(v876, v1387, function() {
    return "Map";
  });
  $m7$isObservableMapPred = $m3$createInstanceofPredicate("ObservableMap", $m36$ObservableMap);
  var $m37$ObservableSetMarker = {};
  var $m37$ObservableSet = (0, function(initialData, enhancerArg, nameArg) {
    var self = this;
    self[$m2$$mobx] = $m37$ObservableSetMarker;
    var name = "ObservableSet";
    name = !(nameArg === void 0) ? nameArg + "" : "ObservableSet@" + $m8$getNextId();
    self.name_ = name;
    var enhancer = $m27$deepEnhancerFn;
    !enhancerArg || (enhancer = enhancerArg);
    self.enhancer_ = function(newV, oldV, _n) {
      return enhancer(newV, oldV, name);
    };
    self.data_ = /* @__PURE__ */ new Set();
    self.changeListeners_ = void 0;
    self.interceptors_ = void 0;
    self.dehancer = void 0;
    $m23$initObservable(function() {
      self.atom_ = $m13$createAtom(self.name_);
      !initialData || self.replace(initialData);
    });
    return self;
  });
  v890 = $m37$ObservableSet.prototype;
  v890.has = function(value) {
    $m10$reportObserved(this.atom_);
    return !!this.data_.has($m37$dehanceMap(this, value));
  };
  v896 = $m37$ObservableSet.prototype;
  v896.add = function(value) {
    $m11$checkIfStateModificationsAreAllowed(this.atom_);
    if ($m18$hasInterceptors(this)) {
      var v17 = $m18$interceptChange(this, { type: "add", object: this, newValue: value });
      if (!v17) return this;
      var v24 = v17.newValue;
      var v90 = v24, v36, v52, v53, v55, v56, v60, v66, v73, v77, v872, v91;
    } else {
      v90 = value;
    }
    if (!this.has(v90)) {
      $m10$startBatch();
      try {
        v36 = this.data_;
        v36.add(this.enhancer_(v90, void 0));
        $m10$reportChanged(this.atom_);
      } finally {
        $m10$endBatch();
      }
      v52 = $m9$isSpyEnabled();
      v53 = v52;
      v55 = $m17$hasListeners(this);
      v56 = null;
      v55 || v53 ? (v66 = this.name_, v73 = { observableKind: "set", debugObjectName: v66, type: "add", object: this, newValue: v90 }, v91 = v73) : v91 = v56;
      v77 = v53;
      v77 && $m9$spyReportStart(v91);
      v55 && $m17$notifyListeners(this, v91);
      v872 = v53;
      v872 && $m9$spyReportEnd(void 0);
    }
    return this;
  };
  v902 = $m37$ObservableSet.prototype;
  v902.delete = function(value) {
    if ($m18$hasInterceptors(this) && !$m18$interceptChange(this, { type: "delete", object: this, oldValue: value })) return false;
    if (this.has(value)) {
      var v26 = $m9$isSpyEnabled();
      var v27 = v26, v29, v30, v34, v40, v47, v51, v61, v72, v76;
      v29 = $m17$hasListeners(this);
      v30 = null;
      v29 || v27 ? (v40 = this.name_, v47 = { observableKind: "set", debugObjectName: v40, type: "delete", object: this, oldValue: value }, v76 = v47) : v76 = v30;
      v51 = v27;
      v51 && $m9$spyReportStart(v76);
      $m10$startBatch();
      try {
        $m10$reportChanged(this.atom_);
        v61 = this.data_;
        v61.delete(value);
      } finally {
        $m10$endBatch();
      }
      v29 && $m17$notifyListeners(this, v76);
      v72 = v27;
      v72 && $m9$spyReportEnd(void 0);
      return true;
    }
    return false;
  };
  v908 = $m37$ObservableSet.prototype;
  v908.values = function() {
    var self = this;
    $m10$reportObserved(self.atom_);
    let v7 = self.data_, v9$2 = v7.values(), v10 = {};
    v10.next = function() {
      var v5 = v9$2.next();
      if (v5.done) return { done: true, value: void 0 };
      return { done: false, value: $m37$dehanceMap(self, v5.value) };
    };
    v10[Symbol.toStringTag] = "SetIterator";
    return $m3$makeIterable(v10);
  };
  v914 = $m37$ObservableSet.prototype;
  v914.keys = function() {
    return this.values();
  };
  v920 = $m37$ObservableSet.prototype;
  v920.entries = function() {
    let v18$2 = this.values(), v6$2 = {};
    v6$2.next = function() {
      var v4 = v18$2.next();
      if (v4.done) return { done: true, value: void 0 };
      var v14 = [];
      var v18 = v4.value;
      v14.push(v18);
      v14.push(v4.value);
      return { done: false, value: v14 };
    };
    v6$2[Symbol.toStringTag] = "SetIterator";
    return $m3$makeIterable(v6$2);
  };
  v926 = $m37$ObservableSet.prototype;
  v926.forEach = function(callback, thisArg) {
    var v31 = this.values();
    var v10 = v31.next();
    var v30 = v10, v28;
    while (!v30.done) {
      callback.call(thisArg, v30.value, v30.value, this);
      v28 = v31.next();
      v30 = v28;
    }
  };
  v932 = $m37$ObservableSet.prototype;
  v932.replace = function(other) {
    var self = this;
    if ($m7$isObservableSet(other)) {
      var v6$2 = new Set(other);
      var v52 = v6$2, v16$2;
    } else {
      v52 = other;
    }
    $m10$startBatch();
    try {
      if (Array.isArray(v52)) {
        self.clear();
        var i = 0;
        for (; ; ) {
          v16$2 = i;
          if (v16$2 >= v52.length) {
            break;
          }
          self.add(v52[i]);
          i = i + 1 | 0;
        }
      } else {
        $m3$isES6Set(v52) ? (self.clear(), v52.forEach(function(value) {
          self.add(value);
        })) : v52 == null || $m4$die1(41, v52);
      }
    } finally {
      $m10$endBatch();
    }
    return self;
  };
  v938 = $m37$ObservableSet.prototype;
  v938.clear = function() {
    $m10$startBatch();
    try {
      var prev = $m11$untrackedStart();
      try {
        var v5 = this.data_;
        var vals = $m3$arrayFrom(v5.values());
        var i = 0;
        for (; ; ) {
          var v10 = i;
          if (v10 >= vals.length) {
            break;
          }
          this.delete(vals[i]);
          i = i + 1 | 0;
        }
      } finally {
        $m11$untrackedEnd(prev);
      }
    } finally {
      $m10$endBatch();
    }
  };
  v944 = $m37$ObservableSet.prototype;
  v944.toJSON = function() {
    return $m3$arrayFrom(this);
  };
  v950 = $m37$ObservableSet.prototype;
  v950.toString = function() {
    return "[object ObservableSet]";
  };
  v956 = $m37$ObservableSet.prototype;
  v1389 = Symbol.iterator;
  v956[v1389] = function() {
    return this.values();
  };
  v963 = $m37$ObservableSet.prototype;
  $m3$defineGetter(v963, "size", function() {
    $m10$reportObserved(this.atom_);
    let v7 = this.data_;
    return v7.size;
  });
  v970 = $m37$ObservableSet.prototype;
  v1391 = Symbol.toStringTag;
  $m3$defineGetter(v970, v1391, function() {
    return "Set";
  });
  $m37$addSetAlgebra("intersection");
  $m37$addSetAlgebra("union");
  $m37$addSetAlgebra("difference");
  $m37$addSetAlgebra("symmetricDifference");
  $m37$addSetAlgebra("isSubsetOf");
  $m37$addSetAlgebra("isSupersetOf");
  $m37$addSetAlgebra("isDisjointFrom");
  $m7$isObservableSetPred = $m3$createInstanceofPredicate("ObservableSet", $m37$ObservableSet);
  var $m38$annotationOwnMake = (0, function(adm, key, descriptor) {
    if (this.extend_(adm, key, descriptor, false) == null) return 0;
    return 1;
  });
  var $m38$observableAnnotationExtend = (0, function(v4, v7, v10, v13) {
    var v19 = !(true === "value" in v10);
    if (v19) {
      var v28 = "Cannot apply '" + this.annotationType_ + "' to '";
      $m4$die(v28 + v4.name_ + "." + $m3$stringifyKey(v7) + "':\n'" + this.annotationType_ + "' cannot be used on getter/setter properties");
    }
    var v49 = $m27$deepEnhancerFn;
    if (this.options_) {
      var v57 = this.options_;
      var v60 = !!v57.enhancer_;
      var v61 = v60, v64, v66, v782;
    } else {
      v61 = false;
    }
    v61 ? (v64 = this.options_, v66 = v64.enhancer_, v782 = v66) : v782 = v49;
    return v4.defineObservableProperty_(v7, v10.value, v782, v13);
  });
  var $m39$computedAnnotationExtend = (0, function(v4, v7, v10, v13) {
    var v20 = !v10.get;
    if (v20) {
      var v29 = "Cannot apply '" + this.annotationType_ + "' to '";
      $m4$die(v29 + v4.name_ + "." + $m3$stringifyKey(v7) + "':\n'" + this.annotationType_ + "' can only be used on getter(+setter) properties.");
    }
    var v54 = $m3$assign({}, this.options_);
    v54.get = v10.get;
    v54.set = v10.set;
    return v4.defineComputedProperty_(v7, v54, v13);
  });
  var $m40$actionAnnotationMake = (0, function(v4, v7, v10, v13) {
    if (this.options_) {
      var v21 = this.options_;
      var v24 = !!v21.bound;
      var v25 = v24;
    } else {
      v25 = false;
    }
    if (v25) {
      if (this.extend_(v4, v7, v10, false) == null) return 0;
      return 1;
    }
    if (v13 === v4.target_) {
      if (this.extend_(v4, v7, v10, false) == null) return 0;
      return 2;
    }
    if ($m7$isAction(v10.value)) return 1;
    $m3$defineProperty(v13, v7, $m40$createActionDescriptor(v4, this, v7, v10, false));
    return 2;
  });
  var $m40$actionAnnotationExtend = (0, function(v4, v112, v19, v27) {
    let v7 = v4.defineProperty_;
    return v4.defineProperty_(v112, $m40$createActionDescriptor(v4, this, v112, v19, !!$m8$globalState.safeDescriptors), v27);
  });
  var $m41$decorateFlow20223_ = function(annotation, mthd, context) {
    var v5$2 = [];
    v5$2.push("method");
    $m21$assert20223DecoratorType(context, v5$2);
    var v14$2 = context.name;
    if (!$m7$isFlow(mthd)) {
      var v21$2 = $m28$flow(mthd);
      var v43 = v21$2, v29, v32, v33;
    } else {
      v43 = mthd;
    }
    annotation.options_ ? (v29 = annotation.options_, v32 = !!v29.bound, v33 = v32) : v33 = false;
    v33 && context.addInitializer(function() {
      let v4 = this[v14$2], v15 = v4.bind(this);
      v15.isMobXFlow = true;
      this[v14$2] = v15;
    });
    return v43;
  };
  var $m41$flowAnnotationMake = (0, function(v4, v7, v10, v13) {
    if (v13 === v4.target_) {
      if (this.extend_(v4, v7, v10, false) == null) return 0;
      return 2;
    }
    if (this.options_) {
      var v39 = this.options_;
      var v42 = !!v39.bound;
      var v43 = v42, v58, v59, v60;
    } else {
      v43 = false;
    }
    v43 ? (!$m3$hasOwn(v4.target_, v7) ? v59 = true : (v58 = !$m7$isFlow(v4.target_[v7]), v59 = v58), v60 = v59) : v60 = false;
    if (v60 && this.extend_(v4, v7, v10, false) == null) return 0;
    if ($m7$isFlow(v10.value)) return 1;
    $m3$defineProperty(v13, v7, $m41$createFlowDescriptor(v4, v10, false, false));
    return 2;
  });
  var $m41$flowAnnotationExtend = (0, function(v4, v25, v33, v42) {
    if (this.options_) {
      var v13 = this.options_;
      var v162 = !!v13.bound;
      var v17 = v162, v21, v44;
    } else {
      v17 = false;
    }
    v44 = v17;
    v21 = v4.defineProperty_;
    v25;
    return v4.defineProperty_(v25, $m41$createFlowDescriptor(v4, v33, v44, !!$m8$globalState.safeDescriptors), v42);
  });
  $m28$extendObservable = function(v4, v7, v16$2, v23) {
    var annotations;
    var options;
    arguments.length > 2 && (annotations = v16$2);
    arguments.length > 3 && (options = v23);
    arguments.length > 4 && $m4$die("'extendObservable' expected 2-4 arguments");
    "object" != typeof v4 && $m4$die("'extendObservable' expects an object as first argument");
    $m7$isObservableMap(v4) && $m4$die("'extendObservable' should not be used on maps, use map.merge instead");
    $m3$isPlainObject(v7) || $m4$die("'extendObservable' only accepts plain objects as second argument");
    if ($m25$isObservableValueCheck(v7)) {
      var v51 = true, v50, v80$2;
    } else {
      v50 = $m25$isObservableValueCheck(annotations);
      v51 = v50;
    }
    v51 && $m4$die("Extending an object with another observable (object) is not supported");
    v80$2 = $m3$objectCtor.getOwnPropertyDescriptors(v7);
    $m23$initObservable(function() {
      var v112 = $m33$asObservableObject(v4, options)[$m2$$mobx];
      var v512 = Reflect.ownKeys(v80$2);
      var v45 = 0, v21, v30, v46, v47;
      var v48 = void 0;
      while (v45 < v512.length) {
        v21 = v512[v45];
        annotations ? (true === v21 in annotations ? (v30 = annotations[v21], v46 = v30) : v46 = true, v47 = v46) : v47 = true;
        v112.extend_(v21, v80$2[v21], v47);
        v45 = v45 + 1;
      }
    });
    return v4;
  };
  var $m43$decorateObservable20223_ = function(annotation, desc, context) {
    if ("field" == context.kind + "") {
      var v18$2 = "Please use `@observable accessor " + $m3$stringifyKey(context.name) + "` instead of `@observable ";
      $m4$die(v18$2 + $m3$stringifyKey(context.name) + "`");
    }
    var v27$2 = [];
    v27$2.push("accessor");
    $m21$assert20223DecoratorType(context, v27$2);
    if ("accessor" != context.kind + "") return;
    var v43 = context.name;
    var v44 = {};
    v44.get = function() {
      var v6 = this[$m2$$mobx];
      if (!v6) {
        var v18 = $m43$registerLazyObservable(this, annotation, v43, desc.get.call(this));
        var v25 = v18;
      } else {
        v25 = v6;
      }
      return v25.getObservablePropValue_(v43);
    };
    v44.set = function(value) {
      var v6 = this[$m2$$mobx];
      if (!v6) {
        var v14 = $m43$registerLazyObservable(this, annotation, v43, value);
        var v22 = v14;
      } else {
        v22 = v6;
      }
      return v22.setObservablePropValue_(v43, value);
    };
    v44.init = function(value) {
      $m43$registerLazyObservable(this, annotation, v43, value);
      return value;
    };
    return v44;
  };
  var $m43$observableAnnotation = $m38$createObservableAnnotation("observable", void 0);
  v1020 = $m38$createObservableAnnotation("observable.ref", { enhancer_: $m27$referenceEnhancerFn });
  v1025 = $m38$createObservableAnnotation("observable.shallow", { enhancer_: v352 });
  v1030 = $m38$createObservableAnnotation("observable.struct", { enhancer_: v354 });
  $m28$observable = $m3$assign(function(value, arg2, arg3) {
    return $m43$createObservable(value, arg2, arg3);
  }, $m43$observableAnnotation);
  $m28$observable.box = function(value, options) {
    let v4 = $m27$asCreateObservableOptions(options);
    return new $m29$ObservableValue(value, $m27$getEnhancerFromOptions(v4), v4.name, true, v4.equals);
  };
  $m28$observable.array = function(initial, options) {
    let v4 = $m27$asCreateObservableOptions(options);
    return $m35$createObservableArray(initial, $m27$getEnhancerFromOptions(v4), v4.name);
  };
  $m28$observable.map = function(initial, options) {
    let v4 = $m27$asCreateObservableOptions(options);
    return new $m36$ObservableMap(initial, $m27$getEnhancerFromOptions(v4), v4.name);
  };
  $m28$observable.set = function(initial, options) {
    let v4 = $m27$asCreateObservableOptions(options);
    return new $m37$ObservableSet(initial, $m27$getEnhancerFromOptions(v4), v4.name);
  };
  $m28$observable.object = function(props, annotations, options) {
    return $m23$initObservable(function() {
      var v4 = $m28$extendObservable;
      var v12 = void 0;
      var v6 = {};
      var v15 = $m33$asObservableObject(v6, options);
      var v17 = v15[$m2$$mobx];
      v17.proxy_ || (v17.proxy_ = new Proxy(v15, $m34$objectProxyTraps));
      var v27 = v17.proxy_;
      return v4(v27, props, annotations);
    });
  };
  $m28$observableRef = $m22$createDecoratorAnnotation(v1020, $m43$decorateObservable20223_);
  v1060 = $m22$createDecoratorAnnotation(v1025, $m43$decorateObservable20223_);
  v1063 = $m22$createDecoratorAnnotation($m43$observableAnnotation, $m43$decorateObservable20223_);
  v1066 = $m22$createDecoratorAnnotation(v1030, $m43$decorateObservable20223_);
  var $m44$decorateComputed20223_ = function(annotation, getter, context) {
    var v5$2 = [];
    v5$2.push("getter");
    $m21$assert20223DecoratorType(context, v5$2);
    var v14$2 = context.name;
    var computedValues;
    var v20 = function(target, adm) {
      var v10 = $m3$assign({}, annotation.options_);
      v10.get = getter;
      v10.context = target;
      v10.name || (v10.name = adm.name_ + "." + $m3$stringifyKey(v14$2));
      return new $m16$ComputedValue(v10);
    };
    context.addInitializer(function() {
      var self = this;
      var v9$2 = $m33$asObservableObject(self)[$m2$$mobx];
      var v12 = v9$2.values_;
      var v51 = v12.get(v14$2);
      if ($m7$isComputedValue(v51)) {
        var v23$2 = !(v51.derivation === getter);
        var v24$2 = v23$2, v27$2;
      } else {
        v24$2 = false;
      }
      v24$2 && (v27$2 = v9$2.values_, v27$2.delete(v14$2));
      v9$2.lazyComputedKeys_ || (v9$2.lazyComputedKeys_ = /* @__PURE__ */ new Map());
      v9$2.lazyComputedKeys_.set(v14$2, function() {
        return v20(self, v9$2);
      });
    });
    return function() {
      var v7 = this[$m2$$mobx];
      var v10 = v7.values_;
      var v58 = v10.get(v14$2);
      if ($m7$isComputedValue(v58)) {
        var v21 = !(v58.derivation === getter);
        var v22 = v21, v31, v39, v56;
      } else {
        v22 = false;
      }
      if (v22) return computedValues = computedValues || /* @__PURE__ */ new WeakMap(), v31 = computedValues.get(this), !v31 ? (v39 = v20(this, v7), computedValues.set(this, v39), v56 = v39) : v56 = v31, v56.get();
      return v7.getObservablePropValue_(v14$2);
    };
  };
  var $m44$computedAnnotation = $m39$createComputedAnnotation("computed", void 0);
  v1077 = $m39$createComputedAnnotation("computed.struct", { equals: function(a$2, b) {
    return $m26$eq(a$2, b, a, void 0, void 0);
  } });
  $m28$computed = $m3$assign(function(arg1, arg2) {
    if (arg2 && "string" == typeof arg2.kind) return $m44$decorateComputed20223_($m44$computedAnnotation, arg1, arg2);
    if ($m3$isPlainObject(arg1)) return $m22$createDecoratorAnnotation($m39$createComputedAnnotation("computed", arg1), $m44$decorateComputed20223_);
    var v28 = !("function" == typeof arg1);
    v28 && $m4$die("First argument to `computed` should be an expression.");
    var v70 = "function" == typeof arg2;
    v70 && $m4$die("A setter as second argument is no longer supported, use `{ set: fn }` option instead");
    var v39 = {};
    if ($m3$isPlainObject(arg2)) {
      var v44 = $m3$assign({}, arg2);
      var v60 = v44, v10, v63;
    } else {
      v60 = v39;
    }
    v60.get = arg1;
    v60.name || (v60.name = arg1.name);
    return new $m16$ComputedValue(v60);
  }, $m44$computedAnnotation);
  v1084 = $m22$createDecoratorAnnotation(v1077, $m44$decorateComputed20223_);
  v1086 = (0, function(fn) {
    var v18 = fn.name + "";
    var v15 = "" == v18 ? "<unnamed action>" : v18;
    return $m14$executeAction(v15, false, fn, this, void 0);
  });
  var $m45$decorateAction20223_ = function(annotation, mthd, context) {
    var v5$2 = [];
    v5$2.push("method");
    v5$2.push("field");
    $m21$assert20223DecoratorType(context, v5$2);
    var v17 = context.name;
    var v21$2 = function(m) {
      var v5 = $m3$stringifyKey(v17);
      if (annotation.options_) {
        var v13 = annotation.options_;
        var v162 = !!v13.name;
        var b = v162, v20, v32, v35, v36, v43, v44, v45;
      } else {
        b = false;
      }
      b ? (v20 = annotation.options_, v45 = v20.name + "", v43 = v45) : v43 = v5;
      annotation.options_ ? (v32 = annotation.options_, v35 = !!v32.autoAction, v36 = v35) : v36 = false;
      v44 = v36;
      return $m14$createAction(v43, m, v44, void 0);
    };
    if ("field" == context.kind + "") return function(initMthd) {
      if (!$m7$isAction(initMthd)) {
        var v34 = v21$2(initMthd);
        var v32 = v34, v19, v22, v23, v31, v36;
      } else {
        v32 = initMthd;
      }
      annotation.options_ ? (v19 = annotation.options_, v22 = !!v19.bound, v23 = v22) : v23 = false;
      v23 ? (v36 = v32.bind(this), v36.isMobxAction = true, v31 = v36) : v31 = v32;
      return v31;
    };
    if ("method" == context.kind + "") {
      if (!$m7$isAction(mthd)) {
        var v89$2 = v21$2(mthd);
        var v77$2 = v89$2, v52, v55, v56, v69, v71, v74$2, v85$2;
      } else {
        v77$2 = mthd;
      }
      annotation.options_ ? (v52 = annotation.options_, v55 = !!v52.bound, v56 = v55) : v56 = false;
      v56 && context.addInitializer(function() {
        let v4 = this[v17], v15 = v4.bind(this);
        v15.isMobxAction = true;
        this[v17] = v15;
      });
      return v77$2;
    }
    v69 = annotation.annotationType_;
    v71 = $m3$stringifyKey(v17);
    v74$2 = context.kind;
    v85$2 = [];
    v85$2.push(v69);
    v85$2.push(v71);
    v85$2.push(v74$2);
    $m4$dieRest(43, v85$2);
  };
  var $m45$actionAnnotation = $m40$createActionAnnotation("action", void 0);
  v1096 = $m40$createActionAnnotation("action.bound", { bound: true });
  var $m45$autoActionAnnotation = $m40$createActionAnnotation("autoAction", { autoAction: true });
  v1108 = $m40$createActionAnnotation("autoAction.bound", { autoAction: true, bound: true });
  $m28$action = $m3$assign(function(arg1, arg2) {
    if (arg2 && "string" == typeof arg2.kind) {
      var v12 = $m45$actionAnnotation;
      var v61 = v12, v112, v14, v62, v63, v66, v73;
      return $m45$decorateAction20223_(v61, arg1, arg2);
    }
    if ("function" == typeof arg1) return v73 = arg1.name + "", v62 = "" == v73 ? "<unnamed action>" : v73, $m14$createAction(v62, arg1, false, void 0);
    if ("function" == typeof arg2) return $m14$createAction(arg1 + "", arg2, false, void 0);
    if ($m3$isStringish(arg1)) return v63 = "action", $m22$createDecoratorAnnotation($m40$createActionAnnotation(v63, { name: arg1, autoAction: false }), $m45$decorateAction20223_);
    $m4$die("Invalid arguments for `action`");
  }, $m45$actionAnnotation);
  $m28$autoAction = $m3$assign(function(arg1, arg2) {
    if (arg2 && "string" == typeof arg2.kind) {
      var v12 = $m45$actionAnnotation;
      var v14 = $m45$autoActionAnnotation;
      var v61 = v14, v112, v62, v63, v66, v73;
      return $m45$decorateAction20223_(v61, arg1, arg2);
    }
    if ("function" == typeof arg1) return v73 = arg1.name + "", v62 = "" == v73 ? "<unnamed action>" : v73, $m14$createAction(v62, arg1, true, void 0);
    if ("function" == typeof arg2) return $m14$createAction(arg1 + "", arg2, true, void 0);
    if ($m3$isStringish(arg1)) return v63 = "autoAction", $m22$createDecoratorAnnotation($m40$createActionAnnotation(v63, { name: arg1, autoAction: true }), $m45$decorateAction20223_);
    $m4$die("Invalid arguments for `action`");
  }, $m45$autoActionAnnotation);
  v1119 = $m22$createDecoratorAnnotation(v1096, $m45$decorateAction20223_);
  $m28$autoActionBound = $m22$createDecoratorAnnotation(v1108, $m45$decorateAction20223_);
  var $m47$FlowCancellationError = (0, function() {
    this.message = "FLOW_CANCELLED";
    this.name = "FlowCancellationError";
    return this;
  });
  v1127 = $m47$FlowCancellationError.prototype;
  v1130 = Error.prototype;
  $m3$objectCtor.setPrototypeOf(v1127, v1130);
  $m47$FlowCancellationError.prototype.constructor = $m47$FlowCancellationError;
  $m3$defineProperty($m47$FlowCancellationError, "name", { value: "FlowCancellationError", configurable: true });
  v1147 = $m47$FlowCancellationError.prototype;
  v1147.toString = function() {
    return "Error: " + this.message;
  };
  v1152 = function(error) {
    return $m3$protoIsPrototypeOf($m47$FlowCancellationError, error);
  };
  var $m47$localGeneratorId = 0;
  v1155 = function(genFn, context) {
    if (context && "string" == typeof context.kind) return $m41$decorateFlow20223_($m28$flow, genFn, context);
    var name = genFn.name + "";
    "" == name && (name = "<unnamed flow>");
    var v29 = (0, function() {
      var runId = 0;
      $m47$localGeneratorId = $m47$localGeneratorId + 1 | 0;
      runId = $m47$localGeneratorId;
      var v19 = name + " - runid: " + runId + " - init";
      var v92 = $m28$action(v19, genFn).apply(this, arguments);
      var v28$2 = {};
      v28$2.rejector = void 0;
      v28$2.pending = void 0;
      v28$2.stepId = 0;
      var onFulfilled;
      var onRejected;
      var nextStep;
      onFulfilled = function(v) {
        v28$2.pending = void 0;
        try {
          var stepName = name;
          var stepId = +v28$2.stepId | 0;
          v28$2.stepId = stepId + 1 | 0;
          stepName = name + " - runid: " + runId + " - yield " + stepId;
          var ret = $m28$action(stepName, v92.next).call(v92, v);
          nextStep(ret);
        } catch (v46) {
          v28$2.rejector(v46);
        }
      };
      onRejected = function(err) {
        v28$2.pending = void 0;
        try {
          var stepName = name;
          var stepId = +v28$2.stepId | 0;
          v28$2.stepId = stepId + 1 | 0;
          stepName = name + " - runid: " + runId + " - yield " + stepId;
          var ret = $m28$action(stepName, v92.throw).call(v92, err);
          nextStep(ret);
        } catch (v46) {
          v28$2.rejector(v46);
        }
      };
      nextStep = function(ret) {
        if ("function" == typeof ret.then) {
          var v12 = nextStep;
          ret.then(v12, v28$2.rejector);
          return;
        }
        if (ret.done) {
          v28$2.resolve(ret.value);
          return;
        }
        v28$2.pending = Promise.resolve(ret.value);
        var v41 = v28$2.pending;
        v41.then(onFulfilled, onRejected);
      };
      var v66 = new Promise(function(v8, v13) {
        v28$2.resolve = v8;
        v28$2.rejector = v13;
        onFulfilled(void 0);
      });
      var v76$2 = name + " - runid: " + runId + " - cancel";
      v66.cancel = $m28$action(v76$2, function() {
        try {
          if (v28$2.pending) {
            var v10 = v28$2.pending;
            var v67 = "function" == typeof v10.cancel;
            var v14 = v67, v17, v55;
          } else {
            v14 = false;
          }
          v14 && (v17 = v28$2.pending, v17.cancel.call(v28$2.pending));
          var ret = v92.return(void 0);
          var yielded = Promise.resolve(ret.value);
          yielded.then($m3$noop, $m3$noop);
          "function" == typeof yielded.cancel && yielded.cancel.call(yielded);
          v28$2.rejector(new $m47$FlowCancellationError());
        } catch (v552) {
          v28$2.rejector(v552);
        }
      });
      return v66;
    });
    v29.isMobXFlow = true;
    return v29;
  };
  v1404 = {};
  v1404.annotationType_ = "flow";
  v1404.options_ = void 0;
  v1404.make_ = $m41$flowAnnotationMake;
  v1404.extend_ = $m41$flowAnnotationExtend;
  $m28$flow = $m3$assign(v1155, v1404);
  v1411 = {};
  v1411.annotationType_ = "flow.bound";
  v1411.options_ = { bound: true };
  v1411.make_ = $m41$flowAnnotationMake;
  v1411.extend_ = $m41$flowAnnotationExtend;
  $m28$flowBound = $m22$createDecoratorAnnotation(v1411, $m41$decorateFlow20223_);
  v1168 = function(result) {
    return result;
  };
  v1170 = function(fn) {
    return $m7$isFlow(fn);
  };
  var $m48$autorun = function(view, optsArg) {
    var v3$2 = $m2$EMPTY_OBJECT;
    var v111$2 = optsArg || v3$2, v11$2, v18$2, v40, v51, v52, v56, v71, v73$2, v94$2, v97$2, v98$2, v105, v106, v112, v114, v118$2, v121;
    v11$2 = !("function" == typeof view);
    v11$2 && $m4$die("Autorun expects a function as first argument");
    v18$2 = $m7$isAction(view);
    v18$2 && $m4$die("Autorun does not accept actions since actions are untrackable");
    v111$2.name ? (v118$2 = v111$2.name + "", v114 = v118$2) : (v121 = view.name + "", "" == v121 ? (v40 = "Autorun@" + $m8$getNextId(), v112 = v40) : v112 = v121, v114 = v112);
    !v111$2.scheduler ? (v51 = !v111$2.delay, v52 = v51) : v52 = false;
    var reactionInst;
    v56 = function() {
      view(reactionInst);
    };
    if (v52) {
      reactionInst = new $m15$Reaction(v114, function() {
        this.track(v56);
      }, v111$2.onError, v111$2.requiresObservable);
    } else {
      v71 = $m48$createSchedulerFromOptions(v111$2);
      var isScheduled = false;
      v73$2 = $m15$Reaction;
      reactionInst = new v73$2(v114, function() {
        var rself = this;
        isScheduled || (isScheduled = true, v71(function() {
          isScheduled = false;
          rself.isDisposed || rself.track(v56);
        }));
      }, v111$2.onError, v111$2.requiresObservable);
    }
    v111$2.signal ? (v94$2 = v111$2.signal, v97$2 = !!v94$2.aborted, v98$2 = v97$2) : v98$2 = false;
    v98$2 || reactionInst.schedule_();
    v105 = reactionInst.getDisposer_;
    v106 = reactionInst;
    return v105.call(v106, v111$2.signal);
  };
  v1174 = function(expression, effect, optsArg) {
    var opts = $m2$EMPTY_OBJECT;
    !optsArg || (opts = optsArg);
    if (opts.name) {
      var v116$2 = opts.name + "";
      var v115 = v116$2, v20, v37, v48, v59, v60$2, v63, v72$2, v73$2, v80$2, v83, v95$2, v98$2, v99$2, v103$2, v108, v109$2;
    } else {
      v20 = "Reaction@" + $m8$getNextId();
      v115 = v20;
    }
    var equalsFn = $m6$defaultEquals;
    !opts.equals || (equalsFn = opts.equals);
    v37 = $m14$createAction(v115, $m48$wrapErrorHandler(opts.onError, effect), false, void 0);
    var currentValue;
    var firstTimeFlag = true;
    var changedFlag = false;
    var r;
    v48 = function() {
      var prevAllow = $m11$allowStateChangesStart(false);
      var nextValue;
      try {
        nextValue = expression(r);
      } finally {
        $m8$globalState.allowStateChanges = prevAllow;
      }
      changedFlag = firstTimeFlag || !(true === equalsFn(currentValue, nextValue));
      currentValue = nextValue;
    };
    !opts.scheduler ? (v59 = !opts.delay, v60$2 = v59) : v60$2 = false;
    var isScheduled = false;
    v63 = $m48$createSchedulerFromOptions(opts);
    v72$2 = function() {
      isScheduled = false;
      if (r.isDisposed) return;
      var v14 = currentValue;
      changedFlag = false;
      var v18 = r.track;
      v18.call(r, v48);
      if (firstTimeFlag) {
        var v28 = !!opts.fireImmediately;
        var v29 = v28, v41, v42;
      } else {
        v29 = false;
      }
      v29 ? v37(currentValue, v14, r) : (!firstTimeFlag ? (v41 = !!changedFlag, v42 = v41) : v42 = false, v42 && v37(currentValue, v14, r));
      firstTimeFlag = false;
    };
    v73$2 = $m15$Reaction;
    v80$2 = function() {
      firstTimeFlag || v60$2 ? v72$2() : isScheduled || (isScheduled = true, v63(v72$2));
    };
    v83 = opts.onError;
    r = new v73$2(v115, v80$2, v83, opts.requiresObservable);
    opts.signal ? (v95$2 = opts.signal, v98$2 = !!v95$2.aborted, v99$2 = v98$2) : v99$2 = false;
    v99$2 || (v103$2 = r.schedule_, v103$2.call(r));
    v108 = r.getDisposer_;
    v109$2 = r;
    return v108.call(v109$2, opts.signal);
  };
  v1176 = function(v4, v36, v46) {
    if (1 == arguments.length) {
      var v28 = true, v18, v19, v26, v27, v48, v57;
    } else {
      arguments.length > 1 ? (v18 = !!v36, v19 = v18) : v19 = false;
      v28 = v19 && "object" == typeof v36;
    }
    if (v28) return v57 = void 0, v48 = arguments.length > 1 ? v36 : v57, $m48$whenPromise(v4, v48);
    return $m48$whenEffect(v4, v36, v46);
  };
  v1178 = function(options) {
    var v4 = options.isolateGlobalState;
    true === v4 && $m8$isolateGlobalState();
    if (!(options.enforceActions === void 0)) {
      var v17 = options.enforceActions;
      "always" === v17 ? ($m8$globalState.enforceActions = "always", $m8$globalState.allowStateChanges = false) : "observed" === v17 ? ($m8$globalState.enforceActions = true, $m8$globalState.allowStateChanges = false) : ($m8$globalState.enforceActions = false, $m8$globalState.allowStateChanges = true);
    }
    if (true === "computedRequiresReaction" in options) {
      var v45 = $m8$globalState;
      v45.computedRequiresReaction = !!options.computedRequiresReaction;
    }
    if (true === "reactionRequiresObservable" in options) {
      var v54 = $m8$globalState;
      v54.reactionRequiresObservable = !!options.reactionRequiresObservable;
    }
    if (true === "observableRequiresReaction" in options) {
      var v63 = $m8$globalState;
      v63.observableRequiresReaction = !!options.observableRequiresReaction;
    }
    if (true === "disableErrorBoundaries" in options) {
      var v72$2 = $m8$globalState;
      v72$2.disableErrorBoundaries = !!options.disableErrorBoundaries;
    }
    if (true === "safeDescriptors" in options) {
      var v81 = $m8$globalState;
      v81.safeDescriptors = !!options.safeDescriptors;
    }
    var v87$2 = $m8$globalState;
    v87$2.allowStateReads = !$m8$globalState.observableRequiresReaction;
    if (options.reactionScheduler) {
      var v100$2 = options.reactionScheduler;
      var v116$2 = $m10$reactionScheduler;
      $m10$reactionScheduler = function(f) {
        return v100$2(function() {
          return v116$2(f);
        });
      };
    }
  };
  var $m50$keysSymbol = Symbol("mobx-keys");
  v1184 = function(target, annotations, options) {
    $m23$initObservable(function() {
      var v10 = $m33$asObservableObject(target, options)[$m2$$mobx];
      var v36 = Reflect.ownKeys(annotations);
      var v32 = 0, v21;
      var v33 = void 0;
      while (v32 < v36.length) {
        v21 = v36[v32];
        $m50$makeAnnotate(v10, v21, annotations[v36[v32]]);
        v32 = v32 + 1;
      }
    });
    return target;
  };
  v1186 = function(target, overrides, options) {
    var v41 = $m3$objectCtor;
    var v39 = v41.getPrototypeOf(target);
    !$m3$isPlainObject(target) && !$m3$isPlainObject(v39) && $m4$die("'makeAutoObservable' can only be used for classes that don't have a superclass");
    $m7$isObservableObject(target) && $m4$die("makeAutoObservable can only be used on objects not already made observable");
    if ($m3$isPlainObject(target)) return $m28$extendObservable(target, target, overrides, options);
    $m23$initObservable(function() {
      var v10 = $m33$asObservableObject(target, options)[$m2$$mobx];
      if (!(true === $m50$keysSymbol in target)) {
        var v76$2 = $m3$objectCtor.getPrototypeOf(target);
        var v18$3 = /* @__PURE__ */ new Set();
        var v80$2 = Reflect.ownKeys(target);
        var v83 = Reflect.ownKeys(v76$2);
        var v70 = 0, v31, v44, v69;
        var v71 = void 0;
        while (v70 < v80$2.length) {
          v31 = v80$2[v70];
          v18$3.add(v31);
          v70 = v70 + 1;
        }
        v69 = 0;
        while (v69 < v83.length) {
          v44 = v83[v69];
          v18$3.add(v44);
          v69 = v69 + 1;
        }
        v18$3.delete("constructor");
        v18$3.delete($m2$$mobx);
        $m3$defineProperty(v76$2, $m50$keysSymbol, { enumerable: false, writable: true, configurable: true, value: v18$3 });
      }
      target[$m50$keysSymbol].forEach(function(key) {
        if (overrides && true === key in overrides) {
          var v14 = overrides[key];
          var v20 = v14, v112, v25;
        } else {
          v20 = true;
        }
        $m50$makeAnnotate(v10, key, v20);
      });
    });
    return target;
  };
  var $m51$keys = function(obj) {
    if ($m7$isObservableObject(obj)) {
      var v6 = obj[$m2$$mobx];
      return v6.keys_.call(obj[$m2$$mobx]);
    }
    if ($m7$isObservableMap(obj) || $m7$isObservableSet(obj)) return $m3$arrayFrom(obj.keys());
    if ($m7$isObservableArray(obj)) {
      var v25 = [];
      var v43 = 0, v17, v18;
      while (v43 < obj.length) {
        v25.push(v43);
        v43 = v43 + 1;
      }
      return v25;
    }
    $m4$die(5);
  };
  v1190 = function(obj) {
    if ($m7$isObservableObject(obj)) {
      var v68 = $m51$keys(obj);
      var v8 = [];
      var v65 = 0, v19, v31, v66, v76;
      var v67 = void 0;
      while (v65 < v68.length) {
        v19 = obj[v68[v65]];
        v8.push(v19);
        v65 = v65 + 1;
      }
      return v8;
    }
    if ($m7$isObservableMap(obj)) {
      v76 = $m51$keys(obj);
      v31 = [];
      v66 = 0;
      var v75 = void 0;
      while (v66 < v76.length) {
        v31.push(obj.get(v76[v66]));
        v66 = v66 + 1;
      }
      return v31;
    }
    if ($m7$isObservableSet(obj)) return $m3$arrayFrom(obj.values());
    if ($m7$isObservableArray(obj)) return obj.slice();
    $m4$die(6);
  };
  v1192 = function(obj) {
    if ($m7$isObservableObject(obj) || $m7$isObservableMap(obj)) {
      var v88 = $m51$keys(obj);
      var v12 = [];
      var v85 = 0, v6, v7, v18, v22, v41, v58, v66, v86;
      var v872 = void 0;
      while (v85 < v88.length) {
        v18 = [];
        v22 = v88[v85];
        v18.push(v22);
        $m7$isObservableMap(obj) ? v18.push(obj.get(v88[v85])) : (v41 = obj[v88[v85]], v18.push(v41));
        v12.push(v18);
        v85 = v85 + 1;
      }
      return v12;
    }
    if ($m7$isObservableSet(obj)) return $m3$arrayFrom(obj.entries());
    if ($m7$isObservableArray(obj)) {
      v58 = [];
      v86 = 0;
      while (v86 < obj.length) {
        v66 = [];
        v66.push(v86);
        v66.push(obj[v86]);
        v58.push(v66);
        v86 = v86 + 1;
      }
      return v58;
    }
    $m4$die(7);
  };
  var $m51$set = function(v4, v7, v15) {
    var v104 = void 0;
    var v103 = arguments.length > 2 ? v15 : v104, v23, v24, v29, v33, v38, v54, v1262;
    if (2 == arguments.length && !$m7$isObservableSet(v4)) {
      $m10$startBatch();
      try {
        var ks = $m3$objectCtor.keys(v7);
        var i = 0;
        for (; ; ) {
          v29 = i;
          if (v29 >= ks.length) {
            break;
          }
          v33 = $m51$set;
          v38 = ks[i];
          v33(v4, v38, v7[ks[i]]);
          i = i + 1 | 0;
        }
      } finally {
        $m10$endBatch();
      }
      return;
    }
    $m7$isObservableObject(v4) ? (v54 = v4[$m2$$mobx], v54.set_.call(v4[$m2$$mobx], v7, v103)) : $m7$isObservableMap(v4) ? v4.set(v7, v103) : $m7$isObservableSet(v4) ? v4.add(v7) : $m7$isObservableArray(v4) ? ($m10$startBatch(), v1262 = +v7 | 0, v1262 >= v4.length && (v4.length = (+v7 | 0) + 1 | 0), v4[v7] = v103, $m10$endBatch()) : $m4$die(8);
  };
  v1196 = function(obj, key) {
    if ($m7$isObservableObject(obj)) {
      var v7 = obj[$m2$$mobx];
      v7.delete_.call(obj[$m2$$mobx], key);
    } else {
      $m7$isObservableMap(obj) || $m7$isObservableSet(obj) ? obj.delete(key) : $m7$isObservableArray(obj) ? obj.splice(key, 1) : $m4$die(9);
    }
  };
  var $m51$has = function(obj, key) {
    if ($m7$isObservableObject(obj)) {
      var v7 = obj[$m2$$mobx];
      return v7.has_.call(obj[$m2$$mobx], key);
    }
    if ($m7$isObservableMap(obj) || $m7$isObservableSet(obj)) return obj.has(key);
    if ($m7$isObservableArray(obj)) {
      if ((+key | 0) >= 0) {
        var v50 = +key | 0;
        var v40 = v50 < obj.length;
        var v41 = v40, v19, v20;
      } else {
        v41 = false;
      }
      return v41;
    }
    $m4$die(10);
    return false;
  };
  v1200 = function(obj, key) {
    if (!$m51$has(obj, key)) return;
    if ($m7$isObservableObject(obj)) {
      var v15 = obj[$m2$$mobx];
      return v15.get_.call(obj[$m2$$mobx], key);
    }
    if ($m7$isObservableMap(obj)) return obj.get(key);
    if ($m7$isObservableArray(obj)) return obj[key];
    $m4$die(11);
  };
  var $m51$ownKeysApi = function(obj) {
    if ($m7$isObservableObject(obj)) {
      var v6 = obj[$m2$$mobx];
      return v6.ownKeys_.call(obj[$m2$$mobx]);
    }
    $m4$die(38);
  };
  v1204 = function(obj, key, descriptor) {
    if ($m7$isObservableObject(obj)) {
      var v8 = obj[$m2$$mobx];
      return v8.defineProperty_.call(obj[$m2$$mobx], key, descriptor);
    }
    $m4$die(39);
  };
  var $m52$getAtom = function(thing, property) {
    if ("object" == typeof thing && !(thing == null)) {
      if ($m7$isObservableArray(thing)) {
        property === void 0 || $m4$die(23);
        var v21 = thing[$m2$$mobx];
        return v21.atom_;
      }
      if ($m7$isObservableSet(thing)) return thing.atom_;
      if ($m7$isObservableMap(thing)) {
        if (property === void 0) return thing.keysAtom_;
        var v38 = thing.data_;
        var v151 = v38.get(property);
        if (!v151) {
          var v46 = thing.hasMap_;
          var v1532 = v46.get(property);
          var v141 = v1532, v112, v56, v66, v67, v83, v86, v114, v121, v122, v125, v1262, v142, v143, v154, v159, v160, v161, v162;
        } else {
          v141 = v151;
        }
        v141 || (v56 = thing.name_, v154 = [], v154.push(property), v154.push(v56), $m4$dieRest(25, v154));
        return v141;
      }
      property ? (v66 = !thing[$m2$$mobx], v67 = v66) : v67 = false;
      if (v67 && thing[property] === void 0) {
      }
      if ($m7$isObservableObject(thing)) return property || $m4$die(26), v83 = thing[$m2$$mobx], v86 = v83.values_, v159 = v86.get(property), !v159 ? (v160 = v83.materializeLazyComputed_(property), v143 = v160) : v143 = v159, !v143 ? (v161 = v83.materializeLazyObservable_(property), v142 = v161) : v142 = v143, v142 || (v114 = v83.name_, v162 = [], v162.push(property), v162.push(v114), $m4$dieRest(27, v162)), v142;
      if ($m7$isAtom(thing) || $m7$isComputedValue(thing) || $m7$isReaction(thing)) return thing;
    } else {
      if ("function" == typeof thing && $m7$isReaction(thing[$m2$$mobx])) return thing[$m2$$mobx];
    }
    $m4$die1(28, thing);
  };
  var $m52$getAdministration = function(thing, property) {
    thing || $m4$die(29);
    if (!(property === void 0)) return $m52$getAdministration($m52$getAtom(thing, property));
    if ($m7$isAtom(thing) || $m7$isComputedValue(thing) || $m7$isReaction(thing) || $m7$isObservableMap(thing) || $m7$isObservableSet(thing)) return thing;
    if (thing[$m2$$mobx]) return thing[$m2$$mobx];
    $m4$die1(24, thing);
  };
  v1210 = function(thing, property) {
    if (!(property === void 0)) {
      var v112 = $m52$getAtom(thing, property);
      var v40 = v112, v21, v22, v25, v26, v31, v35, v39;
    } else {
      if ($m7$isAction(thing)) return thing.name;
      else {
        $m7$isObservableObject(thing) || $m7$isObservableMap(thing) || $m7$isObservableSet(thing) ? (v31 = $m52$getAdministration(thing), v39 = v31) : (v35 = $m52$getAtom(thing), v39 = v35);
      }
      v40 = v39;
    }
    return v40.name_;
  };
  v1212 = function(v4, v29, v12, v22) {
    if (arguments.length > 2 && "function" == typeof v12) {
      if (arguments.length > 3) {
        var v23 = !!v22;
        var v51 = v23, v14, v30, v44, v52, v59;
      } else {
        v51 = false;
      }
      v30 = $m52$getAdministration(v4, v29);
      return $m53$observeValue(v30, v12, v51);
    }
    arguments.length > 2 ? (v44 = !!v12, v52 = v44) : v52 = false;
    return $m53$observeObservable(v4, v29, v52);
  };
  v1214 = function(v4, v20, v12) {
    if (arguments.length > 2 && "function" == typeof v12) {
      var v21 = $m52$getAdministration(v4, v20);
      return $m18$registerInterceptor(v21, v12);
    }
    var v29 = $m52$getAdministration(v4);
    return $m18$registerInterceptor(v29, v20);
  };
  v1216 = function() {
    return $m55$interceptHook("onBOL", arguments);
  };
  v1218 = function() {
    return $m55$interceptHook("onBUOL", arguments);
  };
  v1220 = function(v19, v12) {
    var v7 = arguments.length > 1;
    v7 && v12 && $m4$die("toJS no longer supports options");
    return $m56$toJSHelper(v19, /* @__PURE__ */ new Map());
  };
  v1222 = function(thing, property) {
    return $m57$nodeToDependencyTree($m52$getAtom(thing, property));
  };
  v1224 = function(thing, property) {
    return $m57$nodeToObserverTree($m52$getAtom(thing, property));
  };
  v1226 = function(value) {
    return $m7$isComputedValue(value);
  };
  v1228 = function(value, property) {
    var v7 = !$m3$isStringish(property);
    v7 && $m4$die("isComputed expected a property name as second argument");
    if (!$m7$isObservableObject(value)) return false;
    var v17 = value[$m2$$mobx];
    if (v17.lazyComputedKeys_) {
      var v25 = v17.lazyComputedKeys_;
      var v45 = !!v25.has(property);
      var v28 = v45, v32, v39;
    } else {
      v28 = false;
    }
    if (v28) return true;
    v32 = v17.values_;
    if (!v32.has(property)) return false;
    v39 = v17.values_;
    return $m7$isComputedValue(v39.get(property));
  };
  v1230 = function(v4, v45, v49) {
    var target;
    var v72$2 = void 0;
    if ($m7$isObservableMap(v4)) {
      var v12 = true, v11$2, v16$2, v19, v20, v36, v59, v69, v70, v76$2;
    } else {
      v11$2 = $m7$isObservableArray(v4);
      v12 = v11$2;
    }
    v12 || $m7$isObservableValuePred(v4) || $m7$isObservableSet(v4) ? (target = $m52$getAdministration(v4), v70 = v45) : ($m7$isObservableObject(v4) ? (v36 = !$m3$isStringish(v45), v36 && $m4$die("InterceptReads can only be used with a specific property, not with an object in general"), target = $m52$getAdministration(v4, v45), v69 = v49) : ($m4$die("Expected observable map, object or array as first array"), v69 = v72$2), v70 = v69);
    v59 = !(target.dehancer === void 0);
    v59 && $m4$die("An intercept reader was already established");
    target.dehancer = v70;
    return function() {
      target.dehancer = void 0;
    };
  };
  var $mobx = $m2$$mobx;
  var action = $m28$action;
  var actionBound = v1119;
  var _allowStateChanges = $m11$allowStateChanges;
  var _allowStateChangesInsideComputed = v1086;
  var _allowStateReadsEnd = function(prev) {
    $m8$globalState.allowStateReads = prev;
  };
  var _allowStateReadsStart = function(allowStateReads) {
    let v4 = !!$m8$globalState.allowStateReads;
    $m8$globalState.allowStateReads = allowStateReads;
    return v4;
  };
  var _autoAction = $m28$autoAction;
  var _autoActionBound = $m28$autoActionBound;
  var autorun = $m48$autorun;
  var compareDefault = function(a$2, b) {
    return true === $m3$objectIsFn(a$2, b);
  };
  var compareIdentity = function(a$2, b) {
    return a$2 === b;
  };
  var compareShallow = function(a$2, b) {
    return $m26$eq(a$2, b, 1, void 0, void 0);
  };
  var compareStructural = function(a$2, b) {
    return $m26$eq(a$2, b, a, void 0, void 0);
  };
  var computed = $m28$computed;
  var computedStruct = v1084;
  var configure = v1178;
  var createAtom = $m13$createAtom;
  var defineProperty = v1204;
  var extendObservable = $m28$extendObservable;
  var flow = $m28$flow;
  var flowBound = $m28$flowBound;
  var isFlow = v1170;
  var flowResult = v1168;
  var FlowCancellationError = $m47$FlowCancellationError;
  var isFlowCancellationError = v1152;
  var get = v1200;
  var _getAdministration = $m52$getAdministration;
  var getAtom = $m52$getAtom;
  var getDebugName = v1210;
  var getDependencyTree = v1222;
  var has = $m51$has;
  var _getGlobalState = v78;
  var getObserverTree = v1224;
  var intercept = v1214;
  var _interceptReads = v1230;
  var isAction = $m7$isAction;
  var isBoxedObservable = function(x) {
    return !!$m7$isObservableValuePred(x);
  };
  var isComputed = v1226;
  var isComputedProp = v1228;
  var _isComputingDerivation = function() {
    return !($m8$globalState.trackingDerivation == null);
  };
  var isObservable = v325;
  var isObservableArray = $m7$isObservableArray;
  var isObservableMap = $m7$isObservableMap;
  var isObservableSet = $m7$isObservableSet;
  var isObservableObject = $m7$isObservableObject;
  var isObservableProp = v327;
  var keys = $m51$keys;
  var makeAutoObservable = v1186;
  var makeObservable = v1184;
  var ObservableMap = $m36$ObservableMap;
  var ObservableSet = $m37$ObservableSet;
  var observable = $m28$observable;
  var observableDeep = v1063;
  var observableRef = $m28$observableRef;
  var observableShallow = v1060;
  var observableStruct = v1066;
  var observe = v1212;
  var onReactionError = v239;
  var onBecomeObserved = v1216;
  var onBecomeUnobserved = v1218;
  var ownKeys = $m51$ownKeysApi;
  var Reaction = $m15$Reaction;
  var reaction = v1174;
  var remove = v1196;
  var _resetGlobalState = v80;
  var runInAction = v1086;
  var set = $m51$set;
  var spy = v87;
  var toJS = v1220;
  var transaction = v323;
  var untracked = v98;
  var values = v1190;
  var entries = v1192;
  var when = v1176;
  var _startAction = $m14$_startAction;
  var _endAction = $m14$_endAction;
  var override = v438;
  return __toCommonJS(mobx_dev_esm_exports);
})();
typeof module!=="undefined"&&module.exports&&(module.exports=mobx);
