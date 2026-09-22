//! Formation-time placement must preserve source evaluations while removing
//! unnecessary target temporaries. Expectations belong to fixed source/host
//! fixtures, independently of the placement recognizer and its emitted spelling.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, TacticId,
    WorkDomain, WorkKind,
};
use crate::structured_js::selection::{Objective, Plan, Sizes, Style};
use std::process::Command;

struct Case {
    name: &'static str,
    source: &'static str,
    setup: &'static str,
    expected: &'static str,
}
macro_rules! case {
    ($name:literal) => {
        Case {
            name: $name,
            source: include_str!(concat!("fixtures/value-placement/", $name, ".lil")),
            setup: include_str!(concat!("fixtures/value-placement/", $name, ".setup.js")),
            expected: include_str!(concat!("fixtures/value-placement/", $name, ".expected.out")),
        }
    };
}
fn policy(compact: bool) -> ResolvedPolicy {
    let configuration = format!(
        "[javascript]\nstrip_console=false\n[policy.tactics]\ntarget-compaction='{}'\n",
        if compact { "on" } else { "off" }
    );
    let config: crate::config::ProjectConfig = toml::from_str(&configuration).unwrap();
    let policy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    assert_eq!(policy.tactic(TacticId::TargetCompaction).enabled, compact);
    policy
}
fn compilation<'src>() -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 256_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 12 },
    )
    .unwrap()
}
struct Emission {
    javascript: String,
    sizes: Sizes,
    render_work: u64,
}
fn emit(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    style: Style,
) -> Emission {
    compiler
        .with_implementation_description(candidate, WorkDomain::Optional, |_| ())
        .unwrap();
    let retained = compiler.ledger().retained_bytes();
    let before = compiler.ledger().work_by_kind(WorkKind::Render);
    let (javascript, sizes) = compiler
        .with_javascript_output_in(candidate, policy, WorkDomain::Optional, |output| {
            let artifact = output.render(&Plan::new(style))?;
            for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
                output.measure(artifact, codec)?;
            }
            let sizes = output.with_artifact(artifact, |view| view.sizes)?;
            Ok::<_, CandidateError>((output.take_artifact(artifact)?, sizes))
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        compiler.ledger().retained_bytes(),
        retained,
        "temporary output storage releases after handoff"
    );
    assert_eq!(sizes.raw, javascript.len());
    assert!(sizes.gzip9.is_some_and(|bytes| bytes > 0));
    assert!(sizes.brotli11.is_some_and(|bytes| bytes > 0));
    let render_work = compiler.ledger().work_by_kind(WorkKind::Render) - before;
    assert!(render_work > 0);
    Emission {
        javascript,
        sizes,
        render_work,
    }
}
// Preserve small, completely measured artifacts in the pinned nocapture log.
// These Strings have already crossed the explicit terminal handoff boundary.
fn artifact_evidence(case: &str, style: Style, compact: bool, emitted: &Emission) {
    eprintln!(
        "placement-artifact {}",
        serde_json::json!({
            "case": case,
            "style": format!("{style:?}"),
            "compact": compact,
            "javascript": &emitted.javascript,
            "raw": emitted.sizes.raw,
            "gzip9": emitted.sizes.gzip9,
            "brotli11": emitted.sizes.brotli11,
            "render_work": emitted.render_work,
        })
    );
}

fn execute(case: &Case, representation: &str, compact: bool, style: Style, javascript: &str) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, javascript, oxc_span::SourceType::mjs()).parse();
    assert!(
        !parsed.panicked && parsed.diagnostics.is_empty(),
        "{} {representation} {compact} {style:?}: {:?}\n{javascript}",
        case.name,
        parsed.diagnostics
    );
    let script = format!(
        "{}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));await globalThis.valuePlacementObserve(library);",
        case.setup,
        serde_json::to_string(javascript).unwrap()
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for value-placement observations");
    assert!(
        output.status.success(),
        "{} {representation} compact={compact} {style:?}: {}\n{javascript}",
        case.name,
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        case.expected,
        "{} {representation} compact={compact} {style:?}\n{javascript}",
        case.name
    );
}

