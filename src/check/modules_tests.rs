use super::*;
use crate::module::ModuleSource;
use crate::parser::parse_source;
use bumpalo::Bump;

#[test]
fn module_alias_binding_types_borrow_the_canonical_nested_payload() {
    let sources = [
        "import {values as matrix,echo as call} from \"./barrel\";int[][] copied=call(matrix);print(copied[0][0]);",
        "import {values,echo} from \"./implementation\";export {values,echo};",
        "export int[][] values=[[3]];export int[][] echo(int[][] item){return item;}",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let checked =
        analyze_modules(&programs, &graph(&sources, &[&[1], &[2], &[]], &[2, 1, 0])).unwrap();
    let mut aliases = 0;
    for (module, interface) in checked.interfaces().iter().enumerate() {
        let view = checked.view(module).unwrap();
        for import in &interface.imports {
            let InterfaceTarget::Value(id) = import.target else {
                unreachable!();
            };
            let canonical = &checked.symbols()[id.0 as usize].ty;
            assert!(std::ptr::eq(
                view.binding_type(import.span).unwrap(),
                canonical
            ));
            assert!(matches!(
                checked.facts[module].binding_types.get(&import.span),
                Some(BindingType::Symbol(symbol)) if *symbol == id
            ));
            assert_eq!(checked.symbol_module(id), Some(2));
            aliases += 1;
        }
    }
    assert_eq!(aliases, 4);
    let values = checked
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "values")
        .unwrap();
    let Type::Array(outer) = &values.ty else {
        unreachable!()
    };
    assert!(matches!(&**outer, Type::Array(inner) if **inner == Type::Int));
    for module in 0..2 {
        let interface = &checked.interfaces()[module];
        let alias = interface
            .imports
            .iter()
            .find(|import| import.imported == "values")
            .unwrap();
        let Type::Array(alias_outer) = checked
            .view(module)
            .unwrap()
            .binding_type(alias.span)
            .unwrap()
        else {
            unreachable!();
        };
        assert!(std::ptr::eq(&**outer, &**alias_outer));
    }
    let uses = checked.facts[0].source_info.iter().filter_map(|info| {
        let Expr {
            kind: ExprKind::Ident(ident),
            ..
        } = info.expression?
        else {
            return None;
        };
        Some(ident.span)
    });
    for span in uses {
        assert!(checked.view(0).unwrap().binding_type(span).is_none());
    }
    assert!(checked
        .declarations
        .identifier_index_is_consistent(&checked.facts));
}

#[test]
fn repeated_foreign_binding_types_share_contracts_but_keep_detached_parameters_distinct() {
    let sources = [
        "import \"./other\";extern int[][] host(int[][] input);",
        "extern int[][] host(int[][] input);",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let checked = analyze_modules(&programs, &graph(&sources, &[&[1], &[]], &[1, 0])).unwrap();
    let host = checked
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "host")
        .unwrap();
    assert_eq!(host.origin, DeclarationOrigin::Foreign);
    let mut parameters = Vec::new();
    for (module, program) in programs.iter().enumerate() {
        let declaration = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Extern(declaration) => Some(declaration),
                _ => None,
            })
            .unwrap();
        let view = checked.view(module).unwrap();
        assert_eq!(view.identifier_symbol(declaration.name.span), Some(host.id));
        assert!(std::ptr::eq(
            view.binding_type(declaration.name.span).unwrap(),
            &host.ty
        ));
        let parameter = declaration.params[0].name;
        let id = view.identifier_symbol(parameter.span).unwrap();
        parameters.push(id);
        let canonical = &checked.symbols()[id.0 as usize];
        assert_eq!(canonical.origin, DeclarationOrigin::Source);
        assert_eq!(checked.symbol_module(id), Some(module));
        assert!(std::ptr::eq(
            view.binding_type(parameter.span).unwrap(),
            &canonical.ty
        ));
        assert!(
            matches!(&canonical.ty, Type::Array(outer) if matches!(&**outer, Type::Array(inner) if **inner == Type::Int))
        );
    }
    assert_ne!(parameters[0], parameters[1]);
    assert_eq!(
        checked
            .symbols()
            .iter()
            .filter(|symbol| symbol.name == "host")
            .count(),
        1
    );
    assert!(checked
        .declarations
        .identifier_index_is_consistent(&checked.facts));
}

