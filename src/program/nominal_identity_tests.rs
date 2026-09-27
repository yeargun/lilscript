//! Plan M4.1's exit check: classes, enums and their fields are looked up by
//! identity (`NominalId`, `FieldRef`), never by name, in the program IR, its
//! JavaScript formation, the native plan and lint. Names are display and ABI
//! data. The scan reads the sources with whitespace removed, so a lookup split
//! over several lines is still one match.

const SCANNED: &[&str] = &["src/program", "src/js", "src/lint.rs"];

/// Spellings of a name-keyed nominal lookup. Each one appeared in the code
/// this batch replaced.
const FORBIDDEN: &[&str] = &[
    // A class table searched by spelling.
    "classes.iter().position(",
    "classes.iter().find(",
    ".name==class",
    "class.name==",
    "candidate.name==",
    "definition.name==*name",
    // A base class held or resolved by spelling.
    ".base.as_deref()",
    "base_class_name(",
    // Checker queries keyed by a spelling.
    "class_info(name",
    "class_info(declaration.name",
    "class_info(class.name",
    "enum_info(",
    "struct_info(name",
    "class_method_owner(",
    "is_extern_class(",
    // Types or defaults that name a class or enum by spelling.
    "Type::Class(name)",
    "Type::Enum(name)",
    "ClassInstance{name",
    "NewClass{name",
    "Struct{name,",
    // A class field found by its key.
    ".find(|(field,_)|*field==key)",
    ".position(|(name,_)|*name==key)",
];

fn sources(path: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
    if path.is_dir() {
        for entry in std::fs::read_dir(path).unwrap() {
            sources(&entry.unwrap().path(), found);
        }
    } else if path.extension().is_some_and(|extension| extension == "rs")
        && !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with("_tests.rs") || name == "tests.rs")
    {
        found.push(path.to_owned());
    }
}

#[test]
fn no_name_keyed_nominal_lookup_remains() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for path in SCANNED {
        sources(&root.join(path), &mut files);
    }
    assert!(files.len() > 20, "the scan found the compiler sources");
    let mut hits = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        // Comments may name the old spellings; code may not.
        let code = text
            .lines()
            .map(|line| line.split("//").next().unwrap_or(""))
            .collect::<String>();
        let compact = code
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        for pattern in FORBIDDEN {
            if compact.contains(pattern) {
                hits.push(format!(
                    "{}: {pattern}",
                    file.strip_prefix(root).unwrap().display()
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "name-keyed nominal lookups:\n{}",
        hits.join("\n")
    );
}

fn module_graph(
    sources: &[&str],
    dependencies: &[&[usize]],
    order: &[usize],
) -> crate::module::ModuleSet {
    crate::module::ModuleSet {
        modules: sources
            .iter()
            .zip(dependencies)
            .enumerate()
            .map(|(id, (source, deps))| crate::module::ModuleSource {
                path: format!("/module-{id}.lil").into(),
                source: (*source).into(),
                dependencies: deps.to_vec(),
                foreign_dependencies: Vec::new(),
                dynamic_dependencies: Vec::new(),
                offset: 0,
            })
            .collect(),
        dependency_order: order.to_vec(),
        roots: vec![0],
        root_names: vec!["main".to_string()],
        eager: vec![true; sources.len()],
    }
}

/// Two modules' private `class Node` are two class definitions; every field
/// access is a `FieldRef` place, and every instance allocation names its
/// class.
#[test]
fn same_named_private_classes_are_distinct_definitions_with_identity_places() {
    use super::*;
    let sources = [
        r#"import {left} from "./a";import {right} from "./b";print(left(2));print(right("x"));"#,
        "class Node{int value;init(int value){this.value=value;}int twice(){return this.value*2;}}export int left(int value){Node node=new Node(value);node.value=node.twice();return node.value;}",
        "class Base{string text;}class Node extends Base{int count;init(string text){this.text=text;this.count=1;}}export string right(string text){Node node=new Node(text);return node.text+node.count;}",
    ];
    let arena = bumpalo::Bump::new();
    let programs: Vec<_> = sources
        .iter()
        .map(|source| crate::parser::parse_source(&arena, source).unwrap())
        .collect();
    let checked = crate::check::analyze_modules(
        &programs,
        &module_graph(&sources, &[&[1, 2], &[], &[]], &[1, 2, 0]),
    )
    .unwrap();
    let program = from_checked_modules(&programs, &checked).unwrap();
    program.verify().unwrap();
    let nodes = program
        .classes()
        .iter()
        .filter(|class| class.name == "Node")
        .collect::<Vec<_>>();
    assert_eq!(nodes.len(), 2);
    assert_ne!(nodes[0].identity, nodes[1].identity);
    assert_eq!(
        nodes
            .iter()
            .map(|class| class.module.index())
            .collect::<Vec<_>>(),
        [1, 2]
    );
    let base = program
        .classes()
        .iter()
        .find(|class| class.name == "Base")
        .unwrap();
    assert_eq!(nodes[1].base, Some(base.identity));
    let (mut fields, mut instances) = (0, 0);
    for unit in program.units() {
        let data = unit.data();
        for place in &data.places {
            match place {
                Place::ClassField { field, .. } => {
                    assert!(program.class_field(*field).is_some());
                    fields += 1;
                }
                Place::Member { receiver, .. } => assert!(!matches!(
                    program.ty(data.values[receiver.index()].ty),
                    Some(Type::Class(_) | Type::ClassInstance { .. })
                )),
                _ => {}
            }
        }
        for operation in &data.operations {
            if let OperationKind::Allocate {
                kind: AllocationKind::Instance { class, keys },
                ..
            } = &operation.kind
            {
                let definition = program.class(*class).unwrap();
                assert_eq!(keys.len(), definition.fields.len());
                let Some(Type::Class(declaration)) =
                    program.ty(data.values[operation.result.unwrap().index()].ty)
                else {
                    panic!("an instance allocation is typed by its class");
                };
                assert_eq!(declaration.identity, *class);
                instances += 1;
            }
        }
    }
    assert_eq!(instances, 2);
    assert!(fields >= 6, "{fields}");
    // The inherited field is the base's field, by the base's identity.
    let inherited = program
        .units()
        .iter()
        .flat_map(|unit| unit.data().places.iter())
        .find_map(|place| match place {
            Place::ClassField { field, .. } if field.nominal == base.identity => Some(*field),
            _ => None,
        })
        .unwrap();
    assert_eq!(inherited.slot, 0);
}
