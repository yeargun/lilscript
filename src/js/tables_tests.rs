//! The data-encoding family (M9.8): every alternative of every table shape
//! decodes to the literal's own graph. Each test builds a root table, forms
//! it under each alternative the site offers, runs the printed module in
//! Node and compares the observations (enumeration order, `Object.is`,
//! prototypes, property descriptors) with the literal's own run, which is
//! the oracle: the compiler never writes it.
use super::*;
use std::process::Command;

/// A literal to build: `R` is a null-prototype (`record`) object.
enum Lit {
    N(f64),
    S(&'static str),
    B(bool),
    Null,
    A(Vec<Lit>),
    O(Vec<(&'static str, Lit)>),
    R(Vec<(&'static str, Lit)>),
}

fn build(module: &mut Module, literal: &Lit) -> ExprId {
    let expression = match literal {
        Lit::N(value) if value.is_sign_negative() && *value != 0.0 => {
            let positive = module.expression(Expr::Literal(Literal::Number(-value)), None);
            Expr::Unary {
                op: Unary::Negate,
                value: positive,
            }
        }
        Lit::N(value) => Expr::Literal(Literal::Number(*value)),
        Lit::S(text) => Expr::Literal(Literal::String((*text).into())),
        Lit::B(value) => Expr::Literal(Literal::Bool(*value)),
        Lit::Null => Expr::Literal(Literal::Null),
        Lit::A(items) => Expr::Array(items.iter().map(|item| build(module, item)).collect()),
        Lit::O(entries) | Lit::R(entries) => {
            let mut built = Vec::new();
            if matches!(literal, Lit::R(_)) {
                let null = module.expression(Expr::Literal(Literal::Null), None);
                built.push((Property::Named("__proto__".into()), null));
            }
            for (key, value) in entries {
                let value = build(module, value);
                let key = module.expression(Expr::Literal(Literal::String((*key).into())), None);
                built.push((Property::Computed(key), value));
            }
            Expr::Object(built)
        }
    };
    module.expression(expression, None)
}

/// `let table=<literal>; describe(table)` with pristine builtins.
fn table_module(literal: &Lit) -> (Module, BindingId) {
    let mut module = Module::default();
    module.pristine_builtins = true;
    let table = module.binding(Binding {
        source_symbol: None,
        scope: ScopeId::new(0),
        spelling: "table".into(),
        pinned: false,
        class: None,
        defined: false,
    });
    let value = build(&mut module, literal);
    module.regions[0].statements.push(Statement::Let {
        binding: table,
        value: Some(value),
    });
    let describe = module.expression(Expr::Host("describe".into()), None);
    let read = module.expression(Expr::Binding(table), None);
    let call = module.expression(
        Expr::Call {
            callee: describe,
            arguments: vec![read],
            invocation: Invocation::Value,
        },
        None,
    );
    module.regions[0].statements.push(Statement::Evaluate(call));
    (module, table)
}

/// Every observation a program can make of a data graph.
const DESCRIBE: &str = r#"
function describe(root) {
  const seen = new Set();
  function show(v) {
    if (v === null) return "null";
    if (typeof v === "number") return Object.is(v, -0) ? "-0" : String(v);
    if (typeof v === "string") return JSON.stringify(v);
    if (typeof v !== "object") return typeof v + ":" + String(v);
    if (seen.has(v)) return "<shared>";
    seen.add(v);
    const proto = Object.getPrototypeOf(v);
    const tag = proto === null ? "N" : proto === Object.prototype ? "O"
      : proto === Array.prototype ? "A" : "?";
    const extensible = Object.isExtensible(v) ? "" : "!ext";
    const keys = Reflect.ownKeys(v).map((key) => {
      const d = Object.getOwnPropertyDescriptor(v, key);
      const plain = "value" in d && d.writable && d.configurable
        && (Array.isArray(v) && key === "length" ? !d.enumerable : d.enumerable);
      return String(key) + (plain ? "" : "!") + ":" + show(d.value);
    });
    return tag + extensible + "{" + keys.join(",") + "}";
  }
  console.log(show(root));
}
"#;

fn run(module: &Module) -> String {
    module.verify().unwrap();
    let javascript = module.render(PrintPolicy::default()).unwrap();
    let script = format!("{DESCRIBE}\n{javascript}");
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for data-encoding tests");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Form the table under every alternative its site offers (and under the
/// seed); each must print a program observing exactly the literal's graph.
/// Returns the site and the printed module of each alternative.
fn every_alternative(literal: &Lit) -> (ChoiceSite, Vec<(AltId, String)>) {
    let (module, _) = table_module(literal);
    let oracle = run(&module);
    let mut seeded = module.clone();
    seeded
        .encode_tables(&ChoiceMap::SEEDS, &mut AllocationBudget::new(None))
        .unwrap();
    assert_eq!(seeded.choice_sites.len(), 1, "one site");
    let site = seeded.choice_sites[0].clone();
    assert_eq!(site.applied, site.seed);
    assert_eq!(run(&seeded), oracle, "the seed");
    let mut printed = Vec::new();
    for offered in &site.alternatives {
        let mut formed = module.clone();
        let choices = ChoiceMap::SEEDS.with(site.key, offered.alternative);
        formed
            .encode_tables(&choices, &mut AllocationBudget::new(None))
            .unwrap();
        assert_eq!(formed.choice_sites[0].applied, offered.alternative);
        assert_eq!(run(&formed), oracle, "{}", offered.name);
        printed.push((
            offered.alternative,
            formed.render(PrintPolicy::default()).unwrap(),
        ));
    }
    (site, printed)
}

fn offered(site: &ChoiceSite) -> Vec<&'static str> {
    site.alternatives
        .iter()
        .map(|offered| offered.name)
        .collect()
}

/// Rows of five floats under index keys, as a font-metrics table has them:
/// fractions with five decimals, a −0, an integer, an out-of-order key.
fn metrics() -> Lit {
    let row = |values: [f64; 5]| Lit::A(values.iter().map(|value| Lit::N(*value)).collect());
    let mut fonts = Vec::new();
    for (font, base) in [
        ("Main-Regular", 0.68333),
        ("Main-Bold", 0.68611),
        ("Size1", 0.85),
    ] {
        let mut glyphs = Vec::new();
        for (index, key) in [
            "32", "65", "66", "67", "68", "69", "70", "71", "72", "73", "74", "75", "76", "77",
            "78", "79", "80", "81", "82", "40",
        ]
        .into_iter()
        .enumerate()
        {
            let width = 0.25 + index as f64 * 0.01111;
            glyphs.push((
                key,
                row([
                    if index % 7 == 3 { 0.19444 } else { 0.0 },
                    base,
                    if index == 5 { -0.0 } else { 0.0 },
                    if index % 5 == 0 { 0.05556 } else { 0.0 },
                    (width * 1e5).round() / 1e5,
                ]),
            ));
        }
        fonts.push((font, Lit::O(glyphs)));
    }
    Lit::O(fonts)
}

#[test]
fn a_metrics_table_decodes_exactly_under_every_alternative() {
    let (site, printed) = every_alternative(&metrics());
    assert_eq!(offered(&site), ["literal", "columns", "dictionary"]);
    assert_eq!(site.name, "table");
    let columns = &printed.iter().find(|(id, _)| *id == COLUMNS).unwrap().1;
    // Delta-coded keys, one column per position, five-decimal integers.
    assert!(columns.contains("/1e5"), "{columns}");
    assert!(columns.contains("19444"), "{columns}");
    assert!(!columns.contains(".19444"), "{columns}");
    let dictionary = &printed.iter().find(|(id, _)| *id == DICTIONARY).unwrap().1;
    assert!(dictionary.contains(".19444"), "{dictionary}");
    // The literal prints canonical integer keys bare (L2).
    let literal = &printed.iter().find(|(id, _)| *id == LITERAL).unwrap().1;
    assert!(
        literal.contains("{32:[") && !literal.contains("\"32\""),
        "{literal}"
    );
}

#[test]
fn records_tuples_and_varying_arrays_keep_their_shapes() {
    // Records of one key sequence (a null-prototype one among them), tuples
    // of mixed scalars, arrays of varying length, strings with separators
    // and astral characters.
    let mut rows = Vec::new();
    for index in 0..24 {
        let tags: Vec<Lit> = (0..index % 4)
            .map(|tag| Lit::S(["a|b", "c~d", "𝔄", ""][tag]))
            .collect();
        rows.push(Lit::R(vec![
            (
                "name",
                Lit::S(["alpha", "beta", "gamma|delta", "𝔸"][index % 4]),
            ),
            ("size", Lit::N(index as f64 * 1.5)),
            (
                "pair",
                Lit::A(vec![
                    if index % 3 == 0 {
                        Lit::Null
                    } else {
                        Lit::B(index % 2 == 0)
                    },
                    Lit::N(-(index as f64)),
                ]),
            ),
            ("tags", Lit::A(tags)),
            ("01", Lit::S("kept")),
        ]));
    }
    let (site, printed) = every_alternative(&Lit::A(rows));
    assert!(offered(&site).contains(&"columns"), "{:?}", offered(&site));
    let columns = &printed.iter().find(|(id, _)| *id == COLUMNS).unwrap().1;
    assert!(columns.contains("__proto__:null"), "{columns}");
}

#[test]
fn string_keys_keep_their_order_and_index_keys_enumerate_first() {
    // Keys that look numeric but are not array indices keep insertion
    // order; index keys enumerate first whatever order stored them.
    let keys = [
        "zeta",
        "7",
        "01",
        "-1",
        "4294967295",
        "alpha",
        "3",
        "1.5",
        "4294967294",
        "beta",
        "gamma",
        "delta",
        "epsilon",
        "eta",
        "theta",
        "iota",
        "kappa",
        "lambda",
        "mu",
        "nu",
    ];
    let entries = keys
        .iter()
        .enumerate()
        .map(|(index, key)| {
            (
                *key,
                Lit::O(vec![
                    ("left", Lit::N(index as f64 / 4.0)),
                    ("right", Lit::S(["x", "y"][index % 2])),
                ]),
            )
        })
        .collect();
    let (site, _) = every_alternative(&Lit::O(entries));
    assert!(offered(&site).contains(&"columns"), "{:?}", offered(&site));
}

#[test]
fn values_of_mixed_kinds_stay_literals_in_their_column() {
    // Records of one key sequence whose `extra` field holds objects, arrays,
    // strings and null-prototype records: that column keeps its literals as
    // they are, the others are transposed.
    let mut rows = Vec::new();
    for index in 0..40 {
        let extra = match index % 4 {
            0 => Lit::O(vec![("deep", Lit::A(vec![Lit::N(1.0), Lit::S("x")]))]),
            1 => Lit::A(vec![Lit::N(index as f64), Lit::N(0.5)]),
            2 => Lit::S("plain text value"),
            _ => Lit::R(vec![("inner", Lit::N(-0.0))]),
        };
        rows.push(Lit::O(vec![
            ("identifier", Lit::N(index as f64 * 0.25)),
            ("label", Lit::S(["first", "second", "third"][index % 3])),
            ("extra", extra),
        ]));
    }
    let (site, printed) = every_alternative(&Lit::A(rows));
    assert!(offered(&site).contains(&"columns"), "{:?}", offered(&site));
    let columns = &printed.iter().find(|(id, _)| *id == COLUMNS).unwrap().1;
    assert!(columns.contains("deep:[1,\"x\"]"), "{columns}");
    assert!(columns.contains("identifier:"), "{columns}");
}

#[test]
fn a_flat_string_table_is_front_coded_in_utf16_units() {
    // Sorted keys sharing prefixes, one sharing an astral character (two
    // UTF-16 units) with the one before.
    let mut entries = vec![("7", Lit::S("seven"))];
    let names: Vec<&'static str> = (0..40)
        .map(|index| &*Box::leak(format!("entity{index:02}").into_boxed_str()))
        .collect();
    for (index, name) in names.iter().enumerate() {
        entries.push((*name, Lit::S(["value ü", "𝔄 value", "a~b"][index % 3])));
    }
    entries.push(("𝔄lpha", Lit::S("first")));
    entries.push(("𝔄lpine", Lit::S("second")));
    let (site, printed) = every_alternative(&Lit::O(entries));
    assert_eq!(
        offered(&site),
        ["literal", "front-coded", "columns", "dictionary"]
    );
    let front = &printed.iter().find(|(id, _)| *id == FRONT_CODED).unwrap().1;
    assert!(
        front.contains("4ine"),
        "the astral prefix counts two units: {front}"
    );
}

#[test]
fn tables_that_cannot_be_exact_are_never_sites() {
    let row = |key: &'static str| (key, Lit::A(vec![Lit::N(0.125), Lit::N(0.25)]));
    let mut many: Vec<(&'static str, Lit)> = (0..40)
        .map(|index| row(&*Box::leak(format!("k{index}").into_boxed_str())))
        .collect();
    let encodable = Lit::O(std::mem::take(&mut many));
    let (module, _) = table_module(&encodable);
    let mut formed = module.clone();
    formed
        .encode_tables(&ChoiceMap::SEEDS, &mut AllocationBudget::new(None))
        .unwrap();
    assert_eq!(formed.choice_sites.len(), 1, "the control table is a site");
    let refused = |literal: Lit, protect: bool| {
        let (module, _) = table_module(&literal);
        let mut formed = module.clone();
        // Every literal observed: no pass may rewrite one.
        if protect {
            formed.observed_literals = (0..formed.expressions.len())
                .map(|index| {
                    LiteralAlternative::new(ExprId::new(index), WeakLiteralObservation::Truthy)
                })
                .collect();
        }
        formed
            .encode_tables(&ChoiceMap::SEEDS, &mut AllocationBudget::new(None))
            .unwrap();
        assert!(formed.choice_sites.is_empty());
        assert_eq!(
            formed.render(PrintPolicy::default()).unwrap(),
            module.render(PrintPolicy::default()).unwrap()
        );
    };
    let with = |extra: (&'static str, Lit)| {
        let mut entries: Vec<(&'static str, Lit)> = (0..40)
            .map(|index| row(&*Box::leak(format!("k{index}").into_boxed_str())))
            .collect();
        entries.push(extra);
        Lit::O(entries)
    };
    refused(with(("__proto__", Lit::N(1.0))), false);
    refused(with(("k3", Lit::N(1.0))), false);
    refused(with(("fine", Lit::N(1.0))), true);
    // The decoder is compiler-written code, which takes `String` and `Array`
    // methods to be the originals (R10): the pristine setting does not
    // change the sites.
    let (mut module, _) = table_module(&encodable);
    module.pristine_builtins = false;
    module
        .encode_tables(&ChoiceMap::SEEDS, &mut AllocationBudget::new(None))
        .unwrap();
    assert_eq!(module.choice_sites.len(), 1);
}

#[test]
fn scaling_is_exact_or_absent() {
    assert_eq!(scale(&[0.5, 0.25, 0.68889]), Some(5));
    assert_eq!(scale(&[1.0, 2.0]), None);
    assert_eq!(scale(&[0.1 + 0.2]), None, "not a short decimal");
    assert_eq!(scale(&[-0.0, 0.5]), Some(1));
    assert_eq!(scaled(-0.0, 3).to_bits(), (-0.0f64).to_bits());
    for value in [0.68889, 0.00015, 123.456, 1e-7, -2.5] {
        let scale = scale(&[value]).unwrap();
        assert_eq!(
            (scaled(value, scale) / POWERS[scale]).to_bits(),
            value.to_bits()
        );
    }
}

#[test]
fn the_seed_is_the_largest_estimated_saving() {
    let (module, _) = table_module(&metrics());
    let mut formed = module.clone();
    formed
        .encode_tables(&ChoiceMap::SEEDS, &mut AllocationBudget::new(None))
        .unwrap();
    let site = &formed.choice_sites[0];
    let best = site
        .alternatives
        .iter()
        .max_by_key(|offered| offered.saving)
        .unwrap();
    assert!(best.saving > 0);
    assert_eq!(site.seed, best.alternative);
    // A tiny table saves nothing: not a site, the literal stays.
    let (module, _) = table_module(&Lit::O(vec![("a", Lit::N(0.5)), ("b", Lit::N(0.25))]));
    let mut formed = module.clone();
    formed
        .encode_tables(&ChoiceMap::SEEDS, &mut AllocationBudget::new(None))
        .unwrap();
    assert!(formed.choice_sites.is_empty());
}

#[test]
fn two_tables_of_one_schema_decode_through_one_decoder() {
    // Plan M8.2 A1: a decoder is a function of its schema alone, so formation
    // keys decoders by schema and emits each once. Two tables of one schema,
    // both spelled as columns, share one decoder and each builds its own
    // literal's graph.
    let (mut module, _) = table_module(&metrics());
    let second = module.binding(Binding {
        source_symbol: None,
        scope: ScopeId::new(0),
        spelling: "second".into(),
        pinned: false,
        class: None,
        defined: false,
    });
    let value = build(&mut module, &metrics());
    module.regions[0].statements.push(Statement::Let {
        binding: second,
        value: Some(value),
    });
    let describe = module.expression(Expr::Host("describe".into()), None);
    let read = module.expression(Expr::Binding(second), None);
    let call = module.expression(
        Expr::Call {
            callee: describe,
            arguments: vec![read],
            invocation: Invocation::Value,
        },
        None,
    );
    module.regions[0].statements.push(Statement::Evaluate(call));
    let oracle = run(&module);
    let mut seeded = module.clone();
    seeded
        .encode_tables(&ChoiceMap::SEEDS, &mut AllocationBudget::new(None))
        .unwrap();
    assert_eq!(seeded.choice_sites.len(), 2, "two sites");
    let mut choices = ChoiceMap::SEEDS;
    for site in &seeded.choice_sites {
        let columns = site
            .alternatives
            .iter()
            .find(|offered| offered.name == "columns")
            .expect("columns offered");
        choices = choices.with(site.key, columns.alternative);
    }
    let mut formed = module.clone();
    formed
        .encode_tables(&choices, &mut AllocationBudget::new(None))
        .unwrap();
    let decoders = formed.regions[formed.root.index()]
        .statements
        .iter()
        .filter(|statement| matches!(statement, Statement::Function { .. }))
        .count();
    assert_eq!(decoders, 1, "one decoder for the one schema");
    assert_eq!(run(&formed), oracle);
}


#[test]
fn s4_authored_pool_constrains_array_and_table_encodings() {
    let text="authored-shared-value-with-long-repetition";
    for (literal,packing) in [
        (Lit::A((0..40).map(|_|Lit::S(text)).collect()),true),
        (Lit::A((0..40).map(|i|Lit::A(vec![Lit::S(text),Lit::N(i as f64)])).collect()),false),
    ] {
        let (module,_)=table_module(&literal);
        let expected=run(&module);
        let mut ordinary=module.clone();
        let mut budget=AllocationBudget::new(None);
        if packing {assert!(ordinary.pack_string_arrays(ArrayPacking::Startup,&mut budget).unwrap()>0);}
        else {assert!(ordinary.encode_tables(&ChoiceMap::SEEDS,&mut budget).unwrap()>0);}
        let mut pinned=module;
        pinned.authored_pool.push(text.into());
        assert_eq!(pinned.pack_string_arrays(ArrayPacking::Startup,&mut budget).unwrap(),0);
        assert_eq!(pinned.encode_tables(&ChoiceMap::SEEDS,&mut budget).unwrap(),0);
        assert_eq!(pinned.pool_strings(false,&mut budget).unwrap(),1);
        assert_eq!(pinned.pool_strings(false,&mut budget).unwrap(),0);
        assert_eq!(run(&pinned),expected);
    }
}