fn graph(sources: &[&str], dependencies: &[&[usize]], order: &[usize]) -> ModuleSet {
    ModuleSet {
        modules: sources
            .iter()
            .zip(dependencies)
            .enumerate()
            .map(|(id, (source, deps))| ModuleSource {
                path: format!("/module-{id}.lil").into(),
                source: (*source).into(),
                dependencies: deps.to_vec(),
                foreign_dependencies: Vec::new(),
                dynamic_dependencies: Vec::new(),
                // Deliberately irrelevant: direct checking must never offset spans.
                offset: 100_000 + id * 99_999,
            })
            .collect(),
        dependency_order: order.to_vec(),
        root: 0,
        eager: vec![true; sources.len()],
    }
}

#[test]
fn source_local_nodes_and_private_names_share_only_the_declaration_owner() {
    let sources = [
        "import { two } from \"./two\"; int parse(int x){return x+1;} print(parse(two()));",
        "int parse(int x){return x+2;} export int two(){return parse(2);}",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let checked = analyze_modules(&programs, &graph(&sources, &[&[1], &[]], &[1, 0])).unwrap();
    let first = checked.view(0).unwrap();
    let second = checked.view(1).unwrap();
    assert!(std::ptr::eq(
        first.symbols().as_ptr(),
        second.symbols().as_ptr()
    ));
    assert!(first.belongs_to(programs[0].source_identity()));
    assert!(!first.belongs_to(programs[1].source_identity()));
    assert_eq!(checked.initialization_order(), &[1, 0]);
    assert_eq!(checked.root(), 0);
    let parses: Vec<_> = checked
        .symbols()
        .iter()
        .filter(|symbol| symbol.name == "parse")
        .collect();
    assert_eq!(parses.len(), 2);
    assert_ne!(parses[0].id, parses[1].id);
    assert_eq!(checked.symbol_module(parses[0].id), Some(0));
    assert_eq!(checked.symbol_module(parses[1].id), Some(1));
    assert!(checked
        .symbols()
        .iter()
        .all(|symbol| checked.symbol_module(symbol.id).is_some()));
    for (module, program) in programs.iter().enumerate() {
        for info in &checked.facts[module].source_info {
            if let Some(expr) = info.expression {
                assert!(expr.span().end <= sources[module].len());
                assert!(std::ptr::eq(
                    checked
                        .view(module)
                        .unwrap()
                        .source_expression(expr.id)
                        .unwrap(),
                    expr
                ));
                assert!(checked
                    .view(module)
                    .unwrap()
                    .belongs_to(program.source_identity()));
            }
        }
    }
}

#[test]
fn colliding_spans_do_not_conflate_types_or_declaration_occurrences() {
    let sources = [
        "import \"./other\"; int value=1;print(value);",
        "import \"./root\"; string value=\"x\";print(value);",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let checked = analyze_modules(&programs, &graph(&sources, &[&[1], &[0]], &[1, 0])).unwrap();
    let values: Vec<_> = checked
        .symbols()
        .iter()
        .filter(|symbol| symbol.name == "value")
        .collect();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].ty, Type::Int);
    assert_eq!(values[1].ty, Type::String);
    // Literal SourceNodeId zero in each source has independently owned types.
    let literals: Vec<_> = checked
        .facts
        .iter()
        .map(|facts| {
            facts
                .source_info
                .iter()
                .find_map(|info| {
                    let expr = info.expression?;
                    matches!(expr.kind, ExprKind::Int(..) | ExprKind::String(..)).then_some(expr.id)
                })
                .unwrap()
        })
        .collect();
    assert_eq!(literals[0], literals[1]);
    assert_eq!(
        checked.view(0).unwrap().expression_type(literals[0]),
        Some(&Type::Int)
    );
    assert_eq!(
        checked.view(1).unwrap().expression_type(literals[1]),
        Some(&Type::String)
    );
    assert!(checked
        .declarations
        .identifier_index_is_consistent(&checked.facts));
}