/// Observe the fixed scalar fixtures through Oxc, including function values
/// nested inside assignments/calls. The explicit worklist also handles the deep
/// segmented fixture without making the measurement itself recursively deep.
#[derive(Debug, Default, PartialEq, Eq)]
struct ScalarSyntax {
    declarations: usize,
    normalizations: usize,
}
fn scalar_syntax(javascript: &str) -> ScalarSyntax {
    use oxc_ast::ast::{
        Argument, Declaration, Expression, Function, ObjectPropertyKind, Statement,
        VariableDeclaration,
    };
    enum Node<'tree, 'src> {
        Statement(&'tree Statement<'src>),
        Declaration(&'tree Declaration<'src>),
        Expression(&'tree Expression<'src>),
        Variables(&'tree VariableDeclaration<'src>),
        Function(&'tree Function<'src>),
    }
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, javascript, oxc_span::SourceType::mjs()).parse();
    assert!(!parsed.panicked && parsed.diagnostics.is_empty());
    let mut pending = parsed
        .program
        .body
        .iter()
        .map(Node::Statement)
        .collect::<Vec<_>>();
    let mut result = ScalarSyntax::default();
    while let Some(node) = pending.pop() {
        match node {
            Node::Statement(statement) => match statement {
                Statement::VariableDeclaration(value) => pending.push(Node::Variables(value)),
                Statement::FunctionDeclaration(value) => pending.push(Node::Function(value)),
                Statement::BlockStatement(value) => {
                    pending.extend(value.body.iter().map(Node::Statement))
                }
                Statement::ExportDeclaration(value) => {
                    pending.push(Node::Declaration(&value.declaration))
                }
                Statement::ExpressionStatement(value) => {
                    pending.push(Node::Expression(&value.expression))
                }
                Statement::ReturnStatement(value) => {
                    pending.extend(value.argument.iter().map(Node::Expression))
                }
                Statement::ExportNamedDeclaration(_) | Statement::EmptyStatement(_) => {}
                other => panic!("unexpected control in scalar fixture: {other:?}"),
            },
            Node::Declaration(value) => match value {
                Declaration::VariableDeclaration(value) => pending.push(Node::Variables(value)),
                Declaration::FunctionDeclaration(value) => pending.push(Node::Function(value)),
                other => panic!("unexpected declaration in scalar fixture: {other:?}"),
            },
            Node::Variables(value) => {
                result.declarations += value.declarations.len();
                for declaration in &value.declarations {
                    pending.extend(declaration.init.iter().map(Node::Expression));
                }
            }
            Node::Function(value) => {
                if let Some(body) = &value.body {
                    pending.extend(body.statements.iter().map(Node::Statement));
                }
            }
            Node::Expression(value) => match value {
                Expression::FunctionExpression(value) => pending.push(Node::Function(value)),
                Expression::ArrowFunctionExpression(value) => {
                    if let Some(body) = value.body.as_function_body() {
                        pending.extend(body.statements.iter().map(Node::Statement));
                    } else {
                        pending.push(Node::Expression(value.body.as_expression().unwrap()));
                    }
                }
                Expression::AssignmentExpression(value) => {
                    pending.push(Node::Expression(&value.right))
                }
                Expression::ParenthesizedExpression(value) => {
                    pending.push(Node::Expression(&value.expression))
                }
                Expression::UnaryExpression(value) => {
                    pending.push(Node::Expression(&value.argument))
                }
                Expression::BinaryExpression(value) => {
                    if value.operator.as_str() == "|"
                        && matches!(&value.right, Expression::NumericLiteral(value) if value.value == 0.0)
                    {
                        result.normalizations += 1;
                    }
                    pending.extend([
                        Node::Expression(&value.left),
                        Node::Expression(&value.right),
                    ]);
                }
                Expression::LogicalExpression(value) => pending.extend([
                    Node::Expression(&value.left),
                    Node::Expression(&value.right),
                ]),
                Expression::ConditionalExpression(value) => pending.extend([
                    Node::Expression(&value.test),
                    Node::Expression(&value.consequent),
                    Node::Expression(&value.alternate),
                ]),
                Expression::SequenceExpression(value) => {
                    pending.extend(value.expressions.iter().map(Node::Expression))
                }
                Expression::CallExpression(value) => {
                    pending.push(Node::Expression(&value.callee));
                    for argument in &value.arguments {
                        pending.push(Node::Expression(match argument {
                            Argument::SpreadElement(value) => &value.argument,
                            _ => argument.as_expression().unwrap(),
                        }));
                    }
                }
                Expression::NewExpression(value) => {
                    pending.push(Node::Expression(&value.callee));
                    for argument in &value.arguments {
                        pending.push(Node::Expression(match argument {
                            Argument::SpreadElement(value) => &value.argument,
                            _ => argument.as_expression().unwrap(),
                        }));
                    }
                }
                Expression::ObjectExpression(value) => {
                    for property in &value.properties {
                        match property {
                            ObjectPropertyKind::ObjectProperty(property) => {
                                // A method/getter/setter stores its body in the
                                // same FunctionExpression value as other fields.
                                pending.push(Node::Expression(&property.value));
                                if property.computed {
                                    pending.push(Node::Expression(
                                        property.key.as_expression().unwrap(),
                                    ));
                                }
                            }
                            ObjectPropertyKind::SpreadProperty(property) => {
                                pending.push(Node::Expression(&property.argument));
                            }
                        }
                    }
                }
                Expression::ComputedMemberExpression(value) => pending.extend([
                    Node::Expression(&value.object),
                    Node::Expression(&value.expression),
                ]),
                Expression::StaticMemberExpression(value) => {
                    pending.push(Node::Expression(&value.object))
                }
                Expression::BooleanLiteral(_)
                | Expression::NullLiteral(_)
                | Expression::NumericLiteral(_)
                | Expression::StringLiteral(_)
                | Expression::Identifier(_)
                | Expression::ThisExpression(_) => {}
                other => panic!("unvisited expression kind in scalar fixture: {other:?}"),
            },
        }
    }
    result
}

