//! One portable original-module composition. The maintained integrated value/
//! reference module is consumed byte-for-byte; the factory/entry are explicitly
//! authored companions, not claims of native support for the Marked API factory.
use super::*;

const ENTRY: &str = "native-closures/portable-entry.lil";
const FILES: &[(&str, &str)] = &[
    (
        ENTRY,
        include_str!("fixtures/native-closures/portable-entry.lil"),
    ),
    (
        "native-closures/portable-factory.lil",
        include_str!("fixtures/native-closures/portable-factory.lil"),
    ),
    (
        "integrated-architecture/products.lil",
        include_str!("fixtures/integrated-architecture/products.lil"),
    ),
];
const EXPECTED: &str = include_str!("fixtures/native-closures/portable.expected.txt");

#[test]
fn original_product_module_composes_with_retained_factory_callbacks_in_one_program() {
    let directory = ScratchDirectory::new("portable-native-closure-modules");
    for &(name, source) in FILES {
        let path = directory.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
    let modules = crate::module::discover_modules(&directory.0.join(ENTRY)).unwrap();
    assert_eq!(modules.modules.len(), FILES.len());
    let mut source_rows = Vec::new();
    for module in &modules.modules {
        let name = module
            .path
            .strip_prefix(&directory.0)
            .unwrap()
            .to_str()
            .unwrap();
        let expected = FILES.iter().find(|(file, _)| *file == name).unwrap().1;
        assert_eq!(
            module.source, expected,
            "original source bytes changed: {name}"
        );
        source_rows.push(serde_json::json!({"file":name,"source":module.source,"sha256":digest(&module.source),"bytes":module.source.len(),"origin":if name=="integrated-architecture/products.lil" {"unchanged-integrated-source"} else {"authored-portable-companion"}}));
    }
    let arena = bumpalo::Bump::new();
    let syntax = crate::module::parse_modules(&arena, &modules).unwrap();
    let checked = crate::semantic::analyze_modules(&syntax, &modules).unwrap();
    let program = from_checked_modules(&syntax, &checked).unwrap();
    program.verify().unwrap();
    assert_eq!(program.modules.len(), 3);
    assert_eq!(
        program
            .units
            .iter()
            .filter(|unit| unit.data().kind == UnitKind::ModuleInitialization)
            .count(),
        3
    );
    assert!(program.exports().is_empty());
    let product_cell = program
        .cells
        .iter()
        .find(|cell| cell.name == "productScore")
        .unwrap();
    let factory_cell = program
        .cells
        .iter()
        .find(|cell| cell.name == "installCounter")
        .unwrap();
    let CellBinding::Function(product_body) = product_cell.binding else {
        panic!("productScore must resolve to its canonical callable declaration")
    };
    let CellBinding::Function(factory_body) = factory_cell.binding else {
        panic!("installCounter must resolve to its canonical callable declaration")
    };
    let product_module = program.unit(product_body).unwrap().module;
    let factory_module = program.unit(factory_body).unwrap().module;
    assert_ne!(
        product_module, factory_module,
        "no synthetic combined function owner"
    );
    assert!(program.modules[factory_module.index()]
        .imports
        .iter()
        .any(|import| import.module == product_module));
    let mut row = with_host_program(program, 0, MEMORY, |compilation, source, hosts| {
        let row = fixture_artifacts(
            "portable-modules",
            FILES[0].1,
            EXPECTED,
            compilation,
            source,
            hosts,
        );
        assert_eq!(compilation.ledger().work_used(WorkDomain::Optional), 0);
        row
    });
    row["entry"] = serde_json::json!(ENTRY);
    row["source_kind"] = serde_json::json!("original-module-graph");
    row["optional_work"] = serde_json::json!(0);
    row["original_modules"] = serde_json::json!(source_rows);
    row["qualification"] = serde_json::json!("three original files through shared module discovery/checking/Program; unchanged integrated value/ref consumer plus authored retained factory; generated typed separate-TU host; fixed portable trace");
    eprintln!("native-closure-module-artifact {}", row);
}