#[test]
fn cyclic_live_import_and_reexport_are_the_original_mutable_symbol() {
    let sources = [
        "import { read } from \"./reader\"; import { alias } from \"./barrel\"; export int count=1; export int bump(){count+=1;return count;} print(read());print(alias);",
        "import { count, bump } from \"./state\"; export int read(){bump();return count;}",
        "import { count } from \"./state\"; export { count as alias };",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let checked = analyze_modules(
        &programs,
        &graph(&sources, &[&[1, 2], &[0], &[0]], &[1, 2, 0]),
    )
    .unwrap();
    let count = checked
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "count")
        .unwrap();
    let alias = checked.interfaces()[2]
        .exports
        .iter()
        .find(|export| export.external == "alias")
        .unwrap();
    assert_eq!(alias.target, InterfaceTarget::Value(count.id));
    assert_eq!(
        checked.interfaces()[0]
            .imports
            .iter()
            .find(|import| import.local == "alias")
            .unwrap()
            .target,
        InterfaceTarget::Value(count.id)
    );
    assert_eq!(
        checked.interfaces()[1]
            .imports
            .iter()
            .find(|import| import.local == "count")
            .unwrap()
            .target,
        InterfaceTarget::Value(count.id)
    );
    assert!(checked.view(2).unwrap().symbol_is_assigned(count.id));
    assert_eq!(
        checked
            .symbols()
            .iter()
            .filter(|symbol| symbol.name == "count")
            .count(),
        1
    );
    let program = crate::program::from_checked_modules(&programs, &checked).unwrap();
    let javascript = program
        .to_javascript()
        .unwrap()
        .render(crate::js::PrintPolicy {
            mangle_bindings: true,
        })
        .unwrap();
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &javascript])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "2\n2\n");
}

#[test]
fn deferred_cyclic_value_reads_pass_but_eager_and_owner_forward_reads_keep_source_errors() {
    let arena = Bump::new();
    let sources = [
        "import { reader } from \"./b\";export int later=11;print(reader());",
        "import { later } from \"./a\";export func()->int reader=()=>later;",
    ];
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    analyze_modules(&programs, &graph(&sources, &[&[1], &[0]], &[1, 0])).unwrap();
    let eager = [
        sources[0],
        "import { later } from \"./a\";export int reader=later;",
    ];
    let programs: Vec<_> = eager
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let error = analyze_modules(&programs, &graph(&eager, &[&[1], &[0]], &[1, 0])).unwrap_err();
    assert_eq!(error.module, 1);
    assert!(
        error
            .error
            .message
            .contains("cannot eagerly read module binding"),
        "{error}"
    );
    assert_eq!(
        &eager[1][error.error.span.start..error.error.span.end],
        "later"
    );
    let own = ["int read(){return later;}int later=7;print(read());"];
    let programs = vec![parse_source(&arena, own[0]).unwrap()];
    let error = analyze_modules(&programs, &graph(&own, &[&[]], &[0])).unwrap_err();
    assert_eq!(error.module, 0);
    assert!(
        error.error.message.contains("before its declaration"),
        "{error}"
    );
    assert_eq!(
        &own[0][error.error.span.start..error.error.span.end],
        "later"
    );
}

#[test]
fn separate_export_clauses_retain_canonical_identity_without_span_reresolution() {
    let sources = [
        "import { publicCount as imported } from \"./state\"; export { imported as count };",
        "int value=1;export { value as publicCount };",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let checked = analyze_modules(&programs, &graph(&sources, &[&[1], &[]], &[1, 0])).unwrap();
    let original = checked
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "value")
        .unwrap();
    let state_export = checked.interfaces()[1].exports[0];
    let entry_export = checked.interfaces()[0].exports[0];
    assert_eq!(state_export.external, "publicCount");
    assert_eq!(entry_export.external, "count");
    assert_eq!(state_export.target, InterfaceTarget::Value(original.id));
    assert_eq!(entry_export.target, InterfaceTarget::Value(original.id));
    assert_eq!(
        checked.interfaces()[0].imports[0].target,
        InterfaceTarget::Value(original.id)
    );
    assert_eq!(checked.symbol_module(original.id), Some(1));
    assert_ne!(programs[1].exports[0].local.span, original.span);
    // Exports carry their resolved identity independently of source-expression
    // indexes. Lowering should consume it rather than rediscovering an alias.
    assert_eq!(checked.symbols().len(), 1);
}