#[test]
fn scalar_syntax_observer_visits_nested_function_owners() {
    assert_eq!(
        scalar_syntax("let target;target=(function(){let a,b;return ()=>{let c;return c|0}});"),
        ScalarSyntax {
            declarations: 4,
            normalizations: 1
        }
    );
    assert_eq!(
        scalar_syntax(
            "let target;target=({identity:function(){let kept;return kept|0},[(function(){let key;return 'extra';})()]:function(){let value;return value},method(){let local;return local},...{other:function(){let spread;return spread}}}).identity;"
        ),
        ScalarSyntax {
            declarations: 6,
            normalizations: 1
        },
    );
}

fn check_direct(case: Case, require_reduction: bool) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let revisions = program
        .units
        .iter()
        .map(FrozenUnit::revision)
        .collect::<Vec<_>>();
    let mut compiler = compilation();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let off = policy(false);
    let on = policy(true);
    assert_eq!(
        off.contract(),
        on.contract(),
        "placement changes an implementation permission, not language semantics"
    );
    let candidate = compiler
        .direct_javascript(source, &off, WorkDomain::Baseline)
        .unwrap();
    for style in [Style::Global, Style::Scoped, Style::Source] {
        let baseline = emit(&mut compiler, candidate, &off, style);
        let compact = emit(&mut compiler, candidate, &on, style);
        execute(&case, "direct", false, style, &baseline.javascript);
        execute(&case, "direct", true, style, &compact.javascript);
        if require_reduction {
            let before = scalar_syntax(&baseline.javascript).declarations;
            let after = scalar_syntax(&compact.javascript).declarations;
            assert!(
                after < before,
                "{} {style:?}: placement must remove actual temporaries ({before} -> {after})\nbaseline:{}\ncompact:{}",
                case.name,
                baseline.javascript,
                compact.javascript
            );
            assert!(compact.sizes.raw < baseline.sizes.raw);
            // Measure all complete artifacts; this single tactic need not win
            // every codec. Search retains each objective's eligible incumbent.
            eprintln!(
                "value-placement {} {style:?}: declarations {before}->{after}, raw/gzip9/br11 {:?}->{:?}, render-work {}->{}",
                case.name, baseline.sizes, compact.sizes, baseline.render_work, compact.render_work
            );
            artifact_evidence(case.name, style, false, &baseline);
            artifact_evidence(case.name, style, true, &compact);
        }
    }
    compiler
        .with_semantic(source, |program, _, _| {
            assert_eq!(
                program
                    .units
                    .iter()
                    .map(FrozenUnit::revision)
                    .collect::<Vec<_>>(),
                revisions
            )
        })
        .unwrap();
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn placement_reduces_scalar_temporaries_with_complete_codec_measurements() {
    check_direct(case!("scalar-return"), true);
}
#[test]
fn placement_preserves_exact_site_calls_returns_and_finally_completion() {
    check_direct(case!("exact-site-call"), false);
}
#[test]
fn placement_preserves_prepared_callees_and_value_invocation_receiver_loss() {
    check_direct(case!("prepared-calls"), false);
}
#[test]
fn placement_preserves_mutable_snapshots_across_foreign_reentry() {
    check_direct(case!("snapshot-reentry"), false);
}
#[test]
fn placement_preserves_lazy_operands_loop_iterations_and_throw_finally_order() {
    check_direct(case!("lazy-loop-finally"), false);
}

