use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::stable_hash::{StableHashMap as AHashMap, StableHashSet as AHashSet};

use crate::ir::{
    AggregateField, AggregateLayout, ArrayOperand, BlockId, ConstValue, ControlFlowFunction,
    ControlFlowInstruction, ControlFlowModule, ControlFlowOp, ControlShape, ExportBinding,
    FunctionId, FunctionKind, FunctionOrigin, Instruction, Intrinsic, IrBinaryOp, IrLocal,
    IrModule, IrUnaryOp, JsHostAlias, JsHostAliasConvention, LocalId, Phi, RecordOperand,
    TemplateOperand, Terminator, ValueId,
};
use crate::profile::OptimizationProfile;
use crate::semantic::{EscapeState, SymbolId, Type};
use crate::span::Span;
use crate::value_analysis::analyze_finite_values;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OptimizationReport {
    pub pass_name: &'static str,
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizationOptions {
    pub constant_folding: bool,
    pub algebraic_simplification: bool,
    pub common_subexpression_elimination: bool,
    pub finite_value_propagation: bool,
    pub global_optimization: bool,
    pub inlining: bool,
    pub inline_closure_factories: bool,
    pub scalar_replacement: bool,
    pub dead_store_elimination: bool,
    pub dead_code_elimination: bool,
    pub constant_parameter_specialization: bool,
    pub specialize_tagged_constants: bool,
    pub call_site_specialization: bool,
    pub capture_signature_cloning: bool,
    pub identical_function_folding: bool,
    pub function_subsumption: bool,
    pub pipeline_fusion: bool,
    pub partial_escape_sinking: bool,
    pub region_outlining: bool,
    pub expression_superopt: bool,
    pub path_sensitive_propagation: bool,
    pub parameterized_function_merging: bool,
    pub inline_instruction_limit: usize,
    pub inline_control_flow_limit: usize,
    pub inline_growth_limit: Option<usize>,
}

impl Default for OptimizationOptions {
    fn default() -> Self {
        Self {
            constant_folding: true,
            algebraic_simplification: true,
            common_subexpression_elimination: true,
            finite_value_propagation: true,
            global_optimization: true,
            inlining: true,
            inline_closure_factories: true,
            scalar_replacement: true,
            dead_store_elimination: true,
            dead_code_elimination: true,
            constant_parameter_specialization: true,
            specialize_tagged_constants: false,
            call_site_specialization: true,
            capture_signature_cloning: true,
            identical_function_folding: true,
            function_subsumption: false,
            pipeline_fusion: false,
            partial_escape_sinking: false,
            region_outlining: false,
            expression_superopt: false,
            path_sensitive_propagation: false,
            parameterized_function_merging: false,
            inline_instruction_limit: 12,
            inline_control_flow_limit: 30,
            inline_growth_limit: None,
        }
    }
}

impl OptimizationOptions {
    pub const fn disabled() -> Self {
        Self {
            constant_folding: false,
            algebraic_simplification: false,
            common_subexpression_elimination: false,
            finite_value_propagation: false,
            global_optimization: false,
            inlining: false,
            inline_closure_factories: false,
            scalar_replacement: false,
            dead_store_elimination: false,
            dead_code_elimination: false,
            constant_parameter_specialization: false,
            specialize_tagged_constants: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            identical_function_folding: false,
            function_subsumption: false,
            pipeline_fusion: false,
            partial_escape_sinking: false,
            region_outlining: false,
            expression_superopt: false,
            path_sensitive_propagation: false,
            parameterized_function_merging: false,
            inline_instruction_limit: 0,
            inline_control_flow_limit: 0,
            inline_growth_limit: Some(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationGuidance {
    pub profile: OptimizationProfile,
    pub specialization_min_count: u64,
    pub max_specializations_per_function: usize,
    pub max_clone_instructions: usize,
}

impl Default for OptimizationGuidance {
    fn default() -> Self {
        Self {
            profile: OptimizationProfile::default(),
            specialization_min_count: 100,
            max_specializations_per_function: 8,
            max_clone_instructions: 64,
        }
    }
}

pub trait OptimizationPass {
    fn name(&self) -> &'static str;
    fn run(&mut self, module: &mut IrModule) -> OptimizationReport;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaError {
    pub span: Span,
    pub message: String,
}

impl std::fmt::Display for SsaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at byte range {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for SsaError {}

pub fn promote_locals_to_ssa(
    module: &mut ControlFlowModule<'_>,
) -> Result<OptimizationReport, SsaError> {
    let mut changed = false;
    for function in &mut module.functions {
        if function.locals_promoted {
            continue;
        }
        // A caught exception observes mutable JavaScript locals at the exact
        // instruction that threw. The CFG deliberately represents exception
        // regions structurally instead of adding an exceptional edge after
        // every potentially-throwing instruction, so locals written inside a
        // protected/catch/finally region must stay as native mutable bindings.
        // Other locals do not depend on those implicit edges and can still be
        // promoted normally; withholding SSA from the whole function made one
        // small `try` disable nearly every scalar optimization in large code.
        let mut unpromoted_locals = exception_region_written_locals(function);
        unpromoted_locals.extend(function.mutable_capture_locals.iter().copied());
        promote_function_locals(function, &unpromoted_locals)?;
        function.locals_promoted = true;
        changed = true;
    }
    Ok(OptimizationReport {
        pass_name: "mem2reg",
        changed,
    })
}

pub fn optimize_control_flow(
    module: &mut ControlFlowModule<'_>,
) -> Result<Vec<OptimizationReport>, SsaError> {
    module.exports.clear();
    optimize_control_flow_inner(
        module,
        &OptimizationOptions::default(),
        &OptimizationGuidance::default(),
        false,
    )
}

pub fn optimize_control_flow_for_module(
    module: &mut ControlFlowModule<'_>,
) -> Result<Vec<OptimizationReport>, SsaError> {
    optimize_control_flow_inner(
        module,
        &OptimizationOptions::default(),
        &OptimizationGuidance::default(),
        true,
    )
}

pub fn optimize_control_flow_with_options(
    module: &mut ControlFlowModule<'_>,
    options: &OptimizationOptions,
    preserve_exports: bool,
) -> Result<Vec<OptimizationReport>, SsaError> {
    optimize_control_flow_with_guidance(
        module,
        options,
        preserve_exports,
        &OptimizationGuidance::default(),
    )
}

pub fn optimize_control_flow_with_guidance(
    module: &mut ControlFlowModule<'_>,
    options: &OptimizationOptions,
    preserve_exports: bool,
    guidance: &OptimizationGuidance,
) -> Result<Vec<OptimizationReport>, SsaError> {
    if !preserve_exports {
        module.exports.clear();
    }
    optimize_control_flow_inner(module, options, guidance, preserve_exports)
}

fn optimize_control_flow_inner(
    module: &mut ControlFlowModule<'_>,
    options: &OptimizationOptions,
    guidance: &OptimizationGuidance,
    preserve_exports: bool,
) -> Result<Vec<OptimizationReport>, SsaError> {
    let mut reports = Vec::new();
    if options.global_optimization {
        reports.push(internalize_entry_globals(module));
    }
    if options.dead_code_elimination {
        reports.push(eliminate_unread_globals(module));
    }
    reports.push(promote_locals_to_ssa(module)?);
    optimize_scalar_fixed_point(module, options, &mut reports);

    if options.global_optimization {
        reports.push(propagate_single_assignment_globals(module));
    }
    reports.push(devirtualize_methods(module));
    reports.push(devirtualize_known_closure_calls(module));
    if options.constant_parameter_specialization {
        reports.push(specialize_constant_parameters(
            module,
            options.specialize_tagged_constants,
            options.finite_value_propagation,
        ));
    }
    if options.call_site_specialization {
        reports.push(specialize_profiled_call_sites(
            module,
            options.specialize_tagged_constants,
            guidance,
        ));
    }
    if options.capture_signature_cloning {
        reports.push(clone_constant_capture_signatures(module, guidance));
    }
    reports.push(devirtualize_known_closure_calls(module));
    reports.push(optimize_unused_parameters(module));
    reports.push(optimize_unused_returns(module));
    reports.push(validate_declared_purity(module)?);
    if options.inlining {
        optimize_inlining_fixed_point(module, options, &AHashSet::default(), &mut reports);
        if options.constant_parameter_specialization {
            reports.push(specialize_constant_parameters(
                module,
                options.specialize_tagged_constants,
                options.finite_value_propagation,
            ));
        }
        if options.capture_signature_cloning {
            reports.push(clone_constant_capture_signatures(module, guidance));
            reports.push(devirtualize_known_closure_calls(module));
        }
        reports.push(optimize_unused_parameters(module));
        reports.push(optimize_unused_returns(module));
    }

    optimize_scalar_fixed_point(module, options, &mut reports);

    if options.function_subsumption {
        reports.push(subsume_private_functions(module));
        optimize_scalar_fixed_point(module, options, &mut reports);
    }

    reports.push(analyze_escapes(module));
    if options.scalar_replacement {
        reports.push(scalar_replace_linear_classes(module));
        reports.push(scalar_replace_control_flow_aggregates(module));
    }
    if options.dead_store_elimination {
        reports.push(eliminate_overwritten_field_stores(module));
    }

    let (compress_reports, outlined_helpers) =
        crate::compress_passes::run_compress_passes_tracking_outlined_helpers(
            module,
            &crate::compress_passes::CompressPassOptions {
                pipeline_fusion: options.pipeline_fusion,
                partial_escape_sinking: options.partial_escape_sinking,
                region_outlining: options.region_outlining,
                expression_superopt: options.expression_superopt,
                path_sensitive_propagation: options.path_sensitive_propagation,
            },
        );
    reports.extend(compress_reports);
    let outlined_helpers = outlined_helpers.into_iter().collect::<AHashSet<_>>();

    optimize_scalar_fixed_point(module, options, &mut reports);
    if options.algebraic_simplification {
        reports.push(collapse_single_use_byte_array_buffers(module));
    }
    if options.dead_code_elimination {
        reports.push(eliminate_dead_control_flow_instructions(module));
    }
    reports.push(optimize_unused_parameters(module));
    reports.push(optimize_unused_returns(module));
    optimize_scalar_fixed_point(module, options, &mut reports);

    // Scalar replacement, path-sensitive propagation, and dead-store removal
    // can turn a previously shared or oversized helper into a small single-use
    // function. Rebuild reachability and revisit the call graph here instead of
    // freezing the decisions made against the pre-compression IR.
    if options.inlining {
        // Outlining deliberately creates a reusable boundary. Preserve only
        // those synthesized callees through this late fixed point while still
        // allowing their leaf callees to be simplified or absorbed.
        optimize_inlining_fixed_point(module, options, &outlined_helpers, &mut reports);
        reports.push(optimize_unused_parameters(module));
        reports.push(optimize_unused_returns(module));
        optimize_scalar_fixed_point(module, options, &mut reports);
    }

    if options.parameterized_function_merging {
        reports.push(merge_permuted_private_functions(module));
        reports.push(merge_single_operand_private_functions(module));
        optimize_scalar_fixed_point(module, options, &mut reports);
    }

    if options.identical_function_folding {
        reports.push(fold_identical_private_functions(module));
    }

    if options.dead_code_elimination {
        reports.push(eliminate_dead_control_flow_instructions(module));
        reports.push(eliminate_dead_functions(module));
        let unread_globals = eliminate_unread_globals(module);
        let unread_changed = unread_globals.changed;
        reports.push(unread_globals);
        if unread_changed {
            reports.push(eliminate_dead_control_flow_instructions(module));
            reports.push(eliminate_dead_functions(module));
        }
        // Inlining can leave an internal module binding referenced only by the
        // entry function.  The first global-internalization pass cannot move
        // such a binding while its (then-live) helper functions still refer to
        // it.  Revisit the decision after final reachability, then run mem2reg
        // again so the newly local value participates in constant/range
        // propagation instead of remaining an opaque JavaScript global.
        // Restrict this late representation change to a single internal
        // binding. Rebuilding several interacting bindings after all region
        // compression has run can trade a compact conditional assignment for
        // parallel phi copies. That multi-binding representation belongs in a
        // future whole-IR candidate search, not as an unconditional rewrite.
        let public_globals = exported_globals(module);
        let internal_binding_count = module
            .globals
            .iter()
            .filter(|global| !global.external)
            .filter(|global| !public_globals.contains(&global.symbol))
            .count();
        if options.global_optimization && internal_binding_count == 1 {
            let internalization = internalize_entry_globals(module);
            let internalized = internalization.changed;
            reports.push(internalization);
            if internalized {
                reports.push(promote_locals_to_ssa(module)?);
                optimize_scalar_fixed_point(module, options, &mut reports);
                reports.push(eliminate_dead_control_flow_instructions(module));
                reports.push(eliminate_dead_functions(module));
                let unread_globals = eliminate_unread_globals(module);
                let unread_changed = unread_globals.changed;
                reports.push(unread_globals);
                if unread_changed {
                    reports.push(eliminate_dead_control_flow_instructions(module));
                    reports.push(eliminate_dead_functions(module));
                }
            }
        }
        // Script/native clear exports before optimize so export-only host
        // bindings look dead. Skip prune there so native codegen still sees
        // JavaScript module edges and can reject them. Closed js-module entries
        // keep `preserve_exports` even with an empty export list.
        if preserve_exports {
            reports.push(prune_unused_foreign_imports(module));
        }
    }
    if std::env::var_os("LILSCRIPT_NO_DIRECT_ARRAY").is_none() {
        reports.push(call_array_methods_directly(module));
    }
    Ok(reports)
}

/// `JS.push`/`JS.flat`-style helpers spell `Array.prototype.method.call(x,…)`
/// because an untyped receiver may be array-like rather than a real array. A
/// receiver built by an array literal in the same function is a genuine
/// `Array`, so the direct `x.method(…)` spelling is observably identical and
/// shorter — unless a string-keyed store could have shadowed the method as an
/// own property, which disqualifies the receiver.
fn call_array_methods_directly(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        let mut fresh_arrays = AHashSet::<ValueId>::default();
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let Some(out) = instruction.out {
                    if matches!(
                        instruction.op,
                        ControlFlowOp::Array(_) | ControlFlowOp::ArraySpread(_)
                    ) {
                        fresh_arrays.insert(out);
                    }
                }
            }
        }
        if fresh_arrays.is_empty() {
            continue;
        }
        let value_types = control_flow_value_types(function);
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let ControlFlowOp::IndexSet { object, index, .. } = &instruction.op {
                    if fresh_arrays.contains(object)
                        && !matches!(value_types.get(index), Some(Type::Int | Type::Float))
                    {
                        fresh_arrays.remove(object);
                    }
                }
            }
        }
        if fresh_arrays.is_empty() {
            continue;
        }
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                let ControlFlowOp::Intrinsic {
                    intrinsic,
                    receiver: Some(receiver),
                    args,
                } = &instruction.op
                else {
                    continue;
                };
                if !fresh_arrays.contains(receiver) {
                    continue;
                }
                let method = match intrinsic {
                    Intrinsic::JsArrayPush => "push",
                    Intrinsic::JsArrayPop => "pop",
                    Intrinsic::JsArraySlice => "slice",
                    Intrinsic::JsArrayIndexOf => "indexOf",
                    Intrinsic::JsArraySort => "sort",
                    Intrinsic::JsArraySplice => "splice",
                    Intrinsic::JsArrayJoin => "join",
                    Intrinsic::JsArrayShift => "shift",
                    Intrinsic::JsArrayUnshift => "unshift",
                    Intrinsic::JsArrayFlat => "flat",
                    _ => continue,
                };
                instruction.op = ControlFlowOp::HostCall {
                    receiver: *receiver,
                    method,
                    args: args.clone(),
                    pure: false,
                };
                changed = true;
            }
        }
    }
    OptimizationReport {
        pass_name: "direct-array-method-calls",
        changed,
    }
}

/// Converge reachability and inlining together.
///
/// Inlining removes a call edge but leaves the old callee body in the module
/// until reachability is recomputed. Counting calls in that now-dead body can
/// make the next layer of the call graph appear shared forever. Keeping dead
/// function elimination inside this fixed point gives every iteration an exact
/// live call graph, matching the way source compressors interleave function
/// reduction with unused-code removal.
fn optimize_inlining_fixed_point(
    module: &mut ControlFlowModule<'_>,
    options: &OptimizationOptions,
    protected_callees: &AHashSet<FunctionId>,
    reports: &mut Vec<OptimizationReport>,
) {
    loop {
        let reachability = eliminate_dead_functions(module);
        let inlining = inline_small_functions(module, options, protected_callees);
        let cfg_inlining =
            inline_single_use_control_flow_function(module, options, protected_callees);
        let changed = reachability.changed || inlining.changed || cfg_inlining.changed;
        reports.push(reachability);
        reports.push(inlining);
        reports.push(cfg_inlining);
        if !changed {
            break;
        }
    }
}

pub fn strip_console_output(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let debug_logs = module
        .functions
        .iter()
        .filter(|function| {
            function.kind == FunctionKind::Extern && function.name == Some("debugLog")
        })
        .map(|function| function.id)
        .collect::<AHashSet<_>>();
    let mut changed = false;
    for function in &mut module.functions {
        for block in &mut function.blocks {
            let before = block.instructions.len();
            block
                .instructions
                .retain(|instruction| match &instruction.op {
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::Print,
                        ..
                    } => false,
                    ControlFlowOp::CallDirect { function, .. } if debug_logs.contains(function) => {
                        false
                    }
                    _ => true,
                });
            changed |= block.instructions.len() != before;
        }
    }
    OptimizationReport {
        pass_name: "strip-console",
        changed,
    }
}

const JS_HOST_INLINE_USE_LIMIT: usize = 4;

const DOM_PREPARE_TEMPLATE: &str =
    "e=>{let t=document.createElement(\"template\");t.innerHTML=e;return t.content.firstChild}";
const DOM_TOGGLE_CLASS: &str =
    "(e,t,n)=>{for(let r of t.trim().split(/\\s+/))r&&e.classList.toggle(r,n)}";
const DOM_RECONCILE_ONE: &str =
    "(e,t,n,r)=>{n!==r&&(n?.parentNode??t.parentNode??e).replaceChild(r,n)}";
const DOM_SET_DELEGATED_CLICK_VOID: &str = "((c,s)=>(e,t)=>{e[s]=t;c[0]||(c[0]=1,document.addEventListener(\"click\",e=>{let t=e.target;e.composed&&t?.shadowRoot&&(t=e.composedPath?.()[0]??t);for(;t&&t!==document;){let n=t[s];n&&!t.disabled&&n();if(e.cancelBubble)return;t=t.parentNode??t.host}}))})([0],Symbol())";
const DOM_RECONCILE: &str = "(e,t,n,r)=>{let l=t.parentNode??e;if(!r.length){if(n.length&&l&&t.parentNode===l&&l.childNodes.length===n.length+1){l.textContent=\"\";l.appendChild(t);return}for(let o of n)o.remove();return}if(n===r){for(let o=r.length-1,s=t;o>=0;--o){let u=r[o];u.nextSibling!==s&&e.insertBefore(u,s);s=u}return}if(n.length===r.length){let o=-1,s=-1,u=0;for(let c=0;c<r.length;c++){if(n[c]===r[c])continue;if(o<0)o=c;else if(s<0)s=c;else{u=1;break}}if(!u&&o<0)return;if(!u&&s>=0&&n[o]===r[s]&&n[s]===r[o]){let c=n[s].nextSibling;e.insertBefore(r[o],n[o].nextSibling);e.insertBefore(r[s],c);return}}else if(r.length===n.length-1){let o=-1,s=0,u=0;for(let c=0;c<n.length;c++){let f=c-s;if(!s&&(f>=r.length||n[c]!==r[f])){o=c;s=1;continue}if(f>=r.length||n[c]!==r[f]){u=1;break}}if(!u&&o>=0&&s==1){n[o].remove();return}}let o=new Set(r),s=[];for(let u of n)o.has(u)?s.push(u):u.remove();if(!s.length){for(let o of r)e.insertBefore(o,t);return}for(let o=r.length-1,s=t;o>=0;--o){let u=r[o];u.nextSibling!==s&&e.insertBefore(u,s);s=u}}";

pub(crate) fn js_host_alias_spec(name: &str) -> Option<(&'static str, JsHostAliasConvention)> {
    Some(match name {
        "mathRound" => ("Math.round", JsHostAliasConvention::Callee),
        "mathCeil" => ("Math.ceil", JsHostAliasConvention::Callee),
        "mathMax" => ("Math.max", JsHostAliasConvention::Callee),
        "mathMin" => ("Math.min", JsHostAliasConvention::Callee),
        "mathCos" => ("Math.cos", JsHostAliasConvention::Callee),
        "mathRandom" => ("Math.random", JsHostAliasConvention::Callee),
        "dateNow" => ("Date.now", JsHostAliasConvention::Callee),
        "parseFloatValue" => ("parseFloat", JsHostAliasConvention::Callee),
        "parseIntRadix" => ("parseInt", JsHostAliasConvention::Callee),
        "isFiniteValue" => ("isFinite", JsHostAliasConvention::Callee),
        "encodeURIComponentValue" => ("encodeURIComponent", JsHostAliasConvention::Callee),
        "encodeURIValue" => ("encodeURI", JsHostAliasConvention::Callee),
        "codePointCount" => ("a=>[...a].length", JsHostAliasConvention::Callee),
        "firstCodePointSize" => (
            "a=>{let b=[...a][0];return b==null?0:b.length}",
            JsHostAliasConvention::Callee,
        ),
        "pickRegex" => ("(a,b,c)=>a?b:c", JsHostAliasConvention::Callee),
        "pickRegex3" => ("(a,b,c,d,e)=>a?b:c?d:e", JsHostAliasConvention::Callee),
        "getPrototypeOf" => ("Object.getPrototypeOf", JsHostAliasConvention::Callee),
        "objectCreate" => ("Object.create", JsHostAliasConvention::Callee),
        "objectKeys" => ("Object.keys", JsHostAliasConvention::Callee),
        "arrayIsArray" => ("Array.isArray", JsHostAliasConvention::Callee),
        "newRegexp" => ("RegExp", JsHostAliasConvention::Callee),
        "stringFromCharCode1" | "stringFromCharCode2" => {
            ("String.fromCharCode", JsHostAliasConvention::Callee)
        }
        "noop" => ("()=>{}", JsHostAliasConvention::Callee),
        // The JavaScript backend targets ES2022. `Object.hasOwn` is both the
        // native target primitive and a detached-call-safe static function, so
        // it avoids the legacy prototype-method `.call`/`.bind` wrapper.
        "objectHasOwn" => ("Object.hasOwn", JsHostAliasConvention::Callee),
        "objectToStringTag" => (
            "Object.prototype.toString",
            JsHostAliasConvention::MethodCall,
        ),
        "functionToString" => (
            "Function.prototype.toString",
            JsHostAliasConvention::MethodCall,
        ),
        "arrayPush" => ("Array.prototype.push", JsHostAliasConvention::MethodCall),
        "arrayPop" => ("Array.prototype.pop", JsHostAliasConvention::MethodCall),
        "arraySlice" => ("Array.prototype.slice", JsHostAliasConvention::MethodCall),
        "arrayIndexOf" => ("Array.prototype.indexOf", JsHostAliasConvention::MethodCall),
        "arraySort" => ("Array.prototype.sort", JsHostAliasConvention::MethodCall),
        "arraySplice" => ("Array.prototype.splice", JsHostAliasConvention::MethodCall),
        "arrayJoin" => ("Array.prototype.join", JsHostAliasConvention::MethodCall),
        "arrayShift" => ("Array.prototype.shift", JsHostAliasConvention::MethodCall),
        "arrayUnshift" => ("Array.prototype.unshift", JsHostAliasConvention::MethodCall),
        "arrayConcatApply" => ("Array.prototype.concat", JsHostAliasConvention::Apply),
        "arrayFlat" => ("Array.prototype.flat", JsHostAliasConvention::MethodCall),
        "scheduleTimeoutMs" => ("setTimeout", JsHostAliasConvention::Callee),
        "clearTimeoutId" => ("clearTimeout", JsHostAliasConvention::Callee),
        "typeOf" => ("a=>typeof a", JsHostAliasConvention::Callee),
        "isFunctionValue" => (
            "a=>\"function\"==typeof a&&\"number\"!=typeof a.nodeType&&\"function\"!=typeof a.item",
            JsHostAliasConvention::Callee,
        ),
        "isWindowValue" => ("a=>a!=null&&a===a.window", JsHostAliasConvention::Callee),
        "scheduleTimeout" => ("a=>setTimeout(a)", JsHostAliasConvention::Callee),
        "defineConfigurable" => (
            "(a,b,c)=>Object.defineProperty(a,b,{value:c,configurable:!0})",
            JsHostAliasConvention::Callee,
        ),
        "consoleWarn3" => ("console.warn", JsHostAliasConvention::Callee),
        "throwValue" => ("", JsHostAliasConvention::Throw),
        "throwError" => ("Error", JsHostAliasConvention::ThrowConstruct),
        "throwTypeError" => ("TypeError", JsHostAliasConvention::ThrowConstruct),
        _ => return None,
    })
}

fn js_imported_dom_host_alias_spec(name: &str) -> Option<(&'static str, JsHostAliasConvention)> {
    Some(match name {
        "domQueryRoot" => ("document.querySelector", JsHostAliasConvention::Callee),
        "domCreateText" => ("document.createTextNode", JsHostAliasConvention::Callee),
        "domCreateComment" => (
            "()=>document.createComment(\"\")",
            JsHostAliasConvention::Callee,
        ),
        "domCreateElement" => ("document.createElement", JsHostAliasConvention::Callee),
        "domCreateFragment" => (
            "()=>document.createDocumentFragment()",
            JsHostAliasConvention::Callee,
        ),
        "domCloneNode" => ("a=>a.cloneNode(!0)", JsHostAliasConvention::Callee),
        "domFirstChild" => ("a=>a.firstChild", JsHostAliasConvention::Callee),
        "domNextSibling" => ("a=>a.nextSibling", JsHostAliasConvention::Callee),
        "domAppendChild" => (
            "Node.prototype.appendChild",
            JsHostAliasConvention::MethodCall,
        ),
        "domRemoveNode" => ("a=>a.remove()", JsHostAliasConvention::Callee),
        "domSetText" => ("(a,b)=>{a.data=b}", JsHostAliasConvention::Callee),
        "domSetAttribute" => (
            "(a,b,c)=>a.setAttribute(b,c)",
            JsHostAliasConvention::Callee,
        ),
        "domSetStringProperty" => ("(a,b,c)=>{a[b]=c}", JsHostAliasConvention::Callee),
        "domClear" => ("a=>a.replaceChildren()", JsHostAliasConvention::Callee),
        "hostSchedule" => ("queueMicrotask", JsHostAliasConvention::Callee),
        "domPrepareTemplate" => (DOM_PREPARE_TEMPLATE, JsHostAliasConvention::Callee),
        "domToggleClass" => (DOM_TOGGLE_CLASS, JsHostAliasConvention::Callee),
        "domReconcileOne" => (DOM_RECONCILE_ONE, JsHostAliasConvention::Callee),
        "domReconcile" => (DOM_RECONCILE, JsHostAliasConvention::Callee),
        "domSetDelegatedClickVoid" => (DOM_SET_DELEGATED_CLICK_VOID, JsHostAliasConvention::Callee),
        _ => return None,
    })
}

fn js_host_alias_spec_for_extern(
    name: &str,
    imported: bool,
) -> Option<(&'static str, JsHostAliasConvention)> {
    js_host_alias_spec(name).or_else(|| {
        if imported {
            js_imported_dom_host_alias_spec(name)
        } else {
            None
        }
    })
}

fn js_intrinsic(
    intrinsic: Intrinsic,
    receiver: Option<ValueId>,
    args: Vec<ValueId>,
) -> HostInline<'static> {
    HostInline::Op(ControlFlowOp::Intrinsic {
        intrinsic,
        receiver,
        args,
    })
}

fn host_receiver_args(args: &[ValueId], provided_args: usize) -> Option<(ValueId, Vec<ValueId>)> {
    let receiver = *args.first()?;
    let rest = provided_args.min(args.len()).saturating_sub(1);
    Some((receiver, args.get(1..1 + rest).unwrap_or(&[]).to_vec()))
}

fn js_host_inline_op(
    name: &str,
    args: &[ValueId],
    provided_args: usize,
    out: Option<ValueId>,
) -> Option<HostInline<'static>> {
    match (name, args) {
        ("createEmptyObject", []) => Some(js_intrinsic(Intrinsic::JsPlainObject, None, Vec::new())),
        ("createArray", []) => Some(HostInline::Op(ControlFlowOp::Array(Vec::new()))),
        ("createNullProtoObject", []) => {
            Some(js_intrinsic(Intrinsic::JsNullProtoObject, None, Vec::new()))
        }
        ("typeOf", [value]) => Some(js_intrinsic(Intrinsic::JsTypeOf, Some(*value), Vec::new())),
        ("stringify", [value]) => Some(js_intrinsic(
            Intrinsic::JsStringify,
            Some(*value),
            Vec::new(),
        )),
        ("isNullish", [value]) => Some(js_intrinsic(
            Intrinsic::JsIsNullish,
            Some(*value),
            Vec::new(),
        )),
        ("isUndefined", [value]) => Some(js_intrinsic(
            Intrinsic::JsIsUndefined,
            Some(*value),
            Vec::new(),
        )),
        ("isFalse", [value]) => Some(js_intrinsic(Intrinsic::JsIsFalse, Some(*value), Vec::new())),
        ("jsAssume", [value]) => out.map(|id| HostInline::Alias(id, *value)),
        ("noop", []) => Some(HostInline::Erase),
        ("mathPI", []) => Some(js_intrinsic(Intrinsic::JsMathPI, None, Vec::new())),
        ("jsUndefined", []) => Some(js_intrinsic(Intrinsic::JsUndefined, None, Vec::new())),
        ("windowSelf", []) => Some(js_intrinsic(Intrinsic::JsWindow, None, Vec::new())),
        ("windowDocument", []) => Some(js_intrinsic(Intrinsic::JsDocument, None, Vec::new())),
        ("scheduleTimeout", [callback]) => {
            Some(js_intrinsic(Intrinsic::JsSetTimeout, None, vec![*callback]))
        }
        ("scheduleTimeoutMs", [callback, delay]) => Some(js_intrinsic(
            Intrinsic::JsSetTimeout,
            None,
            vec![*callback, *delay],
        )),
        ("clearTimeoutId", [id]) => Some(js_intrinsic(Intrinsic::JsClearTimeout, None, vec![*id])),
        ("newDOMParser", []) => Some(js_intrinsic(Intrinsic::JsDomParserNew, None, Vec::new())),
        ("newXMLHttpRequest", []) => Some(js_intrinsic(
            Intrinsic::JsXMLHttpRequestNew,
            None,
            Vec::new(),
        )),
        ("objectConstructor", []) => Some(js_intrinsic(
            Intrinsic::JsObjectConstructor,
            None,
            Vec::new(),
        )),
        ("mathRound", [value]) => Some(js_intrinsic(
            Intrinsic::FloatRound,
            Some(*value),
            Vec::new(),
        )),
        ("mathCeil", [value]) => Some(js_intrinsic(Intrinsic::FloatCeil, Some(*value), Vec::new())),
        ("mathCos", [value]) => Some(js_intrinsic(Intrinsic::FloatCos, Some(*value), Vec::new())),
        ("mathMax", [left, right]) => {
            Some(js_intrinsic(Intrinsic::FloatMax, Some(*left), vec![*right]))
        }
        ("mathMin", [left, right]) => {
            Some(js_intrinsic(Intrinsic::FloatMin, Some(*left), vec![*right]))
        }
        ("dateNow", []) => Some(js_intrinsic(Intrinsic::JsDateNow, None, Vec::new())),
        ("parseFloatValue", [value]) => {
            Some(js_intrinsic(Intrinsic::JsParseFloat, None, vec![*value]))
        }
        ("parseIntRadix", [value, radix]) => Some(js_intrinsic(
            Intrinsic::JsParseInt,
            None,
            vec![*value, *radix],
        )),
        ("isFiniteValue", [value]) => Some(js_intrinsic(Intrinsic::JsIsFinite, None, vec![*value])),
        ("encodeURIComponentValue", [value]) => Some(js_intrinsic(
            Intrinsic::JsEncodeURIComponent,
            None,
            vec![*value],
        )),
        ("getPrototypeOf", [value]) => Some(js_intrinsic(
            Intrinsic::JsGetPrototypeOf,
            None,
            vec![*value],
        )),
        ("objectCreate", [value]) => {
            Some(js_intrinsic(Intrinsic::JsObjectCreate, None, vec![*value]))
        }
        ("objectKeys", [value]) => Some(js_intrinsic(Intrinsic::RecordKeys, None, vec![*value])),
        ("arrayIsArray", [value]) => {
            Some(js_intrinsic(Intrinsic::JsIsArray, Some(*value), Vec::new()))
        }
        ("unaryPlus", [value]) => Some(js_intrinsic(Intrinsic::JsNumber, Some(*value), Vec::new())),
        ("objectBox", [value]) => Some(js_intrinsic(Intrinsic::JsBox, Some(*value), Vec::new())),
        ("getProp", [object, key]) => Some(HostInline::Op(ControlFlowOp::IndexGet {
            object: *object,
            index: *key,
        })),
        ("setProp", [object, key, value]) => Some(HostInline::Op(ControlFlowOp::IndexSet {
            object: *object,
            index: *key,
            value: *value,
        })),
        ("deleteProp", [object, key]) => Some(js_intrinsic(
            Intrinsic::JsDeleteProperty,
            Some(*object),
            vec![*key],
        )),
        ("hasProp", [object, key]) => Some(js_intrinsic(
            Intrinsic::JsHasProperty,
            Some(*object),
            vec![*key],
        )),
        ("setLength", [object, length]) => Some(HostInline::Op(ControlFlowOp::HostFieldSet {
            object: *object,
            property: "length",
            value: *length,
        })),
        ("documentElementOf", [doc]) => Some(HostInline::Op(ControlFlowOp::HostFieldGet {
            object: *doc,
            property: "documentElement",
        })),
        ("getTextContent", [elem]) => Some(HostInline::Op(ControlFlowOp::HostFieldGet {
            object: *elem,
            property: "textContent",
        })),
        ("getNodeValue", [elem]) => Some(HostInline::Op(ControlFlowOp::HostFieldGet {
            object: *elem,
            property: "nodeValue",
        })),
        ("regexTest", [regex, value]) => Some(js_intrinsic(
            Intrinsic::RegexTest,
            Some(*regex),
            vec![*value],
        )),
        ("regexExec", [regex, value]) | ("runRegexExec", [regex, value]) => Some(js_intrinsic(
            Intrinsic::JsRegexExec,
            Some(*regex),
            vec![*value],
        )),
        ("stringTrim", [value]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *value,
            method: "trim",
            args: Vec::new(),
            pure: true,
        })),
        ("stringTrimEnd", [value]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *value,
            method: "trimEnd",
            args: Vec::new(),
            pure: true,
        })),
        ("stringTrimStart", [value]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *value,
            method: "trimStart",
            args: Vec::new(),
            pure: true,
        })),
        ("stringSearch", [value, pattern]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *value,
            method: "search",
            args: vec![*pattern],
            pure: false,
        })),
        ("regexSetLastIndex", [regex, value]) => {
            Some(HostInline::Op(ControlFlowOp::HostFieldSet {
                object: *regex,
                property: "lastIndex",
                value: *value,
            }))
        }
        ("arrayPush", [arr, item]) => Some(js_intrinsic(
            Intrinsic::JsArrayPush,
            Some(*arr),
            vec![*item],
        )),
        ("arrayPop", [arr]) => Some(js_intrinsic(Intrinsic::JsArrayPop, Some(*arr), Vec::new())),
        ("arrayShift", [arr]) => Some(js_intrinsic(
            Intrinsic::JsArrayShift,
            Some(*arr),
            Vec::new(),
        )),
        ("arrayUnshift", [arr, item]) => Some(js_intrinsic(
            Intrinsic::JsArrayUnshift,
            Some(*arr),
            vec![*item],
        )),
        ("arrayFlat", [arr]) => Some(js_intrinsic(Intrinsic::JsArrayFlat, Some(*arr), Vec::new())),
        ("isFunctionValue", [value]) => Some(js_intrinsic(
            Intrinsic::JsIsFunctionValue,
            Some(*value),
            Vec::new(),
        )),
        ("isWindowValue", [value]) => Some(js_intrinsic(
            Intrinsic::JsIsWindowValue,
            Some(*value),
            Vec::new(),
        )),
        ("defineConfigurable", [object, key, value]) => Some(js_intrinsic(
            Intrinsic::JsDefineConfigurable,
            None,
            vec![*object, *key, *value],
        )),
        ("defineIterator", [object, iterator]) => Some(js_intrinsic(
            Intrinsic::JsDefineIterator,
            Some(*object),
            vec![*iterator],
        )),
        ("getArrayIterator", []) => {
            Some(js_intrinsic(Intrinsic::JsArrayIterator, None, Vec::new()))
        }
        ("consoleWarn3", [first, second, third]) => Some(js_intrinsic(
            Intrinsic::JsConsoleWarn,
            None,
            vec![*first, *second, *third],
        )),
        ("requestAnimationFrameOrNull", [callback]) => Some(js_intrinsic(
            Intrinsic::JsRequestAnimationFrameOrNull,
            None,
            vec![*callback],
        )),
        ("arrayJoin", [arr, sep]) => {
            Some(js_intrinsic(Intrinsic::JsArrayJoin, Some(*arr), vec![*sep]))
        }
        ("arrayConcatApply", [target, arrays]) => Some(js_intrinsic(
            Intrinsic::JsArrayConcatApply,
            Some(*target),
            vec![*arrays],
        )),
        ("call0", [func, this_arg]) => Some(js_intrinsic(
            Intrinsic::JsCall,
            Some(*func),
            vec![*this_arg],
        )),
        ("call1", [func, this_arg, a]) => Some(js_intrinsic(
            Intrinsic::JsCall,
            Some(*func),
            vec![*this_arg, *a],
        )),
        ("call2", [func, this_arg, a, b]) => Some(js_intrinsic(
            Intrinsic::JsCall,
            Some(*func),
            vec![*this_arg, *a, *b],
        )),
        ("call3", [func, this_arg, a, b, c]) => Some(js_intrinsic(
            Intrinsic::JsCall,
            Some(*func),
            vec![*this_arg, *a, *b, *c],
        )),
        ("call4", [func, this_arg, a, b, c, d]) => Some(js_intrinsic(
            Intrinsic::JsCall,
            Some(*func),
            vec![*this_arg, *a, *b, *c, *d],
        )),
        ("apply", [func, this_arg, values]) => Some(js_intrinsic(
            Intrinsic::JsApply,
            Some(*func),
            vec![*this_arg, *values],
        )),
        ("getAttribute", [elem, name]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *elem,
            method: "getAttribute",
            args: vec![*name],
            pure: false,
        })),
        ("setAttribute", [elem, name, value]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *elem,
            method: "setAttribute",
            args: vec![*name, *value],
            pure: false,
        })),
        ("removeAttribute", [elem, name]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *elem,
            method: "removeAttribute",
            args: vec![*name],
            pure: false,
        })),
        ("createElement", [doc, tag]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *doc,
            method: "createElement",
            args: vec![*tag],
            pure: false,
        })),
        ("appendChild", [parent, child]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *parent,
            method: "appendChild",
            args: vec![*child],
            pure: false,
        })),
        ("domAppendChild", [parent, child]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *parent,
            method: "appendChild",
            args: vec![*child],
            pure: false,
        })),
        ("domRemoveNode", [node]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *node,
            method: "remove",
            args: Vec::new(),
            pure: false,
        })),
        ("domFirstChild", [node]) => Some(HostInline::Op(ControlFlowOp::HostFieldGet {
            object: *node,
            property: "firstChild",
        })),
        ("domNextSibling", [node]) => Some(HostInline::Op(ControlFlowOp::HostFieldGet {
            object: *node,
            property: "nextSibling",
        })),
        ("domSetText", [node, value]) => Some(HostInline::Op(ControlFlowOp::HostFieldSet {
            object: *node,
            property: "data",
            value: *value,
        })),
        ("domSetStringProperty", [node, name, value]) => {
            Some(HostInline::Op(ControlFlowOp::IndexSet {
                object: *node,
                index: *name,
                value: *value,
            }))
        }
        ("domSetAttribute", [elem, name, value]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *elem,
            method: "setAttribute",
            args: vec![*name, *value],
            pure: false,
        })),
        ("domClear", [node]) => Some(HostInline::Op(ControlFlowOp::HostCall {
            receiver: *node,
            method: "replaceChildren",
            args: Vec::new(),
            pure: false,
        })),
        ("domQueryRoot", [selector]) => Some(HostInline::DocumentMethod {
            method: "querySelector",
            args: vec![*selector],
        }),
        ("domCreateText", [value]) => Some(HostInline::DocumentMethod {
            method: "createTextNode",
            args: vec![*value],
        }),
        ("domCreateElement", [tag]) => Some(HostInline::DocumentMethod {
            method: "createElement",
            args: vec![*tag],
        }),
        ("domCreateComment", []) => Some(HostInline::DocumentCreateComment),
        ("domCreateFragment", []) => Some(HostInline::DocumentMethod {
            method: "createDocumentFragment",
            args: Vec::new(),
        }),
        ("domCloneNode", [node]) => Some(HostInline::CloneNodeDeep(*node)),
        ("newRegexp", args) if !args.is_empty() => {
            let regex_args = if provided_args <= 1 || args.len() == 1 {
                vec![args[0]]
            } else {
                args.to_vec()
            };
            Some(js_intrinsic(Intrinsic::RegexNew, None, regex_args))
        }
        ("arraySlice", args) => {
            let (receiver, rest) = host_receiver_args(args, provided_args)?;
            Some(js_intrinsic(Intrinsic::JsArraySlice, Some(receiver), rest))
        }
        ("arrayIndexOf", args) => {
            let (receiver, rest) = host_receiver_args(args, provided_args)?;
            Some(js_intrinsic(
                Intrinsic::JsArrayIndexOf,
                Some(receiver),
                rest,
            ))
        }
        ("arraySort", args) => {
            let (receiver, rest) = host_receiver_args(args, provided_args)?;
            Some(js_intrinsic(Intrinsic::JsArraySort, Some(receiver), rest))
        }
        ("arraySplice", args) => {
            let (receiver, rest) = host_receiver_args(args, provided_args)?;
            Some(js_intrinsic(Intrinsic::JsArraySplice, Some(receiver), rest))
        }
        ("stringSlice", args) => {
            let (receiver, rest) = host_receiver_args(args, provided_args)?;
            Some(js_intrinsic(Intrinsic::JsStringSlice, Some(receiver), rest))
        }
        ("stringIndexOf", args) => {
            let (receiver, rest) = host_receiver_args(args, provided_args)?;
            Some(js_intrinsic(
                Intrinsic::JsStringIndexOf,
                Some(receiver),
                rest,
            ))
        }
        ("stringReplace", [value, pattern, replacement])
        | ("stringReplaceFirst", [value, pattern, replacement])
        | ("stringReplace2", [value, pattern, replacement]) => Some(js_intrinsic(
            Intrinsic::JsStringReplace,
            Some(*value),
            vec![*pattern, *replacement],
        )),
        ("stringMatch", [value, pattern]) => Some(js_intrinsic(
            Intrinsic::JsStringMatch,
            Some(*value),
            vec![*pattern],
        )),
        ("stringSplit", [value, sep]) => Some(js_intrinsic(
            Intrinsic::JsStringSplit,
            Some(*value),
            vec![*sep],
        )),
        ("addEventListener", args) => {
            let (receiver, rest) = host_receiver_args(args, provided_args)?;
            Some(HostInline::Op(ControlFlowOp::HostCall {
                receiver,
                method: "addEventListener",
                args: rest,
                pure: false,
            }))
        }
        ("removeEventListener", args) => {
            let (receiver, rest) = host_receiver_args(args, provided_args)?;
            Some(HostInline::Op(ControlFlowOp::HostCall {
                receiver,
                method: "removeEventListener",
                args: rest,
                pure: false,
            }))
        }
        _ => None,
    }
}

fn js_host_always_inline(name: &str) -> bool {
    matches!(
        name,
        "createEmptyObject"
            | "createArray"
            | "createNullProtoObject"
            | "stringify"
            | "isNullish"
            | "isUndefined"
            | "isFalse"
            | "jsAssume"
            | "mathPI"
            | "jsUndefined"
            | "windowSelf"
            | "windowDocument"
            | "scheduleTimeout"
            | "scheduleTimeoutMs"
            | "clearTimeoutId"
            | "newDOMParser"
            | "newXMLHttpRequest"
            | "objectConstructor"
            | "unaryPlus"
            | "objectBox"
            | "getProp"
            | "setProp"
            | "deleteProp"
            | "hasProp"
            | "setLength"
            | "documentElementOf"
            | "getTextContent"
            | "getNodeValue"
            | "regexTest"
            | "regexExec"
            | "runRegexExec"
            | "stringTrim"
            | "stringTrimEnd"
            | "stringTrimStart"
            | "stringSearch"
            | "regexSetLastIndex"
            | "stringSlice"
            | "stringIndexOf"
            | "stringReplace"
            | "stringReplaceFirst"
            | "stringReplace2"
            | "stringMatch"
            | "stringSplit"
            | "call0"
            | "call1"
            | "call2"
            | "call3"
            | "call4"
            | "apply"
            | "arrayConcatApply"
            | "arrayFlat"
            | "defineConfigurable"
            | "defineIterator"
            | "getArrayIterator"
            | "consoleWarn3"
            | "requestAnimationFrameOrNull"
            | "getAttribute"
            | "setAttribute"
            | "removeAttribute"
            | "createElement"
            | "appendChild"
            | "addEventListener"
            | "removeEventListener"
    )
}

fn js_imported_dom_always_inline(name: &str) -> bool {
    matches!(
        name,
        "domAppendChild"
            | "domRemoveNode"
            | "domFirstChild"
            | "domNextSibling"
            | "domSetText"
            | "domSetStringProperty"
            | "domSetAttribute"
            | "domClear"
            | "domQueryRoot"
            | "domCreateText"
            | "domCreateElement"
            | "domCreateComment"
            | "domCloneNode"
            | "domCreateFragment"
    )
}

pub(crate) fn js_host_alias_is_simple_callee(spelling: &str) -> bool {
    !spelling.is_empty()
        && spelling.split('.').all(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return false;
            };
            (first.is_ascii_alphabetic() || first == '_' || first == '$')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
        })
}

fn bound_js_host_alias_spec(
    spec: (&'static str, JsHostAliasConvention),
) -> (&'static str, JsHostAliasConvention) {
    match spec.1 {
        JsHostAliasConvention::MethodCall => (spec.0, JsHostAliasConvention::BoundMethodCall),
        JsHostAliasConvention::Apply => (spec.0, JsHostAliasConvention::BoundApply),
        convention => (spec.0, convention),
    }
}

fn js_host_always_alias(name: &str) -> bool {
    matches!(
        name,
        "throwError"
            | "throwTypeError"
            | "mathRandom"
            | "encodeURIValue"
            | "codePointCount"
            | "firstCodePointSize"
            | "pickRegex"
            | "pickRegex3"
    )
}

fn js_imported_dom_always_alias(name: &str) -> bool {
    matches!(
        name,
        "hostSchedule"
            | "domPrepareTemplate"
            | "domToggleClass"
            | "domReconcileOne"
            | "domReconcile"
            | "domSetDelegatedClickVoid"
    )
}

enum HostInline<'src> {
    Op(ControlFlowOp<'src>),
    Alias(ValueId, ValueId),
    Erase,
    DocumentMethod {
        method: &'static str,
        args: Vec<ValueId>,
    },
    DocumentCreateComment,
    CloneNodeDeep(ValueId),
}

pub fn lower_known_js_host_calls(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut by_id = AHashMap::<FunctionId, &str>::default();
    for function in &module.functions {
        if function.kind == FunctionKind::Extern {
            if let Some(name) = function.name {
                by_id.insert(function.id, name);
            }
        }
    }
    if by_id.is_empty() {
        return OptimizationReport {
            pass_name: "known-js-host-literal-lowering",
            changed: false,
        };
    }

    let imported_host_names = module
        .foreign_imports
        .iter()
        .flat_map(|import| import.specifiers.iter().map(|specifier| specifier.local))
        .collect::<AHashSet<_>>();

    let mut call_uses = AHashMap::<FunctionId, usize>::default();
    let mut value_uses = AHashMap::<FunctionId, usize>::default();
    for function in &module.functions {
        if !function.live {
            continue;
        }
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            match &instruction.op {
                ControlFlowOp::CallDirect {
                    function: callee, ..
                } => {
                    *call_uses.entry(*callee).or_insert(0) += 1;
                }
                ControlFlowOp::CallMethod {
                    function: callee, ..
                }
                | ControlFlowOp::Closure {
                    function: callee, ..
                } => {
                    *value_uses.entry(*callee).or_insert(0) += 1;
                }
                ControlFlowOp::NewClass {
                    constructor: Some(callee),
                    ..
                } => {
                    *value_uses.entry(*callee).or_insert(0) += 1;
                }
                _ => {}
            }
        }
    }
    for export in module.exports.iter().chain(
        module
            .lazy_modules
            .iter()
            .flat_map(|lazy| lazy.exports.iter()),
    ) {
        if let ExportBinding::Function(callee) = export.binding {
            *value_uses.entry(callee).or_insert(0) += 1;
        }
    }

    let mut inline = AHashSet::default();
    let mut alias = AHashMap::<FunctionId, (&'static str, JsHostAliasConvention)>::default();
    for (id, name) in &by_id {
        if *name == "noop" {
            inline.insert(*id);
            let values = value_uses.get(id).copied().unwrap_or(0);
            if values > 0 {
                alias.insert(*id, ("()=>{}", JsHostAliasConvention::Callee));
            }
            continue;
        }
        if *name == "throwValue" {
            continue;
        }
        let imported = imported_host_names.contains(name);
        if js_host_always_inline(name) || (imported && js_imported_dom_always_inline(name)) {
            inline.insert(*id);
            let values = value_uses.get(id).copied().unwrap_or(0);
            if values > 0 {
                if let Some(spec) = js_host_alias_spec_for_extern(name, imported) {
                    alias.insert(*id, bound_js_host_alias_spec(spec));
                }
            }
            continue;
        }
        let Some(spec) = js_host_alias_spec_for_extern(name, imported) else {
            continue;
        };
        let calls = call_uses.get(id).copied().unwrap_or(0);
        let values = value_uses.get(id).copied().unwrap_or(0);
        let spec = if values > 0 {
            bound_js_host_alias_spec(spec)
        } else {
            spec
        };
        if js_host_always_alias(name) || (imported && js_imported_dom_always_alias(name)) {
            alias.insert(*id, spec);
            continue;
        }
        if values == 0 && calls <= JS_HOST_INLINE_USE_LIMIT {
            inline.insert(*id);
        } else {
            alias.insert(*id, spec);
        }
    }

    let mut changed = false;
    for function in &mut module.functions {
        let mut aliases = AHashMap::<ValueId, ValueId>::default();
        for block_index in 0..function.blocks.len() {
            let mut erase = Vec::new();
            let mut index = 0;
            while index < function.blocks[block_index].instructions.len() {
                let (callee, args, provided_args, out) = {
                    let instruction = &function.blocks[block_index].instructions[index];
                    let ControlFlowOp::CallDirect {
                        function: callee,
                        args,
                        provided_args,
                    } = &instruction.op
                    else {
                        index += 1;
                        continue;
                    };
                    if !inline.contains(callee) {
                        index += 1;
                        continue;
                    }
                    (*callee, args.clone(), *provided_args, instruction.out)
                };
                let Some(name) = by_id.get(&callee).copied() else {
                    index += 1;
                    continue;
                };
                match js_host_inline_op(name, &args, provided_args, out) {
                    Some(HostInline::Op(op)) => {
                        function.blocks[block_index].instructions[index].op = op;
                        changed = true;
                        index += 1;
                    }
                    Some(HostInline::Alias(out, value)) => {
                        aliases.insert(out, value);
                        changed = true;
                        index += 1;
                    }
                    Some(HostInline::Erase) => {
                        erase.push(index);
                        changed = true;
                        index += 1;
                    }
                    Some(HostInline::DocumentMethod { method, args }) => {
                        rewrite_document_host_call(function, block_index, index, method, args);
                        changed = true;
                        index += 2;
                    }
                    Some(HostInline::DocumentCreateComment) => {
                        rewrite_document_create_comment(function, block_index, index);
                        changed = true;
                        index += 3;
                    }
                    Some(HostInline::CloneNodeDeep(node)) => {
                        rewrite_clone_node_deep(function, block_index, index, node);
                        changed = true;
                        index += 2;
                    }
                    None => {
                        index += 1;
                    }
                }
            }
            for index in erase.into_iter().rev() {
                function.blocks[block_index].instructions.remove(index);
            }
            if !aliases.is_empty() {
                function.blocks[block_index]
                    .instructions
                    .retain(|instruction| {
                        !matches!(
                            (&instruction.op, instruction.out),
                            (
                                ControlFlowOp::CallDirect { function: callee, .. },
                                Some(out)
                            ) if aliases.contains_key(&out) && inline.contains(callee)
                        )
                    });
            }
        }
        if !aliases.is_empty() {
            rewrite_control_flow_function(function, &aliases);
            changed = true;
        }
    }

    changed |= rewrite_js_host_throws(module, &by_id);

    let mut strip_names = alias
        .keys()
        .filter_map(|id| by_id.get(id).copied())
        .collect::<AHashSet<_>>();
    for (id, name) in &by_id {
        if alias.contains_key(id) {
            continue;
        }
        let values = value_uses.get(id).copied().unwrap_or(0);
        if values > 0 {
            continue;
        }
        if inline.contains(id)
            || js_host_alias_spec_for_extern(name, imported_host_names.contains(name)).is_some()
        {
            strip_names.insert(*name);
        }
    }
    if !strip_names.is_empty() {
        module.foreign_imports.retain_mut(|import| {
            if import.specifiers.is_empty() {
                return true;
            }
            let before = import.specifiers.len();
            import
                .specifiers
                .retain(|specifier| !strip_names.contains(specifier.local));
            changed |= import.specifiers.len() != before;
            !import.specifiers.is_empty()
        });
    }
    if !alias.is_empty() {
        module.js_host_aliases = alias
            .into_iter()
            .map(|(function, (spelling, convention))| JsHostAlias {
                function,
                spelling,
                convention,
            })
            .collect();
        module
            .js_host_aliases
            .sort_unstable_by_key(|alias| alias.function.0);
        changed = true;
    }

    OptimizationReport {
        pass_name: "known-js-host-literal-lowering",
        changed,
    }
}

fn allocate_ssa_value(function: &mut ControlFlowFunction<'_>) -> ValueId {
    let value = ValueId(function.value_count);
    function.value_count += 1;
    function.value_escapes.push(EscapeState::LocalOnly);
    function.value_local_hints.push(None);
    value
}

fn rewrite_document_host_call(
    function: &mut ControlFlowFunction<'_>,
    block_index: usize,
    index: usize,
    method: &'static str,
    args: Vec<ValueId>,
) {
    let span = function.blocks[block_index].instructions[index].span;
    let document = allocate_ssa_value(function);
    function.blocks[block_index].instructions.insert(
        index,
        ControlFlowInstruction {
            out: Some(document),
            ty: Some(Type::TypeParameter("$js")),
            op: ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::JsDocument,
                receiver: None,
                args: Vec::new(),
            },
            span,
        },
    );
    function.blocks[block_index].instructions[index + 1].op = ControlFlowOp::HostCall {
        receiver: document,
        method,
        args,
        pure: false,
    };
}

fn rewrite_document_create_comment(
    function: &mut ControlFlowFunction<'_>,
    block_index: usize,
    index: usize,
) {
    let span = function.blocks[block_index].instructions[index].span;
    let document = allocate_ssa_value(function);
    let empty = allocate_ssa_value(function);
    function.blocks[block_index].instructions.insert(
        index,
        ControlFlowInstruction {
            out: Some(document),
            ty: Some(Type::TypeParameter("$js")),
            op: ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::JsDocument,
                receiver: None,
                args: Vec::new(),
            },
            span,
        },
    );
    function.blocks[block_index].instructions.insert(
        index + 1,
        ControlFlowInstruction {
            out: Some(empty),
            ty: Some(Type::String),
            op: ControlFlowOp::Const(ConstValue::String(String::new())),
            span,
        },
    );
    function.blocks[block_index].instructions[index + 2].op = ControlFlowOp::HostCall {
        receiver: document,
        method: "createComment",
        args: vec![empty],
        pure: false,
    };
}

fn rewrite_clone_node_deep(
    function: &mut ControlFlowFunction<'_>,
    block_index: usize,
    index: usize,
    node: ValueId,
) {
    let span = function.blocks[block_index].instructions[index].span;
    let deep = allocate_ssa_value(function);
    function.blocks[block_index].instructions.insert(
        index,
        ControlFlowInstruction {
            out: Some(deep),
            ty: Some(Type::Bool),
            op: ControlFlowOp::Const(ConstValue::Bool(true)),
            span,
        },
    );
    function.blocks[block_index].instructions[index + 1].op = ControlFlowOp::HostCall {
        receiver: node,
        method: "cloneNode",
        args: vec![deep],
        pure: false,
    };
}

fn rewrite_js_host_throws(
    module: &mut ControlFlowModule<'_>,
    by_id: &AHashMap<FunctionId, &str>,
) -> bool {
    let mut changed = false;
    for function in &mut module.functions {
        for block in &mut function.blocks {
            let Some(index) = block.instructions.iter().position(|instruction| {
                matches!(
                    &instruction.op,
                    ControlFlowOp::CallDirect { function: callee, .. }
                        if matches!(
                            by_id.get(callee).copied(),
                            Some("throwValue" | "throwError" | "throwTypeError")
                        )
                )
            }) else {
                continue;
            };
            let ControlFlowOp::CallDirect {
                function: callee,
                args,
                ..
            } = &block.instructions[index].op
            else {
                continue;
            };
            match by_id.get(callee).copied() {
                Some("throwValue") => {
                    if let Some(value) = args.first().copied() {
                        block.instructions.truncate(index);
                        block.terminator = Some(Terminator::Throw(value));
                        changed = true;
                    }
                }
                Some("throwError" | "throwTypeError") => {
                    block.instructions.truncate(index + 1);
                    block.terminator = Some(Terminator::Unreachable);
                    changed = true;
                }
                _ => {}
            }
        }
    }
    changed
}

fn prune_unused_foreign_imports(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut live_symbols = AHashSet::default();
    let mut live_names = AHashSet::default();
    for function in module.functions.iter().filter(|function| function.live) {
        if function.kind == FunctionKind::Extern {
            if let Some(name) = function.name {
                live_names.insert(name);
            }
        }
        for block in &function.blocks {
            for instruction in &block.instructions {
                match instruction.op {
                    ControlFlowOp::LoadGlobal(symbol)
                    | ControlFlowOp::StoreGlobal { global: symbol, .. } => {
                        live_symbols.insert(symbol);
                    }
                    _ => {}
                }
            }
        }
    }
    live_symbols.extend(
        module
            .exports
            .iter()
            .filter_map(|export| match export.binding {
                ExportBinding::Global(symbol) => Some(symbol),
                _ => None,
            }),
    );
    for global in &module.globals {
        if live_symbols.contains(&global.symbol) {
            live_names.insert(global.name);
        }
    }

    let mut changed = false;
    module.foreign_imports.retain_mut(|import| {
        if import.specifiers.is_empty() {
            return true;
        }
        let before = import.specifiers.len();
        import
            .specifiers
            .retain(|specifier| live_names.contains(specifier.local));
        if import.specifiers.len() != before {
            changed = true;
        }
        !import.specifiers.is_empty()
    });

    let before_globals = module.globals.len();
    module
        .globals
        .retain(|global| !global.external || live_symbols.contains(&global.symbol));
    if module.globals.len() != before_globals {
        changed = true;
    }

    OptimizationReport {
        pass_name: "unused-foreign-import-elimination",
        changed,
    }
}

/// `new Int8Array(new ArrayBuffer(n))` and the other byte-wide typed-array
/// forms allocate an indistinguishable backing store when the intermediate
/// buffer has no other observer. Reusing `n` avoids a redundant constructor in
/// JavaScript and lets the following SSA DCE remove the buffer allocation.
fn collapse_single_use_byte_array_buffers(
    module: &mut ControlFlowModule<'_>,
) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        let uses = control_flow_use_counts(function);
        let buffers = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (
                    Some(out),
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::ArrayBufferNew,
                        receiver: None,
                        args,
                    },
                ) if args.len() == 1 && uses.get(&out).copied() == Some(1) => Some((out, args[0])),
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        if buffers.is_empty() {
            continue;
        }
        for instruction in function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
        {
            let ControlFlowOp::Intrinsic {
                intrinsic,
                receiver: None,
                args,
            } = &mut instruction.op
            else {
                continue;
            };
            let byte_wide_constructor = crate::typed_array::classify_typed_array_intrinsic(
                *intrinsic,
            )
            .is_some_and(|(kind, operation)| {
                operation == crate::typed_array::TypedArrayIntrinsic::New
                    && kind.bytes_per_element() == 1
            });
            if !byte_wide_constructor || args.len() != 1 {
                continue;
            }
            if let Some(length) = buffers.get(&args[0]).copied() {
                args[0] = length;
                changed = true;
            }
        }
    }
    OptimizationReport {
        pass_name: "byte-array-buffer-collapsing",
        changed,
    }
}

fn optimize_scalar_fixed_point(
    module: &mut ControlFlowModule<'_>,
    options: &OptimizationOptions,
    reports: &mut Vec<OptimizationReport>,
) {
    loop {
        let propagation = options
            .constant_folding
            .then(|| fold_and_propagate_control_flow(module, options.finite_value_propagation));
        let phis = eliminate_redundant_phis(module);
        let algebraic = options
            .algebraic_simplification
            .then(|| simplify_algebraic_expressions(module));
        let value_numbering = options
            .common_subexpression_elimination
            .then(|| eliminate_common_subexpressions(module));
        let unreachable = remove_unreachable_control_flow(module);
        // Keep allocation and mutation distinct in the neutral IR. Unlike
        // literal initialization, `Array#push` and ordinary-object assignment
        // use [[Set]] and can invoke inherited setters in open-world JavaScript.
        let stringify = elide_single_use_stringify(module);
        let changed = propagation.as_ref().is_some_and(|report| report.changed)
            || phis.changed
            || algebraic.as_ref().is_some_and(|report| report.changed)
            || value_numbering
                .as_ref()
                .is_some_and(|report| report.changed)
            || unreachable.changed
            || stringify.changed;
        reports.extend(propagation);
        reports.push(phis);
        reports.extend(algebraic);
        reports.extend(value_numbering);
        reports.push(unreachable);
        reports.push(stringify);
        if !changed {
            break;
        }
    }
}

fn eliminate_overwritten_field_stores(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        if has_exception_region(function) {
            continue;
        }
        for block in &mut function.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut retained = Vec::with_capacity(instructions.len());
            let mut needed = AHashSet::<(ValueId, usize)>::default();
            let mut later_stores = AHashSet::<(ValueId, usize)>::default();
            for instruction in instructions.into_iter().rev() {
                match instruction.op {
                    ControlFlowOp::FieldGet { object, index, .. } => {
                        needed.insert((object, index));
                        retained.push(instruction);
                    }
                    ControlFlowOp::FieldSet { object, index, .. } => {
                        let field = (object, index);
                        if later_stores.contains(&field) && !needed.contains(&field) {
                            changed = true;
                        } else {
                            retained.push(instruction);
                        }
                        needed.remove(&field);
                        later_stores.insert(field);
                    }
                    _ if field_store_barrier(&instruction.op) => {
                        needed.clear();
                        later_stores.clear();
                        retained.push(instruction);
                    }
                    _ => retained.push(instruction),
                }
            }
            retained.reverse();
            block.instructions = retained;
        }
    }
    OptimizationReport {
        pass_name: "dead-field-store-elimination",
        changed,
    }
}

fn fold_known_js_type_check(known_typeof: Option<&str>, target: &Type<'_>) -> Option<bool> {
    let name = known_typeof?;
    match target {
        Type::Int | Type::Float => Some(name == "number"),
        Type::String => Some(name == "string"),
        Type::Bool => Some(name == "boolean"),
        Type::Function(_) | Type::GenericFunction(_) => Some(name == "function"),
        _ => None,
    }
}

fn is_string_prototype_method(name: &str) -> bool {
    matches!(
        name,
        "charAt"
            | "charCodeAt"
            | "concat"
            | "endsWith"
            | "includes"
            | "indexOf"
            | "lastIndexOf"
            | "localeCompare"
            | "match"
            | "matchAll"
            | "normalize"
            | "padEnd"
            | "padStart"
            | "repeat"
            | "replace"
            | "replaceAll"
            | "search"
            | "slice"
            | "split"
            | "startsWith"
            | "substr"
            | "substring"
            | "toLowerCase"
            | "toUpperCase"
            | "trim"
            | "trimEnd"
            | "trimStart"
            | "trimLeft"
            | "trimRight"
    )
}

fn stringify_elision_intrinsic_receiver(intrinsic: Intrinsic) -> bool {
    matches!(
        intrinsic,
        Intrinsic::JsStringSlice
            | Intrinsic::JsStringIndexOf
            | Intrinsic::JsStringReplace
            | Intrinsic::JsStringMatch
            | Intrinsic::JsStringSplit
            | Intrinsic::StringCharAt
            | Intrinsic::StringCharCodeAt
            | Intrinsic::StringIncludes
            | Intrinsic::StringIndexOf
            | Intrinsic::StringLastIndexOf
            | Intrinsic::StringRepeat
            | Intrinsic::StringStartsWith
            | Intrinsic::StringEndsWith
            | Intrinsic::StringToUpperCase
            | Intrinsic::StringToLowerCase
            | Intrinsic::StringTrim
            | Intrinsic::StringTrimStart
            | Intrinsic::StringTrimEnd
            | Intrinsic::StringSearch
            | Intrinsic::StringSlice
            | Intrinsic::StringReplace
            | Intrinsic::StringSplit
            | Intrinsic::StringCodePointLength
    )
}

fn rewrite_stringify_slot(
    slot: &mut ValueId,
    stringify: &AHashMap<ValueId, ValueId>,
    consumed: &mut AHashSet<ValueId>,
) -> bool {
    let Some(inner) = stringify.get(slot).copied() else {
        return false;
    };
    consumed.insert(*slot);
    *slot = inner;
    true
}

fn elide_stringify_in_op(
    op: &mut ControlFlowOp<'_>,
    stringify: &AHashMap<ValueId, ValueId>,
    const_strings: &AHashSet<ValueId>,
    consumed: &mut AHashSet<ValueId>,
) -> bool {
    match op {
        ControlFlowOp::Intrinsic {
            intrinsic: Intrinsic::RegexTest | Intrinsic::JsRegexExec,
            args,
            ..
        } => args
            .first_mut()
            .is_some_and(|needle| rewrite_stringify_slot(needle, stringify, consumed)),
        ControlFlowOp::Intrinsic {
            intrinsic:
                Intrinsic::JsParseFloat
                | Intrinsic::JsParseInt
                | Intrinsic::JsEncodeURI
                | Intrinsic::JsEncodeURIComponent,
            args,
            ..
        } => args
            .first_mut()
            .is_some_and(|value| rewrite_stringify_slot(value, stringify, consumed)),
        ControlFlowOp::Intrinsic {
            intrinsic: Intrinsic::JsAdd,
            receiver,
            args,
            ..
        } => {
            let mut changed = false;
            if args.first().is_some_and(|rhs| const_strings.contains(rhs)) {
                if let Some(lhs) = receiver.as_mut() {
                    changed |= rewrite_stringify_slot(lhs, stringify, consumed);
                }
            }
            if receiver
                .as_ref()
                .is_some_and(|lhs| const_strings.contains(lhs))
            {
                if let Some(rhs) = args.first_mut() {
                    changed |= rewrite_stringify_slot(rhs, stringify, consumed);
                }
            }
            changed
        }
        ControlFlowOp::Intrinsic {
            intrinsic,
            receiver,
            ..
        } if stringify_elision_intrinsic_receiver(*intrinsic) => receiver
            .as_mut()
            .is_some_and(|receiver| rewrite_stringify_slot(receiver, stringify, consumed)),
        ControlFlowOp::Binary {
            op: IrBinaryOp::Add,
            lhs,
            rhs,
        } => {
            let mut changed = false;
            if const_strings.contains(rhs) {
                changed |= rewrite_stringify_slot(lhs, stringify, consumed);
            }
            if const_strings.contains(lhs) {
                changed |= rewrite_stringify_slot(rhs, stringify, consumed);
            }
            changed
        }
        ControlFlowOp::HostCall { method, args, .. } if *method == "removeAttribute" => args
            .first_mut()
            .is_some_and(|name| rewrite_stringify_slot(name, stringify, consumed)),
        _ => false,
    }
}

fn elide_single_use_stringify(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        if !function.live {
            continue;
        }
        let uses = control_flow_use_counts(function);
        let mut stringify = AHashMap::default();
        let mut const_strings = AHashSet::default();
        let mut invoke_keys = AHashMap::<ValueId, String>::default();
        for block in &function.blocks {
            for instruction in &block.instructions {
                match (instruction.out, &instruction.op) {
                    (Some(out), ControlFlowOp::Const(ConstValue::String(text))) => {
                        const_strings.insert(out);
                        invoke_keys.insert(out, text.clone());
                    }
                    (
                        Some(out),
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::JsStringify,
                            receiver: Some(inner),
                            ..
                        },
                    ) if uses.get(&out).copied() == Some(1) => {
                        stringify.insert(out, *inner);
                    }
                    _ => {}
                }
            }
        }
        if stringify.is_empty() {
            continue;
        }
        let mut consumed = AHashSet::default();
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                if elide_stringify_in_op(
                    &mut instruction.op,
                    &stringify,
                    &const_strings,
                    &mut consumed,
                ) {
                    changed = true;
                }
                if let ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::JsInvoke,
                    receiver,
                    args,
                    ..
                } = &mut instruction.op
                {
                    let invoke_name = args.first().and_then(|key| invoke_keys.get(key)).cloned();
                    match invoke_name.as_deref() {
                        Some(name) if is_string_prototype_method(name) => {
                            if let Some(receiver) = receiver.as_mut() {
                                if rewrite_stringify_slot(receiver, &stringify, &mut consumed) {
                                    changed = true;
                                }
                            }
                        }
                        Some("test" | "exec") => {
                            if let Some(subject) = args.get_mut(1) {
                                if rewrite_stringify_slot(subject, &stringify, &mut consumed) {
                                    changed = true;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if consumed.is_empty() {
            continue;
        }
        for block in &mut function.blocks {
            block
                .instructions
                .retain(|instruction| !instruction.out.is_some_and(|out| consumed.contains(&out)));
        }
    }
    OptimizationReport {
        pass_name: "single-use-stringify-elision",
        changed,
    }
}

fn field_store_barrier(op: &ControlFlowOp<'_>) -> bool {
    matches!(
        op,
        ControlFlowOp::StoreGlobal { .. }
            | ControlFlowOp::CallDirect { .. }
            | ControlFlowOp::CallValue { .. }
            | ControlFlowOp::CallMethod { .. }
            | ControlFlowOp::HostFieldGet { .. }
            | ControlFlowOp::HostFieldSet { .. }
            | ControlFlowOp::HostCall { .. }
            | ControlFlowOp::Intrinsic { .. }
            | ControlFlowOp::Closure { .. }
    )
}

fn eliminate_unread_globals(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut loaded = module
        .functions
        .iter()
        .filter(|function| function.live)
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction.op {
            ControlFlowOp::LoadGlobal(symbol) => Some(symbol),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    loaded.extend(
        module
            .exports
            .iter()
            .filter_map(|export| match export.binding {
                ExportBinding::Global(symbol) => Some(symbol),
                _ => None,
            }),
    );
    loaded.extend(
        module
            .lazy_modules
            .iter()
            .flat_map(|lazy| lazy.exports.iter())
            .filter_map(|export| match export.binding {
                ExportBinding::Global(symbol) => Some(symbol),
                _ => None,
            }),
    );
    let unread = module
        .globals
        .iter()
        .filter(|global| !loaded.contains(&global.symbol))
        .map(|global| global.symbol)
        .collect::<AHashSet<_>>();
    if unread.is_empty() {
        return OptimizationReport {
            pass_name: "unused-global-elimination",
            changed: false,
        };
    }
    let mut changed = false;
    for function in &mut module.functions {
        for block in &mut function.blocks {
            let before = block.instructions.len();
            block.instructions.retain(|instruction| {
                !matches!(
                    instruction.op,
                    ControlFlowOp::StoreGlobal { global, .. } if unread.contains(&global)
                )
            });
            changed |= block.instructions.len() != before;
        }
    }
    let before_globals = module.globals.len();
    module
        .globals
        .retain(|global| !unread.contains(&global.symbol));
    changed |= module.globals.len() != before_globals;
    OptimizationReport {
        pass_name: "unused-global-elimination",
        changed,
    }
}

fn propagate_single_assignment_globals(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let exported = exported_globals(module);
    let mut stores = AHashMap::<crate::semantic::SymbolId, Vec<Option<ConstValue>>>::default();
    for function in &module.functions {
        let constants = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (&instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Const(value)) => Some((*out, value.clone())),
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            if let ControlFlowOp::StoreGlobal { global, value } = instruction.op {
                stores
                    .entry(global)
                    .or_default()
                    .push(constants.get(&value).cloned());
            }
        }
    }

    let propagated = stores
        .into_iter()
        .filter_map(|(symbol, values)| {
            if exported.contains(&symbol) {
                None
            } else {
                match values.as_slice() {
                    [Some(value)] => Some((symbol, value.clone())),
                    _ => None,
                }
            }
        })
        .collect::<AHashMap<_, _>>();
    if propagated.is_empty() {
        return OptimizationReport {
            pass_name: "global-constant-propagation",
            changed: false,
        };
    }

    for function in &mut module.functions {
        for block in &mut function.blocks {
            block.instructions.retain_mut(|instruction| {
                if let ControlFlowOp::LoadGlobal(symbol) = instruction.op {
                    if let Some(value) = propagated.get(&symbol) {
                        instruction.op = ControlFlowOp::Const(value.clone());
                    }
                    return true;
                }
                !matches!(
                    instruction.op,
                    ControlFlowOp::StoreGlobal { global, .. }
                        if propagated.contains_key(&global)
                )
            });
        }
    }
    module
        .globals
        .retain(|global| !propagated.contains_key(&global.symbol));
    OptimizationReport {
        pass_name: "global-constant-propagation",
        changed: true,
    }
}

fn eliminate_redundant_phis(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        let mut aliases = AHashMap::<ValueId, ValueId>::default();
        loop {
            let mut local_change = false;
            for block in &mut function.blocks {
                for phi in &mut block.phis {
                    for (_, value) in &mut phi.incoming {
                        *value = resolve_alias(*value, &aliases);
                    }
                }
                block.phis.retain(|phi| {
                    let Some((_, first)) = phi.incoming.first() else {
                        return true;
                    };
                    if *first != phi.out
                        && phi.incoming.iter().all(|(_, incoming)| incoming == first)
                    {
                        aliases.insert(phi.out, *first);
                        local_change = true;
                        false
                    } else {
                        true
                    }
                });
            }
            changed |= local_change;
            if !local_change {
                break;
            }
        }
        rewrite_control_flow_function(function, &aliases);
    }
    OptimizationReport {
        pass_name: "trivial-phi-elimination",
        changed,
    }
}

fn simplify_algebraic_expressions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        let value_types = control_flow_value_types(function);
        let mut constants = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (&instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Const(value)) => Some((*out, value.clone())),
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        let mut aliases = AHashMap::<ValueId, ValueId>::default();

        for block in &mut function.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut retained = Vec::with_capacity(instructions.len());
            for mut instruction in instructions {
                rewrite_control_flow_op(&mut instruction.op, &aliases);
                let Some(out) = instruction.out else {
                    retained.push(instruction);
                    continue;
                };
                let mut alias = None;
                let mut replacement = None;
                match instruction.op {
                    ControlFlowOp::Unary {
                        op: IrUnaryOp::Not,
                        value,
                    } => {
                        if let Some(ConstValue::Bool(value)) = constants.get(&value) {
                            replacement = Some(ConstValue::Bool(!value));
                        }
                    }
                    ControlFlowOp::TypeCheck { value, ref target } => {
                        if let Some(source) = value_types.get(&value) {
                            if !matches!(
                                source,
                                Type::Union(_) | Type::Nullable(_) | Type::TypeParameter(_)
                            ) {
                                replacement = Some(ConstValue::Bool(source == target));
                            }
                        }
                    }
                    ControlFlowOp::Binary { op, lhs, rhs } => {
                        let lhs_const = constants.get(&lhs);
                        let rhs_const = constants.get(&rhs);
                        let output_type = instruction.ty.as_ref();
                        if output_type == Some(&Type::Int) {
                            alias = match op {
                                IrBinaryOp::Add if is_int(rhs_const, 0) => Some(lhs),
                                IrBinaryOp::Add if is_int(lhs_const, 0) => Some(rhs),
                                IrBinaryOp::Sub if is_int(rhs_const, 0) => Some(lhs),
                                IrBinaryOp::Mul if is_int(rhs_const, 1) => Some(lhs),
                                IrBinaryOp::Mul if is_int(lhs_const, 1) => Some(rhs),
                                IrBinaryOp::Mul if is_int(rhs_const, 0) => Some(rhs),
                                IrBinaryOp::Mul if is_int(lhs_const, 0) => Some(lhs),
                                IrBinaryOp::Div if is_int(rhs_const, 1) => Some(lhs),
                                IrBinaryOp::Xor if is_int(rhs_const, 0) => Some(lhs),
                                IrBinaryOp::Xor if is_int(lhs_const, 0) => Some(rhs),
                                _ => None,
                            };
                            if replacement.is_none()
                                && lhs == rhs
                                && matches!(op, IrBinaryOp::Sub | IrBinaryOp::Mod)
                            {
                                replacement = Some(ConstValue::Int(0));
                            }
                        } else if output_type == Some(&Type::Float) {
                            alias = match op {
                                IrBinaryOp::Sub if is_float(rhs_const, 0.0) => Some(lhs),
                                IrBinaryOp::Mul if is_float(rhs_const, 1.0) => Some(lhs),
                                IrBinaryOp::Mul if is_float(lhs_const, 1.0) => Some(rhs),
                                IrBinaryOp::Div if is_float(rhs_const, 1.0) => Some(lhs),
                                _ => None,
                            };
                        } else if output_type == Some(&Type::Bool) {
                            // NOTE: the *result* of `==`/`!=` is always `Type::Bool`
                            // regardless of the operand types (e.g. comparing a
                            // dynamic `JsValue` against a bool literal also
                            // produces `Type::Bool`). The `x == true -> x` /
                            // `x != true -> !x` simplifications below are only
                            // valid when the aliased operand is *itself*
                            // statically typed `bool`, so each arm must check the
                            // operand's own type, not just the comparison result.
                            let lhs_is_bool = matches!(value_types.get(&lhs), Some(Type::Bool));
                            let rhs_is_bool = matches!(value_types.get(&rhs), Some(Type::Bool));

                            // A typed non-null value cannot become `null` (or
                            // `undefined`) inside the closed world. Optional
                            // access lowering deliberately spells its guard as
                            // a loose null comparison so it covers both host
                            // absence values. Once propagation has proved that
                            // receiver non-null, fold the guard before CFG
                            // simplification instead of carrying a dead
                            // `if (value != null)` into JavaScript.
                            let null_comparison = match (lhs_const, rhs_const) {
                                (Some(ConstValue::Null), _) => Some(rhs),
                                (_, Some(ConstValue::Null)) => Some(lhs),
                                _ => None,
                            };
                            if let Some(value) = null_comparison {
                                if value_types
                                    .get(&value)
                                    .is_some_and(type_is_definitely_non_null)
                                {
                                    replacement = match op {
                                        IrBinaryOp::Eq => Some(ConstValue::Bool(false)),
                                        IrBinaryOp::NotEq => Some(ConstValue::Bool(true)),
                                        _ => None,
                                    };
                                }
                            }
                            alias = match op {
                                IrBinaryOp::Eq if rhs_is_bool && is_bool(rhs_const, true) => {
                                    Some(lhs)
                                }
                                IrBinaryOp::Eq if lhs_is_bool && is_bool(lhs_const, true) => {
                                    Some(rhs)
                                }
                                IrBinaryOp::NotEq if lhs_is_bool && is_bool(rhs_const, false) => {
                                    Some(lhs)
                                }
                                IrBinaryOp::NotEq if rhs_is_bool && is_bool(lhs_const, false) => {
                                    Some(rhs)
                                }
                                _ => None,
                            };
                            if alias.is_none() {
                                let negated = match op {
                                    IrBinaryOp::Eq if lhs_is_bool && is_bool(rhs_const, false) => {
                                        Some(lhs)
                                    }
                                    IrBinaryOp::Eq if rhs_is_bool && is_bool(lhs_const, false) => {
                                        Some(rhs)
                                    }
                                    IrBinaryOp::NotEq
                                        if lhs_is_bool && is_bool(rhs_const, true) =>
                                    {
                                        Some(lhs)
                                    }
                                    IrBinaryOp::NotEq
                                        if rhs_is_bool && is_bool(lhs_const, true) =>
                                    {
                                        Some(rhs)
                                    }
                                    _ => None,
                                };
                                if let Some(value) = negated {
                                    instruction.op = ControlFlowOp::Unary {
                                        op: IrUnaryOp::Not,
                                        value,
                                    };
                                    changed = true;
                                } else if lhs == rhs
                                    && matches!(
                                        value_types.get(&lhs),
                                        Some(
                                            Type::Int
                                                | Type::Bool
                                                | Type::String
                                                | Type::Class(_)
                                                | Type::ClassInstance { .. }
                                        )
                                    )
                                {
                                    replacement = match op {
                                        IrBinaryOp::Eq
                                        | IrBinaryOp::LessEq
                                        | IrBinaryOp::GreaterEq => Some(ConstValue::Bool(true)),
                                        IrBinaryOp::NotEq
                                        | IrBinaryOp::Less
                                        | IrBinaryOp::Greater => Some(ConstValue::Bool(false)),
                                        _ => None,
                                    };
                                }
                            }
                        }
                    }
                    _ => {}
                }

                if let Some(alias) = alias {
                    aliases.insert(out, resolve_alias(alias, &aliases));
                    changed = true;
                    continue;
                }
                if let Some(value) = replacement {
                    instruction.op = ControlFlowOp::Const(value.clone());
                    constants.insert(out, value);
                    changed = true;
                }
                retained.push(instruction);
            }
            block.instructions = retained;
        }
        rewrite_control_flow_function(function, &aliases);
    }
    OptimizationReport {
        pass_name: "algebraic-simplification",
        changed,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ValueNumberKey {
    Constant(ConstantNumber),
    Unary(IrUnaryOp, ValueId),
    Binary(IrBinaryOp, ValueId, ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RuntimeTypePredicate {
    String,
    Boolean,
}

fn runtime_type_predicate_for_target(target: &Type<'_>) -> Option<RuntimeTypePredicate> {
    match target {
        Type::String => Some(RuntimeTypePredicate::String),
        Type::Bool => Some(RuntimeTypePredicate::Boolean),
        _ => None,
    }
}

fn runtime_type_predicate_for_name(name: &str) -> Option<RuntimeTypePredicate> {
    match name {
        "string" => Some(RuntimeTypePredicate::String),
        "boolean" => Some(RuntimeTypePredicate::Boolean),
        _ => None,
    }
}

fn runtime_type_predicate_key(
    op: &ControlFlowOp<'_>,
    definitions: &AHashMap<ValueId, &ControlFlowOp<'_>>,
) -> Option<(ValueId, RuntimeTypePredicate)> {
    if let ControlFlowOp::TypeCheck { value, target } = op {
        return runtime_type_predicate_for_target(target).map(|predicate| (*value, predicate));
    }

    let ControlFlowOp::Binary {
        op: IrBinaryOp::Eq,
        lhs,
        rhs,
    } = op
    else {
        return None;
    };
    let predicate = |type_of: ValueId, name: ValueId| {
        let ControlFlowOp::Intrinsic {
            intrinsic: Intrinsic::JsTypeOf,
            receiver: Some(value),
            args,
        } = definitions.get(&type_of)?
        else {
            return None;
        };
        if !args.is_empty() {
            return None;
        }
        let ControlFlowOp::Const(ConstValue::String(name)) = definitions.get(&name)? else {
            return None;
        };
        runtime_type_predicate_for_name(name).map(|predicate| (*value, predicate))
    };
    predicate(*lhs, *rhs).or_else(|| predicate(*rhs, *lhs))
}

/// Value-number the two backend-compatible `typeof` categories across dominated CFG
/// blocks. `value is string` and `JS.typeOf(value) == "string"` have the same
/// predicate key, as do the corresponding boolean spellings. Numeric guards
/// stay distinct because the native backend distinguishes `int` from `float`
/// while JavaScript's `typeof` reports both as `"number"`.
///
/// This deliberately requires the exact same SSA value and strict dominance.
/// It therefore never merges member reads or calls (which may observe getters
/// or proxies), and never performs a coercion.
fn eliminate_dominated_runtime_type_predicates(function: &mut ControlFlowFunction<'_>) -> bool {
    // The ordinary CFG intentionally omits implicit exceptional edges. Do not
    // use its dominance relation for functions whose protected regions can
    // reach catch/finally without executing every preceding instruction.
    if function.blocks.is_empty() || has_exception_region(function) {
        return false;
    }

    let definitions = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| instruction.out.map(|out| (out, &instruction.op)))
        .collect::<AHashMap<_, _>>();
    let mut predicates = Vec::new();
    for (block, body) in function.blocks.iter().enumerate() {
        for (index, instruction) in body.instructions.iter().enumerate() {
            let Some(out) = instruction.out else {
                continue;
            };
            let Some(key) = runtime_type_predicate_key(&instruction.op, &definitions) else {
                continue;
            };
            predicates.push((block, index, out, key));
        }
    }
    if predicates.len() < 2 {
        return false;
    }

    let predecessors = cfg_predecessors(function);
    let reachable = reachable_blocks(function);
    let dominators = compute_dominators(function.entry.0 as usize, &predecessors, &reachable);
    let mut aliases = AHashMap::<ValueId, ValueId>::default();
    for (block, index, out, key) in &predicates {
        let Some((_, _, previous, _)) = predicates.iter().find(
            |(candidate_block, candidate_index, candidate_out, candidate_key)| {
                candidate_key == key
                    && candidate_out != out
                    && if candidate_block == block {
                        candidate_index < index
                    } else {
                        dominators[*block].contains(candidate_block)
                    }
            },
        ) else {
            continue;
        };
        aliases.insert(*out, resolve_alias(*previous, &aliases));
    }
    if aliases.is_empty() {
        return false;
    }
    rewrite_control_flow_function(function, &aliases);
    for block in &mut function.blocks {
        block.instructions.retain(|instruction| {
            instruction
                .out
                .is_none_or(|out| !aliases.contains_key(&out))
        });
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ConstantNumber {
    Int(i64),
    Float(u64),
    Bool(bool),
    String(String),
    Null,
}

impl From<&ConstValue> for ConstantNumber {
    fn from(value: &ConstValue) -> Self {
        match value {
            ConstValue::Int(value) => Self::Int(*value),
            ConstValue::Float(value) => Self::Float(value.to_bits()),
            ConstValue::Bool(value) => Self::Bool(*value),
            ConstValue::String(value) => Self::String(value.clone()),
            ConstValue::Null => Self::Null,
        }
    }
}

fn eliminate_common_subexpressions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        changed |= eliminate_dominated_runtime_type_predicates(function);
        let value_types = control_flow_value_types(function);
        let mut aliases = AHashMap::<ValueId, ValueId>::default();
        for block in &mut function.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut retained = Vec::with_capacity(instructions.len());
            let mut numbers = AHashMap::<ValueNumberKey, ValueId>::default();
            for mut instruction in instructions {
                rewrite_control_flow_op(&mut instruction.op, &aliases);
                let key =
                    if instruction_has_dynamic_observable_evaluation(&instruction, &value_types) {
                        None
                    } else {
                        match &instruction.op {
                            ControlFlowOp::Const(value) => {
                                Some(ValueNumberKey::Constant(ConstantNumber::from(value)))
                            }
                            ControlFlowOp::Unary { op, value } => {
                                Some(ValueNumberKey::Unary(*op, *value))
                            }
                            ControlFlowOp::Binary { op, lhs, rhs } => {
                                let (lhs, rhs) = if is_commutative(*op) && rhs.0 < lhs.0 {
                                    (*rhs, *lhs)
                                } else {
                                    (*lhs, *rhs)
                                };
                                Some(ValueNumberKey::Binary(*op, lhs, rhs))
                            }
                            _ => None,
                        }
                    };
                if let (Some(out), Some(key)) = (instruction.out, key) {
                    if let Some(previous) = numbers.get(&key) {
                        aliases.insert(out, *previous);
                        changed = true;
                        continue;
                    }
                    numbers.insert(key, out);
                }
                retained.push(instruction);
            }
            block.instructions = retained;
        }
        rewrite_control_flow_function(function, &aliases);
    }
    OptimizationReport {
        pass_name: "local-value-numbering",
        changed,
    }
}

fn is_int(value: Option<&ConstValue>, expected: i64) -> bool {
    matches!(value, Some(ConstValue::Int(value)) if *value == expected)
}

fn is_float(value: Option<&ConstValue>, expected: f64) -> bool {
    matches!(value, Some(ConstValue::Float(value)) if *value == expected)
}

fn is_bool(value: Option<&ConstValue>, expected: bool) -> bool {
    matches!(value, Some(ConstValue::Bool(value)) if *value == expected)
}

fn type_is_definitely_non_null(ty: &Type<'_>) -> bool {
    match ty {
        Type::Null | Type::Nullable(_) | Type::Void | Type::TypeParameter(_) => false,
        Type::Union(members) => members.iter().all(type_is_definitely_non_null),
        _ => true,
    }
}

fn is_commutative(op: IrBinaryOp) -> bool {
    matches!(
        op,
        IrBinaryOp::Add
            | IrBinaryOp::Mul
            | IrBinaryOp::BitAnd
            | IrBinaryOp::BitOr
            | IrBinaryOp::Eq
            | IrBinaryOp::NotEq
            | IrBinaryOp::Xor
            | IrBinaryOp::And
            | IrBinaryOp::Or
    )
}

fn control_flow_value_types<'src>(
    function: &ControlFlowFunction<'src>,
) -> AHashMap<ValueId, Type<'src>> {
    let mut types = AHashMap::default();
    for param in &function.params {
        types.insert(param.value, param.ty.clone());
    }
    for block in &function.blocks {
        for phi in &block.phis {
            types.insert(phi.out, phi.ty.clone());
        }
        for instruction in &block.instructions {
            if let (Some(out), Some(ty)) = (instruction.out, &instruction.ty) {
                types.insert(out, ty.clone());
            }
        }
    }
    types
}

fn internalize_entry_globals(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let exported = exported_globals(module);
    let mut shared = AHashSet::default();
    for function in module
        .functions
        .iter()
        .filter(|function| function.live && function.id != module.entry)
    {
        for block in &function.blocks {
            for instruction in &block.instructions {
                match instruction.op {
                    ControlFlowOp::LoadGlobal(symbol) => {
                        shared.insert(symbol);
                    }
                    ControlFlowOp::StoreGlobal { global, .. } => {
                        shared.insert(global);
                    }
                    _ => {}
                }
            }
        }
    }
    // Lazy-module bodies are outside the entry function's eager ownership
    // boundary even when a current chunk plan does not place them beside the
    // entry. A binding used there must remain a module/global binding.
    for lazy in &module.lazy_modules {
        for export in &lazy.exports {
            if let ExportBinding::Global(symbol) = export.binding {
                shared.insert(symbol);
            }
        }
    }

    let mut loaded_by_entry = AHashSet::default();
    for block in &module.functions[module.entry.0 as usize].blocks {
        for instruction in &block.instructions {
            if let ControlFlowOp::LoadGlobal(symbol) = instruction.op {
                loaded_by_entry.insert(symbol);
            }
        }
    }

    let internal = module
        .globals
        .iter()
        .filter(|global| {
            !global.external
                && loaded_by_entry.contains(&global.symbol)
                && !shared.contains(&global.symbol)
                && !exported.contains(&global.symbol)
        })
        .cloned()
        .collect::<Vec<_>>();
    if internal.is_empty() {
        return OptimizationReport {
            pass_name: "global-internalization",
            changed: false,
        };
    }

    let entry = &mut module.functions[module.entry.0 as usize];
    let mut local_by_symbol = AHashMap::default();
    for global in &internal {
        let local = LocalId(entry.locals.len() as u32);
        entry.locals.push(IrLocal {
            id: local,
            symbol: global.symbol,
            name: global.name,
            ty: global.ty.clone(),
            span: global.span,
        });
        local_by_symbol.insert(global.symbol, local);
    }
    for block in &mut entry.blocks {
        for instruction in &mut block.instructions {
            instruction.op = match instruction.op.clone() {
                ControlFlowOp::LoadGlobal(symbol) if local_by_symbol.contains_key(&symbol) => {
                    ControlFlowOp::LoadLocal(local_by_symbol[&symbol])
                }
                ControlFlowOp::StoreGlobal { global, value }
                    if local_by_symbol.contains_key(&global) =>
                {
                    ControlFlowOp::StoreLocal {
                        local: local_by_symbol[&global],
                        value,
                    }
                }
                op => op,
            };
        }
    }
    // This pass normally runs before mem2reg, but final call-graph reduction
    // can expose additional entry-only globals.  Mark the entry for another
    // promotion round when that happens.
    entry.locals_promoted = false;
    module
        .globals
        .retain(|global| !local_by_symbol.contains_key(&global.symbol));

    OptimizationReport {
        pass_name: "global-internalization",
        changed: true,
    }
}

fn devirtualize_methods(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                let replacement = match &instruction.op {
                    ControlFlowOp::CallMethod {
                        receiver,
                        function,
                        args,
                        ..
                    } => {
                        let mut direct_args = Vec::with_capacity(args.len() + 1);
                        direct_args.push(*receiver);
                        direct_args.extend(args.iter().copied());
                        Some(ControlFlowOp::CallDirect {
                            function: *function,
                            provided_args: direct_args.len(),
                            args: direct_args,
                        })
                    }
                    _ => None,
                };
                if let Some(replacement) = replacement {
                    instruction.op = replacement;
                    changed = true;
                }
            }
        }
    }
    OptimizationReport {
        pass_name: "devirtualization",
        changed,
    }
}

fn devirtualize_known_closure_calls(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    let direct_targets = module
        .functions
        .iter()
        .filter(|function| function.kind != FunctionKind::Closure)
        .map(|function| function.id)
        .collect::<AHashSet<_>>();
    for function in &mut module.functions {
        let closures = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Closure { function, captures })
                    if direct_targets.contains(function) =>
                {
                    Some((out, (*function, captures.clone())))
                }
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        for instruction in function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
        {
            let replacement = match &instruction.op {
                ControlFlowOp::CallValue { callee, args } => {
                    closures.get(callee).map(|(target, captures)| {
                        let mut direct_args = captures.clone();
                        direct_args.extend(args);
                        ControlFlowOp::CallDirect {
                            function: *target,
                            provided_args: direct_args.len(),
                            args: direct_args,
                        }
                    })
                }
                _ => None,
            };
            if let Some(replacement) = replacement {
                instruction.op = replacement;
                changed = true;
            }
        }
    }
    OptimizationReport {
        pass_name: "closure-call-devirtualization",
        changed,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum SpecializationValue {
    Constant(ConstantKey),
    Function(FunctionId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum ConstantKey {
    Int(i64),
    Float(u64),
    Bool(bool),
    String(String),
    Null,
}

impl ConstantKey {
    fn from_value(value: &ConstValue) -> Self {
        match value {
            ConstValue::Int(value) => Self::Int(*value),
            ConstValue::Float(value) => Self::Float(value.to_bits()),
            ConstValue::Bool(value) => Self::Bool(*value),
            ConstValue::String(value) => Self::String(value.clone()),
            ConstValue::Null => Self::Null,
        }
    }

    fn to_value(&self) -> ConstValue {
        match self {
            Self::Int(value) => ConstValue::Int(*value),
            Self::Float(value) => ConstValue::Float(f64::from_bits(*value)),
            Self::Bool(value) => ConstValue::Bool(*value),
            Self::String(value) => ConstValue::String(value.clone()),
            Self::Null => ConstValue::Null,
        }
    }
}

#[derive(Debug, Clone)]
struct ProfiledCallSite {
    caller: usize,
    block: usize,
    instruction: usize,
    frequency: u64,
}

#[derive(Debug, Clone)]
struct ProfiledSpecialization {
    callee: FunctionId,
    signature: Vec<(usize, SpecializationValue)>,
    sites: Vec<ProfiledCallSite>,
}

fn specialize_profiled_call_sites(
    module: &mut ControlFlowModule<'_>,
    specialize_tagged_constants: bool,
    guidance: &OptimizationGuidance,
) -> OptimizationReport {
    let mut groups = AHashMap::<
        (FunctionId, Vec<(usize, SpecializationValue)>),
        Vec<ProfiledCallSite>,
    >::default();
    for (caller_index, caller) in module.functions.iter().enumerate() {
        let definitions = specialization_definitions(caller);
        for (block_index, block) in caller.blocks.iter().enumerate() {
            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                let ControlFlowOp::CallDirect { function, args, .. } = &instruction.op else {
                    continue;
                };
                if *function == caller.id {
                    continue;
                }
                let Some(callee) = module.functions.get(function.0 as usize) else {
                    continue;
                };
                if !callee.live
                    || !matches!(
                        callee.kind,
                        FunctionKind::Function | FunctionKind::Method { .. }
                    )
                {
                    continue;
                }
                let signature = callee
                    .params
                    .iter()
                    .zip(args)
                    .enumerate()
                    .filter_map(|(index, (parameter, argument))| {
                        let value = definitions.get(argument)?;
                        match value {
                            SpecializationValue::Function(_)
                                if matches!(parameter.ty, Type::Function(_)) =>
                            {
                                Some((index, value.clone()))
                            }
                            SpecializationValue::Constant(key)
                                if specialize_tagged_constants
                                    || constant_has_direct_native_representation(
                                        &key.to_value(),
                                        &parameter.ty,
                                    ) =>
                            {
                                Some((index, value.clone()))
                            }
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>();
                if signature.is_empty() {
                    continue;
                }
                groups
                    .entry((*function, signature))
                    .or_default()
                    .push(ProfiledCallSite {
                        caller: caller_index,
                        block: block_index,
                        instruction: instruction_index,
                        frequency: guidance.profile.block_count(caller, block.id),
                    });
            }
        }
    }

    let mut candidates = groups
        .into_iter()
        .filter_map(|((callee, signature), sites)| {
            let function = &module.functions[callee.0 as usize];
            let instructions = function_instruction_count(function);
            let frequency = sites
                .iter()
                .map(|site| site.frequency)
                .fold(0u64, u64::saturating_add);
            let call_savings = sites
                .len()
                .saturating_mul(signature.len())
                .saturating_mul(3);
            let clone_cost = instructions
                .saturating_mul(4)
                .saturating_add(function.blocks.len().saturating_mul(3));
            (instructions <= guidance.max_clone_instructions
                && (frequency >= guidance.specialization_min_count || call_savings > clone_cost))
                .then_some((
                    frequency,
                    ProfiledSpecialization {
                        callee,
                        signature,
                        sites,
                    },
                ))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.callee.0.cmp(&right.1.callee.0))
            .then_with(|| left.1.signature.cmp(&right.1.signature))
    });

    let mut per_function = AHashMap::<FunctionId, usize>::default();
    let mut changed = false;
    for (_, candidate) in candidates {
        let count = per_function.entry(candidate.callee).or_default();
        if *count >= guidance.max_specializations_per_function {
            continue;
        }
        *count += 1;
        let new_id = FunctionId(module.functions.len() as u32);
        let clone = clone_function_with_specialization(
            &module.functions[candidate.callee.0 as usize],
            new_id,
            &candidate.signature,
        );
        module.functions.push(clone);
        for site in candidate.sites {
            let instruction = &mut module.functions[site.caller].blocks[site.block].instructions
                [site.instruction];
            let ControlFlowOp::CallDirect {
                function,
                args,
                provided_args,
            } = &mut instruction.op
            else {
                continue;
            };
            *function = new_id;
            remove_specialized_arguments(args, &candidate.signature);
            *provided_args = args.len();
        }
        changed = true;
    }
    OptimizationReport {
        pass_name: "profiled-call-site-specialization",
        changed,
    }
}

fn clone_constant_capture_signatures(
    module: &mut ControlFlowModule<'_>,
    guidance: &OptimizationGuidance,
) -> OptimizationReport {
    let mut groups = AHashMap::<
        (FunctionId, Vec<(usize, SpecializationValue)>),
        Vec<ProfiledCallSite>,
    >::default();
    for (caller_index, caller) in module.functions.iter().enumerate() {
        let definitions = specialization_definitions(caller);
        for (block_index, block) in caller.blocks.iter().enumerate() {
            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                let ControlFlowOp::Closure { function, captures } = &instruction.op else {
                    continue;
                };
                if captures.is_empty() {
                    continue;
                }
                let Some(target) = module.functions.get(function.0 as usize) else {
                    continue;
                };
                if target.kind != FunctionKind::Closure
                    || target.capture_count != captures.len()
                    || !target.mutable_capture_locals.is_empty()
                {
                    continue;
                }
                let signature = captures
                    .iter()
                    .enumerate()
                    .map(|(index, capture)| match definitions.get(capture) {
                        Some(SpecializationValue::Constant(value)) => {
                            Some((index, SpecializationValue::Constant(value.clone())))
                        }
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>();
                let Some(signature) = signature else {
                    continue;
                };
                groups
                    .entry((*function, signature))
                    .or_default()
                    .push(ProfiledCallSite {
                        caller: caller_index,
                        block: block_index,
                        instruction: instruction_index,
                        frequency: guidance.profile.block_count(caller, block.id),
                    });
            }
        }
    }

    let mut candidates = groups.into_iter().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        let left_frequency = left.1.iter().map(|site| site.frequency).sum::<u64>();
        let right_frequency = right.1.iter().map(|site| site.frequency).sum::<u64>();
        right_frequency
            .cmp(&left_frequency)
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut per_function = AHashMap::<FunctionId, usize>::default();
    let mut changed = false;
    for ((target, signature), sites) in candidates {
        let frequency = sites.iter().map(|site| site.frequency).sum::<u64>();
        let function = &module.functions[target.0 as usize];
        let instructions = function_instruction_count(function);
        let allocation_savings = sites
            .len()
            .saturating_mul(signature.len())
            .saturating_mul(4);
        let clone_cost = instructions.saturating_mul(4);
        if instructions > guidance.max_clone_instructions
            || (frequency < guidance.specialization_min_count && allocation_savings <= clone_cost)
        {
            continue;
        }
        let count = per_function.entry(target).or_default();
        if *count >= guidance.max_specializations_per_function {
            continue;
        }
        *count += 1;
        let new_id = FunctionId(module.functions.len() as u32);
        let mut clone = clone_function_with_specialization(
            &module.functions[target.0 as usize],
            new_id,
            &signature,
        );
        clone.capture_count = clone.capture_count.saturating_sub(signature.len());
        module.functions.push(clone);
        for site in sites {
            let instruction = &mut module.functions[site.caller].blocks[site.block].instructions
                [site.instruction];
            let ControlFlowOp::Closure { function, captures } = &mut instruction.op else {
                continue;
            };
            *function = new_id;
            remove_specialized_arguments(captures, &signature);
        }
        changed = true;
    }
    OptimizationReport {
        pass_name: "capture-signature-cloning",
        changed,
    }
}

fn specialization_definitions(
    function: &ControlFlowFunction<'_>,
) -> AHashMap<ValueId, SpecializationValue> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match (instruction.out, &instruction.op) {
            (Some(out), ControlFlowOp::Const(value)) => Some((
                out,
                SpecializationValue::Constant(ConstantKey::from_value(value)),
            )),
            (Some(out), ControlFlowOp::Closure { function, captures }) if captures.is_empty() => {
                Some((out, SpecializationValue::Function(*function)))
            }
            _ => None,
        })
        .collect()
}

fn clone_function_with_specialization<'src>(
    original: &ControlFlowFunction<'src>,
    id: FunctionId,
    signature: &[(usize, SpecializationValue)],
) -> ControlFlowFunction<'src> {
    let mut clone = original.clone();
    clone.id = id;
    let mut replacements = AHashMap::default();
    let mut constants = Vec::new();
    for (index, value) in signature {
        let parameter = &clone.params[*index];
        let parameter_value = parameter.value;
        let parameter_hint = clone
            .value_local_hints
            .get(parameter.value.0 as usize)
            .copied()
            .flatten()
            .or(Some(parameter.name));
        let parameter_span = parameter.span;
        let parameter_ty = parameter.ty.clone();
        let out = ValueId(clone.value_count);
        clone.value_count += 1;
        clone.value_escapes.push(EscapeState::LocalOnly);
        clone.value_local_hints.push(parameter_hint);
        replacements.insert(parameter_value, out);
        for dependent in &mut clone.params {
            let Some(crate::ir::IrParamDefault::Value(default)) = &dependent.default else {
                continue;
            };
            if *default == parameter_value {
                dependent.default = Some(match value {
                    SpecializationValue::Constant(value) => {
                        crate::ir::IrParamDefault::Const(value.to_value())
                    }
                    SpecializationValue::Function(_) => crate::ir::IrParamDefault::Value(out),
                });
            }
        }
        let operation = match value {
            SpecializationValue::Constant(value) => ControlFlowOp::Const(value.to_value()),
            SpecializationValue::Function(function) => ControlFlowOp::Closure {
                function: *function,
                captures: Vec::new(),
            },
        };
        constants.push(ControlFlowInstruction {
            out: Some(out),
            ty: Some(parameter_ty),
            op: operation,
            span: parameter_span,
        });
    }
    rewrite_control_flow_function(&mut clone, &replacements);
    clone.blocks[clone.entry.0 as usize]
        .instructions
        .splice(0..0, constants);
    clone.params = clone
        .params
        .drain(..)
        .enumerate()
        .filter_map(|(index, parameter)| {
            (!signature.iter().any(|(removed, _)| *removed == index)).then_some(parameter)
        })
        .collect();
    clone
}

fn remove_specialized_arguments(
    arguments: &mut Vec<ValueId>,
    signature: &[(usize, SpecializationValue)],
) {
    *arguments = arguments
        .drain(..)
        .enumerate()
        .filter_map(|(index, argument)| {
            (!signature.iter().any(|(removed, _)| *removed == index)).then_some(argument)
        })
        .collect();
}

fn function_instruction_count(function: &ControlFlowFunction<'_>) -> usize {
    function
        .blocks
        .iter()
        .map(|block| block.instructions.len() + block.phis.len())
        .sum()
}

fn optimize_unused_parameters(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let exported = exported_functions(module);
    let indirect = indirectly_referenced_functions(module);
    let removals = module
        .functions
        .iter()
        .filter(|function| {
            function.live
                && matches!(
                    function.kind,
                    FunctionKind::Function | FunctionKind::Method { .. }
                )
                && !exported.contains(&function.id)
                && !indirect.contains(&function.id)
        })
        .filter_map(|function| {
            let uses = control_flow_use_counts(function);
            let unused = function
                .params
                .iter()
                .enumerate()
                .filter_map(|(index, parameter)| {
                    (uses.get(&parameter.value).copied().unwrap_or(0) == 0).then_some(index)
                })
                .collect::<Vec<_>>();
            (!unused.is_empty()).then_some((function.id, unused))
        })
        .collect::<AHashMap<_, _>>();
    if removals.is_empty() {
        return OptimizationReport {
            pass_name: "unused-parameter-optimization",
            changed: false,
        };
    }
    for function in &mut module.functions {
        if let Some(indices) = removals.get(&function.id) {
            function.params = function
                .params
                .drain(..)
                .enumerate()
                .filter_map(|(index, parameter)| (!indices.contains(&index)).then_some(parameter))
                .collect();
        }
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                if let ControlFlowOp::CallDirect {
                    function: callee,
                    args,
                    provided_args,
                } = &mut instruction.op
                {
                    if let Some(indices) = removals.get(callee) {
                        *args = args
                            .drain(..)
                            .enumerate()
                            .filter_map(|(index, argument)| {
                                (!indices.contains(&index)).then_some(argument)
                            })
                            .collect();
                        *provided_args = args.len();
                    }
                }
            }
        }
    }
    OptimizationReport {
        pass_name: "unused-parameter-optimization",
        changed: true,
    }
}

fn specialize_constant_parameters(
    module: &mut ControlFlowModule<'_>,
    specialize_tagged_constants: bool,
    finite_value_propagation: bool,
) -> OptimizationReport {
    let exported = exported_functions(module);
    let indirect = indirectly_referenced_functions(module);
    let finite_values = finite_value_propagation.then(|| analyze_finite_values(module));
    let mut calls = AHashMap::<FunctionId, Vec<Vec<Option<ConstValue>>>>::default();
    for caller in &module.functions {
        let constants = caller
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Const(value)) => Some((out, value.clone())),
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        for instruction in caller.blocks.iter().flat_map(|block| &block.instructions) {
            if let ControlFlowOp::CallDirect { function, args, .. } = &instruction.op {
                calls.entry(*function).or_default().push(
                    args.iter()
                        .map(|argument| {
                            constants.get(argument).cloned().or_else(|| {
                                finite_values
                                    .as_ref()
                                    .and_then(|analysis| {
                                        analysis.function(caller.id).constant(*argument)
                                    })
                                    .cloned()
                            })
                        })
                        .collect(),
                );
            }
        }
    }
    let specializations = module
        .functions
        .iter()
        .filter(|function| {
            function.live
                && matches!(
                    function.kind,
                    FunctionKind::Function | FunctionKind::Method { .. }
                )
                && !exported.contains(&function.id)
                && !indirect.contains(&function.id)
        })
        .filter_map(|function| {
            let sites = calls.get(&function.id)?;
            let constants = function
                .params
                .iter()
                .enumerate()
                .filter_map(|(index, parameter)| {
                    let first = sites.first()?.get(index)?.clone()?;
                    ((specialize_tagged_constants
                        || constant_has_direct_native_representation(&first, &parameter.ty))
                        && sites
                            .iter()
                            .all(|args| args.get(index) == Some(&Some(first.clone()))))
                    .then_some((index, first))
                })
                .collect::<Vec<_>>();
            (!constants.is_empty()).then_some((function.id, constants))
        })
        .collect::<AHashMap<_, _>>();
    if specializations.is_empty() {
        return OptimizationReport {
            pass_name: "constant-parameter-specialization",
            changed: false,
        };
    }
    for function in &mut module.functions {
        if let Some(parameters) = specializations.get(&function.id) {
            let mut replacements = AHashMap::default();
            let mut constants = Vec::new();
            for (index, value) in parameters {
                let parameter = &function.params[*index];
                let out = ValueId(function.value_count);
                function.value_count += 1;
                function.value_escapes.push(EscapeState::LocalOnly);
                function.value_local_hints.push(
                    function
                        .value_local_hints
                        .get(parameter.value.0 as usize)
                        .copied()
                        .flatten()
                        .or(Some(parameter.name)),
                );
                replacements.insert(parameter.value, out);
                constants.push(ControlFlowInstruction {
                    out: Some(out),
                    ty: Some(parameter.ty.clone()),
                    op: ControlFlowOp::Const(value.clone()),
                    span: parameter.span,
                });
            }
            rewrite_control_flow_function(function, &replacements);
            function.blocks[function.entry.0 as usize]
                .instructions
                .splice(0..0, constants);
            function.params = function
                .params
                .drain(..)
                .enumerate()
                .filter_map(|(index, parameter)| {
                    (!parameters.iter().any(|(removed, _)| *removed == index)).then_some(parameter)
                })
                .collect();
        }
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                if let ControlFlowOp::CallDirect {
                    function: callee,
                    args,
                    provided_args,
                } = &mut instruction.op
                {
                    if let Some(parameters) = specializations.get(callee) {
                        *args = args
                            .drain(..)
                            .enumerate()
                            .filter_map(|(index, argument)| {
                                (!parameters.iter().any(|(removed, _)| *removed == index))
                                    .then_some(argument)
                            })
                            .collect();
                        *provided_args = args.len();
                    }
                }
            }
        }
    }
    OptimizationReport {
        pass_name: "constant-parameter-specialization",
        changed: true,
    }
}

fn constant_has_direct_native_representation(value: &ConstValue, ty: &Type<'_>) -> bool {
    matches!(
        (value, ty),
        (ConstValue::Int(_), Type::Int)
            | (ConstValue::Float(_), Type::Float)
            | (ConstValue::Bool(_), Type::Bool)
            | (ConstValue::String(_), Type::String)
    )
}

fn optimize_unused_returns(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let exported = exported_functions(module);
    let indirect = indirectly_referenced_functions(module);
    let mut observed = AHashSet::<FunctionId>::default();
    for function in &module.functions {
        let uses = control_flow_use_counts(function);
        for block in &function.blocks {
            for instruction in &block.instructions {
                let ControlFlowOp::CallDirect {
                    function: callee, ..
                } = instruction.op
                else {
                    continue;
                };
                if instruction
                    .out
                    .is_some_and(|out| uses.get(&out).copied().unwrap_or(0) != 0)
                {
                    observed.insert(callee);
                }
            }
        }
    }
    let candidates = module
        .functions
        .iter()
        .filter(|function| {
            function.live
                && !function.return_type.is_void()
                && matches!(
                    function.kind,
                    FunctionKind::Function | FunctionKind::Method { .. }
                )
                && !exported.contains(&function.id)
                && !indirect.contains(&function.id)
                && !observed.contains(&function.id)
        })
        .map(|function| function.id)
        .collect::<AHashSet<_>>();
    if candidates.is_empty() {
        return OptimizationReport {
            pass_name: "unused-return-optimization",
            changed: false,
        };
    }
    for function in &mut module.functions {
        if candidates.contains(&function.id) {
            function.return_type = Type::Void;
            for block in &mut function.blocks {
                if matches!(block.terminator, Some(Terminator::Return(Some(_)))) {
                    block.terminator = Some(Terminator::Return(None));
                }
            }
        }
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                if matches!(
                    instruction.op,
                    ControlFlowOp::CallDirect { function, .. } if candidates.contains(&function)
                ) {
                    instruction.out = None;
                    instruction.ty = None;
                }
            }
        }
    }
    OptimizationReport {
        pass_name: "unused-return-optimization",
        changed: true,
    }
}

fn indirectly_referenced_functions(module: &ControlFlowModule<'_>) -> AHashSet<FunctionId> {
    let mut indirect = AHashSet::default();
    for function in &module.functions {
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            match &instruction.op {
                ControlFlowOp::Closure { function, .. }
                | ControlFlowOp::NewClass {
                    constructor: Some(function),
                    ..
                } => {
                    indirect.insert(*function);
                }
                _ => {}
            }
        }
    }
    indirect
}

fn fold_and_propagate_control_flow(
    module: &mut ControlFlowModule<'_>,
    finite_value_propagation: bool,
) -> OptimizationReport {
    let mut changed = false;
    let finite_values = finite_value_propagation.then(|| analyze_finite_values(module));
    let effect_summaries = analyze_function_effects(module);
    let array_argument_retention_barriers = array_argument_retention_barriers(module);
    let parameter_array_lengths = analyze_array_parameter_lengths(module, &effect_summaries);
    for function in &mut module.functions {
        let loop_bodies = function
            .shapes
            .iter()
            .filter_map(|shape| match shape {
                crate::ir::ControlShape::Loop { header, body, .. } => Some((*header, *body)),
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        let mut constants = finite_values
            .as_ref()
            .into_iter()
            .flat_map(|analysis| analysis.function(function.id).constants())
            .map(|(value, constant)| (value, constant.clone()))
            .collect::<AHashMap<ValueId, ConstValue>>();
        let array_lengths = stable_array_lengths(
            function,
            &parameter_array_lengths[function.id.0 as usize],
            &effect_summaries,
            &array_argument_retention_barriers,
        );
        let typed_array_lengths = fixed_typed_array_lengths(function);
        let mut local_change = true;
        while local_change {
            local_change = false;
            let mut js_typeof = AHashMap::<ValueId, &'static str>::default();
            let mut empty_plain = AHashSet::<ValueId>::default();
            let mut empty_arrays = AHashSet::<ValueId>::default();
            let mut mutated_objects = AHashSet::<ValueId>::default();
            for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
                match &instruction.op {
                    ControlFlowOp::IndexSet { object, .. }
                    | ControlFlowOp::HostFieldSet { object, .. }
                    | ControlFlowOp::RecordFieldSet { object, .. }
                    | ControlFlowOp::FieldSet { object, .. } => {
                        mutated_objects.insert(*object);
                    }
                    _ => {}
                }
                let Some(out) = instruction.out else {
                    continue;
                };
                if let Some(name) = javascript_typeof_name(&instruction.op) {
                    js_typeof.insert(out, name);
                }
                match &instruction.op {
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::JsPlainObject,
                        args,
                        ..
                    } if args.is_empty() => {
                        empty_plain.insert(out);
                    }
                    ControlFlowOp::Array(values) if values.is_empty() => {
                        empty_arrays.insert(out);
                    }
                    _ => {}
                }
            }
            for object in &mutated_objects {
                empty_plain.remove(object);
                empty_arrays.remove(object);
            }
            for block in &mut function.blocks {
                for phi in &block.phis {
                    let values = phi
                        .incoming
                        .iter()
                        .filter_map(|(_, value)| constants.get(value))
                        .collect::<Vec<_>>();
                    if values.len() == phi.incoming.len()
                        && values
                            .first()
                            .is_some_and(|first| values.iter().all(|value| *value == *first))
                    {
                        let value = values[0].clone();
                        if constants.insert(phi.out, value).is_none() {
                            local_change = true;
                        }
                    }
                }

                for instruction in &mut block.instructions {
                    let Some(out) = instruction.out else {
                        continue;
                    };
                    let folded_type_check = if let ControlFlowOp::TypeCheck {
                        value: input,
                        target,
                    } = &instruction.op
                    {
                        fold_known_js_type_check(js_typeof.get(input).copied(), target)
                    } else {
                        None
                    };
                    if let Some(value) = folded_type_check {
                        let folded = ConstValue::Bool(value);
                        instruction.op = ControlFlowOp::Const(folded.clone());
                        constants.insert(out, folded);
                        js_typeof.insert(out, "boolean");
                        changed = true;
                        local_change = true;
                        continue;
                    }
                    match &instruction.op {
                        ControlFlowOp::Const(value) => {
                            if constants.insert(out, value.clone()).is_none() {
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Array(values) => {
                            let _ = values;
                        }
                        ControlFlowOp::Unary { op, value } => {
                            if let Some(folded) = constants
                                .get(value)
                                .and_then(|value| fold_unary(*op, value))
                            {
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Binary { op, lhs, rhs } => {
                            if let Some(folded) = constants
                                .get(lhs)
                                .zip(constants.get(rhs))
                                .and_then(|(lhs, rhs)| fold_binary(*op, lhs, rhs))
                            {
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::ArrayLength,
                            receiver: Some(receiver),
                            ..
                        } => {
                            if let Some(length) = array_lengths.get(receiver).copied() {
                                let folded = ConstValue::Int(length as i64);
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic,
                            receiver: Some(receiver),
                            ..
                        } if matches!(
                            crate::typed_array::classify_typed_array_intrinsic(*intrinsic),
                            Some((_, crate::typed_array::TypedArrayIntrinsic::Length))
                        ) =>
                        {
                            if let Some(length) = typed_array_lengths.get(receiver).copied() {
                                let folded = ConstValue::Int(length as i64);
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::IntImul,
                            receiver: None,
                            args,
                        } => {
                            let folded =
                                args.as_slice()
                                    .first()
                                    .zip(args.get(1))
                                    .and_then(|(lhs, rhs)| {
                                        match (constants.get(lhs), constants.get(rhs)) {
                                            (
                                                Some(ConstValue::Int(lhs)),
                                                Some(ConstValue::Int(rhs)),
                                            ) => Some(ConstValue::Int(i64::from(
                                                (*lhs as i32).wrapping_mul(*rhs as i32),
                                            ))),
                                            _ => None,
                                        }
                                    });
                            if let Some(folded) = folded {
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic:
                                intrinsic @ (Intrinsic::IntToString | Intrinsic::IntToUnsignedString),
                            receiver: Some(receiver),
                            args,
                        } => {
                            let folded = constants.get(receiver).and_then(|receiver| {
                                let ConstValue::Int(value) = receiver else {
                                    return None;
                                };
                                let radix = args
                                    .first()
                                    .map(|argument| constants.get(argument))
                                    .unwrap_or(Some(&ConstValue::Int(10)));
                                let Some(ConstValue::Int(radix @ 2..=36)) = radix else {
                                    return None;
                                };
                                Some(ConstValue::String(format_i32_radix(
                                    *value as i32,
                                    *radix as u32,
                                    matches!(intrinsic, Intrinsic::IntToUnsignedString),
                                )))
                            });
                            if let Some(folded) = folded {
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::FloatToInt,
                            receiver: Some(receiver),
                            ..
                        } => {
                            if let Some(ConstValue::Float(value)) = constants.get(receiver) {
                                let folded = ConstValue::Int(i64::from(js_to_i32(*value)));
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic:
                                intrinsic @ (Intrinsic::FloatAbs
                                | Intrinsic::FloatFloor
                                | Intrinsic::FloatCeil
                                | Intrinsic::FloatRound
                                | Intrinsic::FloatSqrt
                                | Intrinsic::FloatSin
                                | Intrinsic::FloatCos
                                | Intrinsic::FloatAcos
                                | Intrinsic::FloatExp
                                | Intrinsic::FloatLog
                                | Intrinsic::FloatTan
                                | Intrinsic::FloatAtan2
                                | Intrinsic::FloatHypot
                                | Intrinsic::FloatMin
                                | Intrinsic::FloatMax),
                            receiver: Some(receiver),
                            args,
                        } => {
                            let folded = constants.get(receiver).and_then(|receiver| {
                                let ConstValue::Float(value) = receiver else {
                                    return None;
                                };
                                let argument = || {
                                    args.first()
                                        .and_then(|argument| constants.get(argument))
                                        .and_then(|argument| match argument {
                                            ConstValue::Float(value) => Some(*value),
                                            _ => None,
                                        })
                                };
                                Some(ConstValue::Float(match intrinsic {
                                    Intrinsic::FloatAbs => value.abs(),
                                    Intrinsic::FloatFloor => value.floor(),
                                    Intrinsic::FloatCeil => value.ceil(),
                                    Intrinsic::FloatRound => js_round(*value),
                                    Intrinsic::FloatSqrt => value.sqrt(),
                                    Intrinsic::FloatSin => value.sin(),
                                    Intrinsic::FloatCos => value.cos(),
                                    Intrinsic::FloatAcos => value.acos(),
                                    Intrinsic::FloatExp => value.exp(),
                                    Intrinsic::FloatLog => value.ln(),
                                    Intrinsic::FloatTan => value.tan(),
                                    Intrinsic::FloatAtan2 => value.atan2(argument()?),
                                    Intrinsic::FloatHypot => value.hypot(argument()?),
                                    Intrinsic::FloatMin => js_min(*value, argument()?),
                                    Intrinsic::FloatMax => js_max(*value, argument()?),
                                    _ => unreachable!(),
                                }))
                            });
                            if let Some(folded) = folded {
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic,
                            receiver: Some(receiver),
                            args,
                        } if matches!(
                            intrinsic,
                            Intrinsic::StringLength
                                | Intrinsic::StringCharCodeAt
                                | Intrinsic::StringCharAt
                                | Intrinsic::StringIncludes
                                | Intrinsic::StringIndexOf
                                | Intrinsic::StringLastIndexOf
                                | Intrinsic::StringRepeat
                                | Intrinsic::StringStartsWith
                                | Intrinsic::StringEndsWith
                                | Intrinsic::StringToUpperCase
                                | Intrinsic::StringToLowerCase
                                | Intrinsic::StringTrim
                                | Intrinsic::StringTrimStart
                                | Intrinsic::StringTrimEnd
                                | Intrinsic::StringSlice
                                | Intrinsic::StringSplit
                                | Intrinsic::StringCodePointLength
                        ) =>
                        {
                            let folded = constants.get(receiver).and_then(|receiver| {
                                fold_string_intrinsic(*intrinsic, receiver, args, &constants)
                            });
                            if let Some(folded) = folded {
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::JsTypeOf,
                            receiver: Some(receiver),
                            ..
                        } => {
                            if let Some(name) = js_typeof.get(receiver).copied() {
                                let folded = ConstValue::String(name.to_string());
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                js_typeof.insert(out, "string");
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::JsIsFunctionValue,
                            receiver: Some(receiver),
                            ..
                        } => {
                            if let Some(name) = js_typeof.get(receiver).copied() {
                                let folded = ConstValue::Bool(name == "function");
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                js_typeof.insert(out, "boolean");
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::JsIsWindowValue,
                            receiver: Some(receiver),
                            ..
                        } => {
                            let folded = if matches!(
                                js_typeof.get(receiver).copied(),
                                Some("undefined" | "string" | "number" | "boolean" | "function")
                            ) || empty_plain.contains(receiver)
                                || empty_arrays.contains(receiver)
                            {
                                Some(false)
                            } else {
                                None
                            };
                            if let Some(value) = folded {
                                let folded = ConstValue::Bool(value);
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                js_typeof.insert(out, "boolean");
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::JsIsNullish,
                            receiver: Some(receiver),
                            ..
                        } => {
                            let folded =
                                if matches!(constants.get(receiver), Some(ConstValue::Null))
                                    || js_typeof.get(receiver).copied() == Some("undefined")
                                {
                                    Some(true)
                                } else if matches!(
                                    js_typeof.get(receiver).copied(),
                                    Some("string" | "number" | "boolean" | "function")
                                ) || empty_plain.contains(receiver)
                                    || empty_arrays.contains(receiver)
                                {
                                    Some(false)
                                } else {
                                    None
                                };
                            if let Some(value) = folded {
                                let folded = ConstValue::Bool(value);
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                js_typeof.insert(out, "boolean");
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Template(parts) => {
                            let mut folded_parts = Vec::with_capacity(parts.len());
                            let mut changed_parts = false;
                            for part in parts {
                                match part {
                                    TemplateOperand::String(value) => {
                                        push_template_string(&mut folded_parts, value);
                                    }
                                    TemplateOperand::Value(value) => {
                                        if let Some(value) =
                                            constants.get(value).and_then(constant_string_value)
                                        {
                                            push_template_string(&mut folded_parts, &value);
                                            changed_parts = true;
                                        } else {
                                            folded_parts.push(TemplateOperand::Value(*value));
                                        }
                                    }
                                }
                            }
                            if folded_parts
                                .iter()
                                .all(|part| matches!(part, TemplateOperand::String(_)))
                            {
                                let mut value = String::new();
                                for part in &folded_parts {
                                    if let TemplateOperand::String(part) = part {
                                        value.push_str(part);
                                    }
                                }
                                let folded = ConstValue::String(value);
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            } else if changed_parts || folded_parts.len() != parts.len() {
                                instruction.op = ControlFlowOp::Template(folded_parts);
                                changed = true;
                                local_change = true;
                            }
                        }
                        _ => {}
                    }
                }

                if let Some(Terminator::Branch {
                    condition,
                    then_block,
                    else_block,
                }) = block.terminator.clone()
                {
                    if let Some(ConstValue::Bool(condition)) = constants.get(&condition) {
                        let selected = if *condition { then_block } else { else_block };
                        // Keep a proven-infinite structured loop as a branch. Removing its
                        // exit edge also removes the loop shape, forcing JavaScript emission
                        // into a CFG state machine for an ordinary `while (true)` loop.
                        if loop_bodies.get(&block.id) == Some(&selected) {
                            continue;
                        }
                        block.terminator = Some(Terminator::Jump(selected));
                        changed = true;
                        local_change = true;
                    }
                }
            }
        }
    }
    OptimizationReport {
        pass_name: "constant-propagation",
        changed,
    }
}

/// Lengths of typed arrays backed by buffers created inside typed code are
/// immutable: writes change elements, not the view extent. Values crossing an
/// untyped boundary are excluded because host code could detach their buffer.
pub(crate) fn fixed_typed_array_lengths(
    function: &ControlFlowFunction<'_>,
) -> AHashMap<ValueId, usize> {
    let definitions = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| instruction.out.map(|out| (out, &instruction.op)))
        .collect::<AHashMap<_, _>>();
    let int_constant = |value: ValueId| match definitions.get(&value) {
        Some(ControlFlowOp::Const(ConstValue::Int(value))) => usize::try_from(*value).ok(),
        _ => None,
    };
    let mut buffer_lengths = AHashMap::<ValueId, usize>::default();
    let mut typed_lengths = AHashMap::<ValueId, usize>::default();
    loop {
        let mut changed = false;
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            let Some(out) = instruction.out else {
                continue;
            };
            if function.value_escapes.get(out.0 as usize)
                == Some(&EscapeState::EscapesToUntypedBoundary)
            {
                continue;
            }
            let ControlFlowOp::Intrinsic {
                intrinsic,
                receiver,
                args,
            } = &instruction.op
            else {
                continue;
            };
            if matches!(
                intrinsic,
                Intrinsic::ArrayBufferNew | Intrinsic::SharedArrayBufferNew
            ) && args.len() == 1
            {
                if let Some(length) = int_constant(args[0]) {
                    changed |= buffer_lengths.insert(out, length).is_none();
                }
                continue;
            }
            let Some((kind, operation)) =
                crate::typed_array::classify_typed_array_intrinsic(*intrinsic)
            else {
                continue;
            };
            let length = match operation {
                crate::typed_array::TypedArrayIntrinsic::New if args.len() == 1 => {
                    int_constant(args[0]).or_else(|| {
                        buffer_lengths
                            .get(&args[0])
                            .copied()
                            .filter(|bytes| bytes % kind.bytes_per_element() as usize == 0)
                            .map(|bytes| bytes / kind.bytes_per_element() as usize)
                    })
                }
                crate::typed_array::TypedArrayIntrinsic::Subarray => {
                    let receiver = receiver.and_then(|value| typed_lengths.get(&value).copied());
                    receiver.and_then(|receiver_length| {
                        let start = args.first().and_then(|value| int_constant(*value))?;
                        let end = args
                            .get(1)
                            .and_then(|value| int_constant(*value))
                            .unwrap_or(receiver_length);
                        let start = start.min(receiver_length);
                        let end = end.min(receiver_length);
                        Some(end.saturating_sub(start))
                    })
                }
                _ => None,
            };
            if let Some(length) = length {
                changed |= typed_lengths.insert(out, length).is_none();
            }
        }
        if !changed {
            break;
        }
    }
    typed_lengths
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ArrayLengthFact {
    #[default]
    Bottom,
    Exact(usize),
    Unknown,
}

impl ArrayLengthFact {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Bottom, value) | (value, Self::Bottom) => value,
            (Self::Exact(left), Self::Exact(right)) if left == right => Self::Exact(left),
            _ => Self::Unknown,
        }
    }
}

fn analyze_array_parameter_lengths(
    module: &ControlFlowModule<'_>,
    effect_summaries: &[FunctionEffectSummary],
) -> Vec<Vec<ArrayLengthFact>> {
    let retention_barriers = array_argument_retention_barriers(module);
    let exported = exported_functions(module);
    let indirect = module
        .functions
        .iter()
        .flat_map(|function| function.blocks.iter())
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction.op {
            ControlFlowOp::Closure { function, .. } => Some(function),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let boundary_facts = || {
        module
            .functions
            .iter()
            .map(|function| {
                function
                    .params
                    .iter()
                    .map(|parameter| {
                        if matches!(parameter.ty, Type::Array(_))
                            && (function.kind == FunctionKind::Extern
                                || exported.contains(&function.id)
                                || indirect.contains(&function.id))
                        {
                            ArrayLengthFact::Unknown
                        } else {
                            ArrayLengthFact::Bottom
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let mut parameter_facts = boundary_facts();
    loop {
        let function_lengths = module
            .functions
            .iter()
            .map(|function| {
                stable_array_lengths(
                    function,
                    &parameter_facts[function.id.0 as usize],
                    effect_summaries,
                    &retention_barriers,
                )
            })
            .collect::<Vec<_>>();
        let mut proposed = boundary_facts();
        for caller in &module.functions {
            let caller_lengths = &function_lengths[caller.id.0 as usize];
            for instruction in caller.blocks.iter().flat_map(|block| &block.instructions) {
                let (callee, arguments) = match &instruction.op {
                    ControlFlowOp::CallDirect { function, args, .. } => (*function, args.clone()),
                    ControlFlowOp::CallMethod {
                        receiver,
                        function,
                        args,
                        ..
                    } => {
                        let mut values = vec![*receiver];
                        values.extend(args);
                        (*function, values)
                    }
                    ControlFlowOp::NewClass {
                        constructor: Some(function),
                        args,
                        ..
                    } => {
                        let mut values = instruction.out.into_iter().collect::<Vec<_>>();
                        values.extend(args);
                        (*function, values)
                    }
                    _ => continue,
                };
                let Some(callee_function) = module.functions.get(callee.0 as usize) else {
                    continue;
                };
                for (index, (argument, parameter)) in
                    arguments.iter().zip(&callee_function.params).enumerate()
                {
                    if !matches!(parameter.ty, Type::Array(_)) {
                        continue;
                    }
                    let incoming = caller_lengths
                        .get(argument)
                        .copied()
                        .map_or(ArrayLengthFact::Unknown, ArrayLengthFact::Exact);
                    proposed[callee.0 as usize][index] =
                        proposed[callee.0 as usize][index].join(incoming);
                }
            }
        }
        if proposed == parameter_facts {
            return parameter_facts;
        }
        parameter_facts = proposed;
    }
}

fn stable_array_lengths(
    function: &ControlFlowFunction<'_>,
    parameter_lengths: &[ArrayLengthFact],
    effect_summaries: &[FunctionEffectSummary],
    retention_barriers: &[bool],
) -> AHashMap<ValueId, usize> {
    let mut candidates = function
        .params
        .iter()
        .zip(parameter_lengths)
        .filter_map(|(parameter, length)| match length {
            ArrayLengthFact::Exact(length) => Some((parameter.value, *length)),
            ArrayLengthFact::Bottom | ArrayLengthFact::Unknown => None,
        })
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| match (instruction.out, &instruction.op) {
                    (Some(out), ControlFlowOp::Array(values)) => Some((out, values.len())),
                    _ => None,
                }),
        )
        .collect::<AHashMap<_, _>>();
    if candidates.is_empty() {
        return candidates;
    }
    let mut dependencies = AHashMap::<ValueId, ValueId>::default();
    loop {
        let mut changed = false;
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            let (
                Some(out),
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayMap,
                    receiver: Some(receiver),
                    ..
                },
            ) = (instruction.out, &instruction.op)
            else {
                continue;
            };
            if let Some(length) = candidates.get(receiver).copied() {
                if candidates.insert(out, length).is_none() {
                    dependencies.insert(out, *receiver);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    let mut invalid = AHashSet::default();
    let invalidate = |value: ValueId, invalid: &mut AHashSet<ValueId>| {
        if candidates.contains_key(&value) {
            invalid.insert(value);
        }
    };
    for block in &function.blocks {
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                invalidate(*value, &mut invalid);
            }
        }
        for instruction in &block.instructions {
            match &instruction.op {
                ControlFlowOp::Array(values) | ControlFlowOp::Struct { fields: values, .. } => {
                    for value in values {
                        invalidate(*value, &mut invalid);
                    }
                }
                ControlFlowOp::ArraySpread(operands) => {
                    for value in operands.iter().map(|operand| match operand {
                        ArrayOperand::Value(value) | ArrayOperand::Spread(value) => value,
                    }) {
                        invalidate(*value, &mut invalid);
                    }
                }
                ControlFlowOp::RecordSpread(operands) => {
                    for value in operands.iter().map(|operand| match operand {
                        RecordOperand::Entry(_, value) | RecordOperand::Spread(value) => value,
                    }) {
                        invalidate(*value, &mut invalid);
                    }
                }
                ControlFlowOp::NewClass { args, .. } => {
                    for value in args {
                        invalidate(*value, &mut invalid);
                    }
                }
                ControlFlowOp::CallDirect { function, args, .. } => {
                    invalidate_direct_call_array_arguments(
                        *function,
                        args,
                        effect_summaries,
                        retention_barriers,
                        &mut invalid,
                        &invalidate,
                    );
                }
                ControlFlowOp::Closure { captures, .. } => {
                    for value in captures {
                        invalidate(*value, &mut invalid);
                    }
                }
                ControlFlowOp::StoreGlobal { value, .. } => invalidate(*value, &mut invalid),
                ControlFlowOp::FieldSet { object, value, .. }
                | ControlFlowOp::HostFieldSet { object, value, .. } => {
                    invalidate(*object, &mut invalid);
                    invalidate(*value, &mut invalid);
                }
                ControlFlowOp::HostFieldGet { object, .. } => invalidate(*object, &mut invalid),
                ControlFlowOp::IndexSet {
                    object,
                    index,
                    value,
                } => {
                    invalidate(*object, &mut invalid);
                    invalidate(*index, &mut invalid);
                    invalidate(*value, &mut invalid);
                }
                ControlFlowOp::CallValue { callee, args } => {
                    invalidate(*callee, &mut invalid);
                    for value in args {
                        invalidate(*value, &mut invalid);
                    }
                }
                ControlFlowOp::CallMethod {
                    receiver,
                    function,
                    args,
                    ..
                } => {
                    let mut arguments = vec![*receiver];
                    arguments.extend(args);
                    invalidate_direct_call_array_arguments(
                        *function,
                        &arguments,
                        effect_summaries,
                        retention_barriers,
                        &mut invalid,
                        &invalidate,
                    );
                }
                ControlFlowOp::HostCall { receiver, args, .. } => {
                    invalidate(*receiver, &mut invalid);
                    for value in args {
                        invalidate(*value, &mut invalid);
                    }
                }
                ControlFlowOp::Intrinsic {
                    intrinsic,
                    receiver,
                    args,
                } => {
                    if matches!(
                        intrinsic,
                        Intrinsic::Print
                            | Intrinsic::ArrayPush
                            | Intrinsic::ArrayPop
                            | Intrinsic::ArraySplice
                    ) {
                        if let Some(receiver) = receiver {
                            invalidate(*receiver, &mut invalid);
                        }
                    }
                    for value in args {
                        invalidate(*value, &mut invalid);
                    }
                }
                _ => {}
            }
        }
        if let Some(Terminator::Return(Some(value))) = block.terminator {
            invalidate(value, &mut invalid);
        }
    }
    loop {
        let old_len = invalid.len();
        invalid.extend(
            dependencies
                .iter()
                .filter_map(|(value, source)| invalid.contains(source).then_some(*value))
                .collect::<Vec<_>>(),
        );
        if invalid.len() == old_len {
            break;
        }
    }
    candidates
        .into_iter()
        .filter(|(value, _)| !invalid.contains(value))
        .collect()
}

fn invalidate_direct_call_array_arguments(
    function: FunctionId,
    args: &[ValueId],
    effect_summaries: &[FunctionEffectSummary],
    retention_barriers: &[bool],
    invalid: &mut AHashSet<ValueId>,
    invalidate: &impl Fn(ValueId, &mut AHashSet<ValueId>),
) {
    let summary = effect_summaries.get(function.0 as usize);
    let may_retain_arguments = retention_barriers
        .get(function.0 as usize)
        .copied()
        .unwrap_or(true);
    for (index, argument) in args.iter().enumerate() {
        if may_retain_arguments
            || summary.is_none_or(|summary| {
                summary.inherent || summary.mutated_parameters.contains(&index)
            })
        {
            invalidate(*argument, invalid);
        }
    }
}

fn array_argument_retention_barriers(module: &ControlFlowModule<'_>) -> Vec<bool> {
    module
        .functions
        .iter()
        .map(|function| {
            function.kind == FunctionKind::Extern || type_can_carry_reference(&function.return_type)
        })
        .collect()
}

fn type_can_carry_reference(ty: &Type<'_>) -> bool {
    match ty {
        Type::Int
        | Type::Enum(_)
        | Type::Float
        | Type::String
        | Type::Bool
        | Type::Null
        | Type::Void => false,
        Type::Nullable(inner) => type_can_carry_reference(inner),
        Type::Union(members) => members.iter().any(type_can_carry_reference),
        Type::Array(_)
        | Type::Record(_)
        | Type::Map(_, _)
        | Type::Set(_)
        | Type::ArrayBuffer
        | Type::SharedArrayBuffer
        | Type::Uint8Array
        | Type::Int8Array
        | Type::Uint8ClampedArray
        | Type::Int16Array
        | Type::Uint16Array
        | Type::Int32Array
        | Type::Uint32Array
        | Type::Float32Array
        | Type::Float64Array
        | Type::Symbol
        | Type::Regex
        | Type::Task(_)
        | Type::Generator(_)
        | Type::ModuleNamespace(_)
        | Type::ModuleLoadError
        | Type::Struct(_)
        | Type::Class(_)
        | Type::StructInstance { .. }
        | Type::ClassInstance { .. }
        | Type::TypeParameter(_)
        | Type::Function(_)
        | Type::GenericFunction(_) => true,
    }
}

fn remove_unreachable_control_flow(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        if function
            .shapes
            .iter()
            .any(|shape| matches!(shape, crate::ir::ControlShape::Try { .. }))
        {
            continue;
        }
        let reachable = reachable_blocks(function);
        if reachable.len() == function.blocks.len() {
            continue;
        }
        let mut mapping = vec![None; function.blocks.len()];
        let mut next = 0u32;
        for (old, mapped) in mapping.iter_mut().enumerate() {
            if reachable.contains(&old) {
                *mapped = Some(BlockId(next));
                next += 1;
            }
        }
        let mut blocks = Vec::with_capacity(reachable.len());
        for (old, mut block) in std::mem::take(&mut function.blocks).into_iter().enumerate() {
            let Some(new_id) = mapping[old] else {
                continue;
            };
            block.id = new_id;
            for phi in &mut block.phis {
                phi.incoming.retain_mut(|(incoming, _)| {
                    if let Some(mapped) = mapping[incoming.0 as usize] {
                        *incoming = mapped;
                        true
                    } else {
                        false
                    }
                });
            }
            if let Some(terminator) = &mut block.terminator {
                remap_terminator_blocks(terminator, &mapping);
            }
            blocks.push(block);
        }
        function.entry = mapping[function.entry.0 as usize]
            .expect("the function entry block is always reachable");
        function.shapes.retain_mut(|shape| match shape {
            crate::ir::ControlShape::If {
                header,
                then_block,
                else_block,
                merge_block,
            } => {
                let mapped = [*header, *then_block, *else_block, *merge_block]
                    .map(|block| mapping[block.0 as usize]);
                if let [Some(new_header), Some(new_then), Some(new_else), Some(new_merge)] = mapped
                {
                    *header = new_header;
                    *then_block = new_then;
                    *else_block = new_else;
                    *merge_block = new_merge;
                    true
                } else {
                    false
                }
            }
            crate::ir::ControlShape::Loop {
                header,
                body,
                update,
                exit,
            } => {
                let Some(new_header) = mapping[header.0 as usize] else {
                    return false;
                };
                let Some(new_body) = mapping[body.0 as usize] else {
                    return false;
                };
                let Some(new_exit) = mapping[exit.0 as usize] else {
                    return false;
                };
                let new_update = match *update {
                    Some(block) => match mapping[block.0 as usize] {
                        Some(mapped) => Some(mapped),
                        None => return false,
                    },
                    None => None,
                };
                *header = new_header;
                *body = new_body;
                *update = new_update;
                *exit = new_exit;
                true
            }
            crate::ir::ControlShape::ForIn {
                header, body, exit, ..
            }
            | crate::ir::ControlShape::ForOf {
                header, body, exit, ..
            } => {
                let [Some(new_header), Some(new_body), Some(new_exit)] =
                    [*header, *body, *exit].map(|block| mapping[block.0 as usize])
                else {
                    return false;
                };
                *header = new_header;
                *body = new_body;
                *exit = new_exit;
                true
            }
            crate::ir::ControlShape::Try {
                header,
                body,
                catch_block,
                finally_block,
                merge_block,
                ..
            } => {
                let Some(new_header) = mapping[header.0 as usize] else {
                    return false;
                };
                let Some(new_body) = mapping[body.0 as usize] else {
                    return false;
                };
                let new_catch = match *catch_block {
                    Some(block) => match mapping[block.0 as usize] {
                        Some(mapped) => Some(mapped),
                        None => return false,
                    },
                    None => None,
                };
                let new_finally = match *finally_block {
                    Some(block) => match mapping[block.0 as usize] {
                        Some(mapped) => Some(mapped),
                        None => return false,
                    },
                    None => None,
                };
                let Some(new_merge) = mapping[merge_block.0 as usize] else {
                    return false;
                };
                *header = new_header;
                *body = new_body;
                *catch_block = new_catch;
                *finally_block = new_finally;
                *merge_block = new_merge;
                true
            }
        });
        function.blocks = blocks;
        changed = true;
    }
    OptimizationReport {
        pass_name: "unreachable-block-elimination",
        changed,
    }
}

fn remap_terminator_blocks(terminator: &mut Terminator, mapping: &[Option<BlockId>]) {
    match terminator {
        Terminator::Jump(target) => {
            *target = mapping[target.0 as usize].expect("reachable jump target must be mapped")
        }
        Terminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            *then_block =
                mapping[then_block.0 as usize].expect("reachable branch target must be mapped");
            *else_block =
                mapping[else_block.0 as usize].expect("reachable branch target must be mapped");
        }
        Terminator::Try { body, catch_block } => {
            *body = mapping[body.0 as usize].expect("reachable try body must be mapped");
            if let Some(block) = catch_block {
                *block = mapping[block.0 as usize].expect("reachable catch block must be mapped");
            }
        }
        _ => {}
    }
}

fn is_trivial_js_host_passthrough(function: &ControlFlowFunction<'_>) -> bool {
    if function.blocks.len() != 1 {
        return false;
    }
    let block = &function.blocks[0];
    if block.instructions.is_empty() || block.instructions.len() > 8 {
        return false;
    }
    let mut saw_host = false;
    for instruction in &block.instructions {
        match &instruction.op {
            ControlFlowOp::Const(_) | ControlFlowOp::LoadLocal(_) | ControlFlowOp::Unary { .. } => {
            }
            ControlFlowOp::IndexGet { .. }
            | ControlFlowOp::HostFieldGet { .. }
            | ControlFlowOp::RecordFieldGet { .. } => {
                saw_host = true;
            }
            ControlFlowOp::Binary {
                op: IrBinaryOp::Eq | IrBinaryOp::NotEq,
                ..
            } => {
                saw_host = true;
            }
            ControlFlowOp::Intrinsic {
                intrinsic:
                    Intrinsic::JsStringify
                    | Intrinsic::JsNumber
                    | Intrinsic::JsTypeOf
                    | Intrinsic::JsIsNullish
                    | Intrinsic::JsIsUndefined
                    | Intrinsic::JsIsFalse
                    | Intrinsic::JsStrictEqual
                    | Intrinsic::JsGetProperty
                    | Intrinsic::FloatToInt,
                ..
            } => {
                saw_host = true;
            }
            ControlFlowOp::Intrinsic {
                intrinsic:
                    Intrinsic::JsInvoke
                    | Intrinsic::JsCall
                    | Intrinsic::JsApply
                    | Intrinsic::JsConstruct,
                ..
            } => saw_host = true,
            _ => return false,
        }
    }
    saw_host
}

fn inline_small_functions(
    module: &mut ControlFlowModule<'_>,
    options: &OptimizationOptions,
    protected_callees: &AHashSet<FunctionId>,
) -> OptimizationReport {
    let recursive = recursive_functions(module);
    let exported = exported_functions(module);
    let mut call_counts = AHashMap::<FunctionId, usize>::default();
    let mut address_taken = AHashSet::<FunctionId>::default();
    let mut js_adapter_targets = AHashSet::<FunctionId>::default();
    for caller in module.functions.iter().filter(|function| function.live) {
        let closure_values = caller
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Closure { function, .. }) => Some((out, *function)),
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        for instruction in caller.blocks.iter().flat_map(|block| &block.instructions) {
            let ControlFlowOp::Intrinsic {
                intrinsic:
                    Intrinsic::JsMethod0
                    | Intrinsic::JsMethod1
                    | Intrinsic::JsMethod2
                    | Intrinsic::JsMethod3
                    | Intrinsic::JsMethodRest
                    | Intrinsic::JsStaticRest,
                receiver: None,
                args,
            } = &instruction.op
            else {
                continue;
            };
            if let [callback] = args.as_slice() {
                if let Some(function) = closure_values.get(callback) {
                    js_adapter_targets.insert(*function);
                }
            }
        }
    }
    for instruction in module
        .functions
        .iter()
        .filter(|function| function.live)
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
    {
        match instruction.op {
            ControlFlowOp::CallDirect { function, .. } => {
                *call_counts.entry(function).or_insert(0) += 1;
            }
            ControlFlowOp::NewClass {
                constructor: Some(function),
                ..
            } => {
                *call_counts.entry(function).or_insert(0) += 1;
            }
            ControlFlowOp::Closure { function, .. } => {
                address_taken.insert(function);
            }
            _ => {}
        }
    }
    let candidates = module
        .functions
        .iter()
        .filter(|function| {
            function.live
                && !matches!(function.kind, FunctionKind::Entry | FunctionKind::Extern)
                && function.mutable_capture_locals.is_empty()
                && !function.is_async
                && !function.is_generator
                && !recursive.contains(&function.id)
                && !exported.contains(&function.id)
                && !protected_callees.contains(&function.id)
                // Adapter fusion is an alternate JavaScript ABI. When the
                // same target also has a direct typed call, retain that call
                // in neutral IR so codegen can see both uses and keep the
                // zero-length, anonymous, constructible binder. Erasing the
                // direct use here incorrectly made a MethodRest target look
                // adapter-exclusive and exposed a promoted argument formal
                // through Function#length.
                && !(js_adapter_targets.contains(&function.id)
                    && call_counts.get(&function.id).copied().unwrap_or(0) != 0)
                && !function_has_type_parameters(function)
                && function.blocks.len() == 1
                && function.blocks[0].phis.is_empty()
                && (options.inline_closure_factories
                    || !function.blocks[0]
                        .instructions
                        .iter()
                        .any(|instruction| matches!(instruction.op, ControlFlowOp::Closure { .. })))
                && (is_trivial_js_host_passthrough(function)
                    || (function.blocks[0].instructions.len() <= options.inline_instruction_limit
                        && options.inline_growth_limit.is_none_or(|limit| {
                            let instructions = function.blocks[0].instructions.len();
                            let calls = call_counts.get(&function.id).copied().unwrap_or(0);
                            let retained_instructions = if address_taken.contains(&function.id) {
                                instructions
                            } else {
                                0
                            };
                            let before = instructions + calls;
                            let after = retained_instructions + instructions.saturating_mul(calls);
                            after.saturating_sub(before) <= limit
                        })))
                && matches!(function.blocks[0].terminator, Some(Terminator::Return(_)))
        })
        .map(|function| (function.id, function.clone()))
        .collect::<AHashMap<_, _>>();
    let mut changed = false;

    for caller in module.functions.iter_mut().filter(|function| function.live) {
        let mut aliases = AHashMap::<ValueId, ValueId>::default();
        for block in &mut caller.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut rewritten = Vec::with_capacity(instructions.len());
            for mut instruction in instructions {
                rewrite_control_flow_op(&mut instruction.op, &aliases);
                let constructor_call = match &instruction.op {
                    ControlFlowOp::NewClass {
                        class,
                        constructor: Some(constructor),
                        args,
                    } => Some((*class, *constructor, args.clone())),
                    _ => None,
                };
                if let Some((class, constructor, args)) = constructor_call {
                    let Some(object) = instruction.out else {
                        rewritten.push(instruction);
                        continue;
                    };
                    let Some(callee) = candidates.get(&constructor) else {
                        rewritten.push(instruction);
                        continue;
                    };
                    if callee.params.len() != args.len() + 1
                        || !matches!(callee.blocks[0].terminator, Some(Terminator::Return(None)))
                    {
                        rewritten.push(instruction);
                        continue;
                    }

                    let call_args = std::iter::once(object).chain(args);
                    let mut mapping = callee
                        .params
                        .iter()
                        .zip(call_args)
                        .filter_map(|(param, arg)| {
                            let arg = resolve_alias(arg, &aliases);
                            (param.value != arg).then_some((param.value, arg))
                        })
                        .collect::<AHashMap<_, _>>();
                    instruction.op = ControlFlowOp::NewClass {
                        class,
                        constructor: None,
                        args: Vec::new(),
                    };
                    rewritten.push(instruction);
                    for callee_instruction in &callee.blocks[0].instructions {
                        let mut cloned = callee_instruction.clone();
                        rewrite_control_flow_op_once(&mut cloned.op, &mapping);
                        if let Some(old_out) = cloned.out {
                            let new_out = ValueId(caller.value_count);
                            caller.value_count += 1;
                            let escape = callee
                                .value_escapes
                                .get(old_out.0 as usize)
                                .copied()
                                .unwrap_or(EscapeState::LocalOnly);
                            caller.value_escapes.push(escape);
                            caller.value_local_hints.push(
                                callee
                                    .value_local_hints
                                    .get(old_out.0 as usize)
                                    .copied()
                                    .flatten(),
                            );
                            mapping.insert(old_out, new_out);
                            cloned.out = Some(new_out);
                        }
                        rewritten.push(cloned);
                    }
                    changed = true;
                    continue;
                }
                let ControlFlowOp::CallDirect { function, args, .. } = &instruction.op else {
                    rewritten.push(instruction);
                    continue;
                };
                if *function == caller.id {
                    rewritten.push(instruction);
                    continue;
                }
                let Some(callee) = candidates.get(function) else {
                    rewritten.push(instruction);
                    continue;
                };
                if args.len() != callee.params.len() {
                    rewritten.push(instruction);
                    continue;
                }

                let mut mapping = callee
                    .params
                    .iter()
                    .zip(args)
                    .filter_map(|(param, arg)| {
                        let arg = resolve_alias(*arg, &aliases);
                        (param.value != arg).then_some((param.value, arg))
                    })
                    .collect::<AHashMap<_, _>>();
                for callee_instruction in &callee.blocks[0].instructions {
                    let mut cloned = callee_instruction.clone();
                    rewrite_control_flow_op_once(&mut cloned.op, &mapping);
                    if let Some(old_out) = cloned.out {
                        let new_out = ValueId(caller.value_count);
                        caller.value_count += 1;
                        let escape = callee
                            .value_escapes
                            .get(old_out.0 as usize)
                            .copied()
                            .unwrap_or(EscapeState::LocalOnly);
                        caller.value_escapes.push(escape);
                        caller.value_local_hints.push(
                            callee
                                .value_local_hints
                                .get(old_out.0 as usize)
                                .copied()
                                .flatten(),
                        );
                        mapping.insert(old_out, new_out);
                        cloned.out = Some(new_out);
                    }
                    rewritten.push(cloned);
                }

                if let (Some(call_out), Some(Terminator::Return(Some(returned)))) =
                    (instruction.out, &callee.blocks[0].terminator)
                {
                    aliases.insert(
                        call_out,
                        mapping.get(returned).copied().unwrap_or(*returned),
                    );
                }
                changed = true;
            }
            block.instructions = rewritten;
        }
        rewrite_control_flow_function(caller, &aliases);
    }

    OptimizationReport {
        pass_name: "inlining",
        changed,
    }
}

#[derive(Debug, Clone)]
enum DirectConstructorInitializer<'src> {
    Parameter(usize),
    Constant {
        value: ConstValue,
        ty: Type<'src>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
struct DirectConstructorField<'src> {
    owner: &'src str,
    field: &'src str,
    index: usize,
    value: DirectConstructorInitializer<'src>,
    span: Span,
}

#[derive(Debug, Clone)]
struct DirectConstructorSummary<'src> {
    class: &'src str,
    argument_count: usize,
    fields: Vec<DirectConstructorField<'src>>,
}

/// Whether replacing a class field's initial default with its constructor
/// value can remove only syntax-created literals. Host constructors and
/// factories are deliberately excluded: `Map`, `Symbol`, typed arrays, and
/// similar globals can be replaced by JavaScript consumers and observed.
pub(crate) fn javascript_class_default_is_pure_literal(ty: &Type<'_>) -> bool {
    match ty {
        Type::Map(_, _)
        | Type::Set(_)
        | Type::ArrayBuffer
        | Type::SharedArrayBuffer
        | Type::Int8Array
        | Type::Uint8Array
        | Type::Uint8ClampedArray
        | Type::Int16Array
        | Type::Uint16Array
        | Type::Int32Array
        | Type::Uint32Array
        | Type::Float32Array
        | Type::Float64Array
        | Type::Symbol
        | Type::Regex => false,
        Type::Union(members) => members
            .first()
            .is_none_or(javascript_class_default_is_pure_literal),
        _ => true,
    }
}

fn direct_constructor_summaries<'src>(
    module: &ControlFlowModule<'src>,
) -> AHashMap<FunctionId, DirectConstructorSummary<'src>> {
    module
        .functions
        .iter()
        .filter(|function| function.live)
        .filter_map(|function| {
            let FunctionKind::Constructor { class } = function.kind else {
                return None;
            };
            let layout = module
                .classes
                .iter()
                .find(|layout| layout.name == class && layout.base.is_none())?;
            if !layout
                .fields
                .iter()
                .all(|field| javascript_class_default_is_pure_literal(&field.ty))
            {
                return None;
            }
            let [block] = function.blocks.as_slice() else {
                return None;
            };
            if !block.phis.is_empty()
                || !function.shapes.is_empty()
                || function.capture_count != 0
                || function.is_async
                || function.is_generator
                || !matches!(block.terminator, Some(Terminator::Return(None)))
            {
                return None;
            }
            let receiver = function.params.first()?.value;
            let parameter_positions = function
                .params
                .iter()
                .enumerate()
                .skip(1)
                .map(|(index, parameter)| (parameter.value, index - 1))
                .collect::<AHashMap<_, _>>();
            let mut constants = AHashMap::<ValueId, (ConstValue, Type<'src>, Span)>::default();
            let mut fields = Vec::new();
            let mut next_layout_position = 0;
            let mut next_parameter_position = 0;
            let mut used_parameters = AHashSet::default();
            let mut used_constants = AHashSet::default();
            for instruction in &block.instructions {
                match (&instruction.op, instruction.out, instruction.ty.as_ref()) {
                    (ControlFlowOp::Const(value), Some(out), Some(ty)) => {
                        constants.insert(out, (value.clone(), ty.clone(), instruction.span));
                    }
                    (
                        ControlFlowOp::FieldSet {
                            object,
                            owner,
                            field,
                            index,
                            value,
                        },
                        None,
                        _,
                    ) if *object == receiver && *owner == class => {
                        let layout_position = layout.fields.iter().position(|candidate| {
                            candidate.index == *index && candidate.name == *field
                        })?;
                        if layout_position < next_layout_position
                            || !javascript_class_default_is_pure_literal(
                                &layout.fields[layout_position].ty,
                            )
                        {
                            return None;
                        }
                        next_layout_position = layout_position + 1;
                        let value = if let Some(position) = parameter_positions.get(value).copied()
                        {
                            if position < next_parameter_position {
                                return None;
                            }
                            next_parameter_position = position + 1;
                            used_parameters.insert(position);
                            DirectConstructorInitializer::Parameter(position)
                        } else {
                            let constant_id = *value;
                            let (value, ty, span) = constants.get(&constant_id)?.clone();
                            used_constants.insert(constant_id);
                            DirectConstructorInitializer::Constant { value, ty, span }
                        };
                        fields.push(DirectConstructorField {
                            owner,
                            field,
                            index: *index,
                            value,
                            span: instruction.span,
                        });
                    }
                    _ => return None,
                }
            }
            if fields.is_empty()
                || used_constants.len() != constants.len()
                || used_parameters.len() != function.params.len().saturating_sub(1)
            {
                return None;
            }
            Some((
                function.id,
                DirectConstructorSummary {
                    class,
                    argument_count: function.params.len().saturating_sub(1),
                    fields,
                },
            ))
        })
        .collect()
}

/// Project a narrowly proven constructor into the JavaScript class-literal
/// representation. The ordinary argument-producing instructions remain in
/// their original order. Mapped values become explicit IR uses, so normal
/// liveness and expression materialization see the true multiplicity before
/// code generation folds the field stores into the complete class literal.
pub(crate) fn project_direct_constructor_initializers_for_javascript(
    module: &mut ControlFlowModule<'_>,
) -> OptimizationReport {
    let summaries = direct_constructor_summaries(module);
    if summaries.is_empty() {
        return OptimizationReport {
            pass_name: "javascript-direct-constructor-fusion",
            changed: false,
        };
    }

    let mut changed = false;
    for caller in module.functions.iter_mut().filter(|function| function.live) {
        for block in &mut caller.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut rewritten = Vec::with_capacity(instructions.len());
            for mut instruction in instructions {
                let site = match (&instruction.op, instruction.out) {
                    (
                        ControlFlowOp::NewClass {
                            class,
                            constructor: Some(constructor),
                            args,
                        },
                        Some(object),
                    ) => summaries.get(constructor).and_then(|summary| {
                        (*class == summary.class && args.len() == summary.argument_count)
                            .then_some((object, args.clone(), summary))
                    }),
                    _ => None,
                };
                let Some((object, args, summary)) = site else {
                    rewritten.push(instruction);
                    continue;
                };

                let mut fields = Vec::with_capacity(summary.fields.len());
                for field in &summary.fields {
                    let value = match &field.value {
                        DirectConstructorInitializer::Parameter(position) => args[*position],
                        DirectConstructorInitializer::Constant { value, ty, span } => {
                            let out = ValueId(caller.value_count);
                            caller.value_count += 1;
                            caller.value_escapes.push(EscapeState::LocalOnly);
                            caller.value_local_hints.push(None);
                            rewritten.push(ControlFlowInstruction {
                                out: Some(out),
                                ty: Some(ty.clone()),
                                op: ControlFlowOp::Const(value.clone()),
                                span: *span,
                            });
                            out
                        }
                    };
                    fields.push((field, value));
                }
                instruction.op = ControlFlowOp::NewClass {
                    class: summary.class,
                    constructor: None,
                    args: Vec::new(),
                };
                rewritten.push(instruction);
                rewritten.extend(
                    fields
                        .into_iter()
                        .map(|(field, value)| ControlFlowInstruction {
                            out: None,
                            ty: None,
                            op: ControlFlowOp::FieldSet {
                                object,
                                owner: field.owner,
                                field: field.field,
                                index: field.index,
                                value,
                            },
                            span: field.span,
                        }),
                );
                changed = true;
            }
            block.instructions = rewritten;
        }
    }

    if changed {
        eliminate_dead_functions(module);
    }
    OptimizationReport {
        pass_name: "javascript-direct-constructor-fusion",
        changed,
    }
}

fn inline_single_use_control_flow_function(
    module: &mut ControlFlowModule<'_>,
    options: &OptimizationOptions,
    protected_callees: &AHashSet<FunctionId>,
) -> OptimizationReport {
    let recursive = recursive_functions(module);
    let exported = exported_functions(module);
    let mut call_counts = AHashMap::<FunctionId, usize>::default();
    let mut address_taken = AHashSet::<FunctionId>::default();
    for instruction in module
        .functions
        .iter()
        .filter(|function| function.live)
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
    {
        match instruction.op {
            ControlFlowOp::CallDirect { function, .. } => {
                *call_counts.entry(function).or_insert(0) += 1;
            }
            ControlFlowOp::Closure { function, .. } => {
                address_taken.insert(function);
            }
            _ => {}
        }
    }
    let candidates = module
        .functions
        .iter()
        .filter(|function| {
            function.live
                && matches!(
                    function.kind,
                    FunctionKind::Function | FunctionKind::Method { .. }
                )
                && function.mutable_capture_locals.is_empty()
                && !function.is_async
                && !function.is_generator
                && !function
                    .shapes
                    .iter()
                    .any(|shape| matches!(shape, crate::ir::ControlShape::Try { .. }))
                && !function.blocks.is_empty()
                && !function_has_type_parameters(function)
                && !recursive.contains(&function.id)
                && !exported.contains(&function.id)
                && !protected_callees.contains(&function.id)
                && !address_taken.contains(&function.id)
                && !function_has_structured_early_return(function)
                && (options.inline_closure_factories
                    || !function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.instructions)
                        .any(|instruction| matches!(instruction.op, ControlFlowOp::Closure { .. })))
                && call_counts.get(&function.id) == Some(&1)
                && function
                    .blocks
                    .iter()
                    .map(|block| block.instructions.len())
                    .sum::<usize>()
                    <= options.inline_control_flow_limit
        })
        .map(|function| (function.id, function.clone()))
        .collect::<AHashMap<_, _>>();

    let mut site = None;
    'functions: for (function_index, caller) in module.functions.iter().enumerate() {
        if !caller.live {
            continue;
        }
        let structured_interiors = structured_interior_blocks(caller);
        for (block_index, block) in caller.blocks.iter().enumerate() {
            if structured_interiors.contains(&block.id) {
                continue;
            }
            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                let ControlFlowOp::CallDirect { function, args, .. } = &instruction.op else {
                    continue;
                };
                if *function == caller.id || !candidates.contains_key(function) {
                    continue;
                }
                site = Some((
                    function_index,
                    block_index,
                    instruction_index,
                    *function,
                    args.clone(),
                    instruction.out,
                    instruction.ty.clone(),
                ));
                break 'functions;
            }
        }
    }

    let Some((caller_index, block_index, instruction_index, callee_id, args, out, ty)) = site
    else {
        return OptimizationReport {
            pass_name: "cfg-inlining",
            changed: false,
        };
    };
    let callee = &candidates[&callee_id];
    inline_control_flow_call(
        &mut module.functions[caller_index],
        block_index,
        instruction_index,
        callee,
        &args,
        out,
        ty,
    );
    OptimizationReport {
        pass_name: "cfg-inlining",
        changed: true,
    }
}

fn function_has_type_parameters(function: &ControlFlowFunction<'_>) -> bool {
    function
        .params
        .iter()
        .any(|parameter| type_has_type_parameter(&parameter.ty))
        || type_has_type_parameter(&function.return_type)
}

fn type_has_type_parameter(ty: &Type<'_>) -> bool {
    match ty {
        Type::TypeParameter("$js") => false,
        Type::TypeParameter(_) => true,
        Type::Array(element)
        | Type::Record(element)
        | Type::Task(element)
        | Type::Generator(element) => type_has_type_parameter(element),
        Type::Map(key, value) => type_has_type_parameter(key) || type_has_type_parameter(value),
        Type::Set(element) => type_has_type_parameter(element),
        Type::Nullable(inner) => type_has_type_parameter(inner),
        Type::Union(members) => members.iter().any(type_has_type_parameter),
        Type::StructInstance { args, .. } | Type::ClassInstance { args, .. } => {
            args.iter().any(type_has_type_parameter)
        }
        Type::Function(signature) => {
            signature.params.iter().any(type_has_type_parameter)
                || type_has_type_parameter(&signature.return_type)
        }
        Type::GenericFunction(_) => true,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn inline_control_flow_call<'src>(
    caller: &mut ControlFlowFunction<'src>,
    block_index: usize,
    instruction_index: usize,
    callee: &ControlFlowFunction<'src>,
    args: &[ValueId],
    call_out: Option<ValueId>,
    call_type: Option<Type<'src>>,
) {
    let insertion_block = caller.blocks[block_index].id;
    let shapes_compose_safely =
        callee.shapes.is_empty() || !structured_interior_blocks(caller).contains(&insertion_block);
    let base = caller.blocks.len() as u32;
    let block_mapping = callee
        .blocks
        .iter()
        .map(|block| (block.id, BlockId(base + block.id.0)))
        .collect::<AHashMap<_, _>>();
    let continuation = BlockId(base + callee.blocks.len() as u32);
    let mut value_mapping = callee
        .params
        .iter()
        .zip(args)
        .map(|(parameter, argument)| (parameter.value, *argument))
        .collect::<AHashMap<_, _>>();

    for block in &callee.blocks {
        for phi in &block.phis {
            allocate_inlined_value(caller, callee, phi.out, &mut value_mapping);
        }
        for instruction in &block.instructions {
            if let Some(out) = instruction.out {
                allocate_inlined_value(caller, callee, out, &mut value_mapping);
            }
        }
    }

    let (remaining, original_terminator, original_span) = {
        let block = &mut caller.blocks[block_index];
        let remaining = block.instructions.split_off(instruction_index + 1);
        block.instructions.pop();
        let original_terminator = block.terminator.take();
        block.terminator = Some(Terminator::Jump(block_mapping[&callee.entry]));
        (remaining, original_terminator, block.span)
    };

    let mut returns = Vec::<(BlockId, ValueId)>::new();
    for block in &callee.blocks {
        let id = block_mapping[&block.id];
        let phis = block
            .phis
            .iter()
            .map(|phi| Phi {
                out: value_mapping[&phi.out],
                origin: map_inlined_phi_origin(&phi.origin, &value_mapping),
                ty: phi.ty.clone(),
                incoming: phi
                    .incoming
                    .iter()
                    .map(|(incoming, value)| {
                        (
                            block_mapping[incoming],
                            mapped_value(*value, &value_mapping),
                        )
                    })
                    .collect(),
                span: phi.span,
            })
            .collect();
        let instructions = block
            .instructions
            .iter()
            .map(|instruction| {
                let mut instruction = instruction.clone();
                instruction.out = instruction.out.map(|out| value_mapping[&out]);
                rewrite_control_flow_op_once(&mut instruction.op, &value_mapping);
                instruction
            })
            .collect();
        let terminator = match block
            .terminator
            .as_ref()
            .expect("optimized callee blocks have terminators")
        {
            Terminator::Jump(target) => Terminator::Jump(block_mapping[target]),
            Terminator::Branch {
                condition,
                then_block,
                else_block,
            } => Terminator::Branch {
                condition: mapped_value(*condition, &value_mapping),
                then_block: block_mapping[then_block],
                else_block: block_mapping[else_block],
            },
            Terminator::Try { body, catch_block } => Terminator::Try {
                body: block_mapping[body],
                catch_block: catch_block.map(|block| block_mapping[&block]),
            },
            Terminator::Return(value) => {
                if let Some(value) = value {
                    returns.push((id, mapped_value(*value, &value_mapping)));
                }
                Terminator::Jump(continuation)
            }
            Terminator::Throw(value) => Terminator::Throw(mapped_value(*value, &value_mapping)),
            Terminator::Unreachable => Terminator::Unreachable,
        };
        caller.blocks.push(crate::ir::ControlFlowBlock {
            id,
            phis,
            instructions,
            terminator: Some(terminator),
            span: block.span,
        });
    }

    // Nested region metadata needs a full region-tree rewrite; use the CFG state machine meanwhile.
    if !shapes_compose_safely {
        caller.shapes.clear();
    }
    for shape in callee.shapes.iter().filter(|_| shapes_compose_safely) {
        caller.shapes.push(match shape {
            crate::ir::ControlShape::If {
                header,
                then_block,
                else_block,
                merge_block,
            } => crate::ir::ControlShape::If {
                header: block_mapping[header],
                then_block: block_mapping[then_block],
                else_block: block_mapping[else_block],
                merge_block: block_mapping[merge_block],
            },
            crate::ir::ControlShape::Loop {
                header,
                body,
                update,
                exit,
            } => crate::ir::ControlShape::Loop {
                header: block_mapping[header],
                body: block_mapping[body],
                update: update.map(|block| block_mapping[&block]),
                exit: block_mapping[exit],
            },
            crate::ir::ControlShape::ForIn {
                header,
                body,
                exit,
                object,
                key,
            } => crate::ir::ControlShape::ForIn {
                header: block_mapping[header],
                body: block_mapping[body],
                exit: block_mapping[exit],
                object: mapped_value(*object, &value_mapping),
                key: mapped_value(*key, &value_mapping),
            },
            crate::ir::ControlShape::ForOf {
                header,
                body,
                exit,
                iterable,
                element,
            } => crate::ir::ControlShape::ForOf {
                header: block_mapping[header],
                body: block_mapping[body],
                exit: block_mapping[exit],
                iterable: mapped_value(*iterable, &value_mapping),
                element: mapped_value(*element, &value_mapping),
            },
            crate::ir::ControlShape::Try {
                header,
                body,
                catch_block,
                finally_block,
                merge_block,
                catch_value,
            } => crate::ir::ControlShape::Try {
                header: block_mapping[header],
                body: block_mapping[body],
                catch_block: catch_block.map(|block| block_mapping[&block]),
                finally_block: finally_block.map(|block| block_mapping[&block]),
                merge_block: block_mapping[merge_block],
                catch_value: catch_value.map(|value| mapped_value(value, &value_mapping)),
            },
        });
    }

    let phis = call_out
        .zip(call_type)
        .map(|(out, ty)| {
            vec![Phi {
                out,
                origin: crate::ir::PhiOrigin::Synthetic,
                ty,
                incoming: returns,
                span: original_span,
            }]
        })
        .unwrap_or_default();
    caller.blocks.push(crate::ir::ControlFlowBlock {
        id: continuation,
        phis,
        instructions: remaining,
        terminator: original_terminator,
        span: original_span,
    });

    let continuation_successors = match caller
        .blocks
        .last()
        .and_then(|block| block.terminator.as_ref())
    {
        Some(Terminator::Jump(target)) => vec![*target],
        Some(Terminator::Branch {
            then_block,
            else_block,
            ..
        }) => vec![*then_block, *else_block],
        Some(Terminator::Try { body, catch_block }) => std::iter::once(*body)
            .chain(catch_block.iter().copied())
            .collect(),
        _ => Vec::new(),
    };
    for successor in continuation_successors {
        let Some(block) = caller.blocks.iter_mut().find(|block| block.id == successor) else {
            continue;
        };
        for phi in &mut block.phis {
            for (predecessor, _) in &mut phi.incoming {
                if *predecessor == insertion_block {
                    *predecessor = continuation;
                }
            }
        }
    }
}

fn structured_interior_blocks(function: &ControlFlowFunction<'_>) -> AHashSet<BlockId> {
    let mut blocks = AHashSet::default();
    for shape in &function.shapes {
        match shape {
            crate::ir::ControlShape::If {
                header,
                then_block,
                else_block,
                ..
            } => {
                blocks.extend([*header, *then_block, *else_block]);
            }
            crate::ir::ControlShape::Loop {
                header,
                body,
                update,
                ..
            } => {
                blocks.extend([*header, *body]);
                blocks.extend(update);
            }
            crate::ir::ControlShape::ForIn { header, body, .. } => {
                blocks.extend([*header, *body]);
            }
            crate::ir::ControlShape::ForOf { header, body, .. } => {
                blocks.extend([*header, *body]);
            }
            crate::ir::ControlShape::Try {
                header,
                body,
                catch_block,
                finally_block,
                ..
            } => {
                blocks.extend([*header, *body]);
                blocks.extend(catch_block);
                blocks.extend(finally_block);
            }
        }
    }
    blocks
}

fn function_has_structured_early_return(function: &ControlFlowFunction<'_>) -> bool {
    let interiors = structured_interior_blocks(function);
    function.blocks.iter().any(|block| {
        interiors.contains(&block.id) && matches!(block.terminator, Some(Terminator::Return(_)))
    })
}

fn allocate_inlined_value<'src>(
    caller: &mut ControlFlowFunction<'src>,
    callee: &ControlFlowFunction<'src>,
    old: ValueId,
    mapping: &mut AHashMap<ValueId, ValueId>,
) {
    if mapping.contains_key(&old) {
        return;
    }
    let new = ValueId(caller.value_count);
    caller.value_count += 1;
    caller.value_escapes.push(
        callee
            .value_escapes
            .get(old.0 as usize)
            .copied()
            .unwrap_or(EscapeState::LocalOnly),
    );
    caller.value_local_hints.push(
        callee
            .value_local_hints
            .get(old.0 as usize)
            .copied()
            .flatten(),
    );
    mapping.insert(old, new);
}

fn mapped_value(value: ValueId, mapping: &AHashMap<ValueId, ValueId>) -> ValueId {
    mapping.get(&value).copied().unwrap_or(value)
}

fn map_inlined_phi_origin(
    origin: &crate::ir::PhiOrigin,
    mapping: &AHashMap<ValueId, ValueId>,
) -> crate::ir::PhiOrigin {
    match origin {
        crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::ShortCircuit { op, lhs }) => {
            crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::ShortCircuit {
                op: *op,
                lhs: mapped_value(*lhs, mapping),
            })
        }
        crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Nullish { lhs }) => {
            crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Nullish {
                lhs: mapped_value(*lhs, mapping),
            })
        }
        crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::OptionalAccess { object }) => {
            crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::OptionalAccess {
                object: mapped_value(*object, mapping),
            })
        }
        crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Conditional) => {
            crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Conditional)
        }
        crate::ir::PhiOrigin::Local(_) | crate::ir::PhiOrigin::Synthetic => {
            crate::ir::PhiOrigin::Synthetic
        }
    }
}

fn recursive_functions(module: &ControlFlowModule<'_>) -> AHashSet<crate::ir::FunctionId> {
    let graph = module
        .functions
        .iter()
        .map(|function| {
            let callees = function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| match instruction.op {
                    ControlFlowOp::CallDirect { function, .. }
                    | ControlFlowOp::CallMethod { function, .. }
                    | ControlFlowOp::NewClass {
                        constructor: Some(function),
                        ..
                    } => Some(function),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (function.id, callees)
        })
        .collect::<AHashMap<_, _>>();
    let mut recursive = AHashSet::default();
    for start in graph.keys().copied() {
        let mut visited = AHashSet::default();
        let mut work = graph.get(&start).cloned().unwrap_or_default();
        while let Some(function) = work.pop() {
            if function == start {
                recursive.insert(start);
                break;
            }
            if visited.insert(function) {
                work.extend(graph.get(&function).into_iter().flatten().copied());
            }
        }
    }
    recursive
}

fn scalar_replace_linear_classes(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let layouts = module
        .classes
        .iter()
        .filter_map(|layout| {
            let fields = layout
                .fields
                .iter()
                .map(|field| {
                    default_scalar_constant(&field.ty).map(|value| (field.ty.clone(), value))
                })
                .collect::<Option<Vec<_>>>()?;
            Some((layout.name, fields))
        })
        .collect::<AHashMap<_, _>>();
    let mut changed = false;

    for function in &mut module.functions {
        let mut candidates = AHashMap::<ValueId, (usize, &str)>::default();
        for (block_index, block) in function.blocks.iter().enumerate() {
            for instruction in &block.instructions {
                if let (
                    Some(out),
                    ControlFlowOp::NewClass {
                        class,
                        constructor: None,
                        args,
                    },
                ) = (instruction.out, &instruction.op)
                {
                    if args.is_empty()
                        && layouts.contains_key(class)
                        && function.value_escapes[out.0 as usize] == EscapeState::LocalOnly
                    {
                        candidates.insert(out, (block_index, class));
                    }
                }
            }
        }

        let mut invalid = AHashSet::default();
        for (block_index, block) in function.blocks.iter().enumerate() {
            for instruction in &block.instructions {
                match &instruction.op {
                    ControlFlowOp::FieldGet { object, .. }
                    | ControlFlowOp::FieldSet { object, .. }
                        if candidates
                            .get(object)
                            .is_some_and(|(defined_in, _)| *defined_in == block_index) => {}
                    _ => {
                        for value in control_flow_used_values(&instruction.op) {
                            if candidates.contains_key(&value) {
                                invalid.insert(value);
                            }
                        }
                    }
                }
            }
            for value in block
                .terminator
                .as_ref()
                .into_iter()
                .flat_map(terminator_used_values)
            {
                if candidates.contains_key(&value) {
                    invalid.insert(value);
                }
            }
        }

        let mut aliases = AHashMap::<ValueId, ValueId>::default();
        for (block_index, block) in function.blocks.iter_mut().enumerate() {
            let instructions = std::mem::take(&mut block.instructions);
            let mut rewritten = Vec::with_capacity(instructions.len());
            let mut fields_by_object = AHashMap::<ValueId, Vec<ValueId>>::default();
            for mut instruction in instructions {
                rewrite_control_flow_op(&mut instruction.op, &aliases);
                match (&instruction.out, &instruction.op) {
                    (
                        Some(object),
                        ControlFlowOp::NewClass {
                            class,
                            constructor: None,
                            args,
                        },
                    ) if args.is_empty()
                        && candidates
                            .get(object)
                            .is_some_and(|(defined_in, _)| *defined_in == block_index)
                        && !invalid.contains(object) =>
                    {
                        let mut fields = Vec::new();
                        for (ty, value) in &layouts[class] {
                            let out = ValueId(function.value_count);
                            function.value_count += 1;
                            function.value_escapes.push(EscapeState::LocalOnly);
                            function.value_local_hints.push(None);
                            rewritten.push(ControlFlowInstruction {
                                out: Some(out),
                                ty: Some(ty.clone()),
                                op: ControlFlowOp::Const(value.clone()),
                                span: instruction.span,
                            });
                            fields.push(out);
                        }
                        fields_by_object.insert(*object, fields);
                        changed = true;
                    }
                    (
                        _,
                        ControlFlowOp::FieldSet {
                            object,
                            index,
                            value,
                            ..
                        },
                    ) if candidates.contains_key(object) && !invalid.contains(object) => {
                        if let Some(field) = fields_by_object
                            .get_mut(object)
                            .and_then(|fields| fields.get_mut(*index))
                        {
                            *field = resolve_alias(*value, &aliases);
                            changed = true;
                        } else {
                            rewritten.push(instruction);
                        }
                    }
                    (Some(out), ControlFlowOp::FieldGet { object, index, .. })
                        if candidates.contains_key(object) && !invalid.contains(object) =>
                    {
                        if let Some(value) = fields_by_object
                            .get(object)
                            .and_then(|fields| fields.get(*index))
                            .copied()
                        {
                            aliases.insert(*out, resolve_alias(value, &aliases));
                            changed = true;
                        } else {
                            rewritten.push(instruction);
                        }
                    }
                    _ => rewritten.push(instruction),
                }
            }
            block.instructions = rewritten;
        }
        rewrite_control_flow_function(function, &aliases);
    }

    OptimizationReport {
        pass_name: "class-scalar-replacement",
        changed,
    }
}

fn default_scalar_constant(ty: &Type<'_>) -> Option<ConstValue> {
    match ty {
        Type::Int => Some(ConstValue::Int(0)),
        Type::Float => Some(ConstValue::Float(0.0)),
        Type::Bool => Some(ConstValue::Bool(false)),
        Type::String => Some(ConstValue::String(String::new())),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EscapeNode {
    Value(FunctionId, ValueId),
    Local(FunctionId, LocalId),
    Global(SymbolId),
    AggregateField(u32),
}

fn aggregate_field_slot_ids<'src>(
    module: &ControlFlowModule<'src>,
) -> AHashMap<(&'src str, usize), u32> {
    let layouts = module
        .structs
        .iter()
        .chain(&module.classes)
        .map(|layout| (layout.name, layout))
        .collect::<AHashMap<_, _>>();
    let canonical_owner = |layout: &AggregateLayout<'src>, field: &AggregateField<'src>| {
        let mut owner = layout.name;
        let mut current = layout;
        while let Some(base) = current.base {
            let Some(base_layout) = layouts.get(base).copied() else {
                break;
            };
            let Some(base_field) = base_layout
                .fields
                .iter()
                .find(|candidate| candidate.index == field.index)
            else {
                break;
            };
            if base_field.name != field.name {
                break;
            }
            owner = base;
            current = base_layout;
        }
        owner
    };

    let mut canonical_slots = AHashMap::<(&'src str, usize), u32>::default();
    let mut slots = AHashMap::default();
    for layout in module.structs.iter().chain(&module.classes) {
        for field in &layout.fields {
            let key = (canonical_owner(layout, field), field.index);
            let next = canonical_slots.len() as u32;
            let slot = *canonical_slots.entry(key).or_insert(next);
            slots.insert((layout.name, field.index), slot);
        }
    }
    slots
}

pub(crate) fn analyze_escapes(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let extern_functions = module
        .functions
        .iter()
        .filter(|function| function.kind == FunctionKind::Extern)
        .map(|function| function.id)
        .collect::<AHashSet<_>>();
    let exported_functions = exported_functions(module);
    let exported_globals = exported_globals(module);
    let aggregate_field_slots = aggregate_field_slot_ids(module);
    let dynamic_aggregate_fields = module
        .structs
        .iter()
        .chain(&module.classes)
        .flat_map(|layout| {
            layout
                .fields
                .iter()
                .filter(|field| aggregate_field_erases_nominal_shape(&field.ty))
                .map(|field| (layout.name, field.index))
        })
        .collect::<AHashSet<_>>();
    let returns = module
        .functions
        .iter()
        .map(|function| {
            let mut values = function
                .blocks
                .iter()
                .filter_map(|block| match block.terminator {
                    Some(Terminator::Return(Some(value))) => Some(value),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if function.is_generator {
                values.extend(function.blocks.iter().flat_map(|block| {
                    block
                        .instructions
                        .iter()
                        .filter_map(|instruction| match instruction.op {
                            ControlFlowOp::Intrinsic {
                                intrinsic:
                                    Intrinsic::GeneratorYield | Intrinsic::GeneratorYieldDelegated,
                                receiver: Some(value),
                                ..
                            } => Some(value),
                            _ => None,
                        })
                }));
            }
            (function.id, values)
        })
        .collect::<AHashMap<_, _>>();
    let mut states = AHashMap::<EscapeNode, EscapeState>::default();
    let mut edges = AHashMap::<EscapeNode, AHashSet<EscapeNode>>::default();

    for global in &module.globals {
        let node = EscapeNode::Global(global.symbol);
        mark_escape_node(&mut states, node, EscapeState::EscapesToTypedCode);
        if global.external || exported_globals.contains(&global.symbol) {
            mark_escape_node(&mut states, node, EscapeState::EscapesToUntypedBoundary);
        }
    }

    for function in &module.functions {
        let value_types = control_flow_value_types(function);
        let closure_values = closure_targets(function);
        let exported = exported_functions.contains(&function.id);
        for (index, state) in function.value_escapes.iter().copied().enumerate() {
            if state != EscapeState::LocalOnly {
                mark_escape_node(
                    &mut states,
                    EscapeNode::Value(function.id, ValueId(index as u32)),
                    state,
                );
            }
        }
        if exported {
            for parameter in &function.params {
                mark_escape_node(
                    &mut states,
                    EscapeNode::Value(function.id, parameter.value),
                    EscapeState::EscapesToUntypedBoundary,
                );
            }
            for value in &returns[&function.id] {
                mark_escape_node(
                    &mut states,
                    EscapeNode::Value(function.id, *value),
                    EscapeState::EscapesToUntypedBoundary,
                );
            }
        }
        for shape in &function.shapes {
            if let ControlShape::ForOf {
                iterable, element, ..
            } = shape
            {
                add_escape_flow(
                    &mut edges,
                    EscapeNode::Value(function.id, *element),
                    EscapeNode::Value(function.id, *iterable),
                );
            }
        }
        for block in &function.blocks {
            for phi in &block.phis {
                for (_, incoming) in &phi.incoming {
                    add_escape_edge(
                        &mut edges,
                        EscapeNode::Value(function.id, phi.out),
                        EscapeNode::Value(function.id, *incoming),
                    );
                }
            }
            for instruction in &block.instructions {
                let value_node = |value| EscapeNode::Value(function.id, value);
                match &instruction.op {
                    ControlFlowOp::StoreLocal { local, value } => {
                        add_escape_edge(
                            &mut edges,
                            value_node(*value),
                            EscapeNode::Local(function.id, *local),
                        );
                    }
                    ControlFlowOp::LoadLocal(local) | ControlFlowOp::CaptureLocal(local) => {
                        if let Some(out) = instruction.out {
                            add_escape_edge(
                                &mut edges,
                                value_node(out),
                                EscapeNode::Local(function.id, *local),
                            );
                        }
                    }
                    ControlFlowOp::StoreGlobal { global, value } => {
                        add_escape_edge(
                            &mut edges,
                            value_node(*value),
                            EscapeNode::Global(*global),
                        );
                    }
                    ControlFlowOp::LoadGlobal(global) => {
                        if let Some(out) = instruction.out {
                            add_escape_edge(
                                &mut edges,
                                value_node(out),
                                EscapeNode::Global(*global),
                            );
                        }
                    }
                    ControlFlowOp::CaughtException => {
                        if let Some(out) = instruction.out {
                            mark_escape_node(
                                &mut states,
                                value_node(out),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::Template(parts) => {
                        for value in parts.iter().filter_map(|part| match part {
                            TemplateOperand::Value(value) => Some(*value),
                            TemplateOperand::String(_) => None,
                        }) {
                            mark_escape_node(
                                &mut states,
                                value_node(value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::Binary { op, lhs, rhs }
                        if matches!(op, IrBinaryOp::Add | IrBinaryOp::Eq | IrBinaryOp::NotEq)
                            && binary_requires_untyped_coercion(
                                *op,
                                value_types.get(lhs),
                                value_types.get(rhs),
                            ) =>
                    {
                        for value in [*lhs, *rhs] {
                            mark_escape_node(
                                &mut states,
                                value_node(value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::FieldGet { owner, index, .. } => {
                        if let (Some(out), Some(slot)) = (
                            instruction.out,
                            aggregate_field_slots.get(&(*owner, *index)),
                        ) {
                            add_escape_flow(
                                &mut edges,
                                value_node(out),
                                EscapeNode::AggregateField(*slot),
                            );
                        }
                    }
                    ControlFlowOp::HostFieldGet { object, .. } => {
                        mark_escape_node(
                            &mut states,
                            value_node(*object),
                            EscapeState::EscapesToUntypedBoundary,
                        );
                        if let Some(out) = instruction.out {
                            mark_escape_node(
                                &mut states,
                                value_node(out),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::HostFieldSet { object, value, .. } => {
                        for value in [*object, *value] {
                            mark_escape_node(
                                &mut states,
                                value_node(value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::IndexGet { object, index }
                        if value_types
                            .get(object)
                            .is_some_and(type_contains_untyped_js_value) =>
                    {
                        for value in [*object, *index] {
                            mark_escape_node(
                                &mut states,
                                value_node(value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                        if let Some(out) = instruction.out {
                            mark_escape_node(
                                &mut states,
                                value_node(out),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::IndexGet { object, index }
                        if value_types
                            .get(object)
                            .is_some_and(type_is_dynamic_bracket_receiver_after_ssa) =>
                    {
                        // Mem2reg can replace a dynamic `JsValue` receiver load
                        // with its nominal, wrapped-nominal, or unresolved generic
                        // producer. None supports typed bracket indexing, so this
                        // remains a JavaScript property boundary after aliasing.
                        for value in [*object, *index] {
                            mark_escape_node(
                                &mut states,
                                value_node(value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                        if let Some(out) = instruction.out {
                            mark_escape_node(
                                &mut states,
                                value_node(out),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::IndexGet { object, .. }
                    | ControlFlowOp::ArrayGetOptional { object, .. }
                    | ControlFlowOp::RecordFieldGet { object, .. }
                    | ControlFlowOp::RecordRest { object, .. } => {
                        if let Some(out) = instruction.out {
                            add_escape_flow(&mut edges, value_node(out), value_node(*object));
                        }
                    }
                    ControlFlowOp::Await { task } => {
                        if let Some(out) = instruction.out {
                            add_escape_flow(&mut edges, value_node(out), value_node(*task));
                        }
                    }
                    ControlFlowOp::IndexSet {
                        object,
                        index,
                        value,
                    } if value_types
                        .get(object)
                        .is_some_and(type_contains_untyped_js_value) =>
                    {
                        for value in [*object, *index, *value] {
                            mark_escape_node(
                                &mut states,
                                value_node(value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                        if let Some(out) = instruction.out {
                            mark_escape_node(
                                &mut states,
                                value_node(out),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::Array(values)
                        if instruction.ty.as_ref().is_some_and(|ty| {
                            matches!(ty, Type::Array(element) if type_contains_untyped_js_value(element))
                        }) =>
                    {
                        for value in values {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::Array(values) => {
                        if let Some(out) = instruction.out {
                            for value in values {
                                add_escape_flow(
                                    &mut edges,
                                    value_node(out),
                                    value_node(*value),
                                );
                            }
                        }
                    }
                    ControlFlowOp::ArraySpread(operands)
                        if instruction.ty.as_ref().is_some_and(|ty| {
                            matches!(ty, Type::Array(element) if type_contains_untyped_js_value(element))
                        }) =>
                    {
                        // A spread operand can retain its more precise source type after
                        // contextual typing widens the result to `JsValue[]`. Mark both
                        // direct elements and spread arrays: the latter lets nested
                        // aggregate types keep their JavaScript-visible representation.
                        for value in operands.iter().map(|operand| match operand {
                            crate::ir::ArrayOperand::Value(value)
                            | crate::ir::ArrayOperand::Spread(value) => *value,
                        }) {
                            mark_escape_node(
                                &mut states,
                                value_node(value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::ArraySpread(operands) => {
                        if let Some(out) = instruction.out {
                            for value in operands.iter().map(|operand| match operand {
                                ArrayOperand::Value(value) | ArrayOperand::Spread(value) => *value,
                            }) {
                                add_escape_flow(&mut edges, value_node(out), value_node(value));
                            }
                        }
                    }
                    ControlFlowOp::Record(entries)
                        if instruction.ty.as_ref().is_some_and(|ty| {
                            matches!(ty, Type::Record(value) if type_contains_untyped_js_value(value))
                        }) =>
                    {
                        for (_, value) in entries {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::Record(entries) => {
                        if let Some(out) = instruction.out {
                            for (_, value) in entries {
                                add_escape_flow(
                                    &mut edges,
                                    value_node(out),
                                    value_node(*value),
                                );
                            }
                        }
                    }
                    ControlFlowOp::RecordSpread(operands)
                        if instruction.ty.as_ref().is_some_and(|ty| {
                            matches!(ty, Type::Record(value) if type_contains_untyped_js_value(value))
                        }) =>
                    {
                        for value in operands.iter().map(|operand| match operand {
                            RecordOperand::Entry(_, value) | RecordOperand::Spread(value) => *value,
                        }) {
                            mark_escape_node(
                                &mut states,
                                value_node(value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::RecordSpread(operands) => {
                        if let Some(out) = instruction.out {
                            for value in operands.iter().map(|operand| match operand {
                                RecordOperand::Entry(_, value) | RecordOperand::Spread(value) => {
                                    *value
                                }
                            }) {
                                add_escape_flow(&mut edges, value_node(out), value_node(value));
                            }
                        }
                    }
                    ControlFlowOp::Struct { name, fields } => {
                        if let Some(out) = instruction.out {
                            for value in fields {
                                add_escape_flow(
                                    &mut edges,
                                    value_node(out),
                                    value_node(*value),
                                );
                            }
                        }
                        for (index, value) in fields.iter().enumerate() {
                            if let Some(slot) = aggregate_field_slots.get(&(*name, index)) {
                                add_escape_flow(
                                    &mut edges,
                                    EscapeNode::AggregateField(*slot),
                                    value_node(*value),
                                );
                            }
                            if dynamic_aggregate_fields.contains(&(*name, index)) {
                                mark_escape_node(
                                    &mut states,
                                    value_node(*value),
                                    EscapeState::EscapesToUntypedBoundary,
                                );
                            }
                        }
                    }
                    ControlFlowOp::FieldSet {
                        object,
                        owner,
                        index,
                        value,
                        ..
                    } => {
                        if let Some(slot) = aggregate_field_slots.get(&(*owner, *index)) {
                            add_escape_flow(
                                &mut edges,
                                EscapeNode::AggregateField(*slot),
                                value_node(*value),
                            );
                        }
                        if dynamic_aggregate_fields.contains(&(*owner, *index)) {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                        // A stored value lives at least as long as its owning object. Keep
                        // that owner-to-value lifetime flow so native allocation cannot place
                        // a closure environment on the caller's stack when the object retains
                        // it. The reverse direction is not valid: storing an already-dynamic
                        // `JsValue` does not itself expose the owner's aggregate layout.
                        add_escape_flow(&mut edges, value_node(*object), value_node(*value));
                    }
                    ControlFlowOp::RecordFieldSet { object, value, .. }
                        if value_types.get(object).is_some_and(|ty| {
                            matches!(ty, Type::Record(element) if type_contains_untyped_js_value(element))
                        }) =>
                    {
                        for value in [*object, *value] {
                            mark_escape_node(
                                &mut states,
                                value_node(value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::RecordFieldSet { object, value, .. } => {
                        add_escape_flow(&mut edges, value_node(*object), value_node(*value));
                    }
                    ControlFlowOp::IndexSet { object, value, .. }
                        if value_types.get(object).is_some_and(|ty| {
                            matches!(ty,
                                Type::Array(element) | Type::Record(element)
                                    if type_contains_untyped_js_value(element)
                            )
                        }) =>
                    {
                        for value in [*object, *value] {
                            mark_escape_node(
                                &mut states,
                                value_node(value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::IndexSet {
                        object,
                        index,
                        value,
                    } if value_types
                        .get(object)
                        .is_some_and(type_is_dynamic_bracket_receiver_after_ssa) =>
                    {
                        // Typed indexed writes are not legal on nominal,
                        // wrapped-nominal, or unresolved generic receivers. This
                        // form therefore came from an erased JavaScript property
                        // operation whose receiver and value are observable.
                        for value in [*object, *index, *value] {
                            mark_escape_node(
                                &mut states,
                                value_node(value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::IndexSet { object, value, .. } => {
                        add_escape_flow(&mut edges, value_node(*object), value_node(*value));
                    }
                    ControlFlowOp::HostCall { receiver, args, .. } => {
                        mark_escape_node(
                            &mut states,
                            value_node(*receiver),
                            EscapeState::EscapesToUntypedBoundary,
                        );
                        for value in args {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                        if let Some(out) = instruction.out {
                            mark_escape_node(
                                &mut states,
                                value_node(out),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::CallDirect {
                        function: callee,
                        args,
                        ..
                    } => {
                        if extern_functions.contains(callee) {
                            for value in args {
                                mark_escape_node(
                                    &mut states,
                                    value_node(*value),
                                    EscapeState::EscapesToUntypedBoundary,
                                );
                            }
                            if let Some(out) = instruction.out {
                                mark_escape_node(
                                    &mut states,
                                    value_node(out),
                                    EscapeState::EscapesToUntypedBoundary,
                                );
                            }
                        } else if let Some(callee_function) =
                            module.functions.get(callee.0 as usize)
                        {
                            for (argument, parameter) in args.iter().zip(&callee_function.params) {
                                add_escape_edge(
                                    &mut edges,
                                    value_node(*argument),
                                    EscapeNode::Value(*callee, parameter.value),
                                );
                            }
                            if let Some(out) = instruction.out {
                                for returned in &returns[callee] {
                                    add_escape_edge(
                                        &mut edges,
                                        value_node(out),
                                        EscapeNode::Value(*callee, *returned),
                                    );
                                }
                            }
                        }
                    }
                    ControlFlowOp::CallMethod { receiver, args, .. } => {
                        mark_escape_node(
                            &mut states,
                            value_node(*receiver),
                            EscapeState::EscapesToTypedCode,
                        );
                        for value in args {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToTypedCode,
                            );
                        }
                    }
                    ControlFlowOp::CallValue { callee, args } => {
                        mark_escape_node(
                            &mut states,
                            value_node(*callee),
                            EscapeState::EscapesToUntypedBoundary,
                        );
                        for value in args {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                        if let Some(out) = instruction.out {
                            mark_escape_node(
                                &mut states,
                                value_node(out),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::Print,
                        args,
                        ..
                    } => {
                        for value in args {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::Intrinsic {
                        intrinsic,
                        receiver,
                        args,
                    } => {
                        add_container_retention_flows(
                            &mut edges,
                            function.id,
                            *intrinsic,
                            *receiver,
                            args,
                            instruction.out,
                        );
                        add_array_callback_escape_flows(
                            &mut edges,
                            &mut states,
                            module,
                            function.id,
                            *intrinsic,
                            *receiver,
                            args,
                            instruction.out,
                            &closure_values,
                            &returns,
                        );
                        if *intrinsic == Intrinsic::ArrayLength {
                            if let Some(receiver) = receiver {
                                let receiver_type = value_types.get(receiver);
                                let dynamic_length = receiver_type.is_some_and(|ty| {
                                    type_contains_untyped_js_value(ty)
                                        || !type_has_static_length(ty)
                                });
                                if dynamic_length {
                                    mark_escape_node(
                                        &mut states,
                                        value_node(*receiver),
                                        EscapeState::EscapesToUntypedBoundary,
                                    );
                                    if let Some(out) = instruction.out {
                                        mark_escape_node(
                                            &mut states,
                                            value_node(out),
                                            EscapeState::EscapesToUntypedBoundary,
                                        );
                                    }
                                }
                            }
                        }
                        if *intrinsic == Intrinsic::TaskReject {
                            // Promise rejection reasons cross the JavaScript exception
                            // channel regardless of the task's resolved value type.
                            if let Some(reason) = args.first() {
                                mark_escape_node(
                                    &mut states,
                                    value_node(*reason),
                                    EscapeState::EscapesToUntypedBoundary,
                                );
                            }
                        }
                        if matches!(
                            intrinsic,
                            Intrinsic::UnwrapNullable | Intrinsic::UnwrapUnion
                        ) {
                            if let (Some(out), Some(receiver)) = (instruction.out, receiver) {
                                // These are representation-preserving aliases. In
                                // particular, `JS.assume` must carry an untyped
                                // producer's named-layout requirement to the
                                // statically narrowed aggregate result.
                                add_escape_edge(
                                    &mut edges,
                                    value_node(out),
                                    value_node(*receiver),
                                );
                            }
                        }
                        if intrinsic_uses_untyped_javascript_values(*intrinsic) {
                            if let Some(receiver) = receiver {
                                mark_escape_node(
                                    &mut states,
                                    value_node(*receiver),
                                    EscapeState::EscapesToUntypedBoundary,
                                );
                            }
                            for value in args {
                                mark_escape_node(
                                    &mut states,
                                    value_node(*value),
                                    EscapeState::EscapesToUntypedBoundary,
                                );
                            }
                            if let Some(out) = instruction.out {
                                mark_escape_node(
                                    &mut states,
                                    value_node(out),
                                    EscapeState::EscapesToUntypedBoundary,
                                );
                            }
                        } else if let Some(receiver) = receiver {
                            mark_typed_container_shape_exposures(
                                &mut states,
                                function.id,
                                *intrinsic,
                                *receiver,
                                args,
                                instruction.ty.as_ref(),
                                &value_types,
                            );
                        }
                        if let Some(receiver) = receiver {
                            mark_escape_node(
                                &mut states,
                                value_node(*receiver),
                                EscapeState::EscapesToTypedCode,
                            );
                        }
                        for value in args {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToTypedCode,
                            );
                        }
                    }
                    ControlFlowOp::Closure {
                        function: target,
                        captures,
                    } => {
                        if let Some(out) = instruction.out {
                            let closure = value_node(out);
                            if let Some(target_function) = module.functions.get(target.0 as usize) {
                                for (capture, parameter) in captures.iter().zip(
                                    target_function
                                        .params
                                        .iter()
                                        .take(target_function.capture_count),
                                ) {
                                    add_escape_edge(
                                        &mut edges,
                                        value_node(*capture),
                                        EscapeNode::Value(*target, parameter.value),
                                    );
                                }
                                for parameter in target_function
                                    .params
                                    .iter()
                                    .skip(target_function.capture_count)
                                {
                                    add_escape_flow(
                                        &mut edges,
                                        closure,
                                        EscapeNode::Value(*target, parameter.value),
                                    );
                                }
                            }
                            if let Some(returned_values) = returns.get(target) {
                                for returned in returned_values {
                                    add_escape_flow(
                                        &mut edges,
                                        closure,
                                        EscapeNode::Value(*target, *returned),
                                    );
                                }
                            }
                        }
                        for value in captures {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToTypedCode,
                            );
                        }
                    }
                    ControlFlowOp::NewClass {
                        constructor: Some(constructor),
                        args,
                        ..
                    } => {
                        if let Some(callee) = module.functions.get(constructor.0 as usize) {
                            let values = instruction.out.into_iter().chain(args.iter().copied());
                            for (argument, parameter) in values.zip(&callee.params) {
                                add_escape_edge(
                                    &mut edges,
                                    value_node(argument),
                                    EscapeNode::Value(*constructor, parameter.value),
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            match block.terminator {
                Some(Terminator::Return(Some(value))) => {
                    mark_escape_node(
                        &mut states,
                        EscapeNode::Value(function.id, value),
                        if exported {
                            EscapeState::EscapesToUntypedBoundary
                        } else {
                            EscapeState::EscapesToTypedCode
                        },
                    );
                }
                Some(Terminator::Throw(value)) => {
                    // Thrown values enter JavaScript's untyped exception channel,
                    // including when this module immediately catches them as `$js`.
                    mark_escape_node(
                        &mut states,
                        EscapeNode::Value(function.id, value),
                        EscapeState::EscapesToUntypedBoundary,
                    );
                }
                _ => {}
            }
        }
    }

    propagate_escape_states(&mut states, &edges);

    let mut changed = false;
    for function in &mut module.functions {
        for (index, slot) in function.value_escapes.iter_mut().enumerate() {
            let state = states
                .get(&EscapeNode::Value(function.id, ValueId(index as u32)))
                .copied()
                .unwrap_or(EscapeState::LocalOnly);
            if *slot != state {
                *slot = state;
                changed = true;
            }
        }
    }
    OptimizationReport {
        pass_name: "escape-analysis",
        changed,
    }
}

fn propagate_escape_states(
    states: &mut AHashMap<EscapeNode, EscapeState>,
    edges: &AHashMap<EscapeNode, AHashSet<EscapeNode>>,
) {
    // EscapeState is a three-element monotone lattice. Propagate only nodes
    // whose state can affect an outgoing edge instead of rescanning the whole
    // graph once per path step. A node can rise at most twice, so every
    // adjacency is visited at most twice even when an older queued entry is
    // superseded before it is processed.
    let mut worklist = states
        .iter()
        .filter_map(|(node, state)| (*state != EscapeState::LocalOnly).then_some(*node))
        .collect::<Vec<_>>();

    #[cfg(test)]
    ESCAPE_PROPAGATION_EDGE_VISITS.with(|visits| visits.set(0));

    while let Some(node) = worklist.pop() {
        let state = states.get(&node).copied().unwrap_or(EscapeState::LocalOnly);
        let Some(neighbors) = edges.get(&node) else {
            continue;
        };
        #[cfg(test)]
        ESCAPE_PROPAGATION_EDGE_VISITS.with(|visits| {
            visits.set(visits.get().saturating_add(neighbors.len()));
        });
        for neighbor in neighbors {
            let current = states
                .get(neighbor)
                .copied()
                .unwrap_or(EscapeState::LocalOnly);
            if escape_rank(state) > escape_rank(current) {
                states.insert(*neighbor, state);
                worklist.push(*neighbor);
            }
        }
    }
}

#[cfg(test)]
std::thread_local! {
    static ESCAPE_PROPAGATION_EDGE_VISITS: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

#[cfg(test)]
fn escape_propagation_edge_visits() -> usize {
    ESCAPE_PROPAGATION_EDGE_VISITS.with(std::cell::Cell::get)
}

fn type_contains_untyped_js_value(ty: &Type<'_>) -> bool {
    match ty {
        Type::TypeParameter("$js") => true,
        Type::Nullable(inner) => type_contains_untyped_js_value(inner),
        Type::Union(members) => members.iter().any(type_contains_untyped_js_value),
        _ => false,
    }
}

fn aggregate_field_erases_nominal_shape(ty: &Type<'_>) -> bool {
    if type_contains_untyped_js_value(ty) {
        return true;
    }
    match ty {
        Type::TypeParameter(_) => true,
        Type::Nullable(inner) => aggregate_field_erases_nominal_shape(inner),
        Type::Union(members) => members.iter().any(aggregate_field_erases_nominal_shape),
        _ => false,
    }
}

fn type_is_nominal_aggregate(ty: &Type<'_>) -> bool {
    matches!(
        ty,
        Type::Struct(_) | Type::StructInstance { .. } | Type::Class(_) | Type::ClassInstance { .. }
    )
}

fn type_is_dynamic_bracket_receiver_after_ssa(ty: &Type<'_>) -> bool {
    match ty {
        Type::TypeParameter(_) => true,
        Type::Nullable(inner) => type_is_dynamic_bracket_receiver_after_ssa(inner),
        Type::Union(members) => members
            .iter()
            .any(type_is_dynamic_bracket_receiver_after_ssa),
        ty => type_is_nominal_aggregate(ty),
    }
}

fn type_contains_nominal_aggregate(ty: &Type<'_>) -> bool {
    match ty {
        Type::Struct(_)
        | Type::StructInstance { .. }
        | Type::Class(_)
        | Type::ClassInstance { .. }
        | Type::TypeParameter(_) => true,
        Type::Array(element)
        | Type::Record(element)
        | Type::Set(element)
        | Type::Task(element)
        | Type::Generator(element)
        | Type::Nullable(element) => type_contains_nominal_aggregate(element),
        Type::Map(key, value) => {
            type_contains_nominal_aggregate(key) || type_contains_nominal_aggregate(value)
        }
        Type::Union(members) => members.iter().any(type_contains_nominal_aggregate),
        _ => false,
    }
}

fn type_supports_direct_equality(ty: &Type<'_>) -> bool {
    match ty {
        Type::Struct(_) | Type::StructInstance { .. } | Type::GenericFunction(_) | Type::Void => {
            false
        }
        Type::Nullable(inner) => type_supports_direct_equality(inner),
        Type::Union(members) => members.iter().all(type_supports_direct_equality),
        _ => true,
    }
}

fn binary_requires_untyped_coercion(
    op: IrBinaryOp,
    lhs: Option<&Type<'_>>,
    rhs: Option<&Type<'_>>,
) -> bool {
    if [lhs, rhs]
        .into_iter()
        .flatten()
        .any(type_contains_untyped_js_value)
    {
        return true;
    }
    let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
        return false;
    };
    let contains_nominal =
        type_contains_nominal_aggregate(lhs) || type_contains_nominal_aggregate(rhs);
    match op {
        // Mem2reg can replace a `JsValue` local load with its more precise
        // aggregate producer. These operations would not have type-checked on
        // that aggregate directly, so retain the erased coercion boundary.
        IrBinaryOp::Add => contains_nominal,
        IrBinaryOp::Eq | IrBinaryOp::NotEq => {
            contains_nominal
                && (lhs != rhs
                    || !type_supports_direct_equality(lhs)
                    || !type_supports_direct_equality(rhs))
        }
        _ => false,
    }
}

fn type_has_static_length(ty: &Type<'_>) -> bool {
    match ty {
        Type::Array(_) | Type::String => true,
        Type::Union(members) => members.iter().all(type_has_static_length),
        ty => crate::typed_array::is_typed_array_type(ty),
    }
}

fn intrinsic_uses_untyped_javascript_values(intrinsic: Intrinsic) -> bool {
    matches!(
        intrinsic,
        Intrinsic::RegexTest
            | Intrinsic::JsStringSlice
            | Intrinsic::JsStringIndexOf
            | Intrinsic::JsStringReplace
            | Intrinsic::JsStringMatch
            | Intrinsic::JsStringSplit
            | Intrinsic::JsRegexExec
            | Intrinsic::JsTruthy
            | Intrinsic::JsIsArray
            | Intrinsic::JsIsObject
            | Intrinsic::JsPlainObject
            | Intrinsic::JsUndefined
            | Intrinsic::JsTypeOf
            | Intrinsic::JsIsNullish
            | Intrinsic::JsIsFalse
            | Intrinsic::JsIsUndefined
            | Intrinsic::JsStringify
            | Intrinsic::JsDateNow
            | Intrinsic::JsParseFloat
            | Intrinsic::JsParseInt
            | Intrinsic::JsIsFinite
            | Intrinsic::JsEncodeURI
            | Intrinsic::JsEncodeURIComponent
            | Intrinsic::JsObjectCreate
            | Intrinsic::JsGetPrototypeOf
            | Intrinsic::JsMathPI
            | Intrinsic::JsNullProtoObject
            | Intrinsic::JsObjectConstructor
            | Intrinsic::JsWindow
            | Intrinsic::JsDocument
            | Intrinsic::JsSetTimeout
            | Intrinsic::JsClearTimeout
            | Intrinsic::JsDomParserNew
            | Intrinsic::JsXMLHttpRequestNew
            | Intrinsic::JsNumber
            | Intrinsic::JsAdd
            | Intrinsic::JsMod
            | Intrinsic::JsLessThan
            | Intrinsic::JsLessThanOrEqual
            | Intrinsic::JsGreaterThan
            | Intrinsic::JsGreaterThanOrEqual
            | Intrinsic::JsStrictEqual
            | Intrinsic::JsStrictNotEqual
            | Intrinsic::JsCall
            | Intrinsic::JsConstruct
            | Intrinsic::JsInvoke
            | Intrinsic::JsApply
            | Intrinsic::JsMethod0
            | Intrinsic::JsMethod1
            | Intrinsic::JsMethod2
            | Intrinsic::JsMethod3
            | Intrinsic::JsMethodRest
            | Intrinsic::JsStaticRest
            | Intrinsic::JsGetProperty
            | Intrinsic::JsDeleteProperty
            | Intrinsic::JsHasProperty
            | Intrinsic::JsInProperty
            | Intrinsic::JsBox
            | Intrinsic::JsArrayPush
            | Intrinsic::JsArrayPop
            | Intrinsic::JsArraySlice
            | Intrinsic::JsArrayIndexOf
            | Intrinsic::JsArraySort
            | Intrinsic::JsArraySplice
            | Intrinsic::JsArrayConcatApply
            | Intrinsic::JsArrayJoin
            | Intrinsic::JsArrayShift
            | Intrinsic::JsArrayUnshift
            | Intrinsic::JsArrayFlat
            | Intrinsic::JsIsFunctionValue
            | Intrinsic::JsIsWindowValue
            | Intrinsic::JsDefineConfigurable
            | Intrinsic::JsDefineIterator
            | Intrinsic::JsArrayIterator
            | Intrinsic::JsConsoleWarn
            | Intrinsic::JsRequestAnimationFrameOrNull
            | Intrinsic::JsForInKey
            | Intrinsic::JsForInHasNext
            | Intrinsic::JsForOfValue
            | Intrinsic::JsForOfHasNext
    )
}

fn add_container_retention_flows(
    edges: &mut AHashMap<EscapeNode, AHashSet<EscapeNode>>,
    function: FunctionId,
    intrinsic: Intrinsic,
    receiver: Option<ValueId>,
    args: &[ValueId],
    output: Option<ValueId>,
) {
    let node = |value| EscapeNode::Value(function, value);
    if let (Some(output), Some(receiver)) = (output, receiver) {
        if matches!(
            intrinsic,
            Intrinsic::ArrayPop
                | Intrinsic::ArrayFilter
                | Intrinsic::ArraySlice
                | Intrinsic::ArraySplice
                | Intrinsic::MapGet
                | Intrinsic::ArrayFill
                | Intrinsic::ArrayCopyWithin
                | Intrinsic::ArrayReverse
                | Intrinsic::MapSet
                | Intrinsic::SetAdd
        ) {
            add_escape_flow(edges, node(output), node(receiver));
        }
    }
    let retained_by_receiver = match intrinsic {
        Intrinsic::ArrayPush | Intrinsic::ArrayFill | Intrinsic::SetAdd => args.first().copied(),
        _ => None,
    };
    if let (Some(receiver), Some(value)) = (receiver, retained_by_receiver) {
        add_escape_flow(edges, node(receiver), node(value));
    }
    if intrinsic == Intrinsic::MapSet {
        if let Some(receiver) = receiver {
            for value in args.iter().take(2) {
                add_escape_flow(edges, node(receiver), node(*value));
            }
        }
    }
    if intrinsic == Intrinsic::TaskResolve {
        if let (Some(output), Some(value)) = (output, args.first()) {
            add_escape_flow(edges, node(output), node(*value));
        }
    }
    if intrinsic == Intrinsic::TaskAll {
        if let (Some(output), Some(tasks)) = (output, args.first()) {
            add_escape_flow(edges, node(output), node(*tasks));
        }
    }
    if matches!(
        intrinsic,
        Intrinsic::JsMethod0
            | Intrinsic::JsMethod1
            | Intrinsic::JsMethod2
            | Intrinsic::JsMethod3
            | Intrinsic::JsMethodRest
            | Intrinsic::JsStaticRest
    ) {
        if let (Some(output), Some(callback)) = (output, args.first()) {
            // The wrapper retains its callback. Propagate any later untyped
            // escape through the closure so captured aggregate layouts remain
            // ABI-safe as well.
            add_escape_flow(edges, node(output), node(*callback));
        }
    }
    if intrinsic == Intrinsic::ArrayConcat {
        if let Some(output) = output {
            if let Some(receiver) = receiver {
                add_escape_flow(edges, node(output), node(receiver));
            }
            for value in args {
                add_escape_flow(edges, node(output), node(*value));
            }
        }
    }
    if matches!(intrinsic, Intrinsic::ArrayMap | Intrinsic::ArrayReduce) {
        if let (Some(output), Some(callback)) = (output, args.first()) {
            add_escape_flow(edges, node(output), node(*callback));
        }
        if intrinsic == Intrinsic::ArrayReduce {
            if let (Some(output), Some(initial)) = (output, args.get(1)) {
                add_escape_flow(edges, node(output), node(*initial));
            }
        }
    }
    if intrinsic == Intrinsic::RecordValues {
        if let (Some(output), Some(record)) = (output, args.first()) {
            add_escape_flow(edges, node(output), node(*record));
        }
    }
    if intrinsic == Intrinsic::RecordAssign {
        if let Some(target) = args.first() {
            if let Some(output) = output {
                add_escape_flow(edges, node(output), node(*target));
            }
            if let Some(source) = args.get(1) {
                add_escape_flow(edges, node(*target), node(*source));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn add_array_callback_escape_flows(
    edges: &mut AHashMap<EscapeNode, AHashSet<EscapeNode>>,
    states: &mut AHashMap<EscapeNode, EscapeState>,
    module: &ControlFlowModule<'_>,
    caller: FunctionId,
    intrinsic: Intrinsic,
    receiver: Option<ValueId>,
    args: &[ValueId],
    output: Option<ValueId>,
    closures: &AHashMap<ValueId, FunctionId>,
    returns: &AHashMap<FunctionId, Vec<ValueId>>,
) {
    if !matches!(
        intrinsic,
        Intrinsic::ArrayMap
            | Intrinsic::ArrayFilter
            | Intrinsic::ArrayReduce
            | Intrinsic::ArrayForEach
            | Intrinsic::ArraySome
            | Intrinsic::ArrayEvery
            | Intrinsic::ArrayFindIndex
    ) {
        return;
    }
    let Some(receiver) = receiver else {
        return;
    };
    let node = |function, value| EscapeNode::Value(function, value);
    let known = args
        .first()
        .and_then(|callback| closures.get(callback))
        .and_then(|target| {
            module
                .functions
                .get(target.0 as usize)
                .map(|function| (*target, function))
        });
    if let Some((_, target_function)) = known {
        if target_function.kind == FunctionKind::Extern {
            // A direct extern callback has a stable function ID but no body
            // whose parameter/return provenance can be followed. Its consumed
            // values and produced aggregate result cross the host ABI.
            mark_escape_node(
                states,
                node(caller, receiver),
                EscapeState::EscapesToUntypedBoundary,
            );
            if intrinsic == Intrinsic::ArrayReduce {
                if let Some(initial) = args.get(1) {
                    mark_escape_node(
                        states,
                        node(caller, *initial),
                        EscapeState::EscapesToUntypedBoundary,
                    );
                }
            }
            if matches!(intrinsic, Intrinsic::ArrayMap | Intrinsic::ArrayReduce) {
                if let Some(output) = output {
                    mark_escape_node(
                        states,
                        node(caller, output),
                        EscapeState::EscapesToUntypedBoundary,
                    );
                }
            }
            return;
        }
    }
    let required_parameters = if intrinsic == Intrinsic::ArrayReduce {
        2
    } else {
        1
    };
    let Some((target, target_function)) = known.filter(|(_, function)| {
        function.params.len().saturating_sub(function.capture_count) >= required_parameters
    }) else {
        // A callback obtained through a parameter/local/union may expose array
        // elements beyond typed code. Without a known target ABI, keep the
        // source contents JavaScript-compatible rather than guessing.
        mark_escape_node(
            states,
            node(caller, receiver),
            EscapeState::EscapesToUntypedBoundary,
        );
        if intrinsic == Intrinsic::ArrayReduce {
            if let Some(initial) = args.get(1) {
                mark_escape_node(
                    states,
                    node(caller, *initial),
                    EscapeState::EscapesToUntypedBoundary,
                );
            }
        }
        return;
    };

    let public_parameters = &target_function.params[target_function.capture_count..];
    let element = if intrinsic == Intrinsic::ArrayReduce {
        &public_parameters[1]
    } else {
        &public_parameters[0]
    };
    add_escape_edge(edges, node(target, element.value), node(caller, receiver));

    if intrinsic == Intrinsic::ArrayReduce {
        let accumulator = node(target, public_parameters[0].value);
        if let Some(initial) = args.get(1) {
            add_escape_edge(edges, accumulator, node(caller, *initial));
        }
        if let Some(output) = output {
            add_escape_edge(edges, accumulator, node(caller, output));
        }
        if let Some(returned_values) = returns.get(&target) {
            for returned in returned_values {
                add_escape_edge(edges, accumulator, node(target, *returned));
            }
        }
    }
}

fn mark_typed_container_shape_exposures<'src>(
    states: &mut AHashMap<EscapeNode, EscapeState>,
    function: FunctionId,
    intrinsic: Intrinsic,
    receiver: ValueId,
    args: &[ValueId],
    output_type: Option<&Type<'src>>,
    value_types: &AHashMap<ValueId, Type<'src>>,
) {
    let mark = |states: &mut AHashMap<EscapeNode, EscapeState>, value| {
        mark_escape_node(
            states,
            EscapeNode::Value(function, value),
            EscapeState::EscapesToUntypedBoundary,
        );
    };
    let receiver_type = value_types.get(&receiver);

    let array_accepts_js_value = [receiver_type, output_type]
        .into_iter()
        .flatten()
        .any(|ty| matches!(ty, Type::Array(element) if type_contains_untyped_js_value(element)));
    if array_accepts_js_value {
        match intrinsic {
            Intrinsic::ArrayPush => {
                mark(states, receiver);
                for value in args {
                    mark(states, *value);
                }
            }
            Intrinsic::ArraySplice => {
                mark(states, receiver);
                // Typed `splice` currently has no insertion arguments. Keeping
                // this tail rule makes a future widened signature safe without
                // treating its start/delete-count scalars as stored values.
                for value in args.iter().skip(2) {
                    mark(states, *value);
                }
            }
            Intrinsic::ArrayFill => {
                mark(states, receiver);
                if let Some(value) = args.first() {
                    mark(states, *value);
                }
            }
            Intrinsic::ArrayConcat => {
                mark(states, receiver);
                for value in args {
                    mark(states, *value);
                }
            }
            _ => {}
        }
    }

    let mut js_map_key = false;
    let mut js_map_value = false;
    for ty in [receiver_type, output_type].into_iter().flatten() {
        if let Type::Map(key, value) = ty {
            js_map_key |= type_contains_untyped_js_value(key);
            js_map_value |= type_contains_untyped_js_value(value);
        }
    }
    if intrinsic == Intrinsic::MapSet && (js_map_key || js_map_value) {
        mark(states, receiver);
        if js_map_key {
            if let Some(key) = args.first() {
                mark(states, *key);
            }
        }
        if js_map_value {
            if let Some(value) = args.get(1) {
                mark(states, *value);
            }
        }
    }

    let set_accepts_js_value = [receiver_type, output_type]
        .into_iter()
        .flatten()
        .any(|ty| matches!(ty, Type::Set(element) if type_contains_untyped_js_value(element)));
    if intrinsic == Intrinsic::SetAdd && set_accepts_js_value {
        mark(states, receiver);
        if let Some(value) = args.first() {
            mark(states, *value);
        }
    }
}

fn add_escape_edge(
    edges: &mut AHashMap<EscapeNode, AHashSet<EscapeNode>>,
    left: EscapeNode,
    right: EscapeNode,
) {
    edges.entry(left).or_default().insert(right);
    edges.entry(right).or_default().insert(left);
}

fn add_escape_flow(
    edges: &mut AHashMap<EscapeNode, AHashSet<EscapeNode>>,
    source: EscapeNode,
    retained: EscapeNode,
) {
    edges.entry(source).or_default().insert(retained);
}

fn mark_escape_node(
    states: &mut AHashMap<EscapeNode, EscapeState>,
    node: EscapeNode,
    state: EscapeState,
) {
    let slot = states.entry(node).or_insert(EscapeState::LocalOnly);
    if escape_rank(state) > escape_rank(*slot) {
        *slot = state;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KnownRecordShape<'src> {
    /// Source-encoded keys in insertion order. JavaScript array-index ordering
    /// is applied only when an observer asks for the own-key order.
    entries: Vec<(&'src str, ValueId)>,
}

fn try_decode_ir_source_string(value: &str) -> Option<String> {
    serde_json::from_str(&format!("\"{value}\"")).ok()
}

/// Return record allocations whose identity never reaches a write, aliasing
/// operation, host boundary, phi, or terminator. Such a record has one shape
/// for its entire lifetime, so that shape may be used in blocks dominated by
/// the allocation. A spread only snapshots own data properties and therefore
/// is a read-only use; it does not alias the source record.
fn immutable_closed_record_values(function: &ControlFlowFunction<'_>) -> AHashSet<ValueId> {
    let candidates = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match (&instruction.out, &instruction.op) {
            (Some(out), ControlFlowOp::Record(_) | ControlFlowOp::RecordSpread(_)) => Some(*out),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let mut invalid = AHashSet::default();

    for block in &function.blocks {
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                if candidates.contains(value) {
                    invalid.insert(*value);
                }
            }
        }
        for instruction in &block.instructions {
            for value in control_flow_used_values(&instruction.op) {
                if candidates.contains(&value)
                    && !closed_record_use_is_read_only(&instruction.op, value)
                {
                    invalid.insert(value);
                }
            }
        }
        if let Some(terminator) = &block.terminator {
            for value in terminator_used_values(terminator) {
                if candidates.contains(&value) {
                    invalid.insert(value);
                }
            }
        }
    }

    // Structured loop/iteration metadata can retain an aggregate even when a
    // later lowering helper no longer has an explicit instruction use.
    for shape in &function.shapes {
        match shape {
            crate::ir::ControlShape::ForIn { object, key, .. } => {
                for value in [object, key] {
                    if candidates.contains(value) {
                        invalid.insert(*value);
                    }
                }
            }
            crate::ir::ControlShape::ForOf {
                iterable, element, ..
            } => {
                for value in [iterable, element] {
                    if candidates.contains(value) {
                        invalid.insert(*value);
                    }
                }
            }
            crate::ir::ControlShape::Try {
                catch_value: Some(value),
                ..
            } if candidates.contains(value) => {
                invalid.insert(*value);
            }
            _ => {}
        }
    }

    candidates
        .into_iter()
        .filter(|value| !invalid.contains(value))
        .collect()
}

fn closed_record_use_is_read_only(op: &ControlFlowOp<'_>, value: ValueId) -> bool {
    match op {
        ControlFlowOp::RecordFieldGet { object, .. } => *object == value,
        ControlFlowOp::RecordSpread(operands) => operands
            .iter()
            .all(|operand| !matches!(operand, RecordOperand::Entry(_, entry) if *entry == value)),
        ControlFlowOp::Intrinsic {
            intrinsic:
                Intrinsic::RecordKeys
                | Intrinsic::RecordValues
                | Intrinsic::RecordHasOwn
                | Intrinsic::JsonStringify,
            receiver: None,
            args,
        } => args.first() == Some(&value) && args.iter().skip(1).all(|argument| *argument != value),
        _ => false,
    }
}

/// Infer snapshots for immutable record allocations without deriving mutable
/// facts from block-vector order. Mutable sources participate only within one
/// block, in instruction order; an immutable spread result can then carry its
/// independent snapshot across dominated blocks. Repeating the scan permits a
/// chain of immutable spreads whose defining blocks are not stored in
/// dominance order, without ever propagating a mutable shape across a CFG edge.
fn immutable_closed_record_shapes<'src>(
    function: &ControlFlowFunction<'src>,
    immutable: &AHashSet<ValueId>,
) -> (
    AHashMap<ValueId, KnownRecordShape<'src>>,
    AHashMap<ValueId, usize>,
) {
    let mut shapes = AHashMap::<ValueId, KnownRecordShape<'src>>::default();
    let mut definition_blocks = AHashMap::<ValueId, usize>::default();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for instruction in &block.instructions {
            if let Some(out) = instruction.out.filter(|out| immutable.contains(out)) {
                definition_blocks.insert(out, block_index);
            }
        }
    }

    loop {
        let mut added = false;
        for block in &function.blocks {
            let mut records = shapes.clone();
            for instruction in &block.instructions {
                match (instruction.out, &instruction.op) {
                    (Some(out), ControlFlowOp::Record(entries)) => {
                        let mut shape = KnownRecordShape {
                            entries: Vec::new(),
                        };
                        let mut complete = true;
                        for (key, value) in entries {
                            complete &= known_record_set(&mut shape, key, *value);
                            records.remove(value);
                        }
                        if complete {
                            records.insert(out, shape.clone());
                        }
                        if complete && immutable.contains(&out) && !shapes.contains_key(&out) {
                            shapes.insert(out, shape);
                            added = true;
                        }
                    }
                    (Some(out), ControlFlowOp::RecordSpread(operands)) => {
                        let mut shape = KnownRecordShape {
                            entries: Vec::new(),
                        };
                        let mut complete = true;
                        for operand in operands {
                            match operand {
                                RecordOperand::Entry(key, value) => {
                                    complete &= known_record_set(&mut shape, key, *value);
                                    records.remove(value);
                                }
                                RecordOperand::Spread(source) => {
                                    let Some(source) = records.get(source) else {
                                        complete = false;
                                        break;
                                    };
                                    for (key, value) in &source.entries {
                                        complete &= known_record_set(&mut shape, key, *value);
                                    }
                                }
                            }
                        }
                        if complete {
                            records.insert(out, shape.clone());
                            if immutable.contains(&out) && !shapes.contains_key(&out) {
                                shapes.insert(out, shape);
                                added = true;
                            }
                        }
                    }
                    (
                        _,
                        ControlFlowOp::RecordFieldSet {
                            object,
                            property,
                            value,
                        },
                    ) => {
                        if let Some(shape) = records.get_mut(object) {
                            if !known_record_set(shape, property, *value) {
                                records.remove(object);
                            }
                        }
                        records.remove(value);
                    }
                    (_, operation) => {
                        let preserves_shape = matches!(
                            operation,
                            ControlFlowOp::RecordFieldGet { .. }
                                | ControlFlowOp::Intrinsic {
                                    intrinsic: Intrinsic::RecordKeys
                                        | Intrinsic::RecordValues
                                        | Intrinsic::RecordHasOwn
                                        | Intrinsic::JsonStringify,
                                    ..
                                }
                        );
                        if !preserves_shape {
                            for value in control_flow_used_values(operation) {
                                records.remove(&value);
                            }
                        }
                    }
                }
            }
        }
        if !added {
            break;
        }
    }

    (shapes, definition_blocks)
}

/// Remove only operations on compiler-owned closed records after all of their
/// observers have been projected away. Recomputing uses to a fixed point is
/// important: removing an unused spread can make a later write to its formerly
/// observed source dead. Unlike a block-local "remaining uses" counter, this
/// proof includes phi and terminator uses in every block.
fn eliminate_unobserved_closed_record_operations(
    function: &mut ControlFlowFunction<'_>,
    closed_allocations: &AHashSet<ValueId>,
) -> bool {
    let shape_uses = function
        .shapes
        .iter()
        .flat_map(|shape| match shape {
            crate::ir::ControlShape::ForIn { object, key, .. } => vec![*object, *key],
            crate::ir::ControlShape::ForOf {
                iterable, element, ..
            } => vec![*iterable, *element],
            crate::ir::ControlShape::Try {
                catch_value: Some(value),
                ..
            } => vec![*value],
            _ => Vec::new(),
        })
        .collect::<AHashSet<_>>();
    let mut changed = false;
    loop {
        let uses = control_flow_use_counts(function);
        let mut local_change = false;
        for block in &mut function.blocks {
            block.instructions.retain(|instruction| {
                let removable = match (instruction.out, &instruction.op) {
                    (Some(out), ControlFlowOp::Record(_) | ControlFlowOp::RecordSpread(_)) => {
                        closed_allocations.contains(&out)
                            && !shape_uses.contains(&out)
                            && uses.get(&out).copied().unwrap_or(0) == 0
                    }
                    (None, ControlFlowOp::RecordFieldSet { object, .. }) => {
                        closed_allocations.contains(object)
                            && !shape_uses.contains(object)
                            && uses.get(object).copied() == Some(1)
                    }
                    _ => false,
                };
                local_change |= removable;
                !removable
            });
        }
        changed |= local_change;
        if !local_change {
            return changed;
        }
    }
}

/// Project observations of a fresh, closed `Record<T>` into equivalent scalar
/// IR while retaining the null-prototype representation everywhere else.
///
/// This is deliberately a narrow proof. A record snapshot crosses structured
/// SSA blocks only when its allocation dominates the observer and every use of
/// that identity is read-only. Mutable record shapes remain block-local; a
/// write, phi, aliasing operation, terminator, or unknown/host use disqualifies
/// cross-block propagation. Consequently a missing-field fold never relies on
/// `Object.prototype`, and a spread is modeled only while its source is still
/// a compiler-owned null-prototype record.
/// `JSON.stringify` is folded only for a record whose complete own shape and
/// portable scalar values are constants. A one-use `Object.keys` result that
/// flows directly to `Array.join` becomes an ordinary key array; keeping the
/// actual join preserves its runtime method lookup rather than assuming a
/// pristine `Array.prototype`.
fn project_closed_record_observations(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;

    for function in &mut module.functions {
        if has_exception_region(function) {
            continue;
        }

        let uses = control_flow_use_counts(function);
        let immutable_records = immutable_closed_record_values(function);
        let (immutable_shapes, record_definition_blocks) =
            immutable_closed_record_shapes(function, &immutable_records);
        let predecessors = cfg_predecessors(function);
        let reachable = reachable_blocks(function);
        let dominators = compute_dominators(function.entry.0 as usize, &predecessors, &reachable);
        let global_constants = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Const(value)) => Some((out, value.clone())),
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        let joined_key_arrays = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.op {
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayJoin,
                    receiver: Some(receiver),
                    ..
                } if uses.get(receiver).copied() == Some(1) => Some(*receiver),
                _ => None,
            })
            .collect::<AHashSet<_>>();
        let mut aliases = AHashMap::<ValueId, ValueId>::default();
        let mut closed_allocations = AHashSet::<ValueId>::default();
        let mut next_value = function.value_count;
        for (block_index, block) in function.blocks.iter_mut().enumerate() {
            // Dominance is checked explicitly even though well-formed SSA also
            // guarantees it. Mutable snapshots are added below only from this
            // block's instruction stream and never carried to another block.
            let mut records = immutable_shapes
                .iter()
                .filter(|(value, _)| {
                    record_definition_blocks
                        .get(value)
                        .is_some_and(|definition| dominators[block_index].contains(definition))
                })
                .map(|(value, shape)| (*value, shape.clone()))
                .collect::<AHashMap<_, _>>();
            let mut constants = global_constants.clone();
            let instructions = std::mem::take(&mut block.instructions);
            let mut rewritten = Vec::with_capacity(instructions.len());

            for mut instruction in instructions {
                rewrite_control_flow_op(&mut instruction.op, &aliases);
                let operation = instruction.op.clone();
                match (instruction.out, operation) {
                    (Some(out), ControlFlowOp::Const(value)) => {
                        constants.insert(out, value);
                        rewritten.push(instruction);
                    }
                    (Some(out), ControlFlowOp::Record(entries)) => {
                        let mut shape = KnownRecordShape {
                            entries: Vec::new(),
                        };
                        let mut complete = true;
                        for (key, value) in &entries {
                            complete &= known_record_set(&mut shape, key, *value);
                        }
                        // Storing another record as a field aliases its mutable
                        // identity; nested shapes are outside this scalar proof.
                        for (_, value) in &entries {
                            records.remove(&resolve_alias(*value, &aliases));
                        }
                        if complete {
                            records.insert(out, shape);
                            closed_allocations.insert(out);
                        }
                        rewritten.push(instruction);
                    }
                    (Some(out), ControlFlowOp::RecordSpread(operands)) => {
                        let mut shape = KnownRecordShape {
                            entries: Vec::new(),
                        };
                        let mut complete = true;
                        for operand in &operands {
                            match operand {
                                RecordOperand::Entry(key, value) => {
                                    complete &= known_record_set(&mut shape, key, *value);
                                }
                                RecordOperand::Spread(source) => {
                                    let source = resolve_alias(*source, &aliases);
                                    let Some(source) = records.get(&source) else {
                                        complete = false;
                                        break;
                                    };
                                    for (key, value) in &source.entries {
                                        complete &= known_record_set(&mut shape, key, *value);
                                    }
                                }
                            }
                        }
                        if complete {
                            records.insert(out, shape);
                            closed_allocations.insert(out);
                        }
                        rewritten.push(instruction);
                    }
                    (Some(out), ControlFlowOp::RecordFieldGet { object, property }) => {
                        let object = resolve_alias(object, &aliases);
                        if let Some(shape) = records.get(&object) {
                            if let Some(value) = known_record_get(shape, property) {
                                aliases.insert(out, resolve_alias(value, &aliases));
                            } else {
                                instruction.op = ControlFlowOp::Const(ConstValue::Null);
                                constants.insert(out, ConstValue::Null);
                                rewritten.push(instruction);
                            }
                            changed = true;
                        } else {
                            rewritten.push(instruction);
                        }
                    }
                    (
                        _,
                        ControlFlowOp::RecordFieldSet {
                            object,
                            property,
                            value,
                        },
                    ) => {
                        let object = resolve_alias(object, &aliases);
                        if let Some(shape) = records.get_mut(&object) {
                            if !known_record_set(shape, property, resolve_alias(value, &aliases)) {
                                records.remove(&object);
                            }
                        }
                        // A record value stored inside another record becomes
                        // reachable through that container. Scalar values do
                        // not alias the containing record and must not discard
                        // unrelated shape facts.
                        let value = resolve_alias(value, &aliases);
                        if records.contains_key(&value) {
                            records.remove(&value);
                        }
                        rewritten.push(instruction);
                    }
                    (
                        Some(out),
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::RecordKeys,
                            receiver: None,
                            args,
                        },
                    ) if args.len() == 1 && joined_key_arrays.contains(&out) => {
                        let object = resolve_alias(args[0], &aliases);
                        if let Some(shape) = records.get(&object) {
                            let mut values = Vec::with_capacity(shape.entries.len());
                            for (key, _) in known_record_ordered_entries(shape) {
                                let value = ValueId(next_value);
                                next_value += 1;
                                function.value_escapes.push(EscapeState::LocalOnly);
                                function.value_local_hints.push(None);
                                let constant = ConstValue::String(key.to_string());
                                constants.insert(value, constant.clone());
                                rewritten.push(ControlFlowInstruction {
                                    out: Some(value),
                                    ty: Some(Type::String),
                                    op: ControlFlowOp::Const(constant),
                                    span: instruction.span,
                                });
                                values.push(value);
                            }
                            instruction.op = ControlFlowOp::Array(values);
                            instruction.ty = Some(Type::Array(Box::new(Type::String)));
                            rewritten.push(instruction);
                            changed = true;
                        } else {
                            rewritten.push(instruction);
                        }
                    }
                    (
                        Some(out),
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::JsonStringify,
                            receiver: None,
                            args,
                        },
                    ) if args.len() == 1 => {
                        let object = resolve_alias(args[0], &aliases);
                        if let Some(value) = records
                            .get(&object)
                            .and_then(|shape| known_record_json(shape, &constants, &aliases))
                        {
                            instruction.op = ControlFlowOp::Const(value.clone());
                            constants.insert(out, value);
                            rewritten.push(instruction);
                            changed = true;
                        } else {
                            rewritten.push(instruction);
                        }
                    }
                    (
                        Some(out),
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::UnwrapNullable | Intrinsic::UnwrapUnion,
                            receiver: Some(receiver),
                            args,
                        },
                    ) if args.is_empty() => {
                        let receiver = resolve_alias(receiver, &aliases);
                        if let Some(shape) = records.get(&receiver).cloned() {
                            records.insert(out, shape);
                        }
                        if let Some(value) = constants.get(&receiver).cloned() {
                            constants.insert(out, value);
                        }
                        rewritten.push(instruction);
                    }
                    (_, operation) => {
                        // Pure known observers that were not profitable to
                        // project cannot mutate a record. Every other use ends
                        // the proof before subsequent observations.
                        let preserves_shape = matches!(
                            operation,
                            ControlFlowOp::RecordFieldGet { .. }
                                | ControlFlowOp::Intrinsic {
                                    intrinsic: Intrinsic::RecordKeys
                                        | Intrinsic::RecordValues
                                        | Intrinsic::RecordHasOwn
                                        | Intrinsic::JsonStringify,
                                    ..
                                }
                        );
                        if !preserves_shape {
                            for value in control_flow_used_values(&operation) {
                                let value = resolve_alias(value, &aliases);
                                records.remove(&value);
                            }
                        }
                        rewritten.push(instruction);
                    }
                }
            }
            block.instructions = rewritten;
        }
        function.value_count = next_value;
        rewrite_control_flow_function(function, &aliases);
        changed |= eliminate_unobserved_closed_record_operations(function, &closed_allocations);
    }

    OptimizationReport {
        pass_name: "closed-record-observation-projection",
        changed,
    }
}

/// JavaScript-only scalar projection. The neutral optimizer intentionally does
/// not invoke this pass because native codegen retains explicit record runtime
/// operations. JavaScript candidate search calls it after generic optimization
/// and scores both the projected and unprojected artifacts.
pub fn project_closed_record_observations_for_javascript(
    module: &mut ControlFlowModule<'_>,
) -> OptimizationReport {
    project_closed_record_observations(module)
}

fn known_record_set<'src>(
    shape: &mut KnownRecordShape<'src>,
    key: &'src str,
    value: ValueId,
) -> bool {
    let Some(decoded) = try_decode_ir_source_string(key) else {
        return false;
    };
    if let Some((_, slot)) = shape
        .entries
        .iter_mut()
        .find(|(candidate, _)| try_decode_ir_source_string(candidate).as_deref() == Some(&decoded))
    {
        *slot = value;
    } else {
        shape.entries.push((key, value));
    }
    true
}

fn known_record_get(shape: &KnownRecordShape<'_>, key: &str) -> Option<ValueId> {
    let decoded = try_decode_ir_source_string(key)?;
    shape
        .entries
        .iter()
        .find(|(candidate, _)| try_decode_ir_source_string(candidate).as_deref() == Some(&decoded))
        .map(|(_, value)| *value)
}

fn record_array_index(key: &str) -> Option<u32> {
    if key.is_empty()
        || (key.len() > 1 && key.starts_with('0'))
        || !key.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let value = key.parse::<u64>().ok()?;
    (value < u64::from(u32::MAX)).then_some(value as u32)
}

fn known_record_ordered_entries<'src>(shape: &KnownRecordShape<'src>) -> Vec<(&'src str, ValueId)> {
    let mut indices = shape
        .entries
        .iter()
        .filter_map(|(key, value)| {
            try_decode_ir_source_string(key)
                .and_then(|key| record_array_index(&key))
                .map(|index| (index, *key, *value))
        })
        .collect::<Vec<_>>();
    indices.sort_unstable_by_key(|(index, _, _)| *index);
    indices
        .into_iter()
        .map(|(_, key, value)| (key, value))
        .chain(shape.entries.iter().filter_map(|(key, value)| {
            try_decode_ir_source_string(key)
                .filter(|key| record_array_index(key).is_none())
                .map(|_| (*key, *value))
        }))
        .collect()
}

fn known_record_json(
    shape: &KnownRecordShape<'_>,
    constants: &AHashMap<ValueId, ConstValue>,
    aliases: &AHashMap<ValueId, ValueId>,
) -> Option<ConstValue> {
    let mut json = String::from("{");
    for (index, (key, value)) in known_record_ordered_entries(shape).into_iter().enumerate() {
        if index != 0 {
            json.push(',');
        }
        json.push_str(&serde_json::to_string(&try_decode_ir_source_string(key)?).ok()?);
        json.push(':');
        let value = constants.get(&resolve_alias(value, aliases))?;
        match value {
            ConstValue::Int(value) => json.push_str(&value.to_string()),
            ConstValue::Bool(value) => json.push_str(if *value { "true" } else { "false" }),
            ConstValue::String(value) => {
                json.push_str(&serde_json::to_string(&try_decode_ir_source_string(value)?).ok()?)
            }
            ConstValue::Null => json.push_str("null"),
            // JavaScript number formatting, especially `-0`, exponent
            // thresholds, and non-finite values, is not serde_json's contract.
            // Leave floats to the runtime until an exact ECMAScript formatter
            // is available.
            ConstValue::Float(_) => return None,
        }
    }
    json.push('}');

    let encoded = serde_json::to_string(&json).ok()?;
    Some(ConstValue::String(
        encoded.strip_prefix('"')?.strip_suffix('"')?.to_string(),
    ))
}

fn scalar_replace_control_flow_aggregates(
    module: &mut ControlFlowModule<'_>,
) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        if has_exception_region(function) {
            continue;
        }
        changed |= scalar_replace_loop_carried_structs(function);
        let mut structs = AHashMap::<ValueId, Vec<ValueId>>::default();
        let mut invalid = AHashSet::<ValueId>::default();
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let (Some(out), ControlFlowOp::Struct { fields, .. }) =
                    (instruction.out, &instruction.op)
                {
                    if function.value_escapes[out.0 as usize] == EscapeState::LocalOnly {
                        structs.insert(out, fields.clone());
                    }
                }
            }
        }
        for block in &function.blocks {
            // This scalar replacer only understands aggregates whose complete
            // use-set consists of direct field reads. A phi incoming edge is
            // an ordinary aggregate use, even though it is stored outside the
            // instruction and terminator operand lists below. Treat it as an
            // escape from this deliberately linear representation; otherwise
            // the struct definition is removed while the phi keeps a dangling
            // SSA reference.
            for phi in &block.phis {
                for (_, incoming) in &phi.incoming {
                    if structs.contains_key(incoming) {
                        invalid.insert(*incoming);
                    }
                }
            }
            for instruction in &block.instructions {
                match &instruction.op {
                    ControlFlowOp::FieldGet { object, .. } if structs.contains_key(object) => {}
                    _ => {
                        for used in control_flow_used_values(&instruction.op) {
                            if structs.contains_key(&used) {
                                invalid.insert(used);
                            }
                        }
                    }
                }
            }
            for used in block
                .terminator
                .as_ref()
                .into_iter()
                .flat_map(terminator_used_values)
            {
                if structs.contains_key(&used) {
                    invalid.insert(used);
                }
            }
        }

        let mut aliases = AHashMap::<ValueId, ValueId>::default();
        for block in &mut function.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut retained = Vec::with_capacity(instructions.len());
            for instruction in instructions {
                match (&instruction.out, &instruction.op) {
                    (Some(out), ControlFlowOp::Struct { .. })
                        if structs.contains_key(out) && !invalid.contains(out) =>
                    {
                        changed = true;
                    }
                    (Some(out), ControlFlowOp::FieldGet { object, index, .. })
                        if structs.contains_key(object) && !invalid.contains(object) =>
                    {
                        if let Some(value) = structs
                            .get(object)
                            .and_then(|fields| fields.get(*index))
                            .copied()
                        {
                            aliases.insert(*out, resolve_alias(value, &aliases));
                            changed = true;
                        } else {
                            retained.push(instruction);
                        }
                    }
                    _ => retained.push(instruction),
                }
            }
            block.instructions = retained;
        }
        rewrite_control_flow_function(function, &aliases);
    }
    OptimizationReport {
        pass_name: "scalar-replacement-cfg",
        changed,
    }
}

#[derive(Clone)]
struct LoopCarriedStruct<'src> {
    ty: Type<'src>,
    fields: Vec<ValueId>,
}

#[derive(Clone)]
struct LoopCarriedStructPhi<'src> {
    out: ValueId,
    incoming: Vec<(BlockId, ValueId)>,
    field_types: Vec<Type<'src>>,
    span: Span,
}

/// Explode an immutable, loop-carried value aggregate into one SSA phi per
/// field.  The ordinary scalar replacer deliberately keeps every struct whose
/// value is a phi input, because deleting only the constructor would leave the
/// aggregate phi with a dangling incoming value.  This transform handles the
/// useful closed case atomically: every constructor, aggregate phi, and field
/// read is replaced as one unit, or the original graph is left untouched.
///
/// Keeping this proof narrow is important.  A candidate phi must be a loop
/// header, every incoming value must be a local-only direct struct constructor
/// of the exact same type, and the complete use set of the phi and constructors
/// may contain only those incoming edges and direct field reads.  Mutations,
/// calls, returns, captures, nested aggregate aliases, and branch merges all
/// fall back to the representation-preserving path below.
fn scalar_replace_loop_carried_structs(function: &mut ControlFlowFunction<'_>) -> bool {
    let loop_headers = function
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            crate::ir::ControlShape::Loop { header, .. } => Some(*header),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    if loop_headers.is_empty() {
        return false;
    }

    let value_types = control_flow_value_types(function);
    let structs = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| {
            let out = instruction.out?;
            let ControlFlowOp::Struct { fields, .. } = &instruction.op else {
                return None;
            };
            let ty = instruction.ty.clone()?;
            (function.value_escapes.get(out.0 as usize) == Some(&EscapeState::LocalOnly)).then_some(
                (
                    out,
                    LoopCarriedStruct {
                        ty,
                        fields: fields.clone(),
                    },
                ),
            )
        })
        .collect::<AHashMap<_, _>>();
    if structs.is_empty() {
        return false;
    }

    let tentative = function
        .blocks
        .iter()
        .filter(|block| loop_headers.contains(&block.id))
        .flat_map(|block| &block.phis)
        .filter_map(|phi| {
            if function.value_escapes.get(phi.out.0 as usize) != Some(&EscapeState::LocalOnly)
                || !matches!(phi.ty, Type::Struct(_) | Type::StructInstance { .. })
                || phi.incoming.len() < 2
            {
                return None;
            }
            let incoming_structs = phi
                .incoming
                .iter()
                .map(|(_, value)| structs.get(value))
                .collect::<Option<Vec<_>>>()?;
            let first = *incoming_structs.first()?;
            if first.fields.is_empty()
                || first.ty != phi.ty
                || incoming_structs.iter().any(|candidate| {
                    candidate.ty != phi.ty || candidate.fields.len() != first.fields.len()
                })
            {
                return None;
            }
            let field_types = (0..first.fields.len())
                .map(|index| {
                    let ty = value_types.get(&first.fields[index])?.clone();
                    incoming_structs
                        .iter()
                        .all(|candidate| value_types.get(&candidate.fields[index]) == Some(&ty))
                        .then_some(ty)
                })
                .collect::<Option<Vec<_>>>()?;
            Some(LoopCarriedStructPhi {
                out: phi.out,
                incoming: phi.incoming.clone(),
                field_types,
                span: phi.span,
            })
        })
        .collect::<Vec<_>>();

    let candidates = tentative
        .into_iter()
        .filter(|candidate| {
            aggregate_has_only_field_reads(function, candidate.out, None)
                && candidate.incoming.iter().all(|(_, value)| {
                    aggregate_has_only_field_reads(function, *value, Some(candidate.out))
                })
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return false;
    }

    let mut aggregate_fields = AHashMap::<ValueId, Vec<ValueId>>::default();
    let mut phi_replacements = AHashMap::<ValueId, Vec<Phi<'_>>>::default();
    let mut removed_structs = AHashSet::<ValueId>::default();
    for candidate in candidates {
        let mut scalar_outputs = Vec::with_capacity(candidate.field_types.len());
        for _ in &candidate.field_types {
            let out = ValueId(function.value_count);
            function.value_count += 1;
            function.value_escapes.push(EscapeState::LocalOnly);
            function.value_local_hints.push(None);
            scalar_outputs.push(out);
        }
        let scalar_phis = candidate
            .field_types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| Phi {
                out: scalar_outputs[index],
                origin: crate::ir::PhiOrigin::Synthetic,
                ty,
                incoming: candidate
                    .incoming
                    .iter()
                    .map(|(predecessor, aggregate)| {
                        (*predecessor, structs[aggregate].fields[index])
                    })
                    .collect(),
                span: candidate.span,
            })
            .collect::<Vec<_>>();
        aggregate_fields.insert(candidate.out, scalar_outputs);
        for (_, aggregate) in &candidate.incoming {
            removed_structs.insert(*aggregate);
            aggregate_fields.insert(*aggregate, structs[aggregate].fields.clone());
        }
        phi_replacements.insert(candidate.out, scalar_phis);
    }

    for block in &mut function.blocks {
        let phis = std::mem::take(&mut block.phis);
        for phi in phis {
            if let Some(replacements) = phi_replacements.remove(&phi.out) {
                block.phis.extend(replacements);
            } else {
                block.phis.push(phi);
            }
        }
    }

    let mut aliases = AHashMap::<ValueId, ValueId>::default();
    for block in &mut function.blocks {
        let instructions = std::mem::take(&mut block.instructions);
        for instruction in instructions {
            match (&instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Struct { .. }) if removed_structs.contains(out) => {}
                (Some(out), ControlFlowOp::FieldGet { object, index, .. })
                    if aggregate_fields.contains_key(object) =>
                {
                    aliases.insert(*out, aggregate_fields[object][*index]);
                }
                _ => block.instructions.push(instruction),
            }
        }
    }
    rewrite_control_flow_function(function, &aliases);
    true
}

fn aggregate_has_only_field_reads(
    function: &ControlFlowFunction<'_>,
    aggregate: ValueId,
    allowed_phi: Option<ValueId>,
) -> bool {
    for block in &function.blocks {
        for phi in &block.phis {
            if phi
                .incoming
                .iter()
                .any(|(_, incoming)| *incoming == aggregate)
                && Some(phi.out) != allowed_phi
            {
                return false;
            }
        }
        for instruction in &block.instructions {
            if matches!(instruction.op, ControlFlowOp::FieldGet { object, .. } if object == aggregate)
            {
                continue;
            }
            if control_flow_used_values(&instruction.op).contains(&aggregate) {
                return false;
            }
        }
        if block
            .terminator
            .as_ref()
            .is_some_and(|terminator| terminator_used_values(terminator).contains(&aggregate))
        {
            return false;
        }
    }
    !function.shapes.iter().any(|shape| match shape {
        crate::ir::ControlShape::ForIn { object, key, .. } => {
            *object == aggregate || *key == aggregate
        }
        crate::ir::ControlShape::ForOf {
            iterable, element, ..
        } => *iterable == aggregate || *element == aggregate,
        crate::ir::ControlShape::Try {
            catch_value: Some(value),
            ..
        } => *value == aggregate,
        _ => false,
    })
}

fn has_exception_region(function: &ControlFlowFunction<'_>) -> bool {
    function
        .shapes
        .iter()
        .any(|shape| matches!(shape, crate::ir::ControlShape::Try { .. }))
}

fn eliminate_dead_control_flow_instructions(
    module: &mut ControlFlowModule<'_>,
) -> OptimizationReport {
    let mut changed = false;
    let effect_summaries = analyze_function_effects(module);
    let effectful_functions = effectful_functions(&effect_summaries);
    for function in &mut module.functions {
        let closure_targets = closure_targets(function);
        let dynamic_observable_values = dynamic_observable_values(function);
        loop {
            let uses = control_flow_use_counts(function);
            let (mutable_roots, unobserved_mutables) =
                unobserved_local_mutables(function, &uses, &effect_summaries);
            let mut local_change = false;
            for block in &mut function.blocks {
                let old_len = block.instructions.len();
                block.instructions.retain(|instruction| {
                    let result_is_used = instruction
                        .out
                        .is_some_and(|out| uses.get(&out).copied().unwrap_or(0) != 0);
                    let mutates_only_unobserved_state = mutation_receiver(&instruction.op)
                        .and_then(|receiver| mutable_roots.get(&receiver))
                        .is_some_and(|root| unobserved_mutables.contains(root))
                        || direct_call_mutates_only_unobserved_state(
                            &instruction.op,
                            &effect_summaries,
                            &mutable_roots,
                            &unobserved_mutables,
                        );
                    let has_dynamic_observable_evaluation = instruction
                        .out
                        .is_some_and(|out| dynamic_observable_values.contains(&out));
                    result_is_used
                        || has_dynamic_observable_evaluation
                        || (!mutates_only_unobserved_state
                            && control_flow_op_has_side_effects(
                                &instruction.op,
                                &effectful_functions,
                                &closure_targets,
                            ))
                });
                local_change |= block.instructions.len() != old_len;

                let old_phi_len = block.phis.len();
                block
                    .phis
                    .retain(|phi| uses.get(&phi.out).copied().unwrap_or(0) != 0);
                local_change |= block.phis.len() != old_phi_len;
            }
            changed |= local_change;
            if !local_change {
                break;
            }
        }
    }
    OptimizationReport {
        pass_name: "ssa-dead-code-elimination",
        changed,
    }
}

fn unobserved_local_mutables(
    function: &ControlFlowFunction<'_>,
    uses: &AHashMap<ValueId, usize>,
    effect_summaries: &[FunctionEffectSummary],
) -> (AHashMap<ValueId, ValueId>, AHashSet<ValueId>) {
    let mut roots = local_mutable_roots(function, is_local_mutable_allocation);
    if roots.is_empty() {
        return (roots, AHashSet::default());
    }

    extend_mutable_alias_roots(function, &mut roots);

    let mut observed = AHashSet::default();
    let mut mutation_groups = Vec::new();
    let mut observe = |value: ValueId| {
        if let Some(root) = roots.get(&value) {
            observed.insert(*root);
        }
    };
    for block in &function.blocks {
        for phi in &block.phis {
            if !roots.contains_key(&phi.out) {
                for (_, value) in &phi.incoming {
                    observe(*value);
                }
            }
        }
        for instruction in &block.instructions {
            match &instruction.op {
                ControlFlowOp::CallDirect { function, args, .. }
                    if instruction
                        .out
                        .is_none_or(|out| uses.get(&out).copied().unwrap_or(0) == 0) =>
                {
                    let summary = effect_summaries.get(function.0 as usize);
                    let mutation_roots = summary
                        .filter(|summary| !summary.inherent)
                        .and_then(|summary| local_mutation_roots(summary, args, &roots));
                    let call_is_locally_discardable = summary
                        .is_some_and(|summary| !summary.inherent && mutation_roots.is_some());
                    if let Some(group) = mutation_roots.filter(|group| !group.is_empty()) {
                        mutation_groups.push(group);
                    } else if !call_is_locally_discardable {
                        for value in args {
                            observe(*value);
                        }
                    }
                }
                ControlFlowOp::IndexSet {
                    object,
                    index,
                    value,
                } if roots.contains_key(object) => {
                    observe(*index);
                    observe(*value);
                }
                ControlFlowOp::Intrinsic {
                    intrinsic:
                        intrinsic @ (Intrinsic::ArrayPush
                        | Intrinsic::ArrayPop
                        | Intrinsic::ArraySplice
                        | Intrinsic::ArrayFill
                        | Intrinsic::ArrayCopyWithin
                        | Intrinsic::ArrayReverse
                        | Intrinsic::TypedArraySet
                        | Intrinsic::TypedArrayFill
                        | Intrinsic::TypedArrayCopyWithin
                        | Intrinsic::MapSet
                        | Intrinsic::MapDelete
                        | Intrinsic::MapClear
                        | Intrinsic::SetAdd
                        | Intrinsic::SetDelete
                        | Intrinsic::SetClear),
                    receiver: Some(receiver),
                    args,
                } if roots.contains_key(receiver) => {
                    for value in args {
                        observe(*value);
                    }
                    let fluent_alias = matches!(
                        intrinsic,
                        Intrinsic::ArrayFill
                            | Intrinsic::ArrayCopyWithin
                            | Intrinsic::ArrayReverse
                            | Intrinsic::TypedArrayFill
                            | Intrinsic::TypedArrayCopyWithin
                            | Intrinsic::MapSet
                            | Intrinsic::SetAdd
                    );
                    if !fluent_alias
                        && instruction
                            .out
                            .is_some_and(|out| uses.get(&out).copied().unwrap_or(0) != 0)
                    {
                        observe(*receiver);
                    }
                }
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::UnwrapNullable | Intrinsic::UnwrapUnion,
                    receiver: Some(receiver),
                    ..
                } if roots.contains_key(receiver)
                    && instruction.out.is_some_and(|out| roots.contains_key(&out)) => {}
                op => {
                    for value in control_flow_used_values(op) {
                        observe(value);
                    }
                }
            }
        }
        if let Some(terminator) = &block.terminator {
            for value in terminator_used_values(terminator) {
                observe(value);
            }
        }
    }
    loop {
        let mut changed = false;
        for group in &mutation_groups {
            if group.iter().any(|root| observed.contains(root)) {
                for root in group {
                    changed |= observed.insert(*root);
                }
            }
        }
        if !changed {
            break;
        }
    }
    let all_roots = roots.values().copied().collect::<AHashSet<_>>();
    let unobserved = all_roots.difference(&observed).copied().collect();
    (roots, unobserved)
}

fn local_mutable_roots(
    function: &ControlFlowFunction<'_>,
    is_seed: fn(&ControlFlowOp<'_>) -> bool,
) -> AHashMap<ValueId, ValueId> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| {
            let out = instruction.out?;
            is_seed(&instruction.op).then_some((out, out))
        })
        .collect()
}

fn extend_mutable_alias_roots<Root: Copy + Eq>(
    function: &ControlFlowFunction<'_>,
    roots: &mut AHashMap<ValueId, Root>,
) {
    loop {
        let mut changed = false;
        for block in &function.blocks {
            for phi in &block.phis {
                let mut incoming = phi
                    .incoming
                    .iter()
                    .filter_map(|(_, value)| roots.get(value).copied());
                let Some(root) = incoming.next() else {
                    continue;
                };
                if incoming.all(|candidate| candidate == root)
                    && phi
                        .incoming
                        .iter()
                        .all(|(_, value)| roots.get(value) == Some(&root))
                    && roots.insert(phi.out, root).is_none()
                {
                    changed = true;
                }
            }
            for instruction in &block.instructions {
                let Some(out) = instruction.out else {
                    continue;
                };
                let receiver = match instruction.op {
                    ControlFlowOp::Intrinsic {
                        intrinsic:
                            Intrinsic::ArrayFill
                            | Intrinsic::ArrayCopyWithin
                            | Intrinsic::ArrayReverse
                            | Intrinsic::TypedArrayFill
                            | Intrinsic::TypedArrayCopyWithin
                            | Intrinsic::MapSet
                            | Intrinsic::SetAdd,
                        receiver: Some(receiver),
                        ..
                    } => Some(receiver),
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::UnwrapNullable | Intrinsic::UnwrapUnion,
                        receiver: Some(receiver),
                        ..
                    } => Some(receiver),
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::RecordAssign,
                        ref args,
                        ..
                    } => args.first().copied(),
                    _ => None,
                };
                if let Some(root) = receiver.and_then(|receiver| roots.get(&receiver).copied()) {
                    if roots.insert(out, root).is_none() {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}

fn is_local_mutable_allocation(op: &ControlFlowOp<'_>) -> bool {
    matches!(
        op,
        ControlFlowOp::Array(_)
            | ControlFlowOp::ArraySpread(_)
            | ControlFlowOp::Record(_)
            | ControlFlowOp::RecordSpread(_)
            | ControlFlowOp::RecordRest { .. }
            | ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::MapNew
                    | Intrinsic::SetNew
                    | Intrinsic::RegexNew
                    | Intrinsic::ArrayBufferNew
                    | Intrinsic::SharedArrayBufferNew
                    | Intrinsic::JsPlainObject
                    | Intrinsic::JsObjectCreate
                    | Intrinsic::JsNullProtoObject,
                ..
            }
    ) || matches!(
        op,
        ControlFlowOp::Intrinsic { intrinsic, .. }
            if matches!(
                crate::typed_array::classify_typed_array_intrinsic(*intrinsic),
                Some((_, crate::typed_array::TypedArrayIntrinsic::New))
            )
    )
}

fn is_owned_mutable_allocation(op: &ControlFlowOp<'_>) -> bool {
    is_local_mutable_allocation(op)
        || matches!(
            op,
            ControlFlowOp::Struct { .. } | ControlFlowOp::NewClass { .. }
        )
}

fn mutation_receiver(op: &ControlFlowOp<'_>) -> Option<ValueId> {
    match op {
        ControlFlowOp::IndexSet { object, .. }
        | ControlFlowOp::FieldSet { object, .. }
        | ControlFlowOp::RecordFieldSet { object, .. } => Some(*object),
        ControlFlowOp::Intrinsic {
            intrinsic:
                Intrinsic::ArrayPush
                | Intrinsic::ArrayPop
                | Intrinsic::ArraySplice
                | Intrinsic::ArrayFill
                | Intrinsic::ArrayCopyWithin
                | Intrinsic::ArrayReverse
                | Intrinsic::TypedArraySet
                | Intrinsic::TypedArrayFill
                | Intrinsic::TypedArrayCopyWithin
                | Intrinsic::MapSet
                | Intrinsic::MapDelete
                | Intrinsic::MapClear
                | Intrinsic::SetAdd
                | Intrinsic::SetDelete
                | Intrinsic::SetClear
                | Intrinsic::RegexTest
                | Intrinsic::JsRegexExec
                | Intrinsic::StringSearch
                | Intrinsic::StringReplace
                | Intrinsic::JsDeleteProperty
                | Intrinsic::JsArrayPush
                | Intrinsic::JsArrayPop
                | Intrinsic::JsArraySort
                | Intrinsic::JsArraySplice
                | Intrinsic::JsArrayShift
                | Intrinsic::JsArrayUnshift,
            receiver,
            ..
        } => *receiver,
        ControlFlowOp::Intrinsic {
            intrinsic: Intrinsic::RecordAssign,
            args,
            ..
        } => args.first().copied(),
        _ => None,
    }
}

fn local_mutation_roots(
    summary: &FunctionEffectSummary,
    args: &[ValueId],
    roots: &AHashMap<ValueId, ValueId>,
) -> Option<AHashSet<ValueId>> {
    let mut mutation_roots = AHashSet::default();
    for parameter in &summary.mutated_parameters {
        let root = args
            .get(*parameter)
            .and_then(|value| roots.get(value))
            .copied()?;
        mutation_roots.insert(root);
    }
    Some(mutation_roots)
}

fn direct_call_mutates_only_unobserved_state(
    op: &ControlFlowOp<'_>,
    summaries: &[FunctionEffectSummary],
    roots: &AHashMap<ValueId, ValueId>,
    unobserved: &AHashSet<ValueId>,
) -> bool {
    let ControlFlowOp::CallDirect { function, args, .. } = op else {
        return false;
    };
    let Some(summary) = summaries.get(function.0 as usize) else {
        return false;
    };
    !summary.inherent
        && !summary.mutated_parameters.is_empty()
        && local_mutation_roots(summary, args, roots).is_some_and(|mutation_roots| {
            mutation_roots.iter().all(|root| unobserved.contains(root))
        })
}

const FUNCTION_SUBSUMPTION_COMPARISON_LIMIT: usize = 16_384;
const FUNCTION_SUBSUMPTION_PAIR_LIMIT: usize = 65_536;
const FUNCTION_SUBSUMPTION_BINDING_LIMIT: usize = 64;
const FUNCTION_SUBSUMPTION_MAX_BOUND_PARAMETERS: usize = 3;

#[derive(Debug, Clone)]
struct PrivateFunctionSubsumption<'src> {
    source: FunctionId,
    target: FunctionId,
    target_parameter_count: usize,
    bound_arguments: Vec<(usize, SpecializationValue, Type<'src>)>,
}

fn subsume_private_functions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    let mut comparisons = 0;
    while comparisons < FUNCTION_SUBSUMPTION_COMPARISON_LIMIT {
        let Some(candidate) = find_private_function_subsumption(module, &mut comparisons) else {
            break;
        };
        apply_private_function_subsumption(module, candidate);
        changed = true;
    }
    OptimizationReport {
        pass_name: "private-function-subsumption",
        changed,
    }
}

fn find_private_function_subsumption<'src>(
    module: &ControlFlowModule<'src>,
    comparisons: &mut usize,
) -> Option<PrivateFunctionSubsumption<'src>> {
    let direct_only = private_direct_call_functions(module);
    let mut pairs = Vec::new();
    let mut pair_checks = 0;
    'sources: for source in module
        .functions
        .iter()
        .filter(|function| direct_only.contains(&function.id))
    {
        for target in module
            .functions
            .iter()
            .filter(|function| direct_only.contains(&function.id))
        {
            if pair_checks == FUNCTION_SUBSUMPTION_PAIR_LIMIT {
                break 'sources;
            }
            pair_checks += 1;
            let Some(extra) = target.params.len().checked_sub(source.params.len()) else {
                continue;
            };
            if source.id == target.id
                || extra == 0
                || extra > FUNCTION_SUBSUMPTION_MAX_BOUND_PARAMETERS
                || source.declared_pure != target.declared_pure
                || source.capture_count != target.capture_count
                || source.return_type != target.return_type
                || source.blocks.len() != target.blocks.len()
                || source.shapes.len() != target.shapes.len()
            {
                continue;
            }
            pairs.push((
                extra,
                source.id.0.abs_diff(target.id.0),
                source.id,
                target.id,
            ));
        }
    }
    pairs.sort_unstable();

    let mut normalized_sources = AHashMap::default();
    for (_, _, source_id, target_id) in pairs {
        if *comparisons >= FUNCTION_SUBSUMPTION_COMPARISON_LIMIT {
            break;
        }
        let source = &module.functions[source_id.0 as usize];
        let target = &module.functions[target_id.0 as usize];
        let signatures = function_subsumption_signatures(module, source, target);
        if signatures.is_empty() {
            continue;
        }
        let source_body = normalized_sources
            .entry(source_id)
            .or_insert_with(|| normalize_private_function(source));
        for signature in signatures {
            if *comparisons >= FUNCTION_SUBSUMPTION_COMPARISON_LIMIT {
                break;
            }
            *comparisons += 1;
            let specialized = clone_function_with_specialization(target, target.id, &signature);
            let specialized_body = normalize_private_function(&specialized);
            if specialized_body != *source_body {
                continue;
            }
            let bound_arguments = signature
                .into_iter()
                .map(|(index, value)| (index, value, target.params[index].ty.clone()))
                .collect();
            return Some(PrivateFunctionSubsumption {
                source: source_id,
                target: target_id,
                target_parameter_count: target.params.len(),
                bound_arguments,
            });
        }
    }
    None
}

fn function_subsumption_signatures(
    module: &ControlFlowModule<'_>,
    source: &ControlFlowFunction<'_>,
    target: &ControlFlowFunction<'_>,
) -> Vec<Vec<(usize, SpecializationValue)>> {
    let constants = source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match (&instruction.op, &instruction.ty) {
            (ControlFlowOp::Const(value), Some(ty))
                if constant_has_direct_native_representation(value, ty) =>
            {
                Some((ConstantKey::from_value(value), ty))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut functions = source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match &instruction.op {
            ControlFlowOp::CallDirect { function, .. }
                if *function != source.id && *function != target.id =>
            {
                Some(*function)
            }
            ControlFlowOp::Closure { function, captures }
                if captures.is_empty() && *function != source.id && *function != target.id =>
            {
                Some(*function)
            }
            _ => None,
        })
        .filter(|function| {
            module
                .functions
                .get(function.0 as usize)
                .is_some_and(|function| {
                    function.live
                        && matches!(function.kind, FunctionKind::Function | FunctionKind::Extern)
                })
        })
        .collect::<Vec<_>>();
    functions.sort_unstable();
    functions.dedup();
    let mut signatures = Vec::new();
    for positions in function_subsumption_binding_positions(source, target) {
        let mut position_signatures = vec![Vec::new()];
        for index in positions {
            let parameter = &target.params[index];
            let mut compatible = constants
                .iter()
                .filter_map(|(value, ty)| {
                    (ty == &&parameter.ty).then_some(SpecializationValue::Constant(value.clone()))
                })
                .collect::<Vec<_>>();
            if let Type::Function(signature) = &parameter.ty {
                compatible.extend(functions.iter().filter_map(|function| {
                    function_matches_signature(&module.functions[function.0 as usize], signature)
                        .then_some(SpecializationValue::Function(*function))
                }));
            }
            compatible.sort();
            compatible.dedup();
            if compatible.is_empty() {
                position_signatures.clear();
                break;
            }
            let mut expanded = Vec::new();
            for signature in position_signatures {
                for value in &compatible {
                    let mut candidate = signature.clone();
                    candidate.push((index, value.clone()));
                    expanded.push(candidate);
                    if signatures.len() + expanded.len() == FUNCTION_SUBSUMPTION_BINDING_LIMIT {
                        break;
                    }
                }
                if signatures.len() + expanded.len() == FUNCTION_SUBSUMPTION_BINDING_LIMIT {
                    break;
                }
            }
            position_signatures = expanded;
        }
        signatures.extend(position_signatures);
        if signatures.len() == FUNCTION_SUBSUMPTION_BINDING_LIMIT {
            break;
        }
    }
    signatures
}

fn function_matches_signature(
    function: &ControlFlowFunction<'_>,
    signature: &crate::semantic::FunctionType<'_>,
) -> bool {
    function.params.len() == signature.params.len()
        && function
            .params
            .iter()
            .zip(&signature.params)
            .all(|(parameter, expected)| parameter.ty == *expected)
        && function.return_type == *signature.return_type
}

fn function_subsumption_binding_positions(
    source: &ControlFlowFunction<'_>,
    target: &ControlFlowFunction<'_>,
) -> Vec<Vec<usize>> {
    fn search(
        source: &ControlFlowFunction<'_>,
        target: &ControlFlowFunction<'_>,
        source_index: usize,
        target_index: usize,
        required_bindings: usize,
        bindings: &mut Vec<usize>,
        results: &mut Vec<Vec<usize>>,
    ) {
        if results.len() == FUNCTION_SUBSUMPTION_BINDING_LIMIT {
            return;
        }
        if target_index == target.params.len() {
            if source_index == source.params.len() && bindings.len() == required_bindings {
                results.push(bindings.clone());
            }
            return;
        }
        if source_index < source.params.len()
            && source.params[source_index].ty == target.params[target_index].ty
        {
            search(
                source,
                target,
                source_index + 1,
                target_index + 1,
                required_bindings,
                bindings,
                results,
            );
        }
        if bindings.len() < required_bindings {
            bindings.push(target_index);
            search(
                source,
                target,
                source_index,
                target_index + 1,
                required_bindings,
                bindings,
                results,
            );
            bindings.pop();
        }
    }

    let mut results = Vec::new();
    search(
        source,
        target,
        0,
        0,
        target.params.len() - source.params.len(),
        &mut Vec::new(),
        &mut results,
    );
    results
}

fn apply_private_function_subsumption<'src>(
    module: &mut ControlFlowModule<'src>,
    candidate: PrivateFunctionSubsumption<'src>,
) {
    for caller in &mut module.functions {
        let mut next_value = caller.value_count;
        let mut added_values = 0;
        for block in &mut caller.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut rewritten = Vec::with_capacity(instructions.len());
            for mut instruction in instructions {
                let is_source_call = matches!(
                    instruction.op,
                    ControlFlowOp::CallDirect { function, .. }
                        if function == candidate.source
                );
                if is_source_call {
                    let mut bound_values = Vec::with_capacity(candidate.bound_arguments.len());
                    for (index, value, ty) in &candidate.bound_arguments {
                        let out = ValueId(next_value);
                        next_value += 1;
                        added_values += 1;
                        rewritten.push(ControlFlowInstruction {
                            out: Some(out),
                            ty: Some(ty.clone()),
                            op: match value {
                                SpecializationValue::Constant(value) => {
                                    ControlFlowOp::Const(value.to_value())
                                }
                                SpecializationValue::Function(function) => ControlFlowOp::Closure {
                                    function: *function,
                                    captures: Vec::new(),
                                },
                            },
                            span: instruction.span,
                        });
                        bound_values.push((*index, out));
                    }
                    let ControlFlowOp::CallDirect {
                        function,
                        args,
                        provided_args,
                    } = &mut instruction.op
                    else {
                        unreachable!()
                    };
                    *function = candidate.target;
                    let mut source_arguments = std::mem::take(args).into_iter();
                    *args = (0..candidate.target_parameter_count)
                        .map(|index| {
                            bound_values
                                .iter()
                                .find_map(|(bound, value)| (*bound == index).then_some(*value))
                                .unwrap_or_else(|| {
                                    source_arguments
                                        .next()
                                        .expect("subsumed calls must match source arity")
                                })
                        })
                        .collect();
                    *provided_args = args.len();
                    debug_assert!(source_arguments.next().is_none());
                }
                rewritten.push(instruction);
            }
            block.instructions = rewritten;
        }
        caller.value_count = next_value;
        caller
            .value_escapes
            .extend(std::iter::repeat_n(EscapeState::LocalOnly, added_values));
        caller
            .value_local_hints
            .extend(std::iter::repeat_n(None, added_values));
    }
    module.functions[candidate.source.0 as usize].live = false;
}

fn private_direct_call_functions(module: &ControlFlowModule<'_>) -> AHashSet<FunctionId> {
    let exported = exported_functions(module);
    let mut direct_only = module
        .functions
        .iter()
        .filter(|function| {
            function.live
                && function.kind == FunctionKind::Function
                // A repeated-region helper is a deliberately scored reuse
                // boundary. Post-outline merging could redirect its calls to
                // an ordinary representative and erase the provenance that
                // emission-level helper search needs to preserve it.
                && function.origin != FunctionOrigin::RepeatedRegionOutline
                && function.mutable_capture_locals.is_empty()
                && !function.is_async
                && !function.is_generator
                && !exported.contains(&function.id)
        })
        .map(|function| function.id)
        .collect::<AHashSet<_>>();
    for function in module.functions.iter().filter(|function| function.live) {
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            match instruction.op {
                ControlFlowOp::Closure { function, .. }
                | ControlFlowOp::CallMethod { function, .. } => {
                    direct_only.remove(&function);
                }
                ControlFlowOp::NewClass {
                    constructor: Some(function),
                    ..
                } => {
                    direct_only.remove(&function);
                }
                _ => {}
            }
        }
    }
    direct_only
}

fn merge_permuted_private_functions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    const MAX_ARITY: usize = 4;
    let mut changed = false;
    let mut redirects = AHashMap::<FunctionId, (FunctionId, Vec<usize>)>::default();
    let direct_only = private_direct_call_functions(module);
    let mut ids = direct_only.iter().copied().collect::<Vec<_>>();
    ids.sort_unstable();
    for index in 0..ids.len() {
        if redirects.contains_key(&ids[index]) {
            continue;
        }
        let left = &module.functions[ids[index].0 as usize];
        if left.params.is_empty() || left.params.len() > MAX_ARITY {
            continue;
        }
        for right_id in ids.iter().copied().skip(index + 1) {
            if redirects.contains_key(&right_id) {
                continue;
            }
            let right = &module.functions[right_id.0 as usize];
            if left.params.len() != right.params.len()
                || left.declared_pure != right.declared_pure
                || left.capture_count != right.capture_count
                || left.return_type != right.return_type
            {
                continue;
            }
            let same_order = left
                .params
                .iter()
                .zip(&right.params)
                .all(|(left_param, right_param)| left_param.ty == right_param.ty);
            if same_order {
                continue;
            }
            let mut left_types = left
                .params
                .iter()
                .map(|parameter| format!("{:?}", parameter.ty))
                .collect::<Vec<_>>();
            let mut right_types = right
                .params
                .iter()
                .map(|parameter| format!("{:?}", parameter.ty))
                .collect::<Vec<_>>();
            left_types.sort_unstable();
            right_types.sort_unstable();
            if left_types != right_types {
                continue;
            }
            let arity = left.params.len();
            let mut permutation = (0..arity).collect::<Vec<_>>();
            let mut found = None;
            loop {
                if permutation
                    .iter()
                    .enumerate()
                    .all(|(target, &source)| left.params[source].ty == right.params[target].ty)
                {
                    let mut reordered = left.clone();
                    reordered.params = permutation
                        .iter()
                        .map(|&source| left.params[source].clone())
                        .collect();
                    if normalize_private_function(&reordered) == normalize_private_function(right) {
                        found = Some(permutation.clone());
                        break;
                    }
                }
                if !next_permutation(&mut permutation) {
                    break;
                }
            }
            if let Some(permutation) = found {
                redirects.insert(ids[index], (right_id, permutation));
                break;
            }
        }
    }
    if redirects.is_empty() {
        return OptimizationReport {
            pass_name: "permuted-private-function-merging",
            changed: false,
        };
    }
    for function in &mut module.functions {
        for instruction in function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
        {
            let ControlFlowOp::CallDirect {
                function: callee,
                args,
                provided_args,
            } = &mut instruction.op
            else {
                continue;
            };
            let Some((target, permutation)) = redirects.get(callee) else {
                continue;
            };
            *callee = *target;
            let original = args.clone();
            *args = permutation.iter().map(|&source| original[source]).collect();
            *provided_args = args.len();
            changed = true;
        }
    }
    for duplicate in redirects.keys() {
        module.functions[duplicate.0 as usize].live = false;
    }
    OptimizationReport {
        pass_name: "permuted-private-function-merging",
        changed,
    }
}

fn next_permutation(values: &mut [usize]) -> bool {
    let Some(pivot) = (0..values.len().saturating_sub(1))
        .rev()
        .find(|&index| values[index] < values[index + 1])
    else {
        return false;
    };
    let swap = (pivot + 1..values.len())
        .rev()
        .find(|&index| values[pivot] < values[index])
        .expect("pivot guarantees a successor");
    values.swap(pivot, swap);
    values[pivot + 1..].reverse();
    true
}

fn merge_single_operand_private_functions(
    module: &mut ControlFlowModule<'_>,
) -> OptimizationReport {
    const COMPARISON_LIMIT: usize = 64;
    let direct_only = private_direct_call_functions(module);
    let mut ids = direct_only.iter().copied().collect::<Vec<_>>();
    ids.sort_unstable();
    let mut comparisons = 0;
    for index in 0..ids.len() {
        if !module.functions[ids[index].0 as usize].live {
            continue;
        }
        for right_index in index + 1..ids.len() {
            if comparisons == COMPARISON_LIMIT {
                return OptimizationReport {
                    pass_name: "single-operand-private-function-merging",
                    changed: false,
                };
            }
            comparisons += 1;
            let left_id = ids[index];
            let right_id = ids[right_index];
            if !module.functions[right_id.0 as usize].live {
                continue;
            }
            let left = &module.functions[left_id.0 as usize];
            let right = &module.functions[right_id.0 as usize];
            if left.params.len() != right.params.len()
                || left.declared_pure != right.declared_pure
                || left.capture_count != right.capture_count
                || left.return_type != right.return_type
                || left.blocks.len() != right.blocks.len()
                || !left
                    .params
                    .iter()
                    .zip(&right.params)
                    .all(|(left_param, right_param)| left_param.ty == right_param.ty)
            {
                continue;
            }
            let Some((block_index, instruction_index, left_const, right_const, ty)) =
                single_const_divergence(left, right)
            else {
                continue;
            };
            if left_const == right_const {
                continue;
            }
            let shared_id = synthesize_shared_constant_parameter_function(
                module,
                left_id,
                block_index,
                instruction_index,
                ty,
            );
            rewrite_calls_with_bound_constant(module, left_id, shared_id, left_const);
            rewrite_calls_with_bound_constant(module, right_id, shared_id, right_const);
            module.functions[left_id.0 as usize].live = false;
            module.functions[right_id.0 as usize].live = false;
            return OptimizationReport {
                pass_name: "single-operand-private-function-merging",
                changed: true,
            };
        }
    }
    OptimizationReport {
        pass_name: "single-operand-private-function-merging",
        changed: false,
    }
}

fn single_const_divergence<'src>(
    left: &ControlFlowFunction<'src>,
    right: &ControlFlowFunction<'src>,
) -> Option<(usize, usize, ConstValue, ConstValue, Type<'src>)> {
    let mut divergence = None;
    for (block_index, (left_block, right_block)) in
        left.blocks.iter().zip(&right.blocks).enumerate()
    {
        if left_block.instructions.len() != right_block.instructions.len()
            || left_block.phis.len() != right_block.phis.len()
            || std::mem::discriminant(&left_block.terminator)
                != std::mem::discriminant(&right_block.terminator)
        {
            return None;
        }
        for (instruction_index, (left_instruction, right_instruction)) in left_block
            .instructions
            .iter()
            .zip(&right_block.instructions)
            .enumerate()
        {
            match (&left_instruction.op, &right_instruction.op) {
                (ControlFlowOp::Const(left_const), ControlFlowOp::Const(right_const))
                    if left_const != right_const =>
                {
                    if divergence.is_some() {
                        return None;
                    }
                    let ty = left_instruction
                        .ty
                        .clone()
                        .or_else(|| right_instruction.ty.clone())?;
                    if left_instruction.ty.as_ref() != right_instruction.ty.as_ref() {
                        return None;
                    }
                    if !matches!(
                        left_const,
                        ConstValue::Int(_) | ConstValue::Bool(_) | ConstValue::Float(_)
                    ) {
                        return None;
                    }
                    divergence = Some((
                        block_index,
                        instruction_index,
                        left_const.clone(),
                        right_const.clone(),
                        ty,
                    ));
                }
                (left_op, right_op)
                    if std::mem::discriminant(left_op) != std::mem::discriminant(right_op) =>
                {
                    return None;
                }
                _ => {}
            }
        }
    }
    divergence
}

fn synthesize_shared_constant_parameter_function<'src>(
    module: &mut ControlFlowModule<'src>,
    template: FunctionId,
    block_index: usize,
    instruction_index: usize,
    ty: Type<'src>,
) -> FunctionId {
    let id = FunctionId(module.functions.len() as u32);
    let mut function = module.functions[template.0 as usize].clone();
    function.id = id;
    function.name = None;
    function.live = true;
    let param_value = ValueId(function.value_count);
    function.value_count += 1;
    function.value_escapes.push(EscapeState::LocalOnly);
    function.value_local_hints.push(None);
    let param_local = LocalId(function.locals.len() as u32);
    function.locals.push(IrLocal {
        id: param_local,
        symbol: SymbolId((function.params.len() + function.locals.len()) as u32),
        name: "",
        ty: ty.clone(),
        span: Span::empty(0),
    });
    function.params.push(crate::ir::IrParameter {
        symbol: SymbolId(function.params.len() as u32),
        local: param_local,
        value: param_value,
        name: "",
        ty,
        default: None,
        span: Span::empty(0),
    });
    let replaced = function.blocks.get_mut(block_index).and_then(|block| {
        if instruction_index >= block.instructions.len() {
            return None;
        }
        Some(block.instructions.remove(instruction_index))
    });
    if let Some(instruction) = replaced {
        if let Some(old) = instruction.out {
            let mut aliases = AHashMap::default();
            aliases.insert(old, param_value);
            rewrite_control_flow_function(&mut function, &aliases);
        }
    }
    module.functions.push(function);
    id
}

fn rewrite_calls_with_bound_constant(
    module: &mut ControlFlowModule<'_>,
    source: FunctionId,
    target: FunctionId,
    constant: ConstValue,
) {
    for function in &mut module.functions {
        let mut next_value = function.value_count;
        let mut added = 0;
        for block in &mut function.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut rewritten = Vec::with_capacity(instructions.len());
            for mut instruction in instructions {
                if let ControlFlowOp::CallDirect {
                    function: callee,
                    args,
                    provided_args,
                } = &mut instruction.op
                {
                    if *callee == source {
                        let const_value = ValueId(next_value);
                        next_value += 1;
                        added += 1;
                        rewritten.push(ControlFlowInstruction {
                            out: Some(const_value),
                            ty: Some(match &constant {
                                ConstValue::Int(_) => Type::Int,
                                ConstValue::Bool(_) => Type::Bool,
                                ConstValue::Float(_) => Type::Float,
                                _ => Type::Int,
                            }),
                            span: instruction.span,
                            op: ControlFlowOp::Const(constant.clone()),
                        });
                        *callee = target;
                        args.push(const_value);
                        *provided_args = args.len();
                    }
                }
                rewritten.push(instruction);
            }
            block.instructions = rewritten;
        }
        function.value_count = next_value;
        function
            .value_escapes
            .extend(std::iter::repeat_n(EscapeState::LocalOnly, added));
        function
            .value_local_hints
            .extend(std::iter::repeat_n(None, added));
    }
}

fn fold_identical_private_functions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;

    loop {
        let direct_only = private_direct_call_functions(module);

        if direct_only.len() < 2 {
            break;
        }
        let mut groups = AHashMap::<PrivateFunctionShape, Vec<FunctionId>>::default();
        for function in module
            .functions
            .iter()
            .filter(|function| direct_only.contains(&function.id))
        {
            groups
                .entry(private_function_shape(function))
                .or_default()
                .push(function.id);
        }
        let mut redirects = AHashMap::<FunctionId, FunctionId>::default();
        for group in groups.values().filter(|group| group.len() > 1) {
            let normalized = group
                .iter()
                .map(|function| {
                    (
                        *function,
                        normalize_private_function(&module.functions[function.0 as usize]),
                    )
                })
                .collect::<Vec<_>>();
            let mut representatives = Vec::<usize>::new();
            for (index, (function, body)) in normalized.iter().enumerate() {
                if let Some(representative) = representatives
                    .iter()
                    .copied()
                    .find(|representative| normalized[*representative].1 == *body)
                {
                    redirects.insert(*function, normalized[representative].0);
                } else {
                    representatives.push(index);
                }
            }
        }
        if redirects.is_empty() {
            break;
        }

        for function in &mut module.functions {
            for instruction in function
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
            {
                rewrite_function_reference(&mut instruction.op, &redirects);
            }
        }
        for duplicate in redirects.keys() {
            module.functions[duplicate.0 as usize].live = false;
        }
        changed = true;
    }

    OptimizationReport {
        pass_name: "identical-private-function-folding",
        changed,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PrivateFunctionShape {
    declared_pure: bool,
    parameter_count: usize,
    local_count: usize,
    capture_count: usize,
    shape_count: usize,
    blocks: Vec<(usize, usize, usize, u64)>,
}

fn private_function_shape(function: &ControlFlowFunction<'_>) -> PrivateFunctionShape {
    let blocks = function
        .blocks
        .iter()
        .map(|block| {
            let mut hasher = DefaultHasher::new();
            for phi in &block.phis {
                phi.incoming.len().hash(&mut hasher);
            }
            for instruction in &block.instructions {
                std::mem::discriminant(&instruction.op).hash(&mut hasher);
                instruction.out.is_some().hash(&mut hasher);
            }
            block
                .terminator
                .as_ref()
                .map(std::mem::discriminant)
                .hash(&mut hasher);
            (
                block.phis.len(),
                block.instructions.len(),
                block.phis.iter().map(|phi| phi.incoming.len()).sum(),
                hasher.finish(),
            )
        })
        .collect();
    PrivateFunctionShape {
        declared_pure: function.declared_pure,
        parameter_count: function.params.len(),
        local_count: function.locals.len(),
        capture_count: function.capture_count,
        shape_count: function.shapes.len(),
        blocks,
    }
}

fn rewrite_function_reference(
    op: &mut ControlFlowOp<'_>,
    redirects: &AHashMap<FunctionId, FunctionId>,
) {
    let reference = match op {
        ControlFlowOp::NewClass {
            constructor: Some(function),
            ..
        }
        | ControlFlowOp::Closure { function, .. }
        | ControlFlowOp::CallDirect { function, .. }
        | ControlFlowOp::CallMethod { function, .. } => function,
        _ => return,
    };
    while let Some(representative) = redirects.get(reference) {
        *reference = *representative;
    }
}

fn normalize_private_function<'src>(
    function: &ControlFlowFunction<'src>,
) -> ControlFlowFunction<'src> {
    const SELF_REFERENCE: FunctionId = FunctionId(u32::MAX);
    let mut normalized = function.clone();
    let empty = Span::empty(0);
    let original_id = normalized.id;
    let original_value_escapes = normalized.value_escapes.clone();
    normalized.id = SELF_REFERENCE;
    normalized.name = None;
    normalized.span = empty;
    normalized.live = true;
    canonicalize_private_known_closure_calls(&mut normalized);
    canonicalize_private_constant_order(&mut normalized);
    prune_private_unused_local_metadata(&mut normalized);

    let mut local_ids = AHashMap::<LocalId, LocalId>::default();
    let mut next_local = 0_u32;
    for parameter in &normalized.params {
        local_ids.entry(parameter.local).or_insert_with(|| {
            let id = LocalId(next_local);
            next_local += 1;
            id
        });
    }
    for local in &normalized.locals {
        local_ids.entry(local.id).or_insert_with(|| {
            let id = LocalId(next_local);
            next_local += 1;
            id
        });
    }
    for block in &normalized.blocks {
        for phi in &block.phis {
            if let Some(local) = phi.origin.local() {
                local_ids.entry(local).or_insert_with(|| {
                    let id = LocalId(next_local);
                    next_local += 1;
                    id
                });
            }
        }
        for instruction in &block.instructions {
            let local = match instruction.op {
                ControlFlowOp::CaptureLocal(local)
                | ControlFlowOp::LoadLocal(local)
                | ControlFlowOp::StoreLocal { local, .. } => Some(local),
                _ => None,
            };
            if let Some(local) = local {
                local_ids.entry(local).or_insert_with(|| {
                    let id = LocalId(next_local);
                    next_local += 1;
                    id
                });
            }
        }
    }

    let mut value_ids = AHashMap::<ValueId, ValueId>::default();
    let mut next_value = 0_u32;
    for parameter in &normalized.params {
        value_ids.entry(parameter.value).or_insert_with(|| {
            let id = ValueId(next_value);
            next_value += 1;
            id
        });
    }
    for block in &normalized.blocks {
        for phi in &block.phis {
            value_ids.entry(phi.out).or_insert_with(|| {
                let id = ValueId(next_value);
                next_value += 1;
                id
            });
        }
        for instruction in &block.instructions {
            if let Some(out) = instruction.out {
                value_ids.entry(out).or_insert_with(|| {
                    let id = ValueId(next_value);
                    next_value += 1;
                    id
                });
            }
        }
    }
    normalized.value_escapes = vec![EscapeState::LocalOnly; next_value as usize];
    for (original, canonical) in &value_ids {
        if let Some(state) = original_value_escapes.get(original.0 as usize) {
            normalized.value_escapes[canonical.0 as usize] = *state;
        }
    }

    let block_ids = normalized
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.id, BlockId(index as u32)))
        .collect::<AHashMap<_, _>>();
    normalized.entry = block_ids[&normalized.entry];
    for (index, parameter) in normalized.params.iter_mut().enumerate() {
        parameter.symbol = SymbolId(index as u32);
        parameter.local = local_ids[&parameter.local];
        parameter.value = value_ids[&parameter.value];
        parameter.name = "";
        parameter.span = empty;
    }
    for local in &mut normalized.mutable_capture_locals {
        *local = local_ids[local];
    }
    normalized
        .mutable_capture_locals
        .sort_unstable_by_key(|local| local.0);
    for (index, local) in normalized.locals.iter_mut().enumerate() {
        local.id = local_ids[&local.id];
        local.symbol = SymbolId((normalized.params.len() + index) as u32);
        local.name = "";
        local.span = empty;
    }
    for block in &mut normalized.blocks {
        block.id = block_ids[&block.id];
        block.span = empty;
        for phi in &mut block.phis {
            phi.out = value_ids[&phi.out];
            phi.origin = match &phi.origin {
                crate::ir::PhiOrigin::Local(local) => crate::ir::PhiOrigin::Local(local_ids[local]),
                crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::ShortCircuit {
                    op,
                    lhs,
                }) => crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::ShortCircuit {
                    op: *op,
                    lhs: value_ids[lhs],
                }),
                crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Nullish { lhs }) => {
                    crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Nullish {
                        lhs: value_ids[lhs],
                    })
                }
                crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::OptionalAccess {
                    object,
                }) => crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::OptionalAccess {
                    object: value_ids[object],
                }),
                crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Conditional) => {
                    crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Conditional)
                }
                crate::ir::PhiOrigin::Synthetic => crate::ir::PhiOrigin::Synthetic,
            };
            for (predecessor, value) in &mut phi.incoming {
                *predecessor = block_ids[predecessor];
                *value = value_ids[value];
            }
            phi.span = empty;
        }
        for instruction in &mut block.instructions {
            if let Some(out) = &mut instruction.out {
                *out = value_ids[out];
            }
            rewrite_control_flow_values(&mut instruction.op, |value| {
                *value = value_ids[value];
            });
            match &mut instruction.op {
                ControlFlowOp::CaptureLocal(local) | ControlFlowOp::LoadLocal(local) => {
                    *local = local_ids[local]
                }
                ControlFlowOp::StoreLocal { local, .. } => *local = local_ids[local],
                ControlFlowOp::NewClass {
                    constructor: Some(reference),
                    ..
                }
                | ControlFlowOp::Closure {
                    function: reference,
                    ..
                }
                | ControlFlowOp::CallDirect {
                    function: reference,
                    ..
                }
                | ControlFlowOp::CallMethod {
                    function: reference,
                    ..
                } if *reference == original_id => *reference = SELF_REFERENCE,
                _ => {}
            }
            instruction.span = empty;
        }
        if let Some(terminator) = &mut block.terminator {
            match terminator {
                Terminator::Jump(target) => *target = block_ids[target],
                Terminator::Branch {
                    condition,
                    then_block,
                    else_block,
                } => {
                    *condition = value_ids[condition];
                    *then_block = block_ids[then_block];
                    *else_block = block_ids[else_block];
                }
                Terminator::Try { body, catch_block } => {
                    *body = block_ids[body];
                    if let Some(block) = catch_block {
                        *block = block_ids[block];
                    }
                }
                Terminator::Return(Some(value)) | Terminator::Throw(value) => {
                    *value = value_ids[value]
                }
                Terminator::Return(None) | Terminator::Unreachable => {}
            }
        }
    }
    for shape in &mut normalized.shapes {
        match shape {
            crate::ir::ControlShape::If {
                header,
                then_block,
                else_block,
                merge_block,
            } => {
                *header = block_ids[header];
                *then_block = block_ids[then_block];
                *else_block = block_ids[else_block];
                *merge_block = block_ids[merge_block];
            }
            crate::ir::ControlShape::Loop {
                header,
                body,
                update,
                exit,
            } => {
                *header = block_ids[header];
                *body = block_ids[body];
                if let Some(update) = update {
                    *update = block_ids[update];
                }
                *exit = block_ids[exit];
            }
            crate::ir::ControlShape::ForIn {
                header,
                body,
                exit,
                object,
                key,
            } => {
                *header = block_ids[header];
                *body = block_ids[body];
                *exit = block_ids[exit];
                *object = value_ids[object];
                *key = value_ids[key];
            }
            crate::ir::ControlShape::ForOf {
                header,
                body,
                exit,
                iterable,
                element,
            } => {
                *header = block_ids[header];
                *body = block_ids[body];
                *exit = block_ids[exit];
                *iterable = value_ids[iterable];
                *element = value_ids[element];
            }
            crate::ir::ControlShape::Try {
                header,
                body,
                catch_block,
                finally_block,
                merge_block,
                catch_value,
            } => {
                *header = block_ids[header];
                *body = block_ids[body];
                if let Some(block) = catch_block {
                    *block = block_ids[block];
                }
                if let Some(block) = finally_block {
                    *block = block_ids[block];
                }
                *merge_block = block_ids[merge_block];
                if let Some(value) = catch_value {
                    *value = value_ids[value];
                }
            }
        }
    }
    normalized.value_count = next_value;
    // Source-local affinity only guides final identifier spelling. It is not
    // semantic function structure and must not prevent identical folding or
    // specialized-function subsumption when two CFGs otherwise normalize to
    // the same body.
    normalized.value_local_hints = vec![None; next_value as usize];
    normalized
}

fn canonicalize_private_constant_order(function: &mut ControlFlowFunction<'_>) {
    let mut constants = Vec::new();
    for block in &mut function.blocks {
        let instructions = std::mem::take(&mut block.instructions);
        let (mut block_constants, retained): (Vec<_>, Vec<_>) = instructions
            .into_iter()
            .partition(|instruction| matches!(instruction.op, ControlFlowOp::Const(_)));
        constants.append(&mut block_constants);
        block.instructions = retained;
    }
    constants.sort_by_key(|instruction| match &instruction.op {
        ControlFlowOp::Const(value) => ConstantKey::from_value(value),
        _ => unreachable!(),
    });
    function.blocks[function.entry.0 as usize]
        .instructions
        .splice(0..0, constants);
}

fn canonicalize_private_known_closure_calls(function: &mut ControlFlowFunction<'_>) {
    let closures = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match (instruction.out, &instruction.op) {
            (Some(out), ControlFlowOp::Closure { function, captures }) if captures.is_empty() => {
                Some((out, *function))
            }
            _ => None,
        })
        .collect::<AHashMap<_, _>>();
    if closures.is_empty() {
        return;
    }
    for instruction in function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
    {
        let replacement = match &instruction.op {
            ControlFlowOp::CallValue { callee, args } => {
                closures
                    .get(callee)
                    .map(|target| ControlFlowOp::CallDirect {
                        function: *target,
                        provided_args: args.len(),
                        args: args.clone(),
                    })
            }
            _ => None,
        };
        if let Some(replacement) = replacement {
            instruction.op = replacement;
        }
    }
    let uses = control_flow_use_counts(function);
    for block in &mut function.blocks {
        block.instructions.retain(|instruction| {
            !matches!(
                (instruction.out, &instruction.op),
                (Some(out), ControlFlowOp::Closure { captures, .. })
                    if captures.is_empty() && uses.get(&out).copied().unwrap_or(0) == 0
            )
        });
    }
}

fn prune_private_unused_local_metadata(function: &mut ControlFlowFunction<'_>) {
    let mut referenced = function
        .params
        .iter()
        .map(|parameter| parameter.local)
        .collect::<AHashSet<_>>();
    for block in &function.blocks {
        referenced.extend(block.phis.iter().filter_map(|phi| phi.origin.local()));
        for instruction in &block.instructions {
            match instruction.op {
                ControlFlowOp::CaptureLocal(local)
                | ControlFlowOp::LoadLocal(local)
                | ControlFlowOp::StoreLocal { local, .. } => {
                    referenced.insert(local);
                }
                _ => {}
            }
        }
    }
    function
        .locals
        .retain(|local| referenced.contains(&local.id));
}

fn eliminate_dead_functions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut reachable = AHashSet::default();
    let mut work = vec![module.entry];
    work.extend(exported_functions(module));
    while let Some(function_id) = work.pop() {
        if !reachable.insert(function_id) {
            continue;
        }
        let Some(function) = module.functions.get(function_id.0 as usize) else {
            continue;
        };
        for block in &function.blocks {
            for instruction in &block.instructions {
                match &instruction.op {
                    ControlFlowOp::CallDirect { function, .. }
                    | ControlFlowOp::Closure { function, .. } => work.push(*function),
                    ControlFlowOp::CallMethod { function, .. } => work.push(*function),
                    ControlFlowOp::NewClass {
                        constructor: Some(function),
                        ..
                    } => work.push(*function),
                    _ => {}
                }
            }
        }
    }

    let mut changed = false;
    for function in &mut module.functions {
        let live = reachable.contains(&function.id);
        changed |= function.live != live;
        function.live = live;
    }
    OptimizationReport {
        pass_name: "dead-function-elimination",
        changed,
    }
}

fn exported_functions(module: &ControlFlowModule<'_>) -> AHashSet<FunctionId> {
    module
        .exports
        .iter()
        .chain(
            module
                .lazy_modules
                .iter()
                .flat_map(|module| module.exports.iter()),
        )
        .filter_map(|export| match export.binding {
            ExportBinding::Function(function) => Some(function),
            _ => None,
        })
        .collect()
}

fn exported_globals(module: &ControlFlowModule<'_>) -> AHashSet<SymbolId> {
    module
        .exports
        .iter()
        .chain(
            module
                .lazy_modules
                .iter()
                .flat_map(|module| module.exports.iter()),
        )
        .filter_map(|export| match export.binding {
            ExportBinding::Global(symbol) => Some(symbol),
            _ => None,
        })
        .collect()
}

fn control_flow_use_counts(function: &ControlFlowFunction<'_>) -> AHashMap<ValueId, usize> {
    let mut uses = AHashMap::default();
    for block in &function.blocks {
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                *uses.entry(*value).or_insert(0) += 1;
            }
        }
        for instruction in &block.instructions {
            for value in control_flow_used_values(&instruction.op) {
                *uses.entry(value).or_insert(0) += 1;
            }
        }
        if let Some(terminator) = &block.terminator {
            for value in terminator_used_values(terminator) {
                *uses.entry(value).or_insert(0) += 1;
            }
        }
    }
    uses
}

fn control_flow_used_values(op: &ControlFlowOp<'_>) -> Vec<ValueId> {
    match op {
        ControlFlowOp::Const(_)
        | ControlFlowOp::CaughtException
        | ControlFlowOp::CaptureLocal(_)
        | ControlFlowOp::LoadLocal(_)
        | ControlFlowOp::LoadGlobal(_)
        | ControlFlowOp::DynamicImport { .. } => Vec::new(),
        ControlFlowOp::Unary { value, .. } | ControlFlowOp::TypeCheck { value, .. } => vec![*value],
        ControlFlowOp::Await { task } => vec![*task],
        ControlFlowOp::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        ControlFlowOp::Array(values) => values.clone(),
        ControlFlowOp::ArraySpread(operands) => operands
            .iter()
            .map(|operand| match operand {
                ArrayOperand::Value(value) | ArrayOperand::Spread(value) => *value,
            })
            .collect(),
        ControlFlowOp::Record(entries) => entries.iter().map(|(_, value)| *value).collect(),
        ControlFlowOp::RecordSpread(operands) => operands
            .iter()
            .map(|operand| match operand {
                RecordOperand::Entry(_, value) | RecordOperand::Spread(value) => *value,
            })
            .collect(),
        ControlFlowOp::Struct { fields, .. } => fields.clone(),
        ControlFlowOp::NewClass { args, .. } => args.clone(),
        ControlFlowOp::Closure { captures, .. } => captures.clone(),
        ControlFlowOp::StoreLocal { value, .. } | ControlFlowOp::StoreGlobal { value, .. } => {
            vec![*value]
        }
        ControlFlowOp::FieldGet { object, .. }
        | ControlFlowOp::RecordFieldGet { object, .. }
        | ControlFlowOp::RecordRest { object, .. }
        | ControlFlowOp::HostFieldGet { object, .. } => {
            vec![*object]
        }
        ControlFlowOp::FieldSet { object, value, .. }
        | ControlFlowOp::RecordFieldSet { object, value, .. }
        | ControlFlowOp::HostFieldSet { object, value, .. } => vec![*object, *value],
        ControlFlowOp::IndexGet { object, index } => vec![*object, *index],
        ControlFlowOp::ArrayGetOptional { object, .. } => vec![*object],
        ControlFlowOp::IndexSet {
            object,
            index,
            value,
        } => vec![*object, *index, *value],
        ControlFlowOp::CallDirect { args, .. } => args.clone(),
        ControlFlowOp::CallValue { callee, args } => {
            let mut values = Vec::with_capacity(args.len() + 1);
            values.push(*callee);
            values.extend(args);
            values
        }
        ControlFlowOp::CallMethod { receiver, args, .. } => {
            let mut values = Vec::with_capacity(args.len() + 1);
            values.push(*receiver);
            values.extend(args);
            values
        }
        ControlFlowOp::HostCall { receiver, args, .. } => {
            let mut values = Vec::with_capacity(args.len() + 1);
            values.push(*receiver);
            values.extend(args);
            values
        }
        ControlFlowOp::Intrinsic { receiver, args, .. } => {
            let mut values = Vec::with_capacity(args.len() + usize::from(receiver.is_some()));
            values.extend(receiver);
            values.extend(args);
            values
        }
        ControlFlowOp::Template(parts) => parts
            .iter()
            .filter_map(|part| match part {
                TemplateOperand::Value(value) => Some(*value),
                TemplateOperand::String(_) => None,
            })
            .collect(),
    }
}

fn terminator_used_values(terminator: &Terminator) -> Vec<ValueId> {
    match terminator {
        Terminator::Branch { condition, .. } => vec![*condition],
        Terminator::Return(Some(value)) | Terminator::Throw(value) => vec![*value],
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EffectRoot {
    Parameter(usize),
    Local(ValueId),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct FunctionEffectSummary {
    inherent: bool,
    mutated_parameters: AHashSet<usize>,
}

#[derive(Debug, Clone, Copy, Default)]
struct DynamicCoercionCategories {
    primitive: bool,
    nullish: bool,
    undefined: bool,
    reference: bool,
    dynamic: bool,
    symbol: bool,
}

impl DynamicCoercionCategories {
    fn merge(&mut self, other: Self) {
        self.primitive |= other.primitive;
        self.nullish |= other.nullish;
        self.undefined |= other.undefined;
        self.reference |= other.reference;
        self.dynamic |= other.dynamic;
        self.symbol |= other.symbol;
    }
}

fn dynamic_coercion_categories(ty: &Type<'_>) -> DynamicCoercionCategories {
    match ty {
        Type::Int | Type::Float | Type::Enum(_) | Type::String | Type::Bool => {
            DynamicCoercionCategories {
                primitive: true,
                ..DynamicCoercionCategories::default()
            }
        }
        Type::Symbol => DynamicCoercionCategories {
            primitive: true,
            symbol: true,
            ..DynamicCoercionCategories::default()
        },
        Type::Null => DynamicCoercionCategories {
            nullish: true,
            ..DynamicCoercionCategories::default()
        },
        Type::Void => DynamicCoercionCategories {
            nullish: true,
            undefined: true,
            ..DynamicCoercionCategories::default()
        },
        Type::Nullable(inner) => {
            let mut categories = dynamic_coercion_categories(inner);
            categories.nullish = true;
            categories
        }
        Type::Union(members) => {
            let mut categories = DynamicCoercionCategories::default();
            for member in members {
                categories.merge(dynamic_coercion_categories(member));
            }
            categories
        }
        Type::TypeParameter(_) => DynamicCoercionCategories {
            dynamic: true,
            ..DynamicCoercionCategories::default()
        },
        _ => DynamicCoercionCategories {
            reference: true,
            ..DynamicCoercionCategories::default()
        },
    }
}

fn dynamic_value_categories(
    types: &AHashMap<ValueId, Type<'_>>,
    value: ValueId,
) -> Option<DynamicCoercionCategories> {
    types.get(&value).map(dynamic_coercion_categories)
}

fn dynamic_value_can_run_user_code(types: &AHashMap<ValueId, Type<'_>>, value: ValueId) -> bool {
    dynamic_value_categories(types, value)
        .is_none_or(|categories| categories.dynamic || categories.reference || categories.symbol)
}

fn dynamic_binary_can_observe(
    types: &AHashMap<ValueId, Type<'_>>,
    op: IrBinaryOp,
    lhs: ValueId,
    rhs: ValueId,
) -> bool {
    if matches!(op, IrBinaryOp::And | IrBinaryOp::Or) {
        return false;
    }
    let Some(lhs) = dynamic_value_categories(types, lhs) else {
        return true;
    };
    let Some(rhs) = dynamic_value_categories(types, rhs) else {
        return true;
    };
    if lhs.dynamic || rhs.dynamic {
        return true;
    }
    if matches!(op, IrBinaryOp::Eq | IrBinaryOp::NotEq) {
        return (lhs.reference && rhs.primitive) || (rhs.reference && lhs.primitive);
    }
    lhs.reference || rhs.reference || lhs.symbol || rhs.symbol
}

pub(crate) fn instruction_has_dynamic_observable_evaluation(
    instruction: &crate::ir::ControlFlowInstruction<'_>,
    types: &AHashMap<ValueId, Type<'_>>,
) -> bool {
    match &instruction.op {
        ControlFlowOp::Unary {
            op: IrUnaryOp::Neg,
            value,
        } => dynamic_value_can_run_user_code(types, *value),
        ControlFlowOp::Unary {
            op: IrUnaryOp::Not, ..
        } => false,
        ControlFlowOp::Binary { op, lhs, rhs } => {
            dynamic_binary_can_observe(types, *op, *lhs, *rhs)
        }
        ControlFlowOp::Template(parts) => parts.iter().any(|part| match part {
            TemplateOperand::Value(value) => dynamic_value_can_run_user_code(types, *value),
            TemplateOperand::String(_) => false,
        }),
        ControlFlowOp::IndexGet { object, .. } => types
            .get(object)
            .is_none_or(|ty| dynamic_coercion_categories(ty).dynamic),
        ControlFlowOp::RecordSpread(operands) => operands.iter().any(|operand| match operand {
            crate::ir::RecordOperand::Spread(value) => types
                .get(value)
                .is_none_or(|ty| dynamic_coercion_categories(ty).dynamic),
            crate::ir::RecordOperand::Entry(_, _) => false,
        }),
        ControlFlowOp::Intrinsic {
            intrinsic: Intrinsic::ArrayLength,
            receiver: Some(receiver),
            ..
        } => types
            .get(receiver)
            .is_none_or(|ty| dynamic_coercion_categories(ty).dynamic),
        ControlFlowOp::Intrinsic {
            intrinsic: Intrinsic::JsIsArray,
            ..
        } => true,
        ControlFlowOp::Intrinsic {
            intrinsic: Intrinsic::JsParseFloat | Intrinsic::JsParseInt | Intrinsic::JsIsFinite,
            args,
            ..
        } => args
            .iter()
            .copied()
            .any(|value| dynamic_value_can_run_user_code(types, value)),
        ControlFlowOp::Intrinsic {
            intrinsic: Intrinsic::JsEncodeURI | Intrinsic::JsEncodeURIComponent,
            ..
        } => true,
        ControlFlowOp::Intrinsic {
            intrinsic: Intrinsic::JsObjectCreate,
            args,
            ..
        } => args.iter().copied().any(|value| {
            types.get(&value).is_none_or(|ty| {
                let categories = dynamic_coercion_categories(ty);
                categories.dynamic || categories.primitive || categories.undefined
            })
        }),
        ControlFlowOp::Intrinsic {
            intrinsic: Intrinsic::JsGetPrototypeOf,
            args,
            ..
        } => args.iter().copied().any(|value| {
            types.get(&value).is_none_or(|ty| {
                let categories = dynamic_coercion_categories(ty);
                categories.dynamic || categories.nullish
            })
        }),
        ControlFlowOp::Intrinsic {
            intrinsic: Intrinsic::RecordKeys | Intrinsic::RecordValues | Intrinsic::RecordHasOwn,
            args,
            ..
        } => args.first().is_some_and(|value| {
            types.get(value).is_none_or(|ty| {
                let categories = dynamic_coercion_categories(ty);
                categories.dynamic || categories.nullish
            })
        }),
        _ => false,
    }
}

fn dynamic_observable_values(function: &ControlFlowFunction<'_>) -> AHashSet<ValueId> {
    let types = control_flow_value_types(function);
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| instruction_has_dynamic_observable_evaluation(instruction, &types))
        .filter_map(|instruction| instruction.out)
        .collect()
}

pub fn summarize_module_effects(
    module: &ControlFlowModule<'_>,
) -> crate::package::PackageEffectSummary {
    let summaries = analyze_function_effects(module);
    let mut functions = std::collections::BTreeMap::new();
    for (function, summary) in module.functions.iter().zip(summaries) {
        let Some(name) = function.name else {
            continue;
        };
        let mut mutated_parameters = summary.mutated_parameters.into_iter().collect::<Vec<_>>();
        mutated_parameters.sort_unstable();
        functions.insert(
            name.to_string(),
            crate::package::FunctionEffectMeta {
                pure: !summary.inherent && mutated_parameters.is_empty(),
                mutated_parameters,
            },
        );
    }
    crate::package::PackageEffectSummary { functions }
}

fn analyze_function_effects(module: &ControlFlowModule<'_>) -> Vec<FunctionEffectSummary> {
    let mut summaries = vec![FunctionEffectSummary::default(); module.functions.len()];
    for function in &module.functions {
        if function.kind == FunctionKind::Extern && !function.declared_pure {
            if let Some(summary) = summaries.get_mut(function.id.0 as usize) {
                summary.inherent = true;
            }
        }
    }

    loop {
        let mut changed = false;
        for function in &module.functions {
            if function.kind == FunctionKind::Extern {
                continue;
            }
            let candidate = summarize_function_effects(function, &summaries);
            let summary = &mut summaries[function.id.0 as usize];
            if candidate.inherent && !summary.inherent {
                summary.inherent = true;
                changed = true;
            }
            for parameter in candidate.mutated_parameters {
                changed |= summary.mutated_parameters.insert(parameter);
            }
        }
        if !changed {
            return summaries;
        }
    }
}

fn summarize_function_effects(
    function: &ControlFlowFunction<'_>,
    summaries: &[FunctionEffectSummary],
) -> FunctionEffectSummary {
    let value_types = control_flow_value_types(function);
    let mut roots = function
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.value, EffectRoot::Parameter(index)))
        .collect::<AHashMap<_, _>>();
    roots.extend(
        local_mutable_roots(function, is_owned_mutable_allocation)
            .into_keys()
            .map(|value| (value, EffectRoot::Local(value))),
    );
    extend_mutable_alias_roots(function, &mut roots);

    let closures = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match (instruction.out, &instruction.op) {
            (Some(out), ControlFlowOp::Closure { function, captures }) => {
                Some((out, (*function, captures.clone())))
            }
            _ => None,
        })
        .collect::<AHashMap<_, _>>();

    let mut result = FunctionEffectSummary::default();
    if function
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, Some(Terminator::Throw(_))))
    {
        result.inherent = true;
    }
    if function.shapes.iter().any(|shape| match shape {
        ControlShape::ForIn { object, .. } => value_types
            .get(object)
            .is_none_or(|ty| dynamic_coercion_categories(ty).dynamic),
        // Array and typed-array source loops lower to ordinary indexed Loop
        // shapes. A surviving ForOf therefore consumes a Generator<T> and
        // advances arbitrary user code / iterator state.
        ControlShape::ForOf { .. } => true,
        _ => false,
    }) {
        result.inherent = true;
    }
    for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
        if instruction_has_dynamic_observable_evaluation(instruction, &value_types) {
            result.inherent = true;
        }
        match &instruction.op {
            ControlFlowOp::StoreLocal { local, .. }
                if function.mutable_capture_locals.contains(local) =>
            {
                if let Some(parameter) = function
                    .params
                    .iter()
                    .position(|parameter| parameter.local == *local)
                {
                    result.mutated_parameters.insert(parameter);
                } else {
                    // A closure can make an owner-local mutation observable to
                    // sibling closures and subsequent reads in this frame.
                    result.inherent = true;
                }
            }
            ControlFlowOp::StoreGlobal { .. }
            | ControlFlowOp::HostFieldGet { .. }
            | ControlFlowOp::HostFieldSet { .. }
            | ControlFlowOp::DynamicImport { .. }
            | ControlFlowOp::Await { .. } => result.inherent = true,
            ControlFlowOp::FieldSet { object, .. }
            | ControlFlowOp::RecordFieldSet { object, .. }
            | ControlFlowOp::IndexSet { object, .. } => {
                record_mutation(*object, &roots, &mut result);
            }
            ControlFlowOp::CallDirect { function, args, .. } => {
                apply_callee_summary(
                    summaries.get(function.0 as usize),
                    args,
                    &roots,
                    &mut result,
                );
            }
            ControlFlowOp::CallValue { callee, args } => {
                let Some((function, captures)) = closures.get(callee) else {
                    result.inherent = true;
                    continue;
                };
                let actuals = captures.iter().chain(args).copied().collect::<Vec<_>>();
                apply_callee_summary(
                    summaries.get(function.0 as usize),
                    &actuals,
                    &roots,
                    &mut result,
                );
            }
            ControlFlowOp::CallMethod {
                receiver,
                function,
                args,
                ..
            } => {
                let actuals = std::iter::once(*receiver)
                    .chain(args.iter().copied())
                    .collect::<Vec<_>>();
                apply_callee_summary(
                    summaries.get(function.0 as usize),
                    &actuals,
                    &roots,
                    &mut result,
                );
            }
            ControlFlowOp::NewClass {
                constructor: Some(function),
                args,
                ..
            } => apply_constructor_summary(
                summaries.get(function.0 as usize),
                args,
                &roots,
                &mut result,
            ),
            ControlFlowOp::HostCall { pure, .. } => result.inherent |= !pure,
            ControlFlowOp::Intrinsic {
                intrinsic:
                    Intrinsic::ArrayPush
                    | Intrinsic::ArrayPop
                    | Intrinsic::ArraySplice
                    | Intrinsic::ArrayFill
                    | Intrinsic::ArrayCopyWithin
                    | Intrinsic::ArrayReverse
                    | Intrinsic::TypedArraySet
                    | Intrinsic::TypedArrayFill
                    | Intrinsic::TypedArrayCopyWithin
                    | Intrinsic::MapSet
                    | Intrinsic::MapDelete
                    | Intrinsic::MapClear
                    | Intrinsic::SetAdd
                    | Intrinsic::SetDelete
                    | Intrinsic::SetClear
                    | Intrinsic::RegexTest
                    | Intrinsic::JsRegexExec
                    | Intrinsic::StringSearch
                    | Intrinsic::StringReplace
                    | Intrinsic::JsDeleteProperty
                    | Intrinsic::JsArrayPush
                    | Intrinsic::JsArrayPop
                    | Intrinsic::JsArraySort
                    | Intrinsic::JsArraySplice
                    | Intrinsic::JsArrayShift
                    | Intrinsic::JsArrayUnshift,
                receiver,
                ..
            } => {
                if let Some(receiver) = receiver {
                    record_mutation(*receiver, &roots, &mut result);
                } else {
                    result.inherent = true;
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::RecordAssign,
                args,
                ..
            } => {
                if let Some(target) = args.first() {
                    record_mutation(*target, &roots, &mut result);
                } else {
                    result.inherent = true;
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic:
                    Intrinsic::Print
                    | Intrinsic::JsonParse
                    | Intrinsic::JsStringify
                    | Intrinsic::JsNumber
                    | Intrinsic::JsAdd
                    | Intrinsic::JsMod
                    | Intrinsic::JsLessThan
                    | Intrinsic::JsLessThanOrEqual
                    | Intrinsic::JsGreaterThan
                    | Intrinsic::JsGreaterThanOrEqual
                    | Intrinsic::JsStringReplace
                    | Intrinsic::JsStringMatch
                    | Intrinsic::JsCall
                    | Intrinsic::JsConstruct
                    | Intrinsic::JsInvoke
                    | Intrinsic::JsApply
                    | Intrinsic::JsGetProperty
                    | Intrinsic::JsHasProperty
                    | Intrinsic::JsInProperty
                    | Intrinsic::JsArraySlice
                    | Intrinsic::JsArrayIndexOf
                    | Intrinsic::JsArrayConcatApply
                    | Intrinsic::JsArrayJoin
                    | Intrinsic::JsIsFunctionValue
                    | Intrinsic::JsIsWindowValue
                    | Intrinsic::JsDefineConfigurable
                    | Intrinsic::JsDefineIterator
                    | Intrinsic::JsArrayIterator
                    | Intrinsic::JsConsoleWarn
                    | Intrinsic::JsRequestAnimationFrameOrNull
                    | Intrinsic::TaskResolve
                    | Intrinsic::TaskReject
                    | Intrinsic::TaskAll
                    | Intrinsic::GeneratorYield
                    | Intrinsic::GeneratorYieldDelegated,
                ..
            } => result.inherent = true,
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::RegexNew,
                ..
            } => result.inherent = true,
            ControlFlowOp::Intrinsic {
                intrinsic:
                    Intrinsic::ArrayMap
                    | Intrinsic::ArrayFilter
                    | Intrinsic::ArrayReduce
                    | Intrinsic::ArrayForEach
                    | Intrinsic::ArraySome
                    | Intrinsic::ArrayEvery
                    | Intrinsic::ArrayFindIndex,
                args,
                ..
            } => summarize_callback_effects(args, &closures, summaries, &roots, &mut result),
            ControlFlowOp::Const(_)
            | ControlFlowOp::CaughtException
            | ControlFlowOp::Unary { .. }
            | ControlFlowOp::Binary { .. }
            | ControlFlowOp::TypeCheck { .. }
            | ControlFlowOp::Array(_)
            | ControlFlowOp::ArraySpread(_)
            | ControlFlowOp::Record(_)
            | ControlFlowOp::RecordSpread(_)
            | ControlFlowOp::Struct { .. }
            | ControlFlowOp::NewClass {
                constructor: None, ..
            }
            | ControlFlowOp::Closure { .. }
            | ControlFlowOp::CaptureLocal(_)
            | ControlFlowOp::LoadLocal(_)
            | ControlFlowOp::StoreLocal { .. }
            | ControlFlowOp::LoadGlobal(_)
            | ControlFlowOp::FieldGet { .. }
            | ControlFlowOp::RecordFieldGet { .. }
            | ControlFlowOp::RecordRest { .. }
            | ControlFlowOp::ArrayGetOptional { .. }
            | ControlFlowOp::IndexGet { .. }
            | ControlFlowOp::Intrinsic { .. }
            | ControlFlowOp::Template(_) => {}
        }
    }
    result
}

fn record_mutation(
    value: ValueId,
    roots: &AHashMap<ValueId, EffectRoot>,
    result: &mut FunctionEffectSummary,
) {
    match roots.get(&value) {
        Some(EffectRoot::Parameter(parameter)) => {
            result.mutated_parameters.insert(*parameter);
        }
        Some(EffectRoot::Local(_)) => {}
        None => result.inherent = true,
    }
}

fn apply_callee_summary(
    callee: Option<&FunctionEffectSummary>,
    args: &[ValueId],
    roots: &AHashMap<ValueId, EffectRoot>,
    result: &mut FunctionEffectSummary,
) {
    let Some(callee) = callee else {
        result.inherent = true;
        return;
    };
    result.inherent |= callee.inherent;
    for parameter in &callee.mutated_parameters {
        if let Some(argument) = args.get(*parameter) {
            record_mutation(*argument, roots, result);
        } else {
            result.inherent = true;
        }
    }
}

fn apply_constructor_summary(
    constructor: Option<&FunctionEffectSummary>,
    args: &[ValueId],
    roots: &AHashMap<ValueId, EffectRoot>,
    result: &mut FunctionEffectSummary,
) {
    let Some(constructor) = constructor else {
        result.inherent = true;
        return;
    };
    result.inherent |= constructor.inherent;
    for parameter in &constructor.mutated_parameters {
        if *parameter == 0 {
            continue;
        }
        if let Some(argument) = args.get(parameter - 1) {
            record_mutation(*argument, roots, result);
        } else {
            result.inherent = true;
        }
    }
}

fn summarize_callback_effects(
    args: &[ValueId],
    closures: &AHashMap<ValueId, (FunctionId, Vec<ValueId>)>,
    summaries: &[FunctionEffectSummary],
    roots: &AHashMap<ValueId, EffectRoot>,
    result: &mut FunctionEffectSummary,
) {
    let Some((function, captures)) = args.first().and_then(|callback| closures.get(callback))
    else {
        result.inherent = true;
        return;
    };
    let Some(summary) = summaries.get(function.0 as usize) else {
        result.inherent = true;
        return;
    };
    result.inherent |= summary.inherent;
    for parameter in &summary.mutated_parameters {
        if let Some(capture) = captures.get(*parameter) {
            record_mutation(*capture, roots, result);
        } else {
            result.inherent = true;
        }
    }
}

fn effectful_functions(summaries: &[FunctionEffectSummary]) -> AHashSet<FunctionId> {
    summaries
        .iter()
        .enumerate()
        .filter(|(_, summary)| summary.inherent || !summary.mutated_parameters.is_empty())
        .map(|(index, _)| FunctionId(index as u32))
        .collect()
}

fn find_effectful_functions(module: &ControlFlowModule<'_>) -> AHashSet<FunctionId> {
    effectful_functions(&analyze_function_effects(module))
}

fn validate_declared_purity(
    module: &ControlFlowModule<'_>,
) -> Result<OptimizationReport, SsaError> {
    let effectful = find_effectful_functions(module);
    if let Some(function) = module
        .functions
        .iter()
        .find(|function| function.declared_pure && effectful.contains(&function.id))
    {
        return Err(SsaError {
            span: function.span,
            message: format!(
                "function `{}` is declared `pure` but may perform an observable side effect",
                function.name.unwrap_or("<closure>")
            ),
        });
    }
    Ok(OptimizationReport {
        pass_name: "pure-contract-validation",
        changed: false,
    })
}

fn control_flow_op_has_side_effects(
    op: &ControlFlowOp<'_>,
    effectful_functions: &AHashSet<crate::ir::FunctionId>,
    closure_targets: &AHashMap<ValueId, crate::ir::FunctionId>,
) -> bool {
    match op {
        ControlFlowOp::CaughtException
        | ControlFlowOp::StoreLocal { .. }
        | ControlFlowOp::StoreGlobal { .. }
        | ControlFlowOp::FieldSet { .. }
        | ControlFlowOp::RecordFieldSet { .. }
        | ControlFlowOp::HostFieldGet { .. }
        | ControlFlowOp::HostFieldSet { .. }
        | ControlFlowOp::IndexSet { .. }
        | ControlFlowOp::CallMethod { .. } => true,
        ControlFlowOp::HostCall { pure, .. } => !pure,
        ControlFlowOp::CallValue { callee, .. } => closure_targets
            .get(callee)
            .is_none_or(|function| effectful_functions.contains(function)),
        ControlFlowOp::CallDirect { function, .. } => effectful_functions.contains(function),
        ControlFlowOp::NewClass {
            constructor: Some(function),
            ..
        } => effectful_functions.contains(function),
        ControlFlowOp::NewClass {
            constructor: None, ..
        } => false,
        ControlFlowOp::Intrinsic {
            intrinsic:
                Intrinsic::ArrayMap
                | Intrinsic::ArrayFilter
                | Intrinsic::ArrayReduce
                | Intrinsic::ArrayForEach
                | Intrinsic::ArraySome
                | Intrinsic::ArrayEvery
                | Intrinsic::ArrayFindIndex,
            args,
            ..
        } => args.first().is_none_or(|callback| {
            closure_targets
                .get(callback)
                .is_none_or(|function| effectful_functions.contains(function))
        }),
        ControlFlowOp::Intrinsic { intrinsic, .. } => matches!(
            intrinsic,
            Intrinsic::Print
                | Intrinsic::ArrayPush
                | Intrinsic::ArrayPop
                | Intrinsic::ArraySplice
                | Intrinsic::ArrayFill
                | Intrinsic::ArrayCopyWithin
                | Intrinsic::ArrayReverse
                | Intrinsic::TypedArraySet
                | Intrinsic::TypedArrayFill
                | Intrinsic::TypedArrayCopyWithin
                | Intrinsic::MapSet
                | Intrinsic::MapDelete
                | Intrinsic::MapClear
                | Intrinsic::SetAdd
                | Intrinsic::SetDelete
                | Intrinsic::SetClear
                | Intrinsic::RecordAssign
                | Intrinsic::JsonParse
                | Intrinsic::JsStringify
                | Intrinsic::JsDateNow
                | Intrinsic::JsDocument
                | Intrinsic::JsSetTimeout
                | Intrinsic::JsClearTimeout
                | Intrinsic::JsDomParserNew
                | Intrinsic::JsXMLHttpRequestNew
                | Intrinsic::JsNumber
                | Intrinsic::JsAdd
                | Intrinsic::JsMod
                | Intrinsic::JsLessThan
                | Intrinsic::JsLessThanOrEqual
                | Intrinsic::JsGreaterThan
                | Intrinsic::JsGreaterThanOrEqual
                | Intrinsic::JsStringReplace
                | Intrinsic::JsStringMatch
                | Intrinsic::JsRegexExec
                | Intrinsic::StringSearch
                | Intrinsic::StringReplace
                | Intrinsic::JsCall
                | Intrinsic::JsConstruct
                | Intrinsic::JsInvoke
                | Intrinsic::JsApply
                | Intrinsic::JsGetProperty
                | Intrinsic::JsDeleteProperty
                | Intrinsic::JsHasProperty
                | Intrinsic::JsInProperty
                | Intrinsic::JsArrayPush
                | Intrinsic::JsArrayPop
                | Intrinsic::JsArraySlice
                | Intrinsic::JsArrayIndexOf
                | Intrinsic::JsArraySort
                | Intrinsic::JsArraySplice
                | Intrinsic::JsArrayConcatApply
                | Intrinsic::JsArrayJoin
                | Intrinsic::JsArrayShift
                | Intrinsic::JsArrayUnshift
                | Intrinsic::JsArrayFlat
                | Intrinsic::JsIsFunctionValue
                | Intrinsic::JsIsWindowValue
                | Intrinsic::JsDefineConfigurable
                | Intrinsic::JsDefineIterator
                | Intrinsic::JsArrayIterator
                | Intrinsic::JsConsoleWarn
                | Intrinsic::JsRequestAnimationFrameOrNull
                | Intrinsic::RegexNew
                | Intrinsic::RegexTest
                | Intrinsic::TaskResolve
                | Intrinsic::TaskReject
                | Intrinsic::TaskAll
                | Intrinsic::GeneratorYield
                | Intrinsic::GeneratorYieldDelegated
        ),
        _ => false,
    }
}

fn closure_targets(function: &ControlFlowFunction<'_>) -> AHashMap<ValueId, crate::ir::FunctionId> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match (instruction.out, &instruction.op) {
            (Some(out), ControlFlowOp::Closure { function, .. }) => Some((out, *function)),
            _ => None,
        })
        .collect()
}

fn escape_rank(state: EscapeState) -> u8 {
    match state {
        EscapeState::LocalOnly => 0,
        EscapeState::EscapesToTypedCode => 1,
        EscapeState::EscapesToUntypedBoundary => 2,
    }
}

#[derive(Debug, Default)]
pub struct Pipeline {
    reports: Vec<OptimizationReport>,
}

impl Pipeline {
    pub fn run(&mut self, module: &mut IrModule, passes: &mut [&mut dyn OptimizationPass]) {
        self.reports.clear();
        for pass in passes {
            self.reports.push(pass.run(module));
        }
    }

    pub fn reports(&self) -> &[OptimizationReport] {
        &self.reports
    }
}

#[derive(Debug, Default)]
pub struct ConstantFolding;

impl OptimizationPass for ConstantFolding {
    fn name(&self) -> &'static str {
        "constant-folding"
    }

    fn run(&mut self, module: &mut IrModule) -> OptimizationReport {
        let mut changed = false;
        for function in &mut module.functions {
            let mut constants = AHashMap::default();
            for block in &mut function.blocks {
                for instruction in &mut block.instructions {
                    match instruction {
                        Instruction::Const { out, value, .. } => {
                            constants.insert(*out, value.clone());
                        }
                        Instruction::Binary {
                            out,
                            op,
                            lhs,
                            rhs,
                            span,
                        } => {
                            let folded = constants
                                .get(lhs)
                                .zip(constants.get(rhs))
                                .and_then(|(lhs, rhs)| fold_binary(*op, lhs, rhs));
                            if let Some(value) = folded {
                                let out = *out;
                                let span = *span;
                                constants.insert(out, value.clone());
                                *instruction = Instruction::Const { out, value, span };
                                changed = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        OptimizationReport {
            pass_name: self.name(),
            changed,
        }
    }
}

#[derive(Debug, Default)]
pub struct ScalarReplacement;

impl OptimizationPass for ScalarReplacement {
    fn name(&self) -> &'static str {
        "scalar-replacement"
    }

    fn run(&mut self, module: &mut IrModule) -> OptimizationReport {
        let mut changed = false;
        for function in &mut module.functions {
            let mut aggregates = AHashMap::<ValueId, Vec<ValueId>>::default();
            let mut escaping = AHashSet::<ValueId>::default();

            for block in &function.blocks {
                for instruction in &block.instructions {
                    if let Instruction::Struct { out, fields, .. } = instruction {
                        aggregates.insert(*out, fields.clone());
                    }
                }
            }

            for block in &function.blocks {
                for instruction in &block.instructions {
                    match instruction {
                        Instruction::FieldGet { aggregate, .. } => {
                            if !aggregates.contains_key(aggregate) {
                                escaping.insert(*aggregate);
                            }
                        }
                        _ => {
                            for used in used_values(instruction) {
                                if aggregates.contains_key(&used) {
                                    escaping.insert(used);
                                }
                            }
                        }
                    }
                }
            }

            let mut aliases = AHashMap::<ValueId, ValueId>::default();
            for block in &mut function.blocks {
                let mut rewritten = Vec::with_capacity(block.instructions.len());
                for mut instruction in block.instructions.drain(..) {
                    match &instruction {
                        Instruction::Struct { out, .. } if !escaping.contains(out) => {
                            changed = true;
                            continue;
                        }
                        Instruction::FieldGet {
                            out,
                            aggregate,
                            index,
                            ..
                        } if !escaping.contains(aggregate) => {
                            if let Some(field) = aggregates
                                .get(aggregate)
                                .and_then(|fields| fields.get(*index))
                            {
                                aliases.insert(*out, resolve_alias(*field, &aliases));
                                changed = true;
                                continue;
                            }
                        }
                        _ => {}
                    }
                    rewrite_values(&mut instruction, &aliases);
                    rewritten.push(instruction);
                }
                block.instructions = rewritten;
            }
        }

        OptimizationReport {
            pass_name: self.name(),
            changed,
        }
    }
}

#[derive(Debug, Default)]
pub struct DeadCodeElimination;

impl OptimizationPass for DeadCodeElimination {
    fn name(&self) -> &'static str {
        "dead-code-elimination"
    }

    fn run(&mut self, module: &mut IrModule) -> OptimizationReport {
        let mut changed = false;
        for function in &mut module.functions {
            let mut live = AHashSet::<ValueId>::default();

            for block in function.blocks.iter_mut().rev() {
                let old_len = block.instructions.len();
                let mut retained = Vec::with_capacity(old_len);
                for instruction in block.instructions.drain(..).rev() {
                    match instruction {
                        Instruction::Return { value, .. } => {
                            if let Some(value) = value {
                                live.insert(value);
                            }
                            retained.push(instruction);
                        }
                        Instruction::Call { out, ref args, .. } => {
                            if let Some(out) = out {
                                live.remove(&out);
                            }
                            live.extend(args.iter().copied());
                            retained.push(instruction);
                        }
                        pure => {
                            let Some(out) = result_value(&pure) else {
                                retained.push(pure);
                                continue;
                            };
                            if live.remove(&out) {
                                live.extend(used_values(&pure));
                                retained.push(pure);
                            }
                        }
                    }
                }
                retained.reverse();
                changed |= retained.len() != old_len;
                block.instructions = retained;
            }
        }

        OptimizationReport {
            pass_name: self.name(),
            changed,
        }
    }
}

fn fold_string_intrinsic(
    intrinsic: Intrinsic,
    receiver: &ConstValue,
    args: &[ValueId],
    constants: &AHashMap<ValueId, ConstValue>,
) -> Option<ConstValue> {
    let ConstValue::String(receiver) = receiver else {
        return None;
    };
    let string_argument = || {
        args.first()
            .and_then(|value| constants.get(value))
            .and_then(|value| match value {
                ConstValue::String(value) => Some(value.as_str()),
                _ => None,
            })
    };
    match intrinsic {
        Intrinsic::StringLength => Some(ConstValue::Int(receiver.encode_utf16().count() as i64)),
        Intrinsic::StringCharCodeAt => {
            let index = args
                .first()
                .and_then(|value| constants.get(value))
                .and_then(|value| match value {
                    ConstValue::Int(value) => usize::try_from(*value).ok(),
                    _ => None,
                })?;
            Some(ConstValue::Int(i64::from(
                receiver.encode_utf16().nth(index).unwrap_or(0),
            )))
        }
        Intrinsic::StringCharAt => {
            let index = args
                .first()
                .and_then(|value| constants.get(value))
                .and_then(|value| match value {
                    ConstValue::Int(value) => usize::try_from(*value).ok(),
                    _ => None,
                })?;
            match receiver.encode_utf16().nth(index) {
                None => Some(ConstValue::String(String::new())),
                Some(unit) => char::decode_utf16([unit])
                    .next()
                    .and_then(Result::ok)
                    .map(|value| ConstValue::String(value.to_string())),
            }
        }
        Intrinsic::StringIncludes => Some(ConstValue::Bool(receiver.contains(string_argument()?))),
        Intrinsic::StringIndexOf | Intrinsic::StringLastIndexOf => {
            let position = match args.get(1) {
                Some(value) => match constants.get(value) {
                    Some(ConstValue::Int(value)) => i32::try_from(*value).ok()?,
                    _ => return None,
                },
                None if intrinsic == Intrinsic::StringIndexOf => 0,
                None => i32::MAX,
            };
            Some(ConstValue::Int(i64::from(const_utf16_string_index(
                receiver,
                string_argument()?,
                position,
                intrinsic == Intrinsic::StringLastIndexOf,
            ))))
        }
        Intrinsic::StringRepeat => {
            let count = args
                .first()
                .and_then(|value| constants.get(value))
                .and_then(|value| match value {
                    ConstValue::Int(value) => usize::try_from(*value).ok(),
                    _ => None,
                })?;
            let length = receiver.len().checked_mul(count)?;
            (length <= 4096).then(|| ConstValue::String(receiver.repeat(count)))
        }
        Intrinsic::StringStartsWith => {
            Some(ConstValue::Bool(receiver.starts_with(string_argument()?)))
        }
        Intrinsic::StringEndsWith => Some(ConstValue::Bool(receiver.ends_with(string_argument()?))),
        Intrinsic::StringToUpperCase if receiver.is_ascii() => {
            Some(ConstValue::String(receiver.to_ascii_uppercase()))
        }
        Intrinsic::StringToLowerCase if receiver.is_ascii() => {
            Some(ConstValue::String(receiver.to_ascii_lowercase()))
        }
        _ => None,
    }
}

fn const_utf16_string_index(receiver: &str, needle: &str, position: i32, last: bool) -> i32 {
    let receiver = receiver.encode_utf16().collect::<Vec<_>>();
    let needle = needle.encode_utf16().collect::<Vec<_>>();
    let position = if position < 0 {
        0
    } else {
        usize::try_from(position)
            .unwrap_or(usize::MAX)
            .min(receiver.len())
    };
    if needle.is_empty() {
        return position as i32;
    }
    if needle.len() > receiver.len() {
        return -1;
    }
    if last {
        (0..=position.min(receiver.len() - needle.len()))
            .rev()
            .find(|index| receiver[*index..*index + needle.len()] == needle)
            .map_or(-1, |index| index as i32)
    } else if position + needle.len() > receiver.len() {
        -1
    } else {
        (position..=receiver.len() - needle.len())
            .find(|index| receiver[*index..*index + needle.len()] == needle)
            .map_or(-1, |index| index as i32)
    }
}

fn js_round(value: f64) -> f64 {
    if value.is_sign_negative() && value >= -0.5 {
        -0.0
    } else {
        (value + 0.5).floor()
    }
}

fn js_to_i32(value: f64) -> i32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    let mut normalized = value.trunc() % 4_294_967_296.0;
    if normalized < 0.0 {
        normalized += 4_294_967_296.0;
    }
    if normalized >= 2_147_483_648.0 {
        (normalized - 4_294_967_296.0) as i32
    } else {
        normalized as i32
    }
}

fn js_min(lhs: f64, rhs: f64) -> f64 {
    if lhs.is_nan() || rhs.is_nan() {
        return f64::NAN;
    }
    if lhs == 0.0 && rhs == 0.0 {
        return if lhs.is_sign_negative() || rhs.is_sign_negative() {
            -0.0
        } else {
            0.0
        };
    }
    if lhs < rhs {
        lhs
    } else {
        rhs
    }
}

fn js_max(lhs: f64, rhs: f64) -> f64 {
    if lhs.is_nan() || rhs.is_nan() {
        return f64::NAN;
    }
    if lhs == 0.0 && rhs == 0.0 {
        return if lhs.is_sign_positive() || rhs.is_sign_positive() {
            0.0
        } else {
            -0.0
        };
    }
    if lhs > rhs {
        lhs
    } else {
        rhs
    }
}

fn constant_string_value(value: &ConstValue) -> Option<String> {
    Some(match value {
        ConstValue::Int(value) => value.to_string(),
        ConstValue::Float(value) => value.to_string(),
        ConstValue::Bool(value) => value.to_string(),
        ConstValue::String(value) => value.clone(),
        ConstValue::Null => "null".to_string(),
    })
}

fn push_template_string(parts: &mut Vec<TemplateOperand>, value: &str) {
    if let Some(TemplateOperand::String(previous)) = parts.last_mut() {
        previous.push_str(value);
    } else if !value.is_empty() {
        parts.push(TemplateOperand::String(value.to_string()));
    }
}

fn javascript_typeof_of_const(value: &ConstValue) -> &'static str {
    match value {
        ConstValue::Int(_) | ConstValue::Float(_) => "number",
        ConstValue::Bool(_) => "boolean",
        ConstValue::String(_) => "string",
        ConstValue::Null => "object",
    }
}

fn javascript_typeof_name(op: &ControlFlowOp<'_>) -> Option<&'static str> {
    match op {
        ControlFlowOp::Const(value) => Some(javascript_typeof_of_const(value)),
        ControlFlowOp::Array(_)
        | ControlFlowOp::Record(_)
        | ControlFlowOp::Struct { .. }
        | ControlFlowOp::NewClass { .. } => Some("object"),
        ControlFlowOp::Closure { .. } => Some("function"),
        ControlFlowOp::Intrinsic { intrinsic, .. } => match intrinsic {
            // `document` is undefined off-browser (Node, workers), so its
            // typeof is not statically "object" even though the window root
            // spelling guarantees an object for JsWindow.
            Intrinsic::JsPlainObject
            | Intrinsic::JsNullProtoObject
            | Intrinsic::JsWindow
            | Intrinsic::JsDomParserNew
            | Intrinsic::JsXMLHttpRequestNew
            | Intrinsic::JsArrayFlat
            | Intrinsic::JsArrayIterator => Some("object"),
            Intrinsic::JsUndefined => Some("undefined"),
            Intrinsic::JsObjectConstructor
            | Intrinsic::JsMethod0
            | Intrinsic::JsMethod1
            | Intrinsic::JsMethod2
            | Intrinsic::JsMethod3
            | Intrinsic::JsMethodRest
            | Intrinsic::JsStaticRest => Some("function"),
            Intrinsic::JsTypeOf => Some("string"),
            Intrinsic::JsIsFunctionValue
            | Intrinsic::JsIsWindowValue
            | Intrinsic::JsIsNullish
            | Intrinsic::JsIsFalse
            | Intrinsic::JsIsUndefined
            | Intrinsic::JsIsArray
            | Intrinsic::JsIsObject => Some("boolean"),
            _ => None,
        },
        _ => None,
    }
}

fn fold_binary(op: IrBinaryOp, lhs: &ConstValue, rhs: &ConstValue) -> Option<ConstValue> {
    use ConstValue::{Bool, Float, Int, String};
    use IrBinaryOp::{
        Add, BitAnd, BitOr, Div, Eq, Greater, GreaterEq, Less, LessEq, Mod, Mul, NotEq, ShiftLeft,
        ShiftRight, Sub, UnsignedShiftRight, Xor,
    };

    if let Some((lhs, rhs)) = mixed_numeric_constants(lhs, rhs) {
        return match op {
            Add => Some(Float(lhs + rhs)),
            Sub => Some(Float(lhs - rhs)),
            Mul => Some(Float(lhs * rhs)),
            Div if rhs != 0.0 => Some(Float(lhs / rhs)),
            Eq => Some(Bool(lhs == rhs)),
            NotEq => Some(Bool(lhs != rhs)),
            Less => Some(Bool(lhs < rhs)),
            LessEq => Some(Bool(lhs <= rhs)),
            Greater => Some(Bool(lhs > rhs)),
            GreaterEq => Some(Bool(lhs >= rhs)),
            _ => None,
        };
    }

    match (op, lhs, rhs) {
        (Add, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_add(*rhs as i32)))),
        (Sub, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_sub(*rhs as i32)))),
        (Mul, Int(lhs), Int(rhs)) => {
            Some(Int(i64::from(js_i32_multiply(*lhs as i32, *rhs as i32))))
        }
        (Div, Int(_), Int(0)) | (Mod, Int(_), Int(0)) => None,
        (Div, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_div(*rhs as i32)))),
        (Mod, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_rem(*rhs as i32)))),
        (BitAnd, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32) & (*rhs as i32)))),
        (BitOr, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32) | (*rhs as i32)))),
        (Xor, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32) ^ (*rhs as i32)))),
        (ShiftLeft, Int(lhs), Int(rhs)) => Some(Int(i64::from(
            (*lhs as i32).wrapping_shl((*rhs as u32) & 31),
        ))),
        (ShiftRight, Int(lhs), Int(rhs)) => {
            Some(Int(i64::from((*lhs as i32) >> ((*rhs as u32) & 31))))
        }
        (UnsignedShiftRight, Int(lhs), Int(rhs)) => Some(Int(i64::from(
            (((*lhs as i32) as u32) >> ((*rhs as u32) & 31)) as i32,
        ))),
        (Add, Float(lhs), Float(rhs)) => Some(Float(lhs + rhs)),
        (Sub, Float(lhs), Float(rhs)) => Some(Float(lhs - rhs)),
        (Mul, Float(lhs), Float(rhs)) => Some(Float(lhs * rhs)),
        (Div, Float(_), Float(rhs)) if *rhs == 0.0 => None,
        (Div, Float(lhs), Float(rhs)) => Some(Float(lhs / rhs)),
        (Add, String(lhs), String(rhs)) => Some(String(format!("{lhs}{rhs}"))),
        (Eq, lhs, rhs) => Some(Bool(lhs == rhs)),
        (NotEq, lhs, rhs) => Some(Bool(lhs != rhs)),
        (Less, Int(lhs), Int(rhs)) => Some(Bool(lhs < rhs)),
        (LessEq, Int(lhs), Int(rhs)) => Some(Bool(lhs <= rhs)),
        (Greater, Int(lhs), Int(rhs)) => Some(Bool(lhs > rhs)),
        (GreaterEq, Int(lhs), Int(rhs)) => Some(Bool(lhs >= rhs)),
        (Less, Float(lhs), Float(rhs)) => Some(Bool(lhs < rhs)),
        (LessEq, Float(lhs), Float(rhs)) => Some(Bool(lhs <= rhs)),
        (Greater, Float(lhs), Float(rhs)) => Some(Bool(lhs > rhs)),
        (GreaterEq, Float(lhs), Float(rhs)) => Some(Bool(lhs >= rhs)),
        (IrBinaryOp::And, Bool(lhs), Bool(rhs)) => Some(Bool(*lhs && *rhs)),
        (IrBinaryOp::Or, Bool(lhs), Bool(rhs)) => Some(Bool(*lhs || *rhs)),
        _ => None,
    }
}

fn js_i32_multiply(lhs: i32, rhs: i32) -> i32 {
    let product = f64::from(lhs) * f64::from(rhs);
    (product as i64 as u32) as i32
}

fn format_i32_radix(value: i32, radix: u32, unsigned: bool) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let negative = !unsigned && value < 0;
    let mut magnitude = if unsigned {
        value as u32
    } else {
        value.unsigned_abs()
    };
    let mut reversed = Vec::new();
    loop {
        reversed.push(DIGITS[(magnitude % radix) as usize]);
        magnitude /= radix;
        if magnitude == 0 {
            break;
        }
    }
    if negative {
        reversed.push(b'-');
    }
    reversed.reverse();
    String::from_utf8(reversed).expect("radix digits are ASCII")
}

fn mixed_numeric_constants(lhs: &ConstValue, rhs: &ConstValue) -> Option<(f64, f64)> {
    match (lhs, rhs) {
        (ConstValue::Int(lhs), ConstValue::Float(rhs)) => Some((*lhs as f64, *rhs)),
        (ConstValue::Float(lhs), ConstValue::Int(rhs)) => Some((*lhs, *rhs as f64)),
        _ => None,
    }
}

fn fold_unary(op: IrUnaryOp, value: &ConstValue) -> Option<ConstValue> {
    match (op, value) {
        (IrUnaryOp::Neg, ConstValue::Int(value)) => {
            Some(ConstValue::Int(i64::from((*value as i32).wrapping_neg())))
        }
        (IrUnaryOp::Neg, ConstValue::Float(value)) => Some(ConstValue::Float(-value)),
        (IrUnaryOp::Not, ConstValue::Bool(value)) => Some(ConstValue::Bool(!value)),
        _ => None,
    }
}

fn result_value(instruction: &Instruction) -> Option<ValueId> {
    match instruction {
        Instruction::Const { out, .. }
        | Instruction::Binary { out, .. }
        | Instruction::Struct { out, .. }
        | Instruction::FieldGet { out, .. } => Some(*out),
        Instruction::Call { out, .. } => *out,
        Instruction::Return { .. } => None,
    }
}

fn used_values(instruction: &Instruction) -> Vec<ValueId> {
    match instruction {
        Instruction::Const { .. } => Vec::new(),
        Instruction::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        Instruction::Struct { fields, .. } => fields.clone(),
        Instruction::FieldGet { aggregate, .. } => vec![*aggregate],
        Instruction::Call { args, .. } => args.clone(),
        Instruction::Return { value, .. } => value.iter().copied().collect(),
    }
}

fn rewrite_values(instruction: &mut Instruction, aliases: &AHashMap<ValueId, ValueId>) {
    match instruction {
        Instruction::Const { .. } => {}
        Instruction::Binary { lhs, rhs, .. } => {
            *lhs = resolve_alias(*lhs, aliases);
            *rhs = resolve_alias(*rhs, aliases);
        }
        Instruction::Struct { fields, .. } => {
            for field in fields {
                *field = resolve_alias(*field, aliases);
            }
        }
        Instruction::FieldGet { aggregate, .. } => {
            *aggregate = resolve_alias(*aggregate, aliases);
        }
        Instruction::Call { args, .. } => {
            for arg in args {
                *arg = resolve_alias(*arg, aliases);
            }
        }
        Instruction::Return { value, .. } => {
            if let Some(value) = value {
                *value = resolve_alias(*value, aliases);
            }
        }
    }
}

fn resolve_alias(mut value: ValueId, aliases: &AHashMap<ValueId, ValueId>) -> ValueId {
    while let Some(next) = aliases.get(&value) {
        if *next == value {
            break;
        }
        value = *next;
    }
    value
}

fn exception_region_blocks(function: &ControlFlowFunction<'_>) -> AHashSet<BlockId> {
    let mut region_blocks = AHashSet::<BlockId>::default();
    for shape in &function.shapes {
        let crate::ir::ControlShape::Try {
            body,
            catch_block,
            finally_block,
            merge_block,
            ..
        } = shape
        else {
            continue;
        };
        let continuation = finally_block.unwrap_or(*merge_block);
        collect_region_blocks_until(function, *body, continuation, &mut region_blocks);
        if let Some(catch_block) = catch_block {
            collect_region_blocks_until(function, *catch_block, continuation, &mut region_blocks);
        }
        if let Some(finally_block) = finally_block {
            collect_region_blocks_until(function, *finally_block, *merge_block, &mut region_blocks);
        }
    }
    region_blocks
}

fn exception_region_written_locals(function: &ControlFlowFunction<'_>) -> AHashSet<LocalId> {
    let region_blocks = exception_region_blocks(function);
    let mut finally_blocks = AHashSet::<BlockId>::default();
    for shape in &function.shapes {
        let crate::ir::ControlShape::Try {
            finally_block,
            merge_block,
            ..
        } = shape
        else {
            continue;
        };
        if let Some(finally_block) = finally_block {
            collect_region_blocks_until(
                function,
                *finally_block,
                *merge_block,
                &mut finally_blocks,
            );
        }
    }

    let mut locals = region_blocks
        .into_iter()
        .flat_map(|block| &function.blocks[block.0 as usize].instructions)
        .filter_map(|instruction| match instruction.op {
            ControlFlowOp::StoreLocal { local, .. } => Some(local),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    // `finally` executes through implicit completion edges for throws and
    // early returns. Those edges are deliberately absent from the ordinary
    // CFG, so even a local only *read* by `finally` cannot be represented by
    // the normal dominance-based SSA renamer. Keep that narrow set mutable;
    // unrelated locals in the same function remain fully promotable.
    locals.extend(
        finally_blocks
            .into_iter()
            .flat_map(|block| &function.blocks[block.0 as usize].instructions)
            .filter_map(|instruction| match instruction.op {
                ControlFlowOp::LoadLocal(local) | ControlFlowOp::StoreLocal { local, .. } => {
                    Some(local)
                }
                _ => None,
            }),
    );
    locals
}

fn collect_region_blocks_until(
    function: &ControlFlowFunction<'_>,
    start: BlockId,
    stop: BlockId,
    blocks: &mut AHashSet<BlockId>,
) {
    let mut pending = vec![start];
    let mut visited = AHashSet::default();
    while let Some(block) = pending.pop() {
        if block == stop || !visited.insert(block) {
            continue;
        }
        blocks.insert(block);
        pending.extend(
            terminator_successors(function.blocks[block.0 as usize].terminator.as_ref())
                .into_iter()
                .map(|index| BlockId(index as u32)),
        );
    }
}

fn promote_function_locals(
    function: &mut ControlFlowFunction<'_>,
    unpromoted_locals: &AHashSet<LocalId>,
) -> Result<(), SsaError> {
    let block_count = function.blocks.len();
    if block_count == 0 || function.locals.is_empty() {
        return Ok(());
    }
    let entry = function.entry.0 as usize;
    let predecessors = cfg_predecessors(function);
    let reachable = reachable_blocks(function);
    let dominators = compute_dominators(entry, &predecessors, &reachable);
    let immediate_dominators = compute_immediate_dominators(entry, &dominators, &reachable);
    let dominance_frontiers =
        compute_dominance_frontiers(&predecessors, &immediate_dominators, &reachable);

    let local_count = function.locals.len();
    let mut value_local_hints = function.value_local_hints.clone();
    value_local_hints.resize(function.value_count as usize, None);
    let mut conflicting_local_hints = AHashSet::default();
    let live_in = local_live_in(function, local_count);
    let mut def_blocks = vec![AHashSet::<usize>::default(); local_count];
    for (block_index, block) in function.blocks.iter().enumerate() {
        for instruction in &block.instructions {
            if let ControlFlowOp::StoreLocal { local, .. } = instruction.op {
                def_blocks[local.0 as usize].insert(block_index);
            }
        }
    }

    // `promote_function_locals` is deliberately re-entrant: late global
    // internalization may append locals after an earlier mem2reg round.  Seed
    // placement from existing local phis so a second round does not duplicate
    // them, and only populate incoming edges for phis created in this round.
    let mut has_phi = vec![AHashSet::<usize>::default(); local_count];
    for (block_index, block) in function.blocks.iter().enumerate() {
        for phi in &block.phis {
            if let Some(local) = phi.origin.local() {
                has_phi[local.0 as usize].insert(block_index);
            }
        }
    }
    let mut new_phis = AHashSet::default();
    for local_index in 0..local_count {
        if unpromoted_locals.contains(&LocalId(local_index as u32)) {
            continue;
        }
        let mut work = def_blocks[local_index].iter().copied().collect::<Vec<_>>();
        work.sort_unstable();
        while let Some(block_index) = work.pop() {
            let mut frontier = dominance_frontiers[block_index]
                .iter()
                .copied()
                .collect::<Vec<_>>();
            frontier.sort_unstable();
            for target in frontier {
                if !live_in[target].contains(&local_index) {
                    continue;
                }
                if !has_phi[local_index].insert(target) {
                    continue;
                }
                let out = ValueId(function.value_count);
                function.value_count += 1;
                function.value_escapes.push(EscapeState::LocalOnly);
                let local = &function.locals[local_index];
                value_local_hints.push(Some(local.name));
                let span = function.blocks[target].span;
                function.blocks[target].phis.push(Phi {
                    out,
                    origin: crate::ir::PhiOrigin::Local(LocalId(local_index as u32)),
                    ty: local.ty.clone(),
                    incoming: Vec::new(),
                    span,
                });
                new_phis.insert(out);
                if !def_blocks[local_index].contains(&target) {
                    work.push(target);
                }
            }
        }
    }

    for block in &mut function.blocks {
        block
            .phis
            .sort_by_key(|phi| phi.origin.local().map_or(u32::MAX, |local| local.0));
    }

    let mut dominator_children = vec![Vec::new(); block_count];
    for (block, idom) in immediate_dominators.iter().enumerate() {
        if let Some(idom) = idom {
            dominator_children[*idom].push(block);
        }
    }
    for children in &mut dominator_children {
        children.sort_unstable();
    }

    let mut stacks = vec![Vec::<ValueId>::new(); local_count];
    let mut aliases = AHashMap::<ValueId, ValueId>::default();
    rename_block(
        entry,
        function,
        &dominator_children,
        &mut stacks,
        &mut aliases,
        &mut value_local_hints,
        &mut conflicting_local_hints,
        unpromoted_locals,
        &new_phis,
    )?;

    eliminate_trivial_phis(function, &mut aliases);
    rewrite_control_flow_function(function, &aliases);
    function.value_local_hints = value_local_hints;

    let unexpected_memory_locals = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction.op {
            ControlFlowOp::LoadLocal(local) | ControlFlowOp::StoreLocal { local, .. }
                if !unpromoted_locals.contains(&local) =>
            {
                Some(local)
            }
            _ => None,
        })
        .collect::<AHashSet<_>>();
    if !unexpected_memory_locals.is_empty() {
        let mut unexpected = unexpected_memory_locals
            .iter()
            .map(|local| {
                function
                    .locals
                    .get(local.0 as usize)
                    .map_or_else(|| format!("{:?}", local), |entry| entry.name.to_string())
            })
            .collect::<Vec<_>>();
        unexpected.sort();
        return Err(SsaError {
            span: function.span,
            message: format!(
                "local promotion left memory operations for [{}] in function {:?}",
                unexpected.join(", "),
                function.id,
            ),
        });
    }
    Ok(())
}

fn local_live_in(function: &ControlFlowFunction<'_>, local_count: usize) -> Vec<AHashSet<usize>> {
    let block_count = function.blocks.len();
    let mut uses = vec![AHashSet::<usize>::default(); block_count];
    let mut definitions = vec![AHashSet::<usize>::default(); block_count];
    for (block_index, block) in function.blocks.iter().enumerate() {
        for instruction in &block.instructions {
            match instruction.op {
                ControlFlowOp::LoadLocal(local) => {
                    let local = local.0 as usize;
                    if !definitions[block_index].contains(&local) {
                        uses[block_index].insert(local);
                    }
                }
                ControlFlowOp::StoreLocal { local, .. } => {
                    definitions[block_index].insert(local.0 as usize);
                }
                _ => {}
            }
        }
    }

    let mut live_in = vec![AHashSet::<usize>::default(); block_count];
    let mut live_out = vec![AHashSet::<usize>::default(); block_count];
    loop {
        let mut changed = false;
        for (block_index, block) in function.blocks.iter().enumerate().rev() {
            let mut out = AHashSet::with_capacity_and_hasher(local_count, Default::default());
            for successor in terminator_successors(block.terminator.as_ref()) {
                out.extend(live_in[successor].iter().copied());
            }
            let mut input = uses[block_index].clone();
            input.extend(out.difference(&definitions[block_index]).copied());
            if out != live_out[block_index] {
                live_out[block_index] = out;
                changed = true;
            }
            if input != live_in[block_index] {
                live_in[block_index] = input;
                changed = true;
            }
        }
        if !changed {
            return live_in;
        }
    }
}

fn cfg_predecessors(function: &ControlFlowFunction<'_>) -> Vec<Vec<usize>> {
    let mut predecessors = vec![Vec::new(); function.blocks.len()];
    for (index, block) in function.blocks.iter().enumerate() {
        for successor in terminator_successors(block.terminator.as_ref()) {
            predecessors[successor].push(index);
        }
    }
    for incoming in &mut predecessors {
        incoming.sort_unstable();
        incoming.dedup();
    }
    predecessors
}

fn reachable_blocks(function: &ControlFlowFunction<'_>) -> AHashSet<usize> {
    let mut reachable = AHashSet::default();
    let mut work = vec![function.entry.0 as usize];
    while let Some(block) = work.pop() {
        if !reachable.insert(block) {
            continue;
        }
        work.extend(terminator_successors(
            function.blocks[block].terminator.as_ref(),
        ));
    }
    reachable
}

fn terminator_successors(terminator: Option<&Terminator>) -> Vec<usize> {
    match terminator {
        Some(Terminator::Jump(block)) => vec![block.0 as usize],
        Some(Terminator::Branch {
            then_block,
            else_block,
            ..
        }) => vec![then_block.0 as usize, else_block.0 as usize],
        Some(Terminator::Try { body, catch_block }) => std::iter::once(body.0 as usize)
            .chain(catch_block.iter().map(|block| block.0 as usize))
            .collect(),
        _ => Vec::new(),
    }
}

fn compute_dominators(
    entry: usize,
    predecessors: &[Vec<usize>],
    reachable: &AHashSet<usize>,
) -> Vec<AHashSet<usize>> {
    let all = reachable.iter().copied().collect::<AHashSet<_>>();
    let mut dominators = (0..predecessors.len())
        .map(|block| {
            if block == entry || !reachable.contains(&block) {
                AHashSet::from_iter([block])
            } else {
                all.clone()
            }
        })
        .collect::<Vec<_>>();

    let mut changed = true;
    while changed {
        changed = false;
        for block in 0..predecessors.len() {
            if block == entry || !reachable.contains(&block) {
                continue;
            }
            let mut incoming = predecessors[block]
                .iter()
                .filter(|pred| reachable.contains(pred));
            let mut next = incoming
                .next()
                .map(|pred| dominators[*pred].clone())
                .unwrap_or_default();
            for predecessor in incoming {
                next.retain(|dominator| dominators[*predecessor].contains(dominator));
            }
            next.insert(block);
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
    }
    dominators
}

fn compute_immediate_dominators(
    entry: usize,
    dominators: &[AHashSet<usize>],
    reachable: &AHashSet<usize>,
) -> Vec<Option<usize>> {
    let mut result = vec![None; dominators.len()];
    for block in 0..dominators.len() {
        if block == entry || !reachable.contains(&block) {
            continue;
        }
        result[block] = dominators[block]
            .iter()
            .copied()
            .filter(|candidate| *candidate != block)
            .max_by_key(|candidate| dominators[*candidate].len());
    }
    result
}

fn compute_dominance_frontiers(
    predecessors: &[Vec<usize>],
    immediate_dominators: &[Option<usize>],
    reachable: &AHashSet<usize>,
) -> Vec<AHashSet<usize>> {
    let mut frontiers = vec![AHashSet::default(); predecessors.len()];
    for block in 0..predecessors.len() {
        let incoming = predecessors[block]
            .iter()
            .filter(|pred| reachable.contains(pred))
            .copied()
            .collect::<Vec<_>>();
        if incoming.len() < 2 {
            continue;
        }
        let Some(stop) = immediate_dominators[block] else {
            continue;
        };
        for predecessor in incoming {
            let mut runner = predecessor;
            while runner != stop {
                frontiers[runner].insert(block);
                let Some(next) = immediate_dominators[runner] else {
                    break;
                };
                runner = next;
            }
        }
    }
    frontiers
}

#[allow(clippy::too_many_arguments)]
fn rename_block<'src>(
    block_index: usize,
    function: &mut ControlFlowFunction<'src>,
    dominator_children: &[Vec<usize>],
    stacks: &mut [Vec<ValueId>],
    aliases: &mut AHashMap<ValueId, ValueId>,
    value_local_hints: &mut Vec<Option<&'src str>>,
    conflicting_local_hints: &mut AHashSet<ValueId>,
    unpromoted_locals: &AHashSet<LocalId>,
    new_phis: &AHashSet<ValueId>,
) -> Result<(), SsaError> {
    let mut pushes = vec![0usize; stacks.len()];

    let phi_defs = function.blocks[block_index]
        .phis
        .iter()
        .filter_map(|phi| phi.origin.local().map(|local| (local, phi.out)))
        .collect::<Vec<_>>();
    for (local, out) in phi_defs {
        stacks[local.0 as usize].push(out);
        pushes[local.0 as usize] += 1;
    }

    let instructions = std::mem::take(&mut function.blocks[block_index].instructions);
    let mut retained = Vec::with_capacity(instructions.len());
    for mut instruction in instructions {
        match instruction.op {
            ControlFlowOp::LoadLocal(local) if unpromoted_locals.contains(&local) => {
                retained.push(instruction);
            }
            ControlFlowOp::LoadLocal(local) => {
                let out = instruction.out.ok_or_else(|| SsaError {
                    span: instruction.span,
                    message: "local load has no result value".to_string(),
                })?;
                let value = stacks[local.0 as usize]
                    .last()
                    .copied()
                    .ok_or_else(|| SsaError {
                        span: instruction.span,
                        message: format!("local {:?} is read before definition", local),
                    })?;
                aliases.insert(out, resolve_alias(value, aliases));
            }
            ControlFlowOp::StoreLocal { local, value } if unpromoted_locals.contains(&local) => {
                instruction.op = ControlFlowOp::StoreLocal {
                    local,
                    value: resolve_alias(value, aliases),
                };
                retained.push(instruction);
            }
            ControlFlowOp::StoreLocal { local, value } => {
                let value = resolve_alias(value, aliases);
                record_value_local_hint(
                    value_local_hints,
                    conflicting_local_hints,
                    value,
                    function.locals[local.0 as usize].name,
                );
                stacks[local.0 as usize].push(value);
                pushes[local.0 as usize] += 1;
            }
            _ => {
                rewrite_control_flow_op(&mut instruction.op, aliases);
                retained.push(instruction);
            }
        }
    }
    function.blocks[block_index].instructions = retained;
    if let Some(terminator) = &mut function.blocks[block_index].terminator {
        rewrite_terminator(terminator, aliases);
    }

    let block_span = function.blocks[block_index].span;
    let successors = terminator_successors(function.blocks[block_index].terminator.as_ref());
    for successor in successors {
        for phi in &mut function.blocks[successor].phis {
            if !new_phis.contains(&phi.out) {
                continue;
            }
            let Some(local) = phi.origin.local() else {
                continue;
            };
            let value = stacks[local.0 as usize]
                .last()
                .copied()
                .ok_or_else(|| SsaError {
                    span: block_span,
                    message: format!(
                        "local {:?} has no reaching definition for block {:?}",
                        local, phi.out
                    ),
                })?;
            phi.incoming
                .push((BlockId(block_index as u32), resolve_alias(value, aliases)));
        }
    }

    for child in &dominator_children[block_index] {
        rename_block(
            *child,
            function,
            dominator_children,
            stacks,
            aliases,
            value_local_hints,
            conflicting_local_hints,
            unpromoted_locals,
            new_phis,
        )?;
    }

    for (local, count) in pushes.into_iter().enumerate() {
        let len = stacks[local].len();
        stacks[local].truncate(len - count);
    }
    Ok(())
}

fn record_value_local_hint<'src>(
    hints: &mut Vec<Option<&'src str>>,
    conflicts: &mut AHashSet<ValueId>,
    value: ValueId,
    local: &'src str,
) {
    if conflicts.contains(&value) {
        return;
    }
    if hints.len() <= value.0 as usize {
        hints.resize(value.0 as usize + 1, None);
    }
    match hints[value.0 as usize] {
        None => hints[value.0 as usize] = Some(local),
        Some(previous) if previous != local => {
            hints[value.0 as usize] = None;
            conflicts.insert(value);
        }
        Some(_) => {}
    }
}

fn eliminate_trivial_phis(
    function: &mut ControlFlowFunction<'_>,
    aliases: &mut AHashMap<ValueId, ValueId>,
) {
    let mut changed = true;
    while changed {
        changed = false;
        for block in &mut function.blocks {
            block.phis.retain(|phi| {
                let mut values = phi
                    .incoming
                    .iter()
                    .map(|(_, value)| resolve_alias(*value, aliases))
                    .filter(|value| *value != phi.out);
                let Some(first) = values.next() else {
                    return true;
                };
                if values.all(|value| value == first) {
                    aliases.insert(phi.out, first);
                    changed = true;
                    false
                } else {
                    true
                }
            });
        }
    }
}

fn rewrite_control_flow_function(
    function: &mut ControlFlowFunction<'_>,
    aliases: &AHashMap<ValueId, ValueId>,
) {
    // Parameter defaults are operands too. In particular, capture
    // specialization can replace and then remove a captured parameter that a
    // later public parameter uses as its default. Leaving the metadata behind
    // would create a dangling SSA reference even though every executable
    // operand was rewritten correctly.
    for parameter in &mut function.params {
        if let Some(crate::ir::IrParamDefault::Value(value)) = &mut parameter.default {
            *value = resolve_alias(*value, aliases);
        }
    }
    for block in &mut function.blocks {
        for phi in &mut block.phis {
            for (_, value) in &mut phi.incoming {
                *value = resolve_alias(*value, aliases);
            }
            rewrite_phi_origin(&mut phi.origin, aliases);
        }
        for instruction in &mut block.instructions {
            rewrite_control_flow_op(&mut instruction.op, aliases);
        }
        if let Some(terminator) = &mut block.terminator {
            rewrite_terminator(terminator, aliases);
        }
    }
    for shape in &mut function.shapes {
        match shape {
            crate::ir::ControlShape::ForIn { object, key, .. } => {
                *object = resolve_alias(*object, aliases);
                *key = resolve_alias(*key, aliases);
            }
            crate::ir::ControlShape::ForOf {
                iterable, element, ..
            } => {
                *iterable = resolve_alias(*iterable, aliases);
                *element = resolve_alias(*element, aliases);
            }
            crate::ir::ControlShape::Try {
                catch_value: Some(value),
                ..
            } => *value = resolve_alias(*value, aliases),
            _ => {}
        }
    }
}

fn rewrite_phi_origin(origin: &mut crate::ir::PhiOrigin, aliases: &AHashMap<ValueId, ValueId>) {
    match origin {
        crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::ShortCircuit {
            lhs, ..
        })
        | crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Nullish { lhs }) => {
            *lhs = resolve_alias(*lhs, aliases);
        }
        crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::OptionalAccess { object }) => {
            *object = resolve_alias(*object, aliases);
        }
        crate::ir::PhiOrigin::Local(_)
        | crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Conditional)
        | crate::ir::PhiOrigin::Synthetic => {}
    }
}

fn rewrite_control_flow_op(op: &mut ControlFlowOp<'_>, aliases: &AHashMap<ValueId, ValueId>) {
    rewrite_control_flow_values(op, |value| *value = resolve_alias(*value, aliases));
}

fn rewrite_control_flow_op_once(op: &mut ControlFlowOp<'_>, mapping: &AHashMap<ValueId, ValueId>) {
    rewrite_control_flow_values(op, |value| {
        if let Some(mapped) = mapping.get(value) {
            *value = *mapped;
        }
    });
}

fn rewrite_control_flow_values(op: &mut ControlFlowOp<'_>, mut rewrite: impl FnMut(&mut ValueId)) {
    match op {
        ControlFlowOp::Const(_)
        | ControlFlowOp::CaughtException
        | ControlFlowOp::CaptureLocal(_)
        | ControlFlowOp::LoadLocal(_)
        | ControlFlowOp::LoadGlobal(_)
        | ControlFlowOp::DynamicImport { .. } => {}
        ControlFlowOp::Unary { value, .. } | ControlFlowOp::TypeCheck { value, .. } => {
            rewrite(value)
        }
        ControlFlowOp::Await { task } => rewrite(task),
        ControlFlowOp::Binary { lhs, rhs, .. } => {
            rewrite(lhs);
            rewrite(rhs);
        }
        ControlFlowOp::Array(values) => values.iter_mut().for_each(&mut rewrite),
        ControlFlowOp::ArraySpread(operands) => {
            operands.iter_mut().for_each(|operand| match operand {
                ArrayOperand::Value(value) | ArrayOperand::Spread(value) => rewrite(value),
            })
        }
        ControlFlowOp::Record(entries) => entries.iter_mut().for_each(|(_, value)| rewrite(value)),
        ControlFlowOp::RecordSpread(operands) => {
            operands.iter_mut().for_each(|operand| match operand {
                RecordOperand::Entry(_, value) | RecordOperand::Spread(value) => rewrite(value),
            })
        }
        ControlFlowOp::Struct { fields, .. } => fields.iter_mut().for_each(&mut rewrite),
        ControlFlowOp::NewClass { args, .. } => args.iter_mut().for_each(&mut rewrite),
        ControlFlowOp::Closure { captures, .. } => captures.iter_mut().for_each(&mut rewrite),
        ControlFlowOp::StoreLocal { value, .. } | ControlFlowOp::StoreGlobal { value, .. } => {
            rewrite(value)
        }
        ControlFlowOp::FieldGet { object, .. }
        | ControlFlowOp::RecordFieldGet { object, .. }
        | ControlFlowOp::RecordRest { object, .. }
        | ControlFlowOp::HostFieldGet { object, .. } => rewrite(object),
        ControlFlowOp::FieldSet { object, value, .. }
        | ControlFlowOp::RecordFieldSet { object, value, .. }
        | ControlFlowOp::HostFieldSet { object, value, .. } => {
            rewrite(object);
            rewrite(value);
        }
        ControlFlowOp::IndexGet { object, index } => {
            rewrite(object);
            rewrite(index);
        }
        ControlFlowOp::ArrayGetOptional { object, .. } => rewrite(object),
        ControlFlowOp::IndexSet {
            object,
            index,
            value,
        } => {
            rewrite(object);
            rewrite(index);
            rewrite(value);
        }
        ControlFlowOp::CallDirect { args, .. } => args.iter_mut().for_each(&mut rewrite),
        ControlFlowOp::CallMethod { receiver, args, .. } => {
            rewrite(receiver);
            args.iter_mut().for_each(&mut rewrite);
        }
        ControlFlowOp::HostCall { receiver, args, .. } => {
            rewrite(receiver);
            args.iter_mut().for_each(&mut rewrite);
        }
        ControlFlowOp::CallValue { callee, args } => {
            rewrite(callee);
            args.iter_mut().for_each(&mut rewrite);
        }
        ControlFlowOp::Intrinsic { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                rewrite(receiver);
            }
            args.iter_mut().for_each(&mut rewrite);
        }
        ControlFlowOp::Template(parts) => {
            for part in parts {
                if let crate::ir::TemplateOperand::Value(value) = part {
                    rewrite(value);
                }
            }
        }
    }
}

fn rewrite_terminator(terminator: &mut Terminator, aliases: &AHashMap<ValueId, ValueId>) {
    match terminator {
        Terminator::Branch { condition, .. } => {
            *condition = resolve_alias(*condition, aliases);
        }
        Terminator::Return(Some(value)) | Terminator::Throw(value) => {
            *value = resolve_alias(*value, aliases);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::ir::{BasicBlock, IrFunction};
    use crate::span::Span;
    use crate::{analyze, lower_to_control_flow, parse_source};

    const S: Span = Span::new(0, 0);

    fn run_javascript(script: &str) -> String {
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(script)
            .output()
            .expect("Node.js is required for JavaScript runtime parity tests");
        assert!(
            output.status.success(),
            "node failed with {}:\n{}\nscript:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
            script
        );
        String::from_utf8(output.stdout).expect("node stdout is UTF-8")
    }

    fn module(instructions: Vec<Instruction>) -> IrModule {
        IrModule {
            functions: vec![IrFunction {
                id: None,
                blocks: vec![BasicBlock {
                    id: None,
                    instructions,
                }],
            }],
        }
    }

    fn assert_control_flow_values_are_defined(function: &ControlFlowFunction<'_>) {
        let definitions = function
            .params
            .iter()
            .map(|parameter| parameter.value)
            .chain(function.blocks.iter().flat_map(|block| {
                block.phis.iter().map(|phi| phi.out).chain(
                    block
                        .instructions
                        .iter()
                        .filter_map(|instruction| instruction.out),
                )
            }))
            .collect::<AHashSet<_>>();
        let uses = function.blocks.iter().flat_map(|block| {
            block
                .phis
                .iter()
                .flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value))
                .chain(
                    block
                        .instructions
                        .iter()
                        .flat_map(|instruction| control_flow_used_values(&instruction.op)),
                )
                .chain(
                    block
                        .terminator
                        .as_ref()
                        .into_iter()
                        .flat_map(terminator_used_values),
                )
        });
        for value in uses {
            assert!(
                definitions.contains(&value),
                "SSA value {} has a use but no definition",
                value.0
            );
        }
    }

    #[test]
    fn specializes_constant_arguments_and_removes_unused_call_results() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int work(int mode,int value){if(mode==3){print(value);return 1;}return 2;}work(3,4);work(3,8);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        let reports =
            optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&control_flow).unwrap();

        assert!(reports.iter().any(|report| {
            report.pass_name == "constant-parameter-specialization" && report.changed
        }));
        assert!(reports
            .iter()
            .any(|report| { report.pass_name == "unused-return-optimization" && report.changed }));
        let parameters = output
            .split_once('(')
            .and_then(|(_, rest)| rest.split_once(')'))
            .map(|(parameters, _)| parameters)
            .expect("specialized function must be emitted");
        assert!(
            !parameters.is_empty() && !parameters.contains(','),
            "{output}"
        );
        assert!(!output.contains("return"), "{output}");
        assert!(!output.contains("a(3,"), "{output}");
    }

    #[test]
    fn folds_identical_private_functions_after_inlining_decisions() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int left(int value){return value*3+1;}int right(int item){return item*3+1;}print(left(read())+right(read()));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let right = control_flow
            .functions
            .iter()
            .find(|function| function.name == Some("right"))
            .map(|function| function.id)
            .unwrap();
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            ..OptimizationOptions::default()
        };

        let reports =
            optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert!(reports.iter().any(|report| {
            report.pass_name == "identical-private-function-folding" && report.changed
        }));
        assert!(!control_flow.functions[right.0 as usize].live);
        assert_eq!(
            control_flow
                .functions
                .iter()
                .filter(|function| function.live && function.kind == FunctionKind::Function)
                .count(),
            1
        );
    }

    #[test]
    fn subsumes_private_function_proven_by_constant_binding() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int narrow(int value){int mixed=(value+1)*17+31;mixed=mixed^(mixed>>3);return mixed%997;}int broad(int value,int bias){int mixed=(value+bias)*17+31;mixed=mixed^(mixed>>3);return mixed%997;}print(narrow(read())+broad(read(),2)+broad(read(),3));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let narrow = control_flow
            .functions
            .iter()
            .find(|function| function.name == Some("narrow"))
            .map(|function| function.id)
            .unwrap();
        let broad = control_flow
            .functions
            .iter()
            .find(|function| function.name == Some("broad"))
            .map(|function| function.id)
            .unwrap();
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            identical_function_folding: false,
            function_subsumption: true,
            ..OptimizationOptions::default()
        };

        let reports =
            optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert!(reports.iter().any(|report| {
            report.pass_name == "private-function-subsumption" && report.changed
        }));
        assert!(!control_flow.functions[narrow.0 as usize].live);
        let broad_calls = control_flow
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.op {
                ControlFlowOp::CallDirect { function, args, .. } if *function == broad => {
                    Some(args.len())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(broad_calls, vec![2, 2, 2]);
    }

    #[test]
    fn subsumes_private_function_with_a_middle_constant_parameter() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int narrow(int value,int scale){return (value+1)*scale+31;}int broad(int value,int bias,int scale){return (value+bias)*scale+31;}print(narrow(read(),17)+narrow(read(),19)+broad(read(),2,17)+broad(read(),3,19));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let narrow = control_flow
            .functions
            .iter()
            .find(|function| function.name == Some("narrow"))
            .map(|function| function.id)
            .unwrap();
        let broad = control_flow
            .functions
            .iter()
            .find(|function| function.name == Some("broad"))
            .map(|function| function.id)
            .unwrap();
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            identical_function_folding: false,
            function_subsumption: true,
            ..OptimizationOptions::default()
        };

        let reports =
            optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert!(reports.iter().any(|report| {
            report.pass_name == "private-function-subsumption" && report.changed
        }));
        assert!(!control_flow.functions[narrow.0 as usize].live);
        assert!(control_flow
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.op {
                ControlFlowOp::CallDirect { function, args, .. } if *function == broad => {
                    Some(args.len())
                }
                _ => None,
            })
            .all(|arguments| arguments == 3));
    }

    #[test]
    fn subsumes_private_function_proven_by_known_callback_binding() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int triple(int value){return value*3;}int double(int value){return value*2;}int narrow(int value){return triple(value)*17+31;}int broad(int value,func(int)->int transform){return transform(value)*17+31;}print(narrow(read())+broad(read(),triple)+broad(read(),double));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let narrow = control_flow
            .functions
            .iter()
            .find(|function| function.name == Some("narrow"))
            .map(|function| function.id)
            .unwrap();
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            identical_function_folding: false,
            function_subsumption: true,
            ..OptimizationOptions::default()
        };

        let reports =
            optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert!(reports.iter().any(|report| {
            report.pass_name == "private-function-subsumption" && report.changed
        }));
        assert!(!control_flow.functions[narrow.0 as usize].live);
    }

    #[test]
    fn preserves_exported_identity_during_function_subsumption() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int narrow(int value){return (value+1)*17;}int broad(int value,int bias){return (value+bias)*17;}print(narrow(read())+broad(read(),2)+broad(read(),3));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let narrow = control_flow
            .functions
            .iter()
            .find(|function| function.name == Some("narrow"))
            .map(|function| function.id)
            .unwrap();
        control_flow.exports.push(crate::ir::IrExport {
            name: "narrow",
            binding: ExportBinding::Function(narrow),
            span: S,
        });
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            identical_function_folding: false,
            function_subsumption: true,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, true).unwrap();

        assert!(control_flow.functions[narrow.0 as usize].live);
    }

    #[test]
    fn preserves_address_taken_identity_during_function_subsumption() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();extern void retain(func(int)->int callback);int narrow(int value){return (value+1)*17;}int broad(int value,int bias){return (value+bias)*17;}retain(narrow);print(narrow(read())+broad(read(),2)+broad(read(),3));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let narrow = control_flow
            .functions
            .iter()
            .find(|function| function.name == Some("narrow"))
            .map(|function| function.id)
            .unwrap();
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            identical_function_folding: false,
            function_subsumption: true,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert!(control_flow.functions[narrow.0 as usize].live);
    }

    #[test]
    fn rejects_function_subsumption_without_exact_specialized_cfg() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int narrow(int value){return (value+1)*17+4;}int broad(int value,int bias){return (value+bias)*17+3;}print(narrow(read())+broad(read(),2)+broad(read(),3));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let narrow = control_flow
            .functions
            .iter()
            .find(|function| function.name == Some("narrow"))
            .map(|function| function.id)
            .unwrap();
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            identical_function_folding: false,
            function_subsumption: true,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert!(control_flow.functions[narrow.0 as usize].live);
    }

    #[test]
    fn preserves_exported_function_identity_during_identical_folding() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int left(int value){return value*3+1;}int right(int item){return item*3+1;}print(left(read())+right(read()));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let right = control_flow
            .functions
            .iter()
            .find(|function| function.name == Some("right"))
            .map(|function| function.id)
            .unwrap();
        control_flow.exports.push(crate::ir::IrExport {
            name: "right",
            binding: ExportBinding::Function(right),
            span: S,
        });
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, true).unwrap();

        assert!(control_flow.functions[right.0 as usize].live);
        assert_eq!(
            control_flow
                .functions
                .iter()
                .filter(|function| function.live && function.kind == FunctionKind::Function)
                .count(),
            2
        );
    }

    #[test]
    fn preserves_address_taken_function_identity_during_identical_folding() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();extern void retain(func(int)->int callback);int left(int value){return value*3+1;}int right(int item){return item*3+1;}retain(right);print(left(read()));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert_eq!(
            control_flow
                .functions
                .iter()
                .filter(|function| function.live && function.kind == FunctionKind::Function)
                .count(),
            2
        );
    }

    #[test]
    fn preserves_distinct_escape_contracts_during_identical_folding() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Box{int value;}extern void retain(Box box);int left(Box box){return box.value;}int right(Box item){return item.value;}Box local=Box{1};Box escaped=Box{2};retain(escaped);print(left(local)+right(escaped));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert_eq!(
            control_flow
                .functions
                .iter()
                .filter(|function| function.live && function.kind == FunctionKind::Function)
                .count(),
            2
        );
    }

    #[test]
    fn normalizes_phi_locals_removed_from_function_metadata() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            include_str!("../tests/cases/nested_short_circuit.lil"),
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();

        let reports = optimize_control_flow(&mut control_flow).unwrap();

        assert!(reports
            .iter()
            .any(|report| report.pass_name == "identical-private-function-folding"));
    }

    #[test]
    fn does_not_specialize_tagged_generic_parameters_as_raw_constants() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "T apply<T>(T value,func(T)->T transform){return transform(value);}int triple(int value){return value*3;}print(apply(3,triple));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let generic = control_flow
            .functions
            .iter()
            .find(|function| {
                function
                    .params
                    .iter()
                    .any(|parameter| matches!(parameter.ty, Type::TypeParameter(_)))
            })
            .map(|function| function.id)
            .unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert!(control_flow.functions[generic.0 as usize]
            .params
            .iter()
            .any(|parameter| matches!(parameter.ty, Type::TypeParameter(_))));
    }

    #[test]
    fn specializes_tagged_generic_constants_for_javascript_only() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "T apply<T>(T value,func(T)->T transform){return transform(value);}int triple(int value){return value*3;}print(apply(3,triple));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let generic = control_flow
            .functions
            .iter()
            .find(|function| {
                function
                    .params
                    .iter()
                    .any(|parameter| matches!(parameter.ty, Type::TypeParameter(_)))
            })
            .map(|function| function.id)
            .unwrap();
        let options = OptimizationOptions {
            inlining: false,
            specialize_tagged_constants: true,
            ..OptimizationOptions::default()
        };

        let reports =
            optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert!(reports.iter().any(|report| {
            report.pass_name == "constant-parameter-specialization" && report.changed
        }));
        assert!(control_flow.functions[generic.0 as usize]
            .params
            .iter()
            .all(|parameter| !matches!(parameter.ty, Type::TypeParameter(_))));
    }

    #[test]
    fn folds_interprocedural_finite_values_without_dropping_effectful_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "string mode(){print(\"effect\");return \"active\";}string render(string value){if(value==\"active\"){return \"A\";}return \"B\";}print(render(mode()));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&control_flow).unwrap();

        assert!(output.contains("effect"), "{output}");
        assert!(output.contains("\"A\""), "{output}");
        assert!(!output.contains("===\"active\""), "{output}");
    }

    #[test]
    fn folds_exact_nominal_field_values_across_typed_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Theme{string name;}string render(Theme theme){if(theme.name==\"dark\"){return \"D\";}return \"L\";}Theme theme=Theme{\"dark\"};print(render(theme));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            scalar_replacement: false,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&control_flow).unwrap();

        assert!(output.contains("\"D\""), "{output}");
        assert!(!output.contains("===\"dark\""), "{output}");
    }

    #[test]
    fn folds_optional_access_guard_for_a_proven_non_null_receiver() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int calls=0;int next(){calls++;return 0;}int fallback(){calls+=10;return 9;}int[]? values=[3,5];print(values?.[next()]??fallback());print(calls);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();

        optimize_control_flow(&mut control_flow).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&control_flow).unwrap();

        assert!(!output.contains("!=null"), "{output}");
        assert!(!output.contains("==null"), "{output}");
        assert!(!output.contains("if("), "{output}");
    }

    #[test]
    fn folds_constants() {
        let mut module = module(vec![
            Instruction::Const {
                out: ValueId(0),
                value: ConstValue::Int(1),
                span: S,
            },
            Instruction::Const {
                out: ValueId(1),
                value: ConstValue::Int(6),
                span: S,
            },
            Instruction::Binary {
                out: ValueId(2),
                op: IrBinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(1),
                span: S,
            },
            Instruction::Return {
                value: Some(ValueId(2)),
                span: S,
            },
        ]);

        let report = ConstantFolding.run(&mut module);
        assert!(report.changed);
        assert!(matches!(
            &module.functions[0].blocks[0].instructions[2],
            Instruction::Const {
                value: ConstValue::Int(7),
                ..
            }
        ));
    }

    #[test]
    fn folds_mixed_numeric_constants_by_value() {
        assert_eq!(
            fold_binary(IrBinaryOp::Eq, &ConstValue::Int(7), &ConstValue::Float(7.0),),
            Some(ConstValue::Bool(true))
        );
        assert_eq!(
            fold_binary(
                IrBinaryOp::Add,
                &ConstValue::Float(0.5),
                &ConstValue::Int(2),
            ),
            Some(ConstValue::Float(2.5))
        );
    }

    #[test]
    fn folds_integer_multiplication_with_javascript_operator_semantics() {
        assert_eq!(js_i32_multiply(2_147_483_647, 2_147_483_647), 0);
        assert_eq!(
            js_i32_multiply(2_147_483_647, 2_147_483_646),
            -2_147_483_648
        );
        assert_eq!(js_i32_multiply(123_456_789, 987_654_321), -67_153_024);
    }

    #[test]
    fn removes_dead_value_chains() {
        let mut module = module(vec![
            Instruction::Const {
                out: ValueId(0),
                value: ConstValue::Int(1),
                span: S,
            },
            Instruction::Const {
                out: ValueId(1),
                value: ConstValue::Int(2),
                span: S,
            },
            Instruction::Binary {
                out: ValueId(2),
                op: IrBinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(1),
                span: S,
            },
            Instruction::Return {
                value: None,
                span: S,
            },
        ]);

        let report = DeadCodeElimination.run(&mut module);
        assert!(report.changed);
        assert_eq!(module.functions[0].blocks[0].instructions.len(), 1);
    }

    #[test]
    fn scalar_replaces_non_escaping_structs() {
        let mut module = module(vec![
            Instruction::Const {
                out: ValueId(0),
                value: ConstValue::Int(10),
                span: S,
            },
            Instruction::Const {
                out: ValueId(1),
                value: ConstValue::Int(20),
                span: S,
            },
            Instruction::Struct {
                out: ValueId(2),
                fields: vec![ValueId(0), ValueId(1)],
                span: S,
            },
            Instruction::FieldGet {
                out: ValueId(3),
                aggregate: ValueId(2),
                index: 1,
                span: S,
            },
            Instruction::Return {
                value: Some(ValueId(3)),
                span: S,
            },
        ]);

        let report = ScalarReplacement.run(&mut module);
        assert!(report.changed);
        let instructions = &module.functions[0].blocks[0].instructions;
        assert_eq!(instructions.len(), 3);
        assert!(matches!(
            instructions.last(),
            Some(Instruction::Return {
                value: Some(ValueId(1)),
                ..
            })
        ));
    }

    #[test]
    fn preserves_escaping_structs() {
        let mut module = module(vec![
            Instruction::Struct {
                out: ValueId(0),
                fields: Vec::new(),
                span: S,
            },
            Instruction::Return {
                value: Some(ValueId(0)),
                span: S,
            },
        ]);

        let report = ScalarReplacement.run(&mut module);
        assert!(!report.changed);
        assert_eq!(module.functions[0].blocks[0].instructions.len(), 2);
    }

    #[test]
    fn promotes_cfg_locals_and_inserts_loop_phis() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int total=0;int run(int limit){int sum=0;for(int i=0;i<limit;i++){sum+=i;}return sum;}total=run(4);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let report = promote_locals_to_ssa(&mut module).unwrap();
        assert!(report.changed);
        assert!(module
            .functions
            .iter()
            .all(|function| function.locals_promoted));
        assert!(module.functions.iter().all(|function| function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(
                instruction.op,
                ControlFlowOp::LoadLocal(_) | ControlFlowOp::StoreLocal { .. }
            ))));
        assert!(module.functions[1]
            .blocks
            .iter()
            .any(|block| !block.phis.is_empty()));
    }

    #[test]
    fn mem2reg_is_reentrant_for_newly_internalized_entry_globals() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int calls=0;int helper(){calls+=10;return 8;}int bump(){return helper();}print(bump());print(calls);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();

        // Model the normal early pipeline, where the helper still keeps the
        // binding shared and the first promotion round therefore cannot see
        // it as an entry-local value.
        internalize_entry_globals(&mut module);
        promote_locals_to_ssa(&mut module).unwrap();
        let old_phi_incoming = module.functions[module.entry.0 as usize]
            .blocks
            .iter()
            .flat_map(|block| &block.phis)
            .map(|phi| (phi.out, phi.incoming.clone()))
            .collect::<Vec<_>>();

        // Once the helper is dead, the same binding becomes entry-only.  A
        // second promotion must remove its memory operations without
        // duplicating or mutating phis created by the first round.
        let helper = module
            .functions
            .iter_mut()
            .find(|function| function.name == Some("helper"))
            .unwrap();
        helper.live = false;
        assert!(internalize_entry_globals(&mut module).changed);
        promote_locals_to_ssa(&mut module).unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        assert!(entry.locals_promoted);
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(
                instruction.op,
                ControlFlowOp::LoadLocal(_) | ControlFlowOp::StoreLocal { .. }
            )));
        let new_phi_incoming = entry
            .blocks
            .iter()
            .flat_map(|block| &block.phis)
            .filter(|phi| old_phi_incoming.iter().any(|(out, _)| *out == phi.out))
            .map(|phi| (phi.out, phi.incoming.clone()))
            .collect::<Vec<_>>();
        assert_eq!(new_phi_incoming, old_phi_incoming);
    }

    #[test]
    fn promotes_only_locals_independent_of_exception_edges() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void mayThrow();int run(bool fail){int protected=0;int ordinary=7;try{protected=1;mayThrow();protected=2;}catch{}if(fail){ordinary=9;}return protected+ordinary;}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let run = module
            .functions
            .iter()
            .find(|function| function.name == Some("run"))
            .map(|function| function.id)
            .unwrap();

        promote_locals_to_ssa(&mut module).unwrap();

        let function = &module.functions[run.0 as usize];
        let protected = function
            .locals
            .iter()
            .find(|local| local.name == "protected")
            .unwrap()
            .id;
        let ordinary = function
            .locals
            .iter()
            .find(|local| local.name == "ordinary")
            .unwrap()
            .id;
        let memory_locals = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction.op {
                ControlFlowOp::LoadLocal(local) | ControlFlowOp::StoreLocal { local, .. } => {
                    Some(local)
                }
                _ => None,
            })
            .collect::<AHashSet<_>>();
        assert!(memory_locals.contains(&protected));
        assert!(!memory_locals.contains(&ordinary));
    }

    #[test]
    fn keeps_writes_in_nested_exception_regions_mutable() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void mayThrow();int run(){int value=0;try{try{value=1;mayThrow();}catch{}mayThrow();}catch{}return value;}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let run = module
            .functions
            .iter()
            .find(|function| function.name == Some("run"))
            .map(|function| function.id)
            .unwrap();

        promote_locals_to_ssa(&mut module).unwrap();

        let function = &module.functions[run.0 as usize];
        let value = function
            .locals
            .iter()
            .find(|local| local.name == "value")
            .unwrap()
            .id;
        assert!(function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::StoreLocal { local, .. } if local == value)));
    }

    #[test]
    fn keeps_locals_read_by_finally_mutable_across_early_return() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int callback();int run(bool alternate){int saved=7;int ordinary=1;if(alternate){ordinary=2;}try{return callback();}finally{print(saved);}}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let run = module
            .functions
            .iter()
            .find(|function| function.name == Some("run"))
            .map(|function| function.id)
            .unwrap();

        promote_locals_to_ssa(&mut module).unwrap();

        let function = &module.functions[run.0 as usize];
        let saved = function
            .locals
            .iter()
            .find(|local| local.name == "saved")
            .unwrap()
            .id;
        let ordinary = function
            .locals
            .iter()
            .find(|local| local.name == "ordinary")
            .unwrap()
            .id;
        let memory_locals = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction.op {
                ControlFlowOp::LoadLocal(local) | ControlFlowOp::StoreLocal { local, .. } => {
                    Some(local)
                }
                _ => None,
            })
            .collect::<AHashSet<_>>();
        assert!(memory_locals.contains(&saved));
        assert!(!memory_locals.contains(&ordinary));
    }

    #[test]
    fn runs_the_whole_cfg_optimization_pipeline() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Point{int x;int y;}int sum(){Point p=Point{1,2};return p.x+p.y;}int dead(){return 99;}if(true){print(sum());}else{print(0);}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let reports = optimize_control_flow(&mut module).unwrap();

        assert!(reports
            .iter()
            .any(|report| report.pass_name == "inlining" && report.changed));
        assert!(reports.iter().any(|report| {
            report.pass_name == "unreachable-block-elimination" && report.changed
        }));
        assert!(reports
            .iter()
            .any(|report| { report.pass_name == "scalar-replacement-cfg" && report.changed }));
        assert!(!module.functions[1].live);
        assert!(!module.functions[2].live);
    }

    #[test]
    fn scalar_replacement_explodes_structs_used_by_loop_phis_atomically() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read(int index);extern int count();struct Box{int value;}int sum(){Box box=Box{read(0)};for(int index=1;index<count();index++){box=Box{box.value+read(index)};}return box.value;}print(sum());",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();

        optimize_control_flow(&mut module).unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        let aggregate_phi_inputs = entry
            .blocks
            .iter()
            .flat_map(|block| &block.phis)
            .filter(|phi| matches!(phi.ty, Type::Struct(_)))
            .flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value))
            .collect::<Vec<_>>();
        assert!(aggregate_phi_inputs.is_empty());
        assert!(entry.blocks.iter().all(|block| block
            .instructions
            .iter()
            .all(|instruction| !matches!(instruction.op, ControlFlowOp::Struct { .. }))));
        assert_control_flow_values_are_defined(entry);
        crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();
    }

    #[test]
    fn scalar_replacement_explodes_the_aggregate_ledger_loop() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
                extern int algorithmInt(int index);
                extern int algorithmCount();
                struct Ledger { int total; int weighted; int minimum; int maximum; }
                pure int smaller(int left,int right){if(left<right){return left;}return right;}
                pure int larger(int left,int right){if(left>right){return left;}return right;}
                pure Ledger beginLedger(int value){return Ledger{value,value,value,value};}
                pure Ledger appendLedger(Ledger ledger,int value,int index){
                    return Ledger{
                        ledger.total+value,
                        ledger.weighted+value*(index+1),
                        smaller(ledger.minimum,value),
                        larger(ledger.maximum,value)
                    };
                }
                pure int finishLedger(Ledger ledger){
                    return ledger.total+ledger.weighted+ledger.maximum-ledger.minimum;
                }
                int analyzeLedger(){
                    int count=algorithmCount();
                    Ledger ledger=beginLedger(algorithmInt(0));
                    for(int index=1;index<count;index++){
                        ledger=appendLedger(ledger,algorithmInt(index),index);
                    }
                    return finishLedger(ledger);
                }
                print(analyzeLedger());
            "#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();

        optimize_control_flow(&mut module).unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        assert!(entry.blocks.iter().all(|block| block
            .phis
            .iter()
            .all(|phi| !matches!(phi.ty, Type::Struct(_) | Type::StructInstance { .. }))));
        assert!(entry.blocks.iter().all(|block| block
            .instructions
            .iter()
            .all(|instruction| !matches!(instruction.op, ControlFlowOp::Struct { .. }))));
        assert_control_flow_values_are_defined(entry);
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();
        assert!(output.contains("algorithmCount"), "{output}");
        assert!(output.contains("algorithmInt"), "{output}");
    }

    #[test]
    fn loop_struct_scalar_replacement_rejects_field_mutation() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read(int index);extern int count();struct Box{int value;}Box box=Box{read(0)};for(int index=1;index<count();index++){box.value+=read(index);}print(box.value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();

        optimize_control_flow(&mut module).unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::FieldSet { .. })));
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::Struct { .. })));
        assert_control_flow_values_are_defined(entry);
    }

    #[test]
    fn loop_struct_scalar_replacement_rejects_typed_escape() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read(int index);extern int count();extern void inspect(Box value);struct Box{int value;}Box box=Box{read(0)};for(int index=1;index<count();index++){inspect(box);box=Box{box.value+read(index)};}print(box.value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();

        optimize_control_flow(&mut module).unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::Struct { .. })));
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.phis)
            .any(|phi| matches!(phi.ty, Type::Struct(_) | Type::StructInstance { .. })));
        assert_control_flow_values_are_defined(entry);
    }

    #[test]
    fn loop_struct_scalar_replacement_rejects_branch_merges() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern bool choose();extern int read(int index);struct Box{int value;}Box box=Box{read(0)};if(choose()){box=Box{read(1)};}else{box=Box{read(2)};}print(box.value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();

        optimize_control_flow(&mut module).unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.phis)
            .any(|phi| matches!(phi.ty, Type::Struct(_) | Type::StructInstance { .. })));
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::Struct { .. })));
        assert_control_flow_values_are_defined(entry);
    }

    #[test]
    fn loop_struct_scalar_replacement_rejects_shared_phi_inputs() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern bool choose();extern int read(int index);extern int count();struct Box{int value;}Box box=Box{read(0)};Box saved=box;for(int index=1;index<count();index++){box=Box{box.value+read(index)};if(choose()){saved=box;}}print(box.value+saved.value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();

        optimize_control_flow(&mut module).unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.phis)
            .any(|phi| matches!(phi.ty, Type::Struct(_) | Type::StructInstance { .. })));
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::Struct { .. })));
        assert_control_flow_values_are_defined(entry);
    }

    #[test]
    fn devirtualizes_class_method_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Value{int x;init(int x){this.x=x;}int get(){return this.x;}}Value v=new Value(7);print(v.get());",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        assert!(module
            .functions
            .iter()
            .filter(|function| function.live)
            .flat_map(|function| &function.blocks)
            .all(|block| block
                .instructions
                .iter()
                .all(|instruction| !matches!(instruction.op, ControlFlowOp::CallMethod { .. }))));
    }

    #[test]
    fn escape_worklist_visits_a_directed_chain_once() {
        let function = FunctionId(0);
        let nodes = (0..129)
            .map(|index| EscapeNode::Value(function, ValueId(index)))
            .collect::<Vec<_>>();
        let mut edges = AHashMap::default();
        for pair in nodes.windows(2) {
            add_escape_flow(&mut edges, pair[0], pair[1]);
        }
        let mut states = AHashMap::default();
        mark_escape_node(&mut states, nodes[0], EscapeState::EscapesToUntypedBoundary);

        propagate_escape_states(&mut states, &edges);

        assert_eq!(
            states.get(nodes.last().unwrap()),
            Some(&EscapeState::EscapesToUntypedBoundary)
        );
        assert_eq!(escape_propagation_edge_visits(), nodes.len() - 1);
    }

    #[test]
    fn escape_worklist_matches_full_rescan_with_mixed_ranks_and_cycles() {
        fn propagate_by_full_rescan(
            states: &mut AHashMap<EscapeNode, EscapeState>,
            edges: &AHashMap<EscapeNode, AHashSet<EscapeNode>>,
        ) {
            loop {
                let mut updates = Vec::new();
                for (node, neighbors) in edges {
                    let state = states.get(node).copied().unwrap_or(EscapeState::LocalOnly);
                    for neighbor in neighbors {
                        let current = states
                            .get(neighbor)
                            .copied()
                            .unwrap_or(EscapeState::LocalOnly);
                        if escape_rank(state) > escape_rank(current) {
                            updates.push((*neighbor, state));
                        }
                    }
                }
                if updates.is_empty() {
                    return;
                }
                for (node, state) in updates {
                    mark_escape_node(states, node, state);
                }
            }
        }

        let function = FunctionId(0);
        let nodes = (0..8)
            .map(|index| EscapeNode::Value(function, ValueId(index)))
            .collect::<Vec<_>>();
        let mut edges = AHashMap::default();
        add_escape_flow(&mut edges, nodes[0], nodes[2]);
        add_escape_flow(&mut edges, nodes[1], nodes[3]);
        add_escape_edge(&mut edges, nodes[2], nodes[3]);
        add_escape_flow(&mut edges, nodes[3], nodes[4]);
        add_escape_flow(&mut edges, nodes[4], nodes[2]);
        add_escape_flow(&mut edges, nodes[5], nodes[6]);
        let mut expected = AHashMap::default();
        mark_escape_node(&mut expected, nodes[0], EscapeState::EscapesToTypedCode);
        mark_escape_node(
            &mut expected,
            nodes[1],
            EscapeState::EscapesToUntypedBoundary,
        );
        let mut actual = expected.clone();

        propagate_by_full_rescan(&mut expected, &edges);
        propagate_escape_states(&mut actual, &edges);

        assert_eq!(actual, expected);
        assert!(!actual.contains_key(&nodes[5]));
        assert!(!actual.contains_key(&nodes[6]));
        assert!(!actual.contains_key(&nodes[7]));
    }

    #[test]
    fn marks_extern_arguments_as_untyped_escapes() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Point{int x;int y;}extern void consume(Point p);Point p=Point{1,2};consume(p);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let entry = &module.functions[module.entry.0 as usize];
        let aggregate = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| match instruction.op {
                ControlFlowOp::Struct { .. } => instruction.out,
                _ => None,
            })
            .unwrap();
        assert_eq!(
            entry.value_escapes[aggregate.0 as usize],
            EscapeState::EscapesToUntypedBoundary
        );
    }

    #[test]
    fn extern_aggregate_global_uses_named_field_access() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Entry{int value;}extern Entry host;print(host.value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains("host.value"), "{output}");
        assert!(!output.contains("host[0]"), "{output}");
    }

    #[test]
    fn thrown_aggregate_keeps_a_named_shape() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct ThrownEntry{int thrownValue;}extern void consume(JsValue value);try{throw ThrownEntry{7};}catch(auto error){consume(error);}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains("{thrownValue:7}"), "{output}");
        assert!(!output.contains("[7]"), "{output}");
    }

    #[test]
    fn assumed_host_exception_aggregate_uses_named_fields() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Entry{int value;}extern void hostThrow();try{hostThrow();}catch(auto error){Entry entry=JS.assume(error);print(entry.value);}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains(".value"), "{output}");
        assert!(!output.contains("[0]"), "{output}");
    }

    #[test]
    fn javascript_coercions_keep_js_value_aggregates_named() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct TemplateEntry{int templateValue;}struct AddEntry{int addValue;}struct EqEntry{int eqValue;}JsValue templateEntry=TemplateEntry{7};JsValue addEntry=AddEntry{7};JsValue eqEntry=EqEntry{7};string templateText=`${templateEntry}`;string addedText=\"\"+addEntry;bool equal=eqEntry==\"7\";print(templateText);print(addedText);print(equal);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        for expected in ["{templateValue:7}", "{addValue:7}", "{eqValue:7}"] {
            assert!(output.contains(expected), "{output}");
        }
        assert!(!output.contains("[7]"), "{output}");
        assert_eq!(
            run_javascript(&output),
            "[object Object]\n[object Object]\nfalse\n",
            "{output}"
        );
    }

    #[test]
    fn js_value_length_keeps_object_semantics() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Entry{int value;}JsValue entry=Entry{7};float length=entry.length;print(length);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains("{value:7}"), "{output}");
        assert!(!output.contains("[7]"), "{output}");
        assert_eq!(run_javascript(&output), "undefined\n", "{output}");
    }

    #[test]
    fn js_value_bracket_read_keeps_nominal_object_semantics() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Entry{int value;}JsValue entry=Entry{7};int got=JS.assume(entry[\"value\"]);print(got);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains("{value:7}"), "{output}");
        assert!(!output.contains("[7]"), "{output}");
        assert_eq!(run_javascript(&output), "7\n", "{output}");
    }

    #[test]
    fn wrapped_and_generic_dynamic_aliases_keep_nominal_object_semantics() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Left{int value;}struct Right{int value;}struct GenericEntry{int value;}struct AddEntry{int value;}extern bool coin();Left|Right choose(bool first){if(first){return Left{7};}return Right{8};}int read<T>(T input){JsValue erased=input;return JS.assume(erased[\"value\"]);}string stringify<T>(T input){JsValue erased=input;return \"\"+erased;}JsValue unionValue=choose(coin());int unionGot=JS.assume(unionValue[\"value\"]);print(unionGot);print(read(GenericEntry{9}));print(stringify(AddEntry{10}));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                constant_parameter_specialization: false,
                call_site_specialization: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        for expected in ["{value:7}", "{value:8}", "{value:9}", "{value:10}"] {
            assert!(output.contains(expected), "{output}");
        }
        assert_eq!(
            run_javascript(&format!("function coin(){{return true}}{output}")),
            "7\n9\n[object Object]\n",
            "{output}"
        );
    }

    #[test]
    fn rejected_task_reason_keeps_a_named_shape() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct RejectedEntry{int rejectedValue;}extern void consumeTask(Task<int> task);Task<int> task=Task.reject(RejectedEntry{7});consumeTask(task);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains("{rejectedValue:7}"), "{output}");
        assert!(!output.contains("[7]"), "{output}");
    }

    #[test]
    fn typed_inherited_field_extraction_keeps_the_stored_aggregate_named() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Entry{int value;}class Base{Entry item;init(Entry item){this.item=item;}}class Box extends Base{init(Entry item){super(item);}}extern void consume(JsValue value);Box box=new Box(Entry{7});consume(box.item);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains("{value:7}"), "{output}");
        assert!(!output.contains("[7]"), "{output}");
    }

    #[test]
    fn hidden_captured_class_with_js_value_field_stays_typed_and_positional() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern JsValue source();extern void consume(func()->JsValue callback);class Box{JsValue value;init(JsValue value){this.value=value;}}Box box=new Box(source());consume(()=>box.value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        let box_value = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| {
                instruction
                    .ty
                    .as_ref()
                    .is_some_and(|ty| matches!(ty, Type::Class("Box")))
                    .then_some(instruction.out)
                    .flatten()
            })
            .expect("captured Box allocation");
        assert_eq!(
            entry.value_escapes[box_value.0 as usize],
            EscapeState::EscapesToTypedCode,
            "a captured owner is not stack-local, but its dynamic field does not expose its layout"
        );

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains("[0]"), "{output}");
        assert!(!output.contains(".value"), "{output}");
    }

    #[test]
    fn nominal_values_stored_in_dynamic_fields_keep_named_shapes() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct DirectEntry{int directValue;}struct OptionalEntry{int optionalValue;}struct UnionEntry{int unionValue;}struct LiteralEntry{int literalValue;}struct GenericEntry{int genericValue;}struct Envelope{JsValue payload;}class Box{JsValue direct;JsValue? optional;int|JsValue mixed;init(){this.direct=DirectEntry{1};this.optional=OptionalEntry{2};this.mixed=UnionEntry{3};}}class GenericBox<T>{T value;init(T value){this.value=value;}}extern void consume(JsValue value);Box box=new Box();Envelope envelope=Envelope{LiteralEntry{4}};GenericBox<JsValue> generic=new GenericBox<JsValue>(GenericEntry{5});consume(box.direct);consume(box.optional);consume(box.mixed);consume(envelope.payload);consume(generic.value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let stored = module
            .functions
            .iter()
            .flat_map(|function| {
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .filter_map(
                        move |instruction| match (&instruction.op, instruction.out) {
                            (ControlFlowOp::Struct { name, .. }, Some(out))
                                if name.ends_with("Entry") =>
                            {
                                Some((function, *name, out))
                            }
                            _ => None,
                        },
                    )
            })
            .collect::<Vec<_>>();
        assert_eq!(stored.len(), 5, "{module:#?}");
        for (function, name, value) in stored {
            assert_eq!(
                function.value_escapes[value.0 as usize],
                EscapeState::EscapesToUntypedBoundary,
                "{name} was erased by a dynamic aggregate field"
            );
        }

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        for expected in [
            "{directValue:1}",
            "{optionalValue:2}",
            "{unionValue:3}",
            "{literalValue:4}",
            "{genericValue:5}",
        ] {
            assert!(output.contains(expected), "{output}");
        }
    }

    #[test]
    fn externally_exposed_known_callable_keeps_return_shape_named() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Entry{int value;}Entry make(){return Entry{7};}extern void consumeFactory(func()->JsValue factory);consumeFactory(make);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let make = module
            .functions
            .iter()
            .find(|function| function.name == Some("make"))
            .expect("known callback target");
        let returned = make
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| match (&instruction.op, instruction.out) {
                (ControlFlowOp::Struct { name: "Entry", .. }, Some(out)) => Some(out),
                _ => None,
            })
            .expect("Entry returned by callback");
        assert_eq!(
            make.value_escapes[returned.0 as usize],
            EscapeState::EscapesToUntypedBoundary
        );

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains("{value:7}"), "{output}");
        assert!(!output.contains("[7]"), "{output}");
    }

    #[test]
    fn escaping_generic_closure_keeps_its_captured_return_named() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Entry{int value;}func()->T make<T>(T value){return ()=>value;}extern void consumeFactory(func()->JsValue factory);consumeFactory(make(Entry{7}));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains("{value:7}"), "{output}");
        assert!(!output.contains("[7]"), "{output}");
    }

    #[test]
    fn aggregate_pushed_into_js_value_array_is_untyped_and_named() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Entry{int value;}extern void consume(JsValue[] values);JsValue[] values=[];values.push(Entry{7});consume(values);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        let aggregate = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| {
                matches!(instruction.op, ControlFlowOp::Struct { name: "Entry", .. })
                    .then_some(instruction.out)
                    .flatten()
            })
            .expect("Entry allocation pushed into JsValue[]");
        assert_eq!(
            entry.value_escapes[aggregate.0 as usize],
            EscapeState::EscapesToUntypedBoundary
        );

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains("{value:7}"), "{output}");
        assert!(!output.contains("[7]"), "{output}");
    }

    #[test]
    fn generic_container_result_propagates_dynamic_element_shape() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct DirectEntry{int directValue;}struct SpreadEntry{int spreadValue;}struct PopEntry{int popValue;}struct RecordEntry{int recordValue;}struct LoopEntry{int loopValue;}T[] wrap<T>(T value){return [value];}T[] spread<T>(T value){T[] source=[value];return [...source];}T pop<T>(T value){T[] source=[value];return source.pop();}Record<T> spreadRecord<T>(T value){Record<T> source=record{item:value};return record{...source};}T first<T>(T value,T fallback){T result=fallback;T[] source=[value];for(T item of source){result=item;}return result;}extern void consume(JsValue value);extern void consumeRecord(Record<JsValue> value);JsValue[] direct=wrap(DirectEntry{7});JsValue[] copied=spread(SpreadEntry{8});consume(direct[0]);consume(copied[0]);consume(pop(PopEntry{9}));consumeRecord(spreadRecord(RecordEntry{10}));consume(first(LoopEntry{11},null));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        let aggregates = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (&instruction.op, instruction.out) {
                (ControlFlowOp::Struct { name, .. }, Some(out)) if name.ends_with("Entry") => {
                    Some((*name, out))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(aggregates.len(), 5, "{module:#?}");
        for (name, aggregate) in aggregates {
            assert_eq!(
                entry.value_escapes[aggregate.0 as usize],
                EscapeState::EscapesToUntypedBoundary,
                "{name} lost generic container content provenance"
            );
        }

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        for expected in [
            "{directValue:7}",
            "{spreadValue:8}",
            "{popValue:9}",
            "{recordValue:10}",
            "{loopValue:11}",
        ] {
            assert!(output.contains(expected), "{output}");
        }
    }

    #[test]
    fn generic_array_callbacks_preserve_dynamic_element_shapes() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct VisitEntry{int visitValue;}struct MapEntry{int mapValue;}struct ReduceEntry{int reduceValue;}extern void consume(JsValue value);void visit<T>(T[] values){values.forEach((T value)=>consume(value));}T first<T>(T[] values){return values.map((T value)=>value)[0];}void reduceVisit<T>(T[] values,T initial){values.reduce((T accumulator,T value)=>{consume(accumulator);return value;},initial);}visit([VisitEntry{7}]);consume(first([MapEntry{8}]));reduceVisit([ReduceEntry{9}],ReduceEntry{10});",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        let aggregates = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (&instruction.op, instruction.out) {
                (ControlFlowOp::Struct { name, .. }, Some(out)) if name.ends_with("Entry") => {
                    Some((*name, out))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(aggregates.len(), 4, "{module:#?}");
        for (name, aggregate) in aggregates {
            assert_eq!(
                entry.value_escapes[aggregate.0 as usize],
                EscapeState::EscapesToUntypedBoundary,
                "{name} lost array callback content provenance"
            );
        }

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        for expected in [
            "{visitValue:7}",
            "{mapValue:8}",
            "{reduceValue:9}",
            "{reduceValue:10}",
        ] {
            assert!(output.contains(expected), "{output}");
        }
    }

    #[test]
    fn extern_array_observer_keeps_consumed_aggregates_named() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Entry{int value;}extern void observeExtern(Entry value);Entry[] values=[Entry{7}];values.forEach(observeExtern);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains("{value:7}"), "{output}");
        assert!(!output.contains("[7]"), "{output}");
    }

    #[test]
    fn extern_array_mapper_uses_named_fields_on_host_results() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Entry{int value;}extern Entry makeExternEntry(int value);Entry[] values=[1].map(makeExternEntry);print(values[0].value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains(".value"), "{output}");
        assert!(!output.contains("[0][0]"), "{output}");
    }

    #[test]
    fn wrapped_js_value_container_elements_are_detected_recursively() {
        assert!(type_contains_untyped_js_value(&Type::Nullable(Box::new(
            Type::TypeParameter("$js"),
        ))));
        assert!(type_contains_untyped_js_value(&Type::Union(vec![
            Type::Int,
            Type::TypeParameter("$js"),
        ])));
        assert!(!type_contains_untyped_js_value(&Type::Nullable(Box::new(
            Type::Int,
        ))));
    }

    #[test]
    fn contextual_nullable_js_value_arrays_keep_literal_and_spread_entries_named() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct LiteralEntry{int literalValue;}struct SpreadEntry{int spreadValue;}extern void consume(JsValue value);(JsValue?)[] direct=[LiteralEntry{7}];SpreadEntry[] source=[SpreadEntry{9}];(JsValue?)[] spread=[...source];consume(direct[0]);consume(spread[0]);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        let literal_entry = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| match (&instruction.op, instruction.out) {
                (
                    ControlFlowOp::Struct {
                        name: "LiteralEntry",
                        ..
                    },
                    Some(out),
                ) => Some(out),
                _ => None,
            })
            .expect("LiteralEntry widened directly into the nullable JsValue array");
        assert_eq!(
            entry.value_escapes[literal_entry.0 as usize],
            EscapeState::EscapesToUntypedBoundary
        );
        let spread_source = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| {
                instruction
                    .ty
                    .as_ref()
                    .is_some_and(|ty| {
                        matches!(ty, Type::Array(element) if matches!(element.as_ref(), Type::Struct("SpreadEntry")))
                    })
                    .then_some(instruction.out)
                    .flatten()
            })
            .expect("SpreadEntry[] operand widened by the nullable JsValue array spread");
        assert_eq!(
            entry.value_escapes[spread_source.0 as usize],
            EscapeState::EscapesToUntypedBoundary
        );

        let output = crate::codegen_ir_js::emit_optimized_ir_js_with_options(
            &module,
            &crate::codegen_ir_js::IrJsOptions {
                mangle_identifiers: false,
                mangle_properties: false,
                ..crate::codegen_ir_js::IrJsOptions::default()
            },
        )
        .unwrap();
        assert!(output.contains("{literalValue:7}"), "{output}");
        assert!(output.contains("{spreadValue:9}"), "{output}");
        assert!(!output.contains("[7]"), "{output}");
        assert!(!output.contains("[9]"), "{output}");
    }

    #[test]
    fn dynamic_collection_mutators_and_calls_expose_stored_aggregate_shapes() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct MapEntry{int mapValue;}struct SetEntry{int setValue;}struct RecordEntry{int recordValue;}struct CallEntry{int callValue;}extern JsValue callback();extern void consume(Map<string,JsValue> map,Set<JsValue> set,Record<JsValue> values);Map<string,JsValue> map=new Map<string,JsValue>();map.set(\"key\",MapEntry{1});Set<JsValue> set=new Set<JsValue>();set.add(SetEntry{2});Record<JsValue> values=record{};values.item=RecordEntry{3};consume(map,set,values);JS.call(callback(),null,CallEntry{4});",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                scalar_replacement: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        let entry = &module.functions[module.entry.0 as usize];
        let mut exposed = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (&instruction.op, instruction.out) {
                (ControlFlowOp::Struct { name, .. }, Some(out)) => Some((*name, out)),
                _ => None,
            })
            .collect::<Vec<_>>();
        exposed.sort_unstable_by_key(|(name, _)| *name);
        assert_eq!(exposed.len(), 4, "{module:#?}");
        for (name, value) in exposed {
            assert_eq!(
                entry.value_escapes[value.0 as usize],
                EscapeState::EscapesToUntypedBoundary,
                "{name} was widened into a dynamic collection/call"
            );
        }
    }

    #[test]
    fn pruned_ssa_handles_locals_scoped_inside_nested_loops() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int total=0;for(int outer=0;outer<3;outer++){int inner=0;while(inner<3){total+=outer*inner;inner++;}}print(total);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        assert!(module.functions.iter().all(|function| function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(
                instruction.op,
                ControlFlowOp::LoadLocal(_) | ControlFlowOp::StoreLocal { .. }
            ))));
    }

    #[test]
    fn eliminates_repeated_ssa_expressions_with_value_numbering() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int value=read();print(value*7+value*7);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let reports = optimize_control_flow(&mut module).unwrap();
        let multiplications = module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                matches!(
                    instruction.op,
                    ControlFlowOp::Binary {
                        op: IrBinaryOp::Mul,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(multiplications, 1);
        assert!(reports
            .iter()
            .any(|report| report.pass_name == "local-value-numbering" && report.changed));
    }

    #[test]
    fn canonicalizes_same_block_runtime_type_predicate_spellings() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"export bool inspect(JsValue text,JsValue flag){
                bool textGuard=text is string;
                bool textType=JS.typeOf(text)=="string";
                bool booleanGuard=flag is bool;
                bool booleanType=JS.typeOf(flag)=="boolean";
                return textGuard&&textType&&booleanGuard&&booleanType;
            }"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_for_module(&mut module).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();

        assert_eq!(output.matches("typeof").count(), 2, "{output}");
    }

    #[test]
    fn keeps_numeric_type_guards_separate_from_javascript_typeof() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"export bool inspect(JsValue value){
                bool numericValue=value is float;
                bool numeric=JS.typeOf(value)=="number";
                return numericValue&&numeric;
            }"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_for_module(&mut module).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();
        assert_eq!(output.matches("typeof").count(), 2, "{output}");
    }

    #[test]
    fn reuses_only_dominated_runtime_type_predicates_on_the_same_ssa_value() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"export bool inspect(JsValue value){
                bool isString=value is string;
                if(!isString){
                    if(JS.typeOf(value)=="string"){
                        isString=true;
                    }
                }
                return isString;
            }"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_for_module(&mut module).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();
        assert_eq!(output.matches("typeof").count(), 1, "{output}");

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"export bool inspectReads(JsValue host){
                bool isString=host["value"] is string;
                if(!isString){
                    if(JS.typeOf(host["value"])=="string"){
                        isString=true;
                    }
                }
                return isString;
            }"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_for_module(&mut module).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();
        assert_eq!(output.matches("typeof").count(), 2, "{output}");
    }

    #[test]
    fn propagates_single_assignment_globals_into_functions() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int factor=6;int scale(int value){return value*factor;}print(scale(7));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let reports = optimize_control_flow(&mut module).unwrap();
        assert!(module.globals.is_empty());
        assert!(module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(
                instruction.op,
                ControlFlowOp::LoadGlobal(_) | ControlFlowOp::StoreGlobal { .. }
            )));
        assert!(reports
            .iter()
            .any(|report| { report.pass_name == "global-constant-propagation" && report.changed }));
    }

    #[test]
    fn does_not_inline_recursive_functions() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int factorial(int value){if(value<=1){return 1;}return value*factorial(value-1);}print(factorial(5));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        assert!(module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::CallDirect { .. })));
    }

    #[test]
    fn late_inlining_keeps_protected_composites_and_absorbs_their_leaf_callees() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "pure int leaf(int value){return value+1;}pure int composite(int value){return leaf(value)*3;}extern int read(int index);print(composite(read(0)));print(composite(read(1)));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        promote_locals_to_ssa(&mut module).unwrap();
        let leaf = module
            .functions
            .iter()
            .find(|function| function.name == Some("leaf"))
            .unwrap()
            .id;
        let composite = module
            .functions
            .iter()
            .find(|function| function.name == Some("composite"))
            .unwrap()
            .id;
        let protected = AHashSet::from_iter([composite]);
        let mut reports = Vec::new();

        optimize_inlining_fixed_point(
            &mut module,
            &OptimizationOptions {
                inline_instruction_limit: 64,
                inline_control_flow_limit: 64,
                inline_growth_limit: None,
                ..OptimizationOptions::default()
            },
            &protected,
            &mut reports,
        );

        let composite_calls = module
            .functions
            .iter()
            .filter(|function| function.live)
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                matches!(
                    instruction.op,
                    ControlFlowOp::CallDirect { function, .. } if function == composite
                )
            })
            .count();
        assert_eq!(composite_calls, 2);
        assert!(!has_direct_call(&module, leaf));
        assert!(!module.functions[leaf.0 as usize].live);
        assert!(module.functions[composite.0 as usize].live);
    }

    #[test]
    fn optimizer_keeps_its_tracked_entry_outline_through_late_inlining() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int input(int index);print(((input(0)+1)*3-2)^7);print(((input(1)+1)*3-2)^7);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let original_functions = module.functions.len();

        let reports = optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: true,
                region_outlining: true,
                inline_instruction_limit: 64,
                inline_control_flow_limit: 64,
                inline_growth_limit: None,
                ..OptimizationOptions::disabled()
            },
            false,
        )
        .unwrap();

        assert!(reports
            .iter()
            .any(|report| { report.pass_name == "repeated-region-outlining" && report.changed }));
        let helper = module
            .functions
            .iter()
            .find(|function| {
                function.live
                    && function.id.0 as usize >= original_functions
                    && function.kind == FunctionKind::Function
            })
            .expect("tracked outlined helper must remain live");
        let calls = module.functions[module.entry.0 as usize]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                matches!(instruction.op, ControlFlowOp::CallDirect { function, .. } if function == helper.id)
            })
            .count();
        assert_eq!(calls, 2);
    }

    #[test]
    fn outlined_aggregate_results_recompute_untyped_escape_state() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Payload{int a;int b;int c;int d;}extern void consume(Payload value);void run(int first,int second){consume(Payload{first+1,first*2,first-3,first^4});consume(Payload{second+1,second*2,second-3,second^4});}run(10,20);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let original_functions = module.functions.len();
        promote_locals_to_ssa(&mut module).unwrap();
        analyze_escapes(&mut module);
        let (reports, outlined) =
            crate::compress_passes::run_compress_passes_tracking_outlined_helpers(
                &mut module,
                &crate::compress_passes::CompressPassOptions {
                    pipeline_fusion: false,
                    partial_escape_sinking: false,
                    region_outlining: true,
                    expression_superopt: false,
                    path_sensitive_propagation: false,
                },
            );
        assert!(!outlined.is_empty(), "{module:#?}");
        assert!(reports
            .iter()
            .any(|report| { report.pass_name == "escape-analysis" && report.changed }));

        let helper = module
            .functions
            .iter()
            .find(|function| {
                function.live
                    && function.id.0 as usize >= original_functions
                    && function.blocks.iter().any(|block| {
                        block.instructions.iter().any(|instruction| {
                            matches!(instruction.op, ControlFlowOp::Struct { .. })
                        })
                    })
            })
            .expect("repeated aggregate region must outline");
        let aggregate = helper
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| {
                matches!(instruction.op, ControlFlowOp::Struct { .. })
                    .then_some(instruction.out)
                    .flatten()
            })
            .expect("outlined aggregate result");
        assert_eq!(
            helper.value_escapes[aggregate.0 as usize],
            EscapeState::EscapesToUntypedBoundary
        );
        let call_results = module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction.op {
                ControlFlowOp::CallDirect { function, .. } if function == helper.id => {
                    instruction.out
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(call_results.len() >= 2);
        for result in call_results {
            let owner = module
                .functions
                .iter()
                .find(|function| {
                    function.blocks.iter().any(|block| {
                        block.instructions.iter().any(|instruction| {
                            instruction.out == Some(result)
                                && matches!(instruction.op, ControlFlowOp::CallDirect { function, .. } if function == helper.id)
                        })
                    })
                })
                .expect("call owner");
            assert_eq!(
                owner.value_escapes[result.0 as usize],
                EscapeState::EscapesToUntypedBoundary
            );
        }
    }

    #[test]
    fn independently_controls_closure_factory_inlining() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "func(int)->int make(int offset){return (int value)=>value+offset;}func(int)->int add=make(2);print(add(3));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let original = lower_to_control_flow(&program, &semantics).unwrap();
        let factory = original
            .functions
            .iter()
            .find(|function| function.name == Some("make"))
            .unwrap()
            .id;

        let mut inlined = original.clone();
        optimize_control_flow(&mut inlined).unwrap();
        assert!(!has_direct_call(&inlined, factory));

        let mut outlined = original;
        optimize_control_flow_with_options(
            &mut outlined,
            &OptimizationOptions {
                inline_closure_factories: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();
        assert!(has_direct_call(&outlined, factory));
    }

    fn has_direct_call(module: &ControlFlowModule<'_>, target: FunctionId) -> bool {
        module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .any(|instruction| {
                matches!(instruction.op, ControlFlowOp::CallDirect { function, .. } if function == target)
            })
    }

    #[test]
    fn removes_field_stores_overwritten_before_observation() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Box{int value;}extern void consume(Box box);Box box=new Box();box.value=1;box.value=2;consume(box);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let reports = optimize_control_flow(&mut module).unwrap();
        let stores = module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|instruction| matches!(instruction.op, ControlFlowOp::FieldSet { .. }))
            .count();
        assert_eq!(stores, 1);
        assert!(reports.iter().any(|report| {
            report.pass_name == "dead-field-store-elimination" && report.changed
        }));
    }

    #[test]
    fn folds_literal_array_lengths_across_unrelated_effects_only() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void tick();int[] values=[1,2,3];tick();print(values.length);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(
                instruction.op,
                ControlFlowOp::Array(_)
                    | ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::ArrayLength,
                        ..
                    }
            )));

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void mutate(int[] values);int[] values=[1,2,3];mutate(values);print(values.length);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction.op,
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayLength,
                    ..
                }
            )));

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1,2,3];int[] mapped=values.map((int value)=>value*2);mapped.push(8);print(mapped.length);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction.op,
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayLength,
                    ..
                }
            )));
    }

    #[test]
    fn preserves_array_push_inherited_setter_observation() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();extern void consume(int[] values);int[] values=[];values.push(read());consume(values);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut module, &options, false).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();
        let trace = run_javascript(&format!(
            "let calls=0,seen=0,own=false,value=0,length=0;Object.defineProperty(Array.prototype,'0',{{configurable:true,get(){{return -1}},set(item){{calls++;seen=item}}}});function read(){{return 9}}function consume(array){{own=Object.hasOwn(array,0);value=array[0];length=array.length}}try{{{output}}}finally{{delete Array.prototype[0]}}process.stdout.write('TRACE:'+calls+':'+own+':'+value+':'+length+':'+seen)"
        ));

        assert_eq!(trace, "TRACE:1:false:-1:1:9", "{output}");
    }

    #[test]
    fn preserves_plain_object_inherited_setter_observation() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();extern void consume(JsValue value);JsValue value=JS.object();value[\"_lilPlainObjectProbe\"]=read();consume(value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut module, &options, false).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();
        let trace = run_javascript(&format!(
            "let calls=0,seen=0,own=false,value=0;Object.defineProperty(Object.prototype,'_lilPlainObjectProbe',{{configurable:true,get(){{return -1}},set(item){{calls++;seen=item}}}});function read(){{return 9}}function consume(object){{own=Object.hasOwn(object,'_lilPlainObjectProbe');value=object._lilPlainObjectProbe}}try{{{output}}}finally{{delete Object.prototype._lilPlainObjectProbe}}process.stdout.write('TRACE:'+calls+':'+own+':'+value+':'+seen)"
        ));

        assert_eq!(trace, "TRACE:1:false:-1:9", "{output}");
    }

    #[test]
    fn projects_closed_null_prototype_record_observations() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Record<int> source=record{left:1,right:2,middle:3};Record<int> copy=record{...source,right:11};source.left=21;print(copy.left??0);print(copy.right??0);print(copy.absent??-1);print(Object.keys(copy).join(\",\"));print(JSON.stringify(copy));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let report = project_closed_record_observations_for_javascript(&mut module);
        assert!(report.changed);
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();
        assert!(!output.contains(".left=21"), "{output}");
        assert!(!output.contains("__proto__"), "{output}");
        assert!(!output.contains("Object.keys"), "{output}");
        assert!(!output.contains("JSON.stringify"), "{output}");
    }

    #[test]
    fn closed_record_projection_stops_at_unknown_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void touch(Record<int> value);Record<int> value=record{left:1};touch(value);print(value.missing??-1);print(JSON.stringify(value));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        project_closed_record_observations_for_javascript(&mut module);
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();

        assert!(output.contains("{__proto__:null"), "{output}");
        assert!(output.contains("JSON.stringify"), "{output}");
    }

    #[test]
    fn closed_record_json_projection_requires_portable_constants() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();Record<int> value=record{number:read()};print(JSON.stringify(value));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        project_closed_record_observations_for_javascript(&mut module);
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();

        assert!(output.contains("{__proto__:null"), "{output}");
        assert!(output.contains("JSON.stringify"), "{output}");
    }

    #[test]
    fn closed_record_projection_rejects_non_json_source_escapes() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"Record<string> value=record{"\q":"\q"};print(value["\q"]??"");print(Object.keys(value).join(","));print(JSON.stringify(value));"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        project_closed_record_observations_for_javascript(&mut module);
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();

        assert!(output.contains("__proto__"), "{output}");
        assert!(output.contains("Object.keys"), "{output}");
        assert!(output.contains("JSON.stringify"), "{output}");
    }

    #[test]
    fn closed_record_projection_does_not_carry_mutation_facts_across_branches() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern bool choose();Record<int> value=record{left:1};if(choose()){value.left=2;}print(value.left??0);print(JSON.stringify(value));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        project_closed_record_observations_for_javascript(&mut module);
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();

        assert!(output.contains("JSON.stringify"), "{output}");
        assert!(output.contains("?"), "{output}");
    }

    #[test]
    fn closed_record_projection_preserves_a_store_before_a_later_observer() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Record<int> value=record{left:1};value.left=2;print(value.left??0);print(JSON.stringify(value));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        // The neutral optimizer must preserve the source semantics before the
        // JS-only projection is offered.
        optimize_control_flow(&mut module).unwrap();
        project_closed_record_observations_for_javascript(&mut module);
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();

        assert!(output.contains(".left=2"), "{output}");
        assert!(output.contains("JSON.stringify"), "{output}");
    }

    #[test]
    fn closed_record_projection_crosses_an_unrelated_branch_by_dominance() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern bool choose();Record<int> value=record{left:1,right:2};if(choose()){print(7);}else{print(8);}print(value.left??0);print(JSON.stringify(value));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        project_closed_record_observations_for_javascript(&mut module);
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();

        assert!(!output.contains("__proto__"), "{output}");
        assert!(!output.contains("JSON.stringify"), "{output}");
        assert!(!output.contains(".left"), "{output}");
    }

    #[test]
    fn closed_record_projection_does_not_cross_a_loop_mutation() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int count();Record<int> value=record{left:1};for(int i=0;i<count();i++){value.left=i;}print(value.left??0);print(JSON.stringify(value));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        project_closed_record_observations_for_javascript(&mut module);
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();

        assert!(output.contains("__proto__"), "{output}");
        assert!(output.contains("JSON.stringify"), "{output}");
        assert!(output.contains(".left="), "{output}");
    }

    #[test]
    fn closed_record_projection_preserves_null_prototype_observations() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void pollutePrototype();Record<int> value=record{safe:1};pollutePrototype();print(value.toString??-1);print(JSON.stringify(value));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        project_closed_record_observations_for_javascript(&mut module);
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();

        assert!(output.contains("pollutePrototype()"), "{output}");
        assert!(!output.contains(".toString"), "{output}");
        assert!(!output.contains("JSON.stringify"), "{output}");
        let trace = run_javascript(&format!(
            "let calls=0;function pollutePrototype(){{calls++;Object.prototype.toString=99}}{output};process.stdout.write('TRACE:'+calls)"
        ));
        assert_eq!(trace, "-1\n{\"safe\":1}\nTRACE:1", "{output}");
    }

    #[test]
    fn closed_record_json_projection_uses_ecmascript_own_key_order() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Record<int> value=record{\"10\":10,\"2\":2,b:3,a:1};print(JSON.stringify(value));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        project_closed_record_observations_for_javascript(&mut module);
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();

        assert!(!output.contains("JSON.stringify"), "{output}");
        assert!(
            output.contains(r#"{\"2\":2,\"10\":10,\"b\":3,\"a\":1}"#),
            "{output}"
        );
    }

    #[test]
    fn erases_explicit_js_assume_to_the_original_runtime_value() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern JsValue read();float value=JS.assume(read());print(value);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&module).unwrap();
        assert!(!output.contains("JS.assume"), "{output}");
        assert!(output.contains("read()"), "{output}");
        assert!(!output.contains("typeof"), "{output}");
    }

    #[test]
    fn folds_interprocedural_array_lengths_only_for_closed_stable_calls() {
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        let has_array_length = |source: &str, preserve_exports: bool| {
            let arena = Bump::new();
            let program = parse_source(&arena, source).unwrap();
            let semantics = analyze(&program).unwrap();
            let mut module = lower_to_control_flow(&program, &semantics).unwrap();
            optimize_control_flow_with_options(&mut module, &options, preserve_exports).unwrap();
            module
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .any(|instruction| {
                    matches!(
                        instruction.op,
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::ArrayLength,
                            ..
                        }
                    )
                })
        };

        assert!(!has_array_length(
            "int count(int[] values){return values.length;}print(count([1,2,3]));print(count([4,5,6]));",
            false,
        ));
        assert!(has_array_length(
            "int count(int[] values){return values.length;}print(count([1]));print(count([1,2]));",
            false,
        ));
        assert!(has_array_length(
            "int count(int[] values){values.push(1);return values.length;}print(count([1,2,3]));",
            false,
        ));
        assert!(has_array_length(
            "export int count(int[] values){return values.length;}print(count([1,2,3]));",
            true,
        ));
    }

    #[test]
    fn removes_unobserved_local_collection_mutation_graphs() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1];values.push(2);values[0]=3;values.fill(4);Map<string,int> map=new Map<string,int>();map.set(\"a\",1).set(\"b\",2);Set<int> set=new Set<int>();set.add(1).add(2);print(7);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(
                instruction.op,
                ControlFlowOp::Array(_)
                    | ControlFlowOp::IndexSet { .. }
                    | ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::ArrayPush
                            | Intrinsic::ArrayFill
                            | Intrinsic::MapNew
                            | Intrinsic::MapSet
                            | Intrinsic::SetNew
                            | Intrinsic::SetAdd,
                        ..
                    }
            )));
    }

    #[test]
    fn infers_fluent_local_collection_helpers_are_effect_free() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "void scratch(){Map<string,int> map=new Map<string,int>();map.set(\"a\",1).set(\"b\",2);Set<int> set=new Set<int>();set.add(1).add(2);}scratch();print(7);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut module, &options, false).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(instruction.op, ControlFlowOp::CallDirect { .. })));
        assert!(module
            .functions
            .iter()
            .any(|function| { function.name == Some("scratch") && !function.live }));
    }

    #[test]
    fn rejects_declared_pure_dynamic_javascript_observations() {
        for source in [
            "extern JsValue input();pure bool inspect(JsValue value){return value==0;}print(inspect(input()));",
            "extern JsValue input();pure string inspect(JsValue value){return \"\"+value;}print(inspect(input()));",
            "extern int[]|string first();extern int[]|string second();pure bool inspect(int[]|string left,int[]|string right){return left==right;}print(inspect(first(),second()));",
            "extern JsValue input();pure bool inspect(JsValue value){return value.isArray();}print(inspect(input()));",
            "extern JsValue input();pure void inspect(JsValue value){for(string key in value){}}inspect(input());",
            "extern Generator<int> input();pure void inspect(Generator<int> values){for(int value of values){}}inspect(input());",
        ] {
            let arena = Bump::new();
            let program = parse_source(&arena, source).unwrap();
            let semantics = analyze(&program).unwrap();
            let mut module = lower_to_control_flow(&program, &semantics).unwrap();
            let error = optimize_control_flow_with_options(
                &mut module,
                &OptimizationOptions {
                    inlining: false,
                    ..OptimizationOptions::default()
                },
                false,
            )
            .unwrap_err();
            assert!(
                error
                    .message
                    .contains("declared `pure` but may perform an observable side effect"),
                "{source}: {error:?}"
            );
        }
    }

    #[test]
    fn keeps_typed_record_for_in_pure_and_dynamic_iteration_calls_effectful() {
        let arena = Bump::new();
        let record_program = parse_source(
            &arena,
            "pure int count(Record<int> value){int total=0;for(string key in value){total+=1;}return total;}print(count(record{a:1,b:2}));",
        )
        .unwrap();
        let record_semantics = analyze(&record_program).unwrap();
        let mut record_module = lower_to_control_flow(&record_program, &record_semantics).unwrap();
        optimize_control_flow_with_options(
            &mut record_module,
            &OptimizationOptions {
                inlining: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();

        for source in [
            "extern JsValue input();void observe(JsValue value){for(string key in value){}}observe(input());",
            "extern Generator<int> input();void observe(Generator<int> values){for(int value of values){}}observe(input());",
        ] {
            let arena = Bump::new();
            let program = parse_source(&arena, source).unwrap();
            let semantics = analyze(&program).unwrap();
            let mut module = lower_to_control_flow(&program, &semantics).unwrap();
            let observe = module
                .functions
                .iter()
                .find(|function| function.name == Some("observe"))
                .unwrap()
                .id;
            optimize_control_flow_with_options(
                &mut module,
                &OptimizationOptions {
                    inlining: false,
                    ..OptimizationOptions::default()
                },
                false,
            )
            .unwrap();
            let entry = &module.functions[module.entry.0 as usize];
            assert!(entry
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction.op,
                    ControlFlowOp::CallDirect { function, .. } if function == observe
                )), "iteration call was dropped: {source}");
        }
    }

    #[test]
    fn dce_retains_each_unused_dynamic_coercion_and_array_check() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern JsValue input();void observe(JsValue value){string first=\"\"+value;string second=\"\"+value;bool array=value.isArray();JsValue property=value[\"answer\"];}observe(input());",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow_with_options(
            &mut module,
            &OptimizationOptions {
                inlining: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();
        let observe = module
            .functions
            .iter()
            .find(|function| function.name == Some("observe"))
            .expect("effectful dynamic observer remains live");
        let instructions = observe
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| matches!(
                    instruction.op,
                    ControlFlowOp::Binary {
                        op: IrBinaryOp::Add,
                        ..
                    }
                ))
                .count(),
            2,
            "dynamic coercions must neither be merged nor dropped"
        );
        assert!(instructions.iter().any(|instruction| matches!(
            instruction.op,
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::JsIsArray,
                ..
            }
        )));
        assert!(instructions
            .iter()
            .any(|instruction| matches!(instruction.op, ControlFlowOp::IndexGet { .. })));
    }

    #[test]
    fn removes_parameter_mutation_calls_only_for_unobserved_roots() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "void fill(int[] left,int[] right){left.push(1);right.push(2);}void forward(int[] left,int[] right){fill(left,right);}int[] deadLeft=[];int[] deadRight=[];forward(deadLeft,deadRight);int[] live=[2];int[] linked=[];forward(live,linked);print(live.length);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut module, &options, false).unwrap();
        let entry = &module.functions[module.entry.0 as usize];
        let calls = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| matches!(instruction.op, ControlFlowOp::CallDirect { .. }))
            .count();
        let arrays = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| matches!(instruction.op, ControlFlowOp::Array(_)))
            .count();

        assert_eq!(calls, 1);
        assert_eq!(arrays, 2);
    }

    #[test]
    fn preserves_parameter_mutation_calls_with_inherent_effects() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "void noisy(int[] values){values.push(1);print(9);}int[] values=[];noisy(values);print(7);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut module, &options, false).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::CallDirect { .. })));
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::Array(_))));
    }

    #[test]
    fn preserves_collection_mutations_with_observed_state_or_results() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1];int length=values.push(2);print(length);int[] filled=[1];int[] alias=filled.fill(2);print(alias[0]);Map<string,int> map=new Map<string,int>();map.set(\"a\",1);print(map.get(\"a\"));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction.op,
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayPush,
                    ..
                }
            )));
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction.op,
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayFill,
                    ..
                }
            )));
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction.op,
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::MapSet,
                    ..
                }
            )));
    }

    #[test]
    fn profile_guidance_specializes_higher_order_calls_and_devirtualizes_them() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int apply(func(int)->int operation,int value){return operation(value);}int increment(int value){return value+1;}print(apply(increment,41));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            specialize_tagged_constants: true,
            ..OptimizationOptions::default()
        };
        let mut profile = OptimizationProfile::default();
        profile.functions.insert("$entry".to_string(), 10_000);
        let reports = optimize_control_flow_with_guidance(
            &mut module,
            &options,
            false,
            &OptimizationGuidance {
                profile,
                specialization_min_count: 10,
                max_specializations_per_function: 4,
                max_clone_instructions: 64,
            },
        )
        .unwrap();

        assert!(reports.iter().any(|report| {
            report.pass_name == "profiled-call-site-specialization" && report.changed
        }));
        assert!(module
            .functions
            .iter()
            .filter(|function| function.live)
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(instruction.op, ControlFlowOp::CallValue { .. })));
    }

    #[test]
    fn profile_guidance_clones_constant_capture_signatures() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "func(int)->int make(int offset){return (int value)=>value+offset;}func(int)->int add=make(2);print(add(40));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        let mut profile = OptimizationProfile::default();
        profile.functions.insert("$entry".to_string(), 10_000);
        profile.functions.insert("make".to_string(), 10_000);
        let reports = optimize_control_flow_with_guidance(
            &mut module,
            &options,
            false,
            &OptimizationGuidance {
                profile,
                specialization_min_count: 10,
                max_specializations_per_function: 4,
                max_clone_instructions: 64,
            },
        )
        .unwrap();

        assert!(reports
            .iter()
            .any(|report| { report.pass_name == "capture-signature-cloning" && report.changed }));
        assert!(module
            .functions
            .iter()
            .filter(|function| function.live)
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.op {
                ControlFlowOp::Closure { captures, .. } => Some(captures),
                _ => None,
            })
            .all(Vec::is_empty));
    }
}