#[test]
fn equal_foreign_contracts_share_identity_and_conflicts_keep_declaring_source() {
    let sources = [
        "import \"./b\";extern int host(int value);print(host(1));",
        "extern int host(int value);print(host(2));",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let checked = analyze_modules(&programs, &graph(&sources, &[&[1], &[]], &[1, 0])).unwrap();
    assert_eq!(
        checked
            .symbols()
            .iter()
            .find(|symbol| symbol.name == "host")
            .unwrap()
            .origin,
        DeclarationOrigin::Foreign
    );
    assert!(checked
        .symbols()
        .iter()
        .filter(|symbol| symbol.name == "value")
        .all(|symbol| symbol.origin == DeclarationOrigin::Source));
    assert_eq!(
        checked
            .symbols()
            .iter()
            .filter(|symbol| symbol.name == "host")
            .count(),
        1
    );
    assert_eq!(
        checked
            .symbols()
            .iter()
            .filter(|symbol| symbol.name == "value")
            .count(),
        2
    );
    let sources = [sources[0], "extern string host(int value);"];
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let error = analyze_modules(&programs, &graph(&sources, &[&[1], &[]], &[1, 0])).unwrap_err();
    assert_eq!(error.module, 1);
    assert!(
        error.error.message.contains("conflicting extern contracts"),
        "{error}"
    );
    assert_eq!(
        &sources[1][error.error.span.start..error.error.span.end],
        "host"
    );
}

#[test]
fn unseeded_alias_cycles_and_missing_exports_are_different_errors() {
    let arena = Bump::new();
    for (sources, expected) in [
        (
            [
                "import { value } from \"./b\"; export { value };",
                "import { value } from \"./a\"; export { value };",
            ],
            "cyclic module binding",
        ),
        (
            [
                "import { value } from \"./b\";print(value);",
                "import \"./a\";int hidden=1;",
            ],
            "does not export `value`",
        ),
    ] {
        let programs: Vec<_> = sources
            .iter()
            .map(|source| parse_source(&arena, source).unwrap())
            .collect();
        let error =
            analyze_modules(&programs, &graph(&sources, &[&[1], &[0]], &[1, 0])).unwrap_err();
        assert!(error.error.message.contains(expected), "{error}");
    }
}

#[test]
fn module_checking_admits_parameter_defaults_for_caller_and_callee_evaluation() {
    // The compiler evaluates an omitted default at each typed call site
    // and again in the body for an omitting host; checking only records it.
    let arena = Bump::new();
    for source in [
        "int read(int value=1){return value;}print(read());",
        "auto read=(int value=1)=>value;print(read(2));",
    ] {
        let program = parse_source(&arena, source).unwrap();
        analyze_modules(&[program], &graph(&[source], &[&[]], &[0])).unwrap();
    }
}

#[test]
fn unsupported_interfaces_reject_before_publishing_unqualified_types() {
    let arena = Bump::new();
    // Extern classes, async functions and generators are admitted.
    for source in [
        "extern class Box{int value;}",
        "async int load(){return await Task.resolve(1);}",
        "generator int count(){yield 1;}",
    ] {
        let program = parse_source(&arena, source).unwrap();
        analyze_modules(&[program], &graph(&[source], &[&[]], &[0])).unwrap();
    }
    for (source, expected) in [
        (
            "Record<Box[]> values=record{};",
            "nominal or unknown type `Box`",
        ),
        ("export auto value=1;", "inferred export interfaces"),
    ] {
        let program = parse_source(&arena, source).unwrap();
        let error = analyze_modules(&[program], &graph(&[source], &[&[]], &[0])).unwrap_err();
        assert_eq!(error.module, 0);
        assert!(error.error.message.contains(expected), "{source}: {error}");
    }
    // Existing single-source nominal/default support still uses this same checker.
    analyze(
        &parse_source(
            &arena,
            "struct Box{int value;}int read(int value=1){return value;}Box value=Box{read()};",
        )
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn original_graph_order_and_side_effect_only_edges_are_preserved() {
    let sources = [
        "import \"./b\";import \"./c\";print(0);",
        "import \"./c\";print(1);",
        "print(2);",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let mut modules = graph(&sources, &[&[1, 2], &[2], &[]], &[2, 1, 0]);
    let checked = analyze_modules(&programs, &modules).unwrap();
    assert_eq!(checked.initialization_order(), &[2, 1, 0]);
    assert_eq!(checked.interfaces()[0].dependencies, [1, 2]);
    assert!(checked
        .interfaces()
        .iter()
        .all(|interface| interface.imports.is_empty()));
    modules.dependency_order = vec![1, 2, 0];
    assert!(analyze_modules(&programs, &modules)
        .unwrap_err()
        .error
        .message
        .contains("initialization order"));
    modules.dependency_order = vec![2, 1, 0];
    modules.modules[1].dependencies = vec![30];
    assert!(analyze_modules(&programs, &modules)
        .unwrap_err()
        .error
        .message
        .contains("dependency mismatch"));
}

#[test]
fn mutable_reference_module_interfaces_are_private_until_the_entry_exports_them() {
    let sources = [
        "import { bump } from \"./helper\"; int value=1;bump(ref value);print(value);",
        "export void bump(ref int value){value+=1;}",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let checked = analyze_modules(&programs, &graph(&sources, &[&[1], &[]], &[1, 0])).unwrap();
    let helper = &checked.interfaces[1].exports[0];
    let InterfaceTarget::Value(helper_symbol) = helper.target else {
        panic!("value helper");
    };
    let Type::Function(signature) = &checked.declarations.symbols[helper_symbol.0 as usize].ty
    else {
        panic!("helper")
    };
    assert_eq!(
        signature.params[0].passing,
        ParameterPassing::MutableReference
    );
    let exported = [
        "import { bump } from \"./helper\"; export { bump };",
        sources[1],
    ];
    let programs: Vec<_> = exported
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let error = analyze_modules(&programs, &graph(&exported, &[&[1], &[]], &[1, 0])).unwrap_err();
    assert_eq!(error.module, 0);
    assert!(error.error.message.contains("public exports"));
}

#[test]
fn nominal_module_diamond_keeps_original_identity_and_type_only_occurrences() {
    let sources = [
        r#"import {P as Left} from "./left";import {P as Right} from "./right";Left a=Left{3};Right b=a;export {Left as PublicP};export int result=b.x;"#,
        r#"import {P} from "./shape";export {P};"#,
        r#"import {P} from "./shape";export {P};"#,
        "export struct P{int x;}",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let checked = analyze_modules(
        &programs,
        &graph(&sources, &[&[1, 2], &[3], &[3], &[]], &[3, 1, 2, 0]),
    )
    .unwrap();
    let view = checked.view(0).unwrap();
    let declaration = view.struct_type("Left").unwrap();
    assert_eq!(view.struct_type("Right"), Some(declaration));
    assert_eq!(declaration.name, "P");
    assert_eq!(checked.nominal_module(declaration.identity), Some(3));
    assert_eq!(checked.declarations.structs.len(), 1);
    assert_eq!(
        checked.interfaces()[0].exports[0].target,
        InterfaceTarget::Struct(declaration.identity)
    );
    assert_eq!(
        view.export_target(programs[0].exports[0].local.span),
        Some(InterfaceTarget::Struct(declaration.identity))
    );
    for specifier in programs[0]
        .imports
        .iter()
        .flat_map(|import| import.specifiers)
    {
        assert!(view.identifier_symbol(specifier.local.span).is_none());
        assert_eq!(
            view.binding_type(specifier.local.span),
            Some(&Type::Struct(declaration))
        );
    }
    assert!(checked
        .symbols()
        .iter()
        .all(|symbol| !matches!(symbol.name, "P" | "Left" | "Right" | "PublicP")));
    let field = view.nominal_struct(declaration.identity).unwrap().fields["x"].member;
    assert!(
        matches!(view.nominal_member(field),Some(NominalMember::Field { owner, .. }) if owner==declaration.identity)
    );
    assert!(checked
        .source(3)
        .unwrap()
        .same(&programs[3].source_identity()));
}

#[test]
fn nominal_module_private_return_types_do_not_resolve_through_consumer_spelling() {
    let sources = [
        r#"import {makeA} from "./a";import {makeB} from "./b";auto a=makeA();auto b=makeB();print(a.x);print(b.x);"#,
        "struct Pair{int x;}export Pair makeA(){return Pair{1};}",
        "struct Pair{int x;}export Pair makeB(){return Pair{2};}",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let checked = analyze_modules(
        &programs,
        &graph(&sources, &[&[1, 2], &[], &[]], &[1, 2, 0]),
    )
    .unwrap();
    let a = checked.view(1).unwrap().struct_type("Pair").unwrap();
    let b = checked.view(2).unwrap().struct_type("Pair").unwrap();
    assert_eq!(a.name, b.name);
    assert_ne!(a, b);
    assert!(checked.view(0).unwrap().struct_type("Pair").is_none());
    let mut incompatible = sources;
    incompatible[0] =
        r#"import {makeA} from "./a";import {makeB} from "./b";auto a=makeA();a=makeB();"#;
    let programs: Vec<_> = incompatible
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let failure = analyze_modules(
        &programs,
        &graph(&incompatible, &[&[1, 2], &[], &[]], &[1, 2, 0]),
    )
    .unwrap_err();
    assert_eq!(failure.module, 0);
    assert!(
        failure
            .error
            .message
            .contains("distinct struct declarations"),
        "{}",
        failure.error
    );
}

#[test]
fn nominal_module_generic_substitution_preserves_the_imported_declaration() {
    let sources = [
        r#"import {Box as Wrapped} from "./box";Wrapped<int> item=Wrapped{7};int value=item.value;"#,
        "export struct Box<T>{T value;}",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let checked = analyze_modules(&programs, &graph(&sources, &[&[1], &[]], &[1, 0])).unwrap();
    let declaration = checked.view(1).unwrap().struct_type("Box").unwrap();
    let item = checked
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "item")
        .unwrap();
    assert_eq!(
        item.ty,
        Type::StructInstance {
            declaration,
            args: vec![Type::Int]
        }
    );
    let value = checked
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "value")
        .unwrap();
    assert_eq!(value.ty, Type::Int);
    let wrong = [
        r#"import {Box as Wrapped} from "./box";Wrapped<int,string> item=Wrapped{7};"#,
        sources[1],
    ];
    let programs: Vec<_> = wrong
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    assert!(
        analyze_modules(&programs, &graph(&wrong, &[&[1], &[]], &[1, 0]))
            .unwrap_err()
            .error
            .message
            .contains("expects 1 type argument")
    );
}

#[test]
fn nominal_export_occurrences_distinguish_value_type_and_ambiguous_bare_names() {
    for (source, expected_type) in [
        ("struct Item{int x;}export int Item=7;", false),
        ("export struct Item{int x;}int Item=7;", true),
        ("struct Item{int x;}export Item value=Item{7};", false),
        (
            "struct Item{int x;}Item value=Item{7};export {value};",
            false,
        ),
    ] {
        let arena = Bump::new();
        let program = parse_source(&arena, source).unwrap();
        let single = analyze(&program).unwrap();
        let expected = single.export_target(program.exports[0].local.span).unwrap();
        assert_eq!(
            matches!(expected, InterfaceTarget::Struct(_)),
            expected_type,
            "{source}"
        );
        let checked = analyze_modules(&[program], &graph(&[source], &[&[]], &[0])).unwrap();
        assert_eq!(
            matches!(
                checked.interfaces()[0].exports[0].target,
                InterfaceTarget::Struct(_)
            ),
            expected_type,
            "{source}"
        );
    }
    let source = "struct Item{int x;}int Item=7;export {Item};";
    let arena = Bump::new();
    let program = parse_source(&arena, source).unwrap();
    assert!(analyze(&program)
        .unwrap_err()
        .message
        .contains("ambiguous export"));
    assert!(analyze_modules(&[program], &graph(&[source], &[&[]], &[0]))
        .unwrap_err()
        .error
        .message
        .contains("ambiguous export"));
}

#[test]
fn nominal_module_reference_forwarding_uses_canonical_field_storage() {
    let sources = [
        r#"import {Pair as P,replace,forward} from "./pair";P original=P{1,2};P snapshot=original;forward(ref original);replace(ref original);print(snapshot.x);"#,
        "export struct Pair{int x;int y;}void leaf(ref int value){value+=1;}export void forward(ref Pair value){leaf(ref value.x);}export void replace(ref Pair value){value=Pair{8,9};}",
    ];
    let arena = Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| parse_source(&arena, source).unwrap())
        .collect();
    let checked = analyze_modules(&programs, &graph(&sources, &[&[1], &[]], &[1, 0])).unwrap();
    let identity = checked.view(0).unwrap().struct_type("P").unwrap().identity;
    let InterfaceTarget::Value(forward) = checked.interfaces()[1]
        .exports
        .iter()
        .find(|export| export.external == "forward")
        .unwrap()
        .target
    else {
        panic!("forward value")
    };
    let Type::Function(signature) = &checked.symbols()[forward.0 as usize].ty else {
        panic!("forward callable")
    };
    assert_eq!(
        signature.params[0].passing,
        ParameterPassing::MutableReference
    );
    assert!(
        matches!(signature.params[0].ty,Type::Struct(declaration) if declaration.identity==identity)
    );
}