fn named_cell(program: &Program<'_>, name: &str) -> CellId {
    let mut found = program
        .cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.name == name);
    let (index, _) = found
        .next()
        .unwrap_or_else(|| panic!("missing fixture cell {name}"));
    assert!(found.next().is_none(), "ambiguous fixture cell {name}");
    CellId::from_index(index).unwrap()
}
fn facts_request() -> LocalFactsRequest {
    LocalFactsRequest {
        work_quota: 100_000,
        result_bytes: 100_000,
    }
}
fn string_candidate(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    definition: ValueRef,
    choice: StringChoice,
    policy: &ResolvedPolicy,
) -> CandidateId {
    match compiler
        .represent_string_javascript(
            base,
            &[definition],
            choice,
            StringRequest {
                max_work: 1_000_000,
                scratch_bytes: 1_000_000,
                output_bytes: 1_000_000,
                local_facts: facts_request(),
            },
            policy,
            WorkDomain::Optional,
        )
        .unwrap()
        .outcome
    {
        StringOutcome::Published(candidate) => candidate,
        other => panic!("composition string family was not admitted: {other:?}"),
    }
}
#[test]
fn placement_composes_with_shared_inline_helpers_scalar_records_and_literal_choices() {
    let case = case!("representation-composition");
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let state = named_cell(&program, "state");
    let helper = named_cell(&program, "createLabel");
    let definition = {
        let program = &program;
        let mut definitions = program.units.iter().enumerate().flat_map(|(unit, data)| {
            data.data().operations.iter().filter_map(move |operation| {
                (matches!(operation.kind, OperationKind::Binary(BinaryOp::Add))
                    && operation.result.is_some_and(|value| {
                        matches!(
                            program.types[data.data().values[value.index()].ty.index()],
                            Type::String
                        )
                    }))
                .then(|| ValueRef {
                    unit: UnitId::from_index(unit).unwrap(),
                    value: operation.result.unwrap(),
                })
            })
        });
        let definition = definitions
            .next()
            .expect("computed string fixture definition");
        assert!(definitions.next().is_none());
        definition
    };
    let mut compiler = compilation();
    compiler
        .enable_local_facts(
            CacheLimits {
                entries: 32,
                bytes: 1_000_000,
                result_bytes: 100_000,
            },
            WorkDomain::Baseline,
        )
        .unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let off = policy(false);
    let on = policy(true);
    let direct = compiler
        .direct_javascript(source, &off, WorkDomain::Baseline)
        .unwrap();
    let scalar = match compiler
        .scalar_javascript(
            direct,
            state,
            ScalarRequest {
                max_work: 1_000_000,
                scratch_bytes: 1_000_000,
                output_bytes: 1_000_000,
            },
            &off,
            WorkDomain::Optional,
        )
        .unwrap()
        .outcome
    {
        ScalarOutcome::Published(candidate) => candidate,
        other => panic!("composition record family was not admitted: {other:?}"),
    };
    let mut candidates = vec![
        ("direct/computed", direct),
        ("scalar/shared-helper/computed", scalar),
    ];
    for (name, base) in [
        ("direct/inline/literal", direct),
        ("scalar/inline/literal", scalar),
    ] {
        let inline = match compiler
            .inline_helper_javascript(
                base,
                helper,
                HelperRequest {
                    max_work: 1_000_000,
                    scratch_bytes: 1_000_000,
                    output_bytes: 1_000_000,
                    local_facts: facts_request(),
                },
                &off,
                WorkDomain::Optional,
            )
            .unwrap()
            .outcome
        {
            HelperOutcome::Published(candidate) => candidate,
            other => panic!("composition helper family was not admitted: {other:?}"),
        };
        candidates.push((
            if base == direct {
                "direct/inline/computed"
            } else {
                "scalar/inline/computed"
            },
            inline,
        ));
        let literal = string_candidate(
            &mut compiler,
            inline,
            definition,
            StringChoice::LiteralAtDefinition,
            &off,
        );
        candidates.push((name, literal));
        let shared = string_candidate(
            &mut compiler,
            inline,
            definition,
            StringChoice::SharedLiteral {
                activation: definition.unit,
            },
            &off,
        );
        candidates.push((
            if base == direct {
                "direct/inline/shared-string"
            } else {
                "scalar/inline/shared-string"
            },
            shared,
        ));
    }
    for (name, candidate) in candidates {
        for style in [Style::Global, Style::Scoped, Style::Source] {
            for (compact, policy) in [(false, &off), (true, &on)] {
                let emitted = emit(&mut compiler, candidate, policy, style);
                execute(&case, name, compact, style, &emitted.javascript);
            }
        }
    }
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn deep_scalar_formation_segments_expressions_without_losing_useful_compaction() {
    // Construct a verified semantic chain from one small checked function. This
    // targets Formation/Module/printer depth, independently of frontend AST depth.
    let arena = bumpalo::Bump::new();
    let syntax =
        crate::parse_source(&arena, "export int chain(int input){return input+1;}").unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let mut program = from_checked_source(&syntax, &semantics).unwrap();
    let unit = program
        .units
        .iter()
        .position(|unit| unit.data().kind == UnitKind::Function)
        .unwrap();
    let steps = crate::structured_js::MAX_NESTING * 3;
    let mut changed = program.units[unit].clone().into_working();
    let data = changed.get_mut();
    let mut ret = data.operations.pop().unwrap();
    assert!(matches!(ret.kind, OperationKind::Return));
    let old_return = data.regions[ret.region.index()].operations.pop().unwrap();
    assert_eq!(old_return.index(), data.operations.len());
    assert_eq!(
        ret.operands.start as usize + ret.operands.len as usize,
        data.operands.len()
    );
    data.operands.truncate(ret.operands.start as usize);
    let prototype = data
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntBinary(crate::primitive::IntBinary::Add)
            )
        })
        .unwrap()
        .clone();
    let one = data.operands(prototype.operands).unwrap()[1];
    let mut previous = prototype.result.unwrap();
    let ty = data.values[previous.index()].ty;
    for _ in 1..steps {
        let operation = OpId::from_index(data.operations.len()).unwrap();
        let value = ValueId::from_index(data.values.len()).unwrap();
        let operands = OperandRange {
            start: u32::try_from(data.operands.len()).unwrap(),
            len: 2,
        };
        data.operands.extend([previous, one]);
        data.values.push(Value {
            ty,
            definition: operation,
        });
        data.operations.push(Operation {
            kind: prototype.kind.clone(),
            operands,
            result: Some(value),
            region: prototype.region,
            origin: None,
            span: prototype.span,
        });
        data.regions[prototype.region.index()]
            .operations
            .push(operation);
        previous = value;
    }
    ret.operands = OperandRange {
        start: u32::try_from(data.operands.len()).unwrap(),
        len: 1,
    };
    data.operands.push(previous);
    data.regions[ret.region.index()]
        .operations
        .push(OpId::from_index(data.operations.len()).unwrap());
    data.operations.push(ret);
    program.units[unit] = changed.freeze();
    program.verify().unwrap();

    let mut compiler = compilation();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let off = policy(false);
    let on = policy(true);
    let candidate = compiler
        .direct_javascript(source, &off, WorkDomain::Baseline)
        .unwrap();
    let baseline = emit(&mut compiler, candidate, &off, Style::Global);
    let compact = emit(&mut compiler, candidate, &on, Style::Global);
    let before = scalar_syntax(&baseline.javascript).declarations;
    let after = scalar_syntax(&compact.javascript).declarations;
    assert!(
        after >= 2,
        "a chain three times the shared target limit still needs real segment boundaries"
    );
    assert!(
        after < before,
        "segmentation must retain useful fusion: {before}->{after}"
    );
    assert!(compact.sizes.raw < baseline.sizes.raw);
    let expected = serde_json::json!([
        steps as i32,
        7 + steps as i32,
        i32::MAX.wrapping_add(steps as i32)
    ]);
    for javascript in [&baseline.javascript, &compact.javascript] {
        let script = format!(
            "const library=await import('data:text/javascript,'+encodeURIComponent({}));console.log(JSON.stringify([library.chain(0),library.chain(7),library.chain(2147483647)]));",
            serde_json::to_string(javascript).unwrap()
        );
        let output = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            expected
        );
    }
    eprintln!(
        "deep value-placement: {steps} additions, declarations {before}->{after}, raw/gzip9/br11 {:?}->{:?}",
        baseline.sizes, compact.sizes
    );
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn placement_number_facts_follow_raw_operations_and_preserve_binary64_coercion() {
    check_direct(
        Case {
            name: "raw-number-normalization",
            source: r#"
            extern int raw();extern void mark(int value);
            export int add(int left,int right){return left+right;}
            export int negate(int value){return -value;}
            export int multiply(int left,int right){return left*right;}
            export int divide(int left,int right){return left/right;}
            export int remainder(int left,int right){return left%right;}
            export int one(){return raw()+1;}
            export int ordered(){int answer=raw()+raw();mark(9);return answer;}
        "#,
            setup: r#"
            const trace=[],sentinel={};
            globalThis.mark=value=>trace.push('mark:'+value);
            globalThis.valuePlacementObserve=library=>{
                const sample=(name,value)=>trace.push([name,value,Object.is(value,-0)]);
                const imul=Math.imul;
                Math.imul=()=>{throw Error('ordinary multiplication must not use imul');};
                sample('add-overflow',library.add(2147483647,1));
                sample('negate-min',library.negate(-2147483648));
                sample('negate-zero',library.negate(0));
                sample('multiply-negative-zero',library.multiply(-0,5));
                sample('remainder-negative-zero',library.remainder(-4,2));
                sample('divide-zero',library.divide(1,0));
                sample('divide-overflow',library.divide(-2147483648,-1));
                sample('multiply-rounding',library.multiply(2147483647,2147483647));
                trace.push(['imul-control',imul(2147483647,2147483647)]);
                globalThis.raw=()=> '7';
                trace.push(['string-add',library.one()]);
                globalThis.raw=()=>NaN;
                sample('nan',library.one());
                let next=0,fail=false;
                globalThis.raw=()=>{
                    const id=++next;trace.push('raw:'+id);
                    return {[Symbol.toPrimitive](hint){trace.push('coerce:'+id+':'+hint);if(fail)throw sentinel;return id===1?4.75:8.5;}};
                };
                trace.push(['ordered',library.ordered()]);
                next=0;fail=true;
                try{library.ordered();trace.push('unexpected return');}catch(error){trace.push(['throw',error===sentinel]);}
                console.log(JSON.stringify(trace));
            };
        "#,
            expected: "[[\"add-overflow\",-2147483648,false],[\"negate-min\",-2147483648,false],[\"negate-zero\",0,false],[\"multiply-negative-zero\",0,false],[\"remainder-negative-zero\",0,false],[\"divide-zero\",0,false],[\"divide-overflow\",-2147483648,false],[\"multiply-rounding\",0,false],[\"imul-control\",1],[\"string-add\",71],[\"nan\",0,false],\"raw:1\",\"raw:2\",\"coerce:1:default\",\"coerce:2:default\",\"mark:9\",[\"ordered\",13],\"raw:1\",\"raw:2\",\"coerce:1:default\",[\"throw\",true]]\n",
        },
        false,
    );
}

#[test]
fn bounded_mask_arithmetic_omits_a_proved_normalization_without_changing_coercion() {
    let case = Case {
        name: "mask-range-normalization",
        source: "export int byte(int value){return (value&255)+1;}",
        setup: r#"
            globalThis.valuePlacementObserve=library=>{
                const trace=[0,2147483647,-2147483648,-1,256,'257',-0,NaN].map(value=>library.byte(value));
                const value=library.byte({[Symbol.toPrimitive](hint){trace.push('coerce:'+hint);return 511.8;}});
                trace.push(value,Object.is(value,-0));
                const sentinel={};
                try{library.byte({[Symbol.toPrimitive](){trace.push('coerce:throw');throw sentinel;}});}
                catch(error){trace.push(error===sentinel);}
                console.log(JSON.stringify(trace));
            };
        "#,
        expected: "[1,256,1,256,1,2,1,1,\"coerce:number\",256,false,\"coerce:throw\",true]\n",
    };
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let mut compiler = compilation();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let off = policy(false);
    let on = policy(true);
    let candidate = compiler
        .direct_javascript(source, &off, WorkDomain::Baseline)
        .unwrap();
    for style in [Style::Global, Style::Scoped, Style::Source] {
        let baseline = emit(&mut compiler, candidate, &off, style);
        let compact = emit(&mut compiler, candidate, &on, style);
        execute(&case, "direct", false, style, &baseline.javascript);
        execute(&case, "direct", true, style, &compact.javascript);
        let before = scalar_syntax(&baseline.javascript);
        let after = scalar_syntax(&compact.javascript);
        assert!(
            after.normalizations < before.normalizations,
            "range [0,255]+1 must discharge a real conversion: {before:?}->{after:?}\nbaseline:{}\ncompact:{}",
            baseline.javascript,
            compact.javascript
        );
        assert!(compact.sizes.raw < baseline.sizes.raw);
        eprintln!(
            "mask value-placement {style:?}: conversions {}->{}, raw/gzip9/br11 {:?}->{:?}",
            before.normalizations, after.normalizations, baseline.sizes, compact.sizes
        );
        artifact_evidence(case.name, style, false, &baseline);
        artifact_evidence(case.name, style, true, &compact);
    }
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
