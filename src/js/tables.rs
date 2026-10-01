//! Constant data as a codec-judged choice (plan M9.8; architecture §9 and
//! §12 L2). A root `let t=<literal>` whose value is a tree of constant data
//! (object literals with literal keys, arrays, numbers, strings, booleans,
//! `null`) is a *site* of the data-encoding family. Each site offers:
//!
//! * **literal** — the table as written;
//! * **front-coded** — a flat object of strings as two strings, the keys
//!   front-coded (each key is the number of UTF-16 units it shares with the
//!   one before, one digit, then the rest) and the values joined, read by one
//!   shared decoder:
//!   `function d(k,v,s){k=k.split(s);v=v.split(s);let o={},p="",i=0;
//!   for(let e of k){p=p.slice(0,+e[0])+e.slice(1);o[p]=v[i];i=i+1}return o}`;
//! * **columns** — a schema derived from the table: collections of objects
//!   or arrays become a key column, a count column and the columns of their
//!   items; arrays of one length and objects of one key sequence are
//!   transposed into one column per position; integer keys that are array
//!   indices are delta-coded; a number column whose values are all
//!   `n/10^p` for integers `n` is spelled as those integers; a string column
//!   is one joined string. A decoder specialized to the schema rebuilds it;
//! * **dictionary** — the columns, with repeated leaf values read through
//!   one value table sorted by frequency.
//!
//! **The codec judges** (L8). The estimator is the raw bytes each form
//! saves against the literal; it orders the work and supplies the seed (the
//! best saving above zero, else the literal), never the decision. The
//! artifact's `ChoiceMap` names the alternative formation applies; the
//! terminal stage offers the others, each kept only when the whole artifact
//! shrinks under the requested codec. Raw savings estimate the work order;
//! codec-specific savings need an exact complete-artifact measurement.
//! Historical experiments are retained outside compiler policy sources.
//!
//! **Exactness is the legality.** The decoded graph equals the literal's:
//! the same keys stored in the same order (index keys enumerate first either
//! way, and are the only keys ever reordered), the same values under
//! `Object.is` (−0 and NaN), the same prototypes (`{__proto__:null,…}` stays
//! null-prototype) and plain data properties. A table with a `__proto__` key,
//! a duplicate key, a non-finite number, a lone surrogate or an observed
//! literal is never encoded. The decoder runs once, where the literal was
//! evaluated (module root), and calls only `String` and `Array` methods, which
//! compiler-written code takes to be the originals (R10). Every encoding is
//! decoded here, in Rust, with the decoder's semantics and compared with the
//! literal before it is offered.
//!
//! Prior art: no competitor re-encodes constant data. Closure's
//! `AliasStrings` pools repeated strings; Terser, esbuild and Oxc print data
//! literals as written; none has a codec to judge an encoding.
use super::choices::{
    seed, AltId, ChoiceAlternative, ChoiceFamily, ChoiceKey, ChoiceMap, ChoiceSite, SiteId,
};
use super::print::number_spelling;
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

/// The table as written.
pub const LITERAL: AltId = AltId(0);
/// A flat object of strings, front-coded.
pub const FRONT_CODED: AltId = AltId(1);
/// Schema columns and a specialized decoder.
pub const COLUMNS: AltId = AltId(2);
/// The columns with value dictionaries.
pub const DICTIONARY: AltId = AltId(3);

/// Separators tried in order: printable, never escaped, never a digit.
const SEPARATORS: [char; 10] = ['|', '~', '^', '`', ';', '#', '@', '!', '%', '&'];

/// The powers of ten a double holds exactly: an integer divided by one is a
/// single correctly rounded IEEE division, in JavaScript as in Rust.
const POWERS: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
    1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
];

/// The largest integer a double holds with every smaller one.
const SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

// ---------------------------------------------------------------------------
// The data a literal denotes.

#[derive(Debug)]
struct Node {
    /// The literal's own expression, moved as it is where a column keeps it.
    expr: ExprId,
    value: Value,
}

#[derive(Debug)]
enum Value {
    Number(f64),
    String(String),
    Bool(bool),
    Null,
    Array(Vec<Node>),
    Object {
        null_proto: bool,
        entries: Vec<(String, Node)>,
    },
}

impl Value {
    fn scalar(&self) -> bool {
        !matches!(self, Self::Array(_) | Self::Object { .. })
    }
}

/// A value a decoder builds, compared with the literal's under JavaScript's
/// observations.
#[derive(Debug, Clone)]
enum Decoded {
    Number(f64),
    String(String),
    Bool(bool),
    Null,
    Array(Vec<Decoded>),
    Object {
        null_proto: bool,
        /// Stores in the order the decoder makes them.
        stores: Vec<(String, Decoded)>,
    },
}

impl Decoded {
    fn of(node: &Node) -> Self {
        match &node.value {
            Value::Number(value) => Self::Number(*value),
            Value::String(value) => Self::String(value.clone()),
            Value::Bool(value) => Self::Bool(*value),
            Value::Null => Self::Null,
            Value::Array(items) => Self::Array(items.iter().map(Self::of).collect()),
            Value::Object {
                null_proto,
                entries,
            } => Self::Object {
                null_proto: *null_proto,
                stores: entries
                    .iter()
                    .map(|(key, value)| (key.clone(), Self::of(value)))
                    .collect(),
            },
        }
    }
}

/// An array index: a canonical integer string below 2^32 − 1. Ordinary
/// objects enumerate these first, ascending, whatever order stored them.
fn array_index(key: &str) -> Option<u32> {
    if !simple_number_key(key, false) || key.len() > 10 {
        return None;
    }
    key.parse::<u64>()
        .ok()
        .filter(|&index| index < u64::from(u32::MAX))
        .map(|index| index as u32)
}

/// An object's own keys in enumeration order (OrdinaryOwnPropertyKeys):
/// array indices ascending, then every other key in the order it was first
/// stored; a later store to a key replaces its value, not its place.
fn enumeration(stores: &[(String, Decoded)]) -> Vec<(&str, &Decoded)> {
    let mut indices: Vec<(u32, &str, &Decoded)> = Vec::new();
    let mut others: Vec<(&str, &Decoded)> = Vec::new();
    let mut places: ahash::AHashMap<&str, usize> = ahash::AHashMap::new();
    for (key, value) in stores {
        match places.get(key.as_str()) {
            Some(&place) => match array_index(key) {
                Some(_) => indices[place].2 = value,
                None => others[place].1 = value,
            },
            None => match array_index(key) {
                Some(index) => {
                    places.insert(key, indices.len());
                    indices.push((index, key, value));
                }
                None => {
                    places.insert(key, others.len());
                    others.push((key, value));
                }
            },
        }
    }
    indices.sort_by_key(|entry| entry.0);
    indices
        .into_iter()
        .map(|(_, key, value)| (key, value))
        .chain(others)
        .collect()
}

/// `Object.is` on numbers and strings, and structurally on the graph: the
/// same kind, prototype, enumeration order, keys and values.
fn same(left: &Decoded, right: &Decoded) -> bool {
    match (left, right) {
        (Decoded::Number(a), Decoded::Number(b)) => {
            a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
        }
        (Decoded::String(a), Decoded::String(b)) => a == b,
        (Decoded::Bool(a), Decoded::Bool(b)) => a == b,
        (Decoded::Null, Decoded::Null) => true,
        (Decoded::Array(a), Decoded::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        }
        (
            Decoded::Object {
                null_proto: a_proto,
                stores: a,
            },
            Decoded::Object {
                null_proto: b_proto,
                stores: b,
            },
        ) => {
            let (a, b) = (enumeration(a), enumeration(b));
            a_proto == b_proto
                && a.len() == b.len()
                && a.iter()
                    .zip(&b)
                    .all(|((a_key, a), (b_key, b))| a_key == b_key && same(a, b))
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Printed lengths: the raw estimator.

fn number_length(value: f64) -> usize {
    if value == 0.0 && value.is_sign_negative() {
        2
    } else {
        number_spelling(value).len()
    }
}

fn string_length(text: &str) -> usize {
    2 + text
        .chars()
        .map(|ch| match ch {
            '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
            ch if ch < ' ' => 6,
            ch => ch.len_utf8(),
        })
        .sum::<usize>()
}

fn key_length(key: &str) -> usize {
    if identifier_name(key) || simple_number_key(key, false) {
        key.len()
    } else {
        string_length(key)
    }
}

/// `{__proto__:null,` before a null-prototype literal's first entry.
const NULL_PROTO: usize = 15;

fn literal_length(node: &Node) -> usize {
    match &node.value {
        Value::Number(value) => number_length(*value),
        Value::String(text) => string_length(text),
        Value::Bool(_) => 2,
        Value::Null => 4,
        Value::Array(items) => {
            2 + items.len().saturating_sub(1) + items.iter().map(literal_length).sum::<usize>()
        }
        Value::Object {
            null_proto,
            entries,
        } => {
            2 + if *null_proto { NULL_PROTO } else { 0 }
                + entries.len().saturating_sub(1)
                + entries
                    .iter()
                    .map(|(key, value)| key_length(key) + 1 + literal_length(value))
                    .sum::<usize>()
        }
    }
}

fn list_length(lengths: impl Iterator<Item = usize>) -> usize {
    let mut count = 0usize;
    let mut total = 2usize;
    for length in lengths {
        total += length;
        count += 1;
    }
    total + count.saturating_sub(1)
}

/// A separator no string in `texts` contains.
fn separator<'t>(texts: impl Iterator<Item = &'t str> + Clone) -> Option<char> {
    SEPARATORS
        .into_iter()
        .find(|&separator| texts.clone().all(|text| !text.contains(separator)))
}

// ---------------------------------------------------------------------------
// Front-coded string tables.

/// `d(keys,values,separator)` on a flat object of strings.
struct FrontCoded {
    keys: String,
    values: String,
    separator: char,
}

/// The shared decoder's printed length with one-letter names.
const FRONT_CODED_DECODER: usize = 113;

impl FrontCoded {
    fn plan(root: &Node) -> Option<Self> {
        let Value::Object {
            null_proto: false,
            entries,
        } = &root.value
        else {
            return None;
        };
        if entries.is_empty() {
            return None;
        }
        let mut pairs = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            let Value::String(text) = &value.value else {
                return None;
            };
            pairs.push((key.as_str(), text.as_str()));
        }
        let separator = separator(pairs.iter().flat_map(|(key, value)| [*key, *value]))?;
        let mut keys = String::new();
        let mut values = String::new();
        let mut previous = "";
        for (position, &(key, value)) in pairs.iter().enumerate() {
            if position > 0 {
                keys.push(separator);
                values.push(separator);
            }
            // The decoder slices UTF-16 units: share whole characters, at
            // most nine units, so the digit counts units and never splits a
            // surrogate pair.
            let mut units = 0;
            let mut bytes = 0;
            for (a, b) in previous.chars().zip(key.chars()) {
                if a != b || units + a.len_utf16() > 9 {
                    break;
                }
                units += a.len_utf16();
                bytes += a.len_utf8();
            }
            keys.push(char::from(b'0' + units as u8));
            keys.push_str(&key[bytes..]);
            values.push_str(value);
            previous = key;
        }
        Some(Self {
            keys,
            values,
            separator,
        })
    }

    /// The decoder's own semantics, in UTF-16 units.
    fn decode(&self) -> Option<Decoded> {
        let keys: Vec<&str> = self.keys.split(self.separator).collect();
        let values: Vec<&str> = self.values.split(self.separator).collect();
        let mut prefix: Vec<u16> = Vec::new();
        let mut stores = Vec::with_capacity(keys.len());
        for (index, entry) in keys.iter().enumerate() {
            let units: Vec<u16> = entry.encode_utf16().collect();
            let shared = char::from_u32(u32::from(*units.first()?))?.to_digit(10)? as usize;
            prefix.truncate(shared.min(prefix.len()));
            prefix.extend_from_slice(&units[1..]);
            let key = String::from_utf16(&prefix).ok()?;
            let value = values.get(index).copied()?;
            stores.push((key, Decoded::String(value.to_owned())));
        }
        Some(Decoded::Object {
            null_proto: false,
            stores,
        })
    }

    fn length(&self) -> usize {
        // `d("…","…","|")` and the decoder, charged to each table.
        FRONT_CODED_DECODER + 12 + self.keys.len() + self.values.len()
    }
}

// ---------------------------------------------------------------------------
// Schema columns.

/// Where a value sits in the schema.
#[derive(Debug, Clone, PartialEq)]
enum Shape {
    /// One column entry per row of the enclosing collection.
    Leaf(usize),
    /// Arrays of one length: one shape per position.
    Tuple(Vec<Shape>),
    /// Objects of one key sequence and prototype.
    Record {
        null_proto: bool,
        keys: Vec<String>,
        fields: Vec<Shape>,
    },
    /// Arrays or objects of any size: a nested collection.
    Collection(usize),
}

#[derive(Debug, Clone, PartialEq)]
struct Collection {
    array: bool,
    null_proto: bool,
    /// The count column, read at the parent's row; the root has none.
    counts: Option<usize>,
    /// The key column, for objects.
    keys: Option<usize>,
    /// Every key is an array index: stored ascending, delta-coded.
    delta: bool,
    item: Shape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Numbers,
    Strings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Leaf,
    Count,
    Key,
}

#[derive(Debug, Clone)]
enum Data<'d> {
    Numbers(Vec<f64>),
    Strings(Vec<&'d str>),
    /// Literals kept as they are: mixed scalars, or values of mixed kinds.
    Nodes(Vec<&'d Node>),
}

#[derive(Debug, Clone)]
struct Column<'d> {
    role: Role,
    data: Data<'d>,
    /// Numbers stored as `value × 10^scale`, read as `n/1e<scale>`.
    scale: usize,
    /// Read through the dictionary of this kind.
    dictionary: Option<Kind>,
    /// Strings joined on this separator and split by the decoder.
    split: Option<char>,
}

/// A dictionary of leaf values, most frequent first.
#[derive(Debug, Clone)]
enum Dictionary<'d> {
    Numbers(Vec<f64>),
    Strings(Vec<&'d str>, Option<char>),
}

#[derive(Debug, Clone)]
struct Columns<'d> {
    collections: Vec<Collection>,
    columns: Vec<Column<'d>>,
    numbers: Option<Dictionary<'d>>,
    strings: Option<Dictionary<'d>>,
}

/// Everything a columns decoder is a function of, and nothing else: the
/// collections and their shapes, how each column is read (scale,
/// dictionary, split separator) and which dictionaries exist. The data
/// stays in the call's streams. Two tables of one schema decode through
/// one decoder: formation keys decoders by their complete schema, so equal
/// decoders share code independently of table values or source identity.
#[derive(Debug, Clone, PartialEq)]
struct Schema {
    collections: Vec<Collection>,
    /// Per column: `scale`, `dictionary` and `split`.
    columns: Vec<(usize, Option<Kind>, Option<char>)>,
    numbers: bool,
    /// The strings dictionary's split separator, when there is one.
    strings: Option<Option<char>>,
}

impl Schema {
    /// The column the root loop's bound reads (`Columns::bound`).
    fn bound(&self) -> Option<usize> {
        bound_of(&self.collections)
    }
}

/// The column the root loop's bound reads: the root's keys, or the first
/// column read at its own row.
fn bound_of(collections: &[Collection]) -> Option<usize> {
    let root = &collections[0];
    if let Some(keys) = root.keys {
        return Some(keys);
    }
    fn first(collections: &[Collection], shape: &Shape) -> Option<usize> {
        match shape {
            Shape::Leaf(column) => Some(*column),
            Shape::Tuple(fields) | Shape::Record { fields, .. } => {
                fields.iter().find_map(|field| first(collections, field))
            }
            Shape::Collection(index) => collections[*index].counts,
        }
    }
    first(collections, &root.item)
}

fn number_key(value: f64) -> u64 {
    if value.is_nan() {
        f64::NAN.to_bits()
    } else {
        value.to_bits()
    }
}

impl<'d> Columns<'d> {
    fn plan(root: &'d Node) -> Option<Self> {
        let mut plan = Self {
            collections: Vec::new(),
            columns: Vec::new(),
            numbers: None,
            strings: None,
        };
        let shape = plan.unify(&[root], true);
        debug_assert!(matches!(shape, Shape::Collection(0)));
        plan.fill(&shape, root);
        // The root loop runs over a column read at its own row.
        plan.bound()?;
        for index in 0..plan.columns.len() {
            plan.spell(index);
        }
        Some(plan)
    }

    fn column(&mut self, role: Role, data: Data<'d>) -> usize {
        self.columns.push(Column {
            role,
            data,
            scale: 0,
            dictionary: None,
            split: None,
        });
        self.columns.len() - 1
    }

    fn leaf(&mut self, instances: &[&'d Node]) -> Shape {
        let data = if instances
            .iter()
            .all(|node| matches!(node.value, Value::Number(_)))
        {
            Data::Numbers(Vec::new())
        } else if instances
            .iter()
            .all(|node| matches!(node.value, Value::String(_)))
        {
            Data::Strings(Vec::new())
        } else {
            Data::Nodes(Vec::new())
        };
        Shape::Leaf(self.column(Role::Leaf, data))
    }

    /// The shape every instance at one position shares.
    fn unify(&mut self, instances: &[&'d Node], root: bool) -> Shape {
        if instances.iter().all(|node| node.value.scalar()) {
            return self.leaf(instances);
        }
        if instances
            .iter()
            .all(|node| matches!(node.value, Value::Array(_)))
        {
            let items = |node: &&'d Node| match &node.value {
                Value::Array(items) => items,
                _ => unreachable!(),
            };
            let length = items(&instances[0]).len();
            // A column holds one entry per instance; transposing pays only
            // when there are more instances than positions.
            if !root
                && instances.len() > length
                && instances.iter().all(|node| items(node).len() == length)
            {
                let mut fields = Vec::with_capacity(length);
                for position in 0..length {
                    let column: Vec<&Node> = instances
                        .iter()
                        .map(|node| &items(node)[position])
                        .collect();
                    fields.push(self.unify(&column, false));
                }
                return Shape::Tuple(fields);
            }
            let index = self.collections.len();
            self.collections.push(Collection {
                array: true,
                null_proto: false,
                counts: None,
                keys: None,
                delta: false,
                item: Shape::Tuple(Vec::new()),
            });
            let counts = (!root).then(|| self.column(Role::Count, Data::Numbers(Vec::new())));
            let all: Vec<&Node> = instances.iter().flat_map(|node| items(node)).collect();
            let item = self.unify(&all, false);
            let collection = &mut self.collections[index];
            collection.counts = counts;
            collection.item = item;
            return Shape::Collection(index);
        }
        let prototype = |node: &&'d Node| match &node.value {
            Value::Object { null_proto, .. } => Some(*null_proto),
            _ => None,
        };
        let first = prototype(&instances[0]);
        if first.is_some() && instances.iter().all(|node| prototype(node) == first) {
            let null_proto = first.unwrap();
            let entries = |node: &&'d Node| match &node.value {
                Value::Object { entries, .. } => entries,
                _ => unreachable!(),
            };
            let keys: Vec<&str> = entries(&instances[0])
                .iter()
                .map(|(key, _)| key.as_str())
                .collect();
            if !root
                && instances.len() > keys.len()
                && instances.iter().all(|node| {
                    let entries = entries(node);
                    entries.len() == keys.len()
                        && entries
                            .iter()
                            .zip(&keys)
                            .all(|((key, _), other)| key == other)
                })
            {
                let mut fields = Vec::with_capacity(keys.len());
                for position in 0..keys.len() {
                    let column: Vec<&Node> = instances
                        .iter()
                        .map(|node| &entries(node)[position].1)
                        .collect();
                    fields.push(self.unify(&column, false));
                }
                return Shape::Record {
                    null_proto,
                    keys: keys.into_iter().map(str::to_owned).collect(),
                    fields,
                };
            }
            let delta = instances.iter().all(|node| {
                entries(node)
                    .iter()
                    .all(|(key, _)| array_index(key).is_some())
            });
            let index = self.collections.len();
            self.collections.push(Collection {
                array: false,
                null_proto,
                counts: None,
                keys: None,
                delta,
                item: Shape::Tuple(Vec::new()),
            });
            let counts = (!root).then(|| self.column(Role::Count, Data::Numbers(Vec::new())));
            let keys = self.column(
                Role::Key,
                if delta {
                    Data::Numbers(Vec::new())
                } else {
                    Data::Strings(Vec::new())
                },
            );
            let all: Vec<&Node> = instances
                .iter()
                .flat_map(|node| entries(node).iter().map(|(_, value)| value))
                .collect();
            let item = self.unify(&all, false);
            let collection = &mut self.collections[index];
            collection.counts = counts;
            collection.keys = Some(keys);
            collection.item = item;
            return Shape::Collection(index);
        }
        // Values of mixed kinds stay literals, in a column of their own.
        Shape::Leaf(self.column(Role::Leaf, Data::Nodes(Vec::new())))
    }

    /// Append one instance to its columns, in the order the decoder reads.
    fn fill(&mut self, shape: &Shape, node: &'d Node) {
        match shape {
            Shape::Leaf(column) => match (&mut self.columns[*column].data, &node.value) {
                (Data::Numbers(values), Value::Number(value)) => values.push(*value),
                (Data::Strings(values), Value::String(text)) => values.push(text),
                (Data::Nodes(values), _) => values.push(node),
                _ => unreachable!("a leaf column holds its unified kind"),
            },
            Shape::Tuple(fields) => {
                let Value::Array(items) = &node.value else {
                    unreachable!()
                };
                for (field, item) in fields.iter().zip(items) {
                    self.fill(field, item);
                }
            }
            Shape::Record { fields, .. } => {
                let Value::Object { entries, .. } = &node.value else {
                    unreachable!()
                };
                for (field, (_, item)) in fields.iter().zip(entries) {
                    self.fill(field, item);
                }
            }
            Shape::Collection(index) => {
                let collection = self.collections[*index].clone();
                let count = match &node.value {
                    Value::Array(items) => items.len(),
                    Value::Object { entries, .. } => entries.len(),
                    _ => unreachable!(),
                };
                if let Some(counts) = collection.counts {
                    let Data::Numbers(values) = &mut self.columns[counts].data else {
                        unreachable!()
                    };
                    values.push(count as f64);
                }
                match &node.value {
                    Value::Array(items) => {
                        for item in items {
                            self.fill(&collection.item, item);
                        }
                    }
                    Value::Object { entries, .. } => {
                        let keys = collection.keys.unwrap();
                        let mut order: Vec<&(String, Node)> = entries.iter().collect();
                        if collection.delta {
                            order.sort_by_key(|(key, _)| array_index(key).unwrap());
                        }
                        let mut previous = 0.0;
                        for (key, value) in order {
                            match &mut self.columns[keys].data {
                                Data::Numbers(values) => {
                                    let index = f64::from(array_index(key).unwrap());
                                    values.push(index - previous);
                                    previous = index;
                                }
                                Data::Strings(values) => values.push(key),
                                Data::Nodes(_) => unreachable!(),
                            }
                            self.fill(&collection.item, value);
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    /// The column the root loop's bound reads: its keys, or the first
    /// column read at its own row.
    fn bound(&self) -> Option<usize> {
        bound_of(&self.collections)
    }

    /// The decoder's input: this plan without its data.
    fn schema(&self) -> Schema {
        Schema {
            collections: self.collections.clone(),
            columns: self
                .columns
                .iter()
                .map(|column| (column.scale, column.dictionary, column.split))
                .collect(),
            numbers: self.numbers.is_some(),
            strings: match &self.strings {
                Some(Dictionary::Strings(_, separator)) => Some(*separator),
                Some(Dictionary::Numbers(_)) => Some(None),
                None => None,
            },
        }
    }

    /// Spell one column by the raw estimator: scaled integers for numbers
    /// when shorter, one joined string for strings when shorter.
    fn spell(&mut self, index: usize) {
        let column = &mut self.columns[index];
        match &column.data {
            Data::Numbers(values) if column.role == Role::Leaf => {
                if let Some(scale) = scale(values) {
                    let plain = list_length(values.iter().map(|value| number_length(*value)));
                    let scaled = list_length(
                        values
                            .iter()
                            .map(|value| number_length(scaled(*value, scale))),
                    ) + 1
                        + number_length(POWERS[scale]);
                    if scaled < plain {
                        column.scale = scale;
                    }
                }
            }
            Data::Strings(values) => column.split = split(values),
            _ => {}
        }
    }

    /// The same columns with value dictionaries, where the estimator says
    /// they are shorter; `None` when neither kind's is.
    fn with_dictionaries(&self) -> Option<Self> {
        let mut plan = self.clone();
        let before = plan.length();
        let mut numbers = plan.clone();
        numbers.dictionary_of_numbers();
        let mut strings = plan.clone();
        strings.dictionary_of_strings();
        let mut both = numbers.clone();
        both.dictionary_of_strings();
        let mut best = None;
        for candidate in [numbers, strings, both] {
            if candidate.numbers.is_none() && candidate.strings.is_none() {
                continue;
            }
            let length = candidate.length();
            if length < before && best.as_ref().is_none_or(|(best, _)| length < *best) {
                best = Some((length, candidate));
            }
        }
        plan = best?.1;
        Some(plan)
    }

    fn dictionary_of_numbers(&mut self) {
        let mut counts: Vec<(u64, f64, usize, usize)> = Vec::new();
        let mut positions: ahash::AHashMap<u64, usize> = ahash::AHashMap::new();
        for column in &self.columns {
            let (Role::Leaf, Data::Numbers(values)) = (column.role, &column.data) else {
                continue;
            };
            for &value in values {
                let key = number_key(value);
                let position = *positions.entry(key).or_insert_with(|| {
                    counts.push((key, value, 0, counts.len()));
                    counts.len() - 1
                });
                counts[position].2 += 1;
            }
        }
        if counts.is_empty() {
            return;
        }
        counts.sort_by(|a, b| b.2.cmp(&a.2).then(a.3.cmp(&b.3)));
        let rank: ahash::AHashMap<u64, usize> = counts
            .iter()
            .enumerate()
            .map(|(rank, entry)| (entry.0, rank))
            .collect();
        for column in &mut self.columns {
            if let (Role::Leaf, Data::Numbers(values)) = (column.role, &mut column.data) {
                for value in values.iter_mut() {
                    *value = rank[&number_key(*value)] as f64;
                }
                column.dictionary = Some(Kind::Numbers);
                column.scale = 0;
            }
        }
        self.numbers = Some(Dictionary::Numbers(
            counts.into_iter().map(|entry| entry.1).collect(),
        ));
    }

    fn dictionary_of_strings(&mut self) {
        let mut counts: Vec<(&'d str, usize, usize)> = Vec::new();
        let mut positions: ahash::AHashMap<&'d str, usize> = ahash::AHashMap::new();
        for column in &self.columns {
            let (Role::Leaf, Data::Strings(values)) = (column.role, &column.data) else {
                continue;
            };
            for &value in values {
                let position = *positions.entry(value).or_insert_with(|| {
                    counts.push((value, 0, counts.len()));
                    counts.len() - 1
                });
                counts[position].1 += 1;
            }
        }
        if counts.is_empty() {
            return;
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
        let rank: ahash::AHashMap<&str, usize> = counts
            .iter()
            .enumerate()
            .map(|(rank, entry)| (entry.0, rank))
            .collect();
        let mut indices = Vec::new();
        for (index, column) in self.columns.iter().enumerate() {
            if let (Role::Leaf, Data::Strings(values)) = (column.role, &column.data) {
                indices.push((
                    index,
                    values
                        .iter()
                        .map(|value| rank[value] as f64)
                        .collect::<Vec<_>>(),
                ));
            }
        }
        for (index, values) in indices {
            let column = &mut self.columns[index];
            column.data = Data::Numbers(values);
            column.dictionary = Some(Kind::Strings);
            column.split = None;
        }
        let values: Vec<&'d str> = counts.into_iter().map(|entry| entry.0).collect();
        let split = split(&values);
        self.strings = Some(Dictionary::Strings(values, split));
    }

    /// The raw estimate of the call: its streams and the decoder.
    fn length(&self) -> usize {
        let streams: usize = self
            .columns
            .iter()
            .map(|column| stream_length(column))
            .chain(self.numbers.iter().map(dictionary_length))
            .chain(self.strings.iter().map(dictionary_length))
            .sum();
        let parameters = self.columns.len()
            + usize::from(self.numbers.is_some())
            + usize::from(self.strings.is_some());
        // `d(…)` with its commas, then `function d(…){` and `return o}`.
        let call = 3 + parameters.saturating_sub(1);
        let mut decoder = 12 + 2 * parameters + 9;
        decoder += self
            .columns
            .iter()
            .filter(|column| column.split.is_some())
            .count()
            * 15;
        decoder += [&self.numbers, &self.strings]
            .into_iter()
            .filter(|dictionary| matches!(dictionary, Some(Dictionary::Strings(_, Some(_)))))
            .count()
            * 15;
        // `let a=0,b=0;`
        decoder += 4 + 4 * self.collections.len();
        decoder += self.shape_length(&Shape::Collection(0), true);
        streams + call + decoder
    }

    fn shape_length(&self, shape: &Shape, root: bool) -> usize {
        match shape {
            Shape::Leaf(column) => {
                let column = &self.columns[*column];
                4 + if column.dictionary.is_some() {
                    3
                } else if column.scale > 0 {
                    1 + number_length(POWERS[column.scale])
                } else {
                    0
                }
            }
            Shape::Tuple(fields) => {
                list_length(fields.iter().map(|field| self.shape_length(field, false)))
            }
            Shape::Record {
                null_proto,
                keys,
                fields,
            } => {
                list_length(
                    keys.iter()
                        .zip(fields)
                        .map(|(key, field)| key_length(key) + 1 + self.shape_length(field, false)),
                ) + if *null_proto { NULL_PROTO } else { 0 }
            }
            Shape::Collection(index) => {
                let collection = &self.collections[*index];
                // `let m={}` and the loop with its braces and update.
                let mut length = 9 + 16 + if collection.null_proto { NULL_PROTO } else { 0 };
                if !root {
                    // `,e=b+C[a]` and the bound `b<e`, and the value `m`.
                    length += 10 + 1;
                }
                if collection.delta {
                    length += 4 + 7;
                }
                // `m[K[b]]=…` or `m.push(…)`.
                length += 9 + self.shape_length(&collection.item, false);
                length
            }
        }
    }

    /// Run the decoder's semantics on the streams: the graph it builds.
    fn decode(&self) -> Option<Decoded> {
        // The streams as the decoder receives them, split where it splits.
        let mut streams: Vec<Stream<'_>> = Vec::with_capacity(self.columns.len());
        for column in &self.columns {
            streams.push(match (&column.data, column.split) {
                // The stream holds what `Emit::stream` prints.
                (Data::Numbers(values), _) => Stream::Numbers(
                    values
                        .iter()
                        .map(|value| scaled_or_plain(*value, column.scale))
                        .collect(),
                ),
                (Data::Strings(values), Some(separator)) => {
                    let joined = values.join(&separator.to_string());
                    Stream::Strings(joined.split(separator).map(str::to_owned).collect())
                }
                (Data::Strings(values), None) => {
                    Stream::Strings(values.iter().map(|value| (*value).to_owned()).collect())
                }
                (Data::Nodes(values), _) => Stream::Nodes(values.clone()),
            });
        }
        let strings = match &self.strings {
            Some(Dictionary::Strings(values, Some(separator))) => Some(
                values
                    .join(&separator.to_string())
                    .split(*separator)
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
            ),
            Some(Dictionary::Strings(values, None)) => {
                Some(values.iter().map(|value| (*value).to_owned()).collect())
            }
            _ => None,
        };
        let numbers = match &self.numbers {
            Some(Dictionary::Numbers(values)) => Some(values.clone()),
            _ => None,
        };
        let mut decoder = Simulation {
            plan: self,
            streams,
            numbers,
            strings,
            cursors: vec![0; self.collections.len()],
        };
        decoder.collection(0, None)
    }
}

/// The smallest `p` with every value exactly `n/10^p` for a safe integer
/// `n`; `None` when the values are already integers or no `p` exists.
fn scale(values: &[f64]) -> Option<usize> {
    if values.is_empty() || values.iter().all(|value| value.fract() == 0.0) {
        return None;
    }
    (1..POWERS.len()).find(|&scale| {
        values.iter().all(|&value| {
            let scaled = scaled(value, scale);
            scaled.fract() == 0.0
                && scaled.abs() <= SAFE_INTEGER
                && (scaled / POWERS[scale]).to_bits() == value.to_bits()
        })
    })
}

/// `value × 10^scale`, rounded to the integer it must be; −0 stays −0.
fn scaled(value: f64, scale: usize) -> f64 {
    let product = (value * POWERS[scale]).round();
    if product == 0.0 && value.is_sign_negative() {
        -0.0
    } else {
        product
    }
}

/// One joined string when shorter than the array of strings and some
/// separator is in none of them; never for an empty column, which a split
/// would read as one empty string.
fn split(values: &[&str]) -> Option<char> {
    if values.is_empty() {
        return None;
    }
    let separator = separator(values.iter().copied())?;
    let joined = 2 + values.iter().map(|value| value.len()).sum::<usize>() + values.len() - 1 + 15;
    let listed = list_length(values.iter().map(|value| string_length(value)));
    (joined < listed).then_some(separator)
}

fn stream_length(column: &Column<'_>) -> usize {
    match &column.data {
        Data::Numbers(values) => list_length(
            values
                .iter()
                .map(|value| number_length(scaled_or_plain(*value, column.scale))),
        ),
        Data::Strings(values) => match column.split {
            Some(_) => 2 + values.iter().map(|value| value.len()).sum::<usize>() + values.len() - 1,
            None => list_length(values.iter().map(|value| string_length(value))),
        },
        Data::Nodes(values) => list_length(values.iter().map(|node| literal_length(node))),
    }
}

fn dictionary_length(dictionary: &Dictionary<'_>) -> usize {
    match dictionary {
        Dictionary::Numbers(values) => {
            list_length(values.iter().map(|value| number_length(*value)))
        }
        Dictionary::Strings(values, Some(_)) => {
            2 + values.iter().map(|value| value.len()).sum::<usize>() + values.len() - 1
        }
        Dictionary::Strings(values, None) => {
            list_length(values.iter().map(|value| string_length(value)))
        }
    }
}

fn scaled_or_plain(value: f64, scale: usize) -> f64 {
    if scale == 0 {
        value
    } else {
        scaled(value, scale)
    }
}

enum Stream<'d> {
    Numbers(Vec<f64>),
    Strings(Vec<String>),
    Nodes(Vec<&'d Node>),
}

impl Stream<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Numbers(values) => values.len(),
            Self::Strings(values) => values.len(),
            Self::Nodes(values) => values.len(),
        }
    }
}

/// The specialized decoder, run in Rust.
struct Simulation<'p, 'd> {
    plan: &'p Columns<'d>,
    streams: Vec<Stream<'d>>,
    numbers: Option<Vec<f64>>,
    strings: Option<Vec<String>>,
    cursors: Vec<usize>,
}

impl Simulation<'_, '_> {
    fn number(&self, column: usize, row: usize) -> Option<f64> {
        match &self.streams[column] {
            Stream::Numbers(values) => values.get(row).copied(),
            _ => None,
        }
    }

    fn collection(&mut self, index: usize, parent: Option<usize>) -> Option<Decoded> {
        let collection = &self.plan.collections[index];
        // `for(let e=b+C[a];b<e;b++)`, or the root's `a<K.length`.
        let end = match (collection.counts, parent) {
            (Some(counts), Some(parent)) => {
                let count = self.number(counts, parent)?;
                (count >= 0.0 && count.fract() == 0.0)
                    .then(|| self.cursors[index] + count as usize)?
            }
            (None, None) => self.streams[self.plan.bound()?].len(),
            _ => return None,
        };
        let mut items = Vec::new();
        let mut stores = Vec::new();
        let mut key = 0.0;
        while self.cursors[index] < end {
            let row = self.cursors[index];
            let name = match collection.keys {
                None => None,
                Some(keys) if collection.delta => {
                    key += self.number(keys, row)?;
                    // A number as a property key is its ToString: these
                    // are integers below 2^32.
                    (key.fract() == 0.0 && (0.0..4_294_967_295.0).contains(&key))
                        .then(|| (key as u64).to_string())
                }
                Some(keys) => match &self.streams[keys] {
                    Stream::Strings(values) => values.get(row).cloned(),
                    _ => None,
                },
            };
            let value = self.value(&collection.item, row)?;
            match name {
                Some(name) => stores.push((name, value)),
                None if collection.array => items.push(value),
                None => return None,
            }
            self.cursors[index] += 1;
        }
        Some(if collection.array {
            Decoded::Array(items)
        } else {
            Decoded::Object {
                null_proto: collection.null_proto,
                stores,
            }
        })
    }

    fn value(&mut self, shape: &Shape, row: usize) -> Option<Decoded> {
        Some(match shape {
            Shape::Leaf(index) => {
                let column = &self.plan.columns[*index];
                match &self.streams[*index] {
                    Stream::Numbers(values) => {
                        let value = *values.get(row)?;
                        match column.dictionary {
                            None if column.scale == 0 => Decoded::Number(value),
                            None => Decoded::Number(value / POWERS[column.scale]),
                            Some(Kind::Numbers) => {
                                Decoded::Number(*self.numbers.as_ref()?.get(index_of(value)?)?)
                            }
                            Some(Kind::Strings) => Decoded::String(
                                self.strings.as_ref()?.get(index_of(value)?)?.clone(),
                            ),
                        }
                    }
                    Stream::Strings(values) => Decoded::String(values.get(row)?.clone()),
                    Stream::Nodes(values) => Decoded::of(values.get(row)?),
                }
            }
            Shape::Tuple(fields) => {
                let mut items = Vec::with_capacity(fields.len());
                for field in fields {
                    items.push(self.value(field, row)?);
                }
                Decoded::Array(items)
            }
            Shape::Record {
                null_proto,
                keys,
                fields,
            } => {
                let mut stores = Vec::with_capacity(fields.len());
                for (key, field) in keys.iter().zip(fields) {
                    stores.push((key.clone(), self.value(field, row)?));
                }
                Decoded::Object {
                    null_proto: *null_proto,
                    stores,
                }
            }
            Shape::Collection(index) => self.collection(*index, Some(row))?,
        })
    }
}

/// A dictionary index read from a stream.
fn index_of(value: f64) -> Option<usize> {
    (value >= 0.0 && value.fract() == 0.0 && value <= SAFE_INTEGER).then_some(value as usize)
}

// ---------------------------------------------------------------------------
// The family on the tree.

/// One table's chosen encoding, before it is applied.
enum Encoding<'d> {
    FrontCoded(FrontCoded),
    Columns(Columns<'d>),
}

impl Module {
    /// The constant data a literal denotes, if it is only constant data
    /// with no observed literal, no `__proto__` or duplicate key, no lone
    /// surrogate and no non-finite number.
    fn datum(&self, expr: ExprId, depth: usize, nodes: &mut u64) -> Option<Node> {
        if depth > MAX_NESTING || self.observed(expr) {
            return None;
        }
        *nodes += 1;
        let number = |value: f64| value.is_finite().then_some(Value::Number(value));
        let value = match &self.expressions[expr.index()] {
            Expr::Literal(Literal::Number(value)) => number(*value)?,
            Expr::Unary {
                op: Unary::Negate,
                value,
            } if !self.observed(*value) => match &self.expressions[value.index()] {
                Expr::Literal(Literal::Number(value)) => number(-*value)?,
                _ => return None,
            },
            Expr::Literal(Literal::String(text)) => Value::String(text.as_unicode()?.to_owned()),
            Expr::Literal(Literal::Bool(value)) => Value::Bool(*value),
            Expr::Literal(Literal::Null) => Value::Null,
            Expr::Array(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.datum(*item, depth + 1, nodes)?);
                }
                Value::Array(values)
            }
            Expr::Object(entries) => {
                let mut null_proto = false;
                let mut values = Vec::with_capacity(entries.len());
                let mut seen = ahash::AHashSet::with_capacity(entries.len());
                for (position, (key, value)) in entries.iter().enumerate() {
                    let key = match key {
                        // `__proto__:null` first makes a null-prototype
                        // literal (a `record`); anything else is refused.
                        Property::Named(name) if name == "__proto__" => {
                            if position != 0
                                || !matches!(
                                    self.expressions[value.index()],
                                    Expr::Literal(Literal::Null)
                                )
                            {
                                return None;
                            }
                            null_proto = true;
                            continue;
                        }
                        Property::Named(name) => name.as_str(),
                        Property::Computed(key) => {
                            if self.observed(*key) {
                                return None;
                            }
                            match &self.expressions[key.index()] {
                                Expr::Literal(Literal::String(text)) => text.as_unicode()?,
                                _ => return None,
                            }
                        }
                    };
                    if key == "__proto__" || !seen.insert(key) {
                        return None;
                    }
                    let value = self.datum(*value, depth + 1, nodes)?;
                    values.push((key.to_owned(), value));
                }
                Value::Object {
                    null_proto,
                    entries: values,
                }
            }
            _ => return None,
        };
        Some(Node { expr, value })
    }

    /// The data-encoding family (M9.8) on the root's constant tables: every
    /// site with an alternative the estimator says saves bytes is recorded
    /// in `choice_sites`, once, and takes the alternative `choices` names,
    /// or its seed. A table kept as its literal is found again by the
    /// scheduler's next round: its site is already recorded. Returns how
    /// many tables are encoded.
    pub(crate) fn encode_tables(
        &mut self,
        choices: &ChoiceMap,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let root = self.root.index();
        let mut sites: Vec<(usize, BindingId, Node)> = Vec::new();
        for (index, statement) in self.regions[root].statements.iter().enumerate() {
            budget.work(Analysis, 1)?;
            let Statement::Let {
                binding,
                value: Some(value),
            } = *statement
            else {
                continue;
            };
            if !matches!(
                self.expressions[value.index()],
                Expr::Array(_) | Expr::Object(_)
            ) {
                continue;
            }
            if self.contains_pooled_string(value, budget)? { continue; }
            let mut nodes = 0;
            let node = self.datum(value, 0, &mut nodes);
            budget.work(Analysis, nodes)?;
            let Some(node) = node else {
                continue;
            };
            let worth = match &node.value {
                // A flat array of strings is the string-pooling family's
                // packed array.
                Value::Array(items) => {
                    !items.is_empty()
                        && !items
                            .iter()
                            .all(|item| matches!(item.value, Value::String(_)))
                }
                Value::Object { entries, .. } => !entries.is_empty(),
                _ => false,
            };
            if worth {
                sites.push((index, binding, node));
            }
        }
        let mut encodings = Vec::new();
        for (index, binding, node) in &sites {
            let literal = literal_length(node) as i64;
            // Planning and verifying every alternative is linear in the
            // table, a few passes each.
            budget.work(Analysis, 8 * literal as u64)?;
            let original = Decoded::of(node);
            let mut alternatives = vec![ChoiceAlternative {
                alternative: LITERAL,
                name: "literal",
                saving: 0,
            }];
            let mut plans = Vec::new();
            if let Some(front) = FrontCoded::plan(node) {
                let exact = front
                    .decode()
                    .is_some_and(|decoded| same(&decoded, &original));
                debug_assert!(exact, "a front-coded table decodes to its literal");
                if exact {
                    alternatives.push(ChoiceAlternative {
                        alternative: FRONT_CODED,
                        name: "front-coded",
                        saving: literal - front.length() as i64,
                    });
                    plans.push((FRONT_CODED, Encoding::FrontCoded(front)));
                }
            }
            if let Some(columns) = Columns::plan(node) {
                let dictionary = columns.with_dictionaries();
                for (alternative, name, plan) in [
                    (COLUMNS, "columns", Some(columns)),
                    (DICTIONARY, "dictionary", dictionary),
                ] {
                    let Some(plan) = plan else {
                        continue;
                    };
                    let exact = plan
                        .decode()
                        .is_some_and(|decoded| same(&decoded, &original));
                    debug_assert!(exact, "a {name} table decodes to its literal");
                    if exact {
                        alternatives.push(ChoiceAlternative {
                            alternative,
                            name,
                            saving: literal - plan.length() as i64,
                        });
                        plans.push((alternative, Encoding::Columns(plan)));
                    }
                }
            }
            if alternatives
                .iter()
                .all(|alternative| alternative.saving <= 0)
            {
                continue;
            }
            let key = ChoiceKey {
                family: ChoiceFamily::DataEncoding,
                site: match self.bindings[binding.index()].source_symbol {
                    Some(symbol) => SiteId::Symbol(symbol.0),
                    None => SiteId::Formed(binding.index() as u32),
                },
            };
            if self.choice_sites.iter().any(|site| site.key == key) {
                continue;
            }
            let seed = seed(&alternatives);
            let applied = choices
                .get(key)
                .filter(|chosen| {
                    alternatives
                        .iter()
                        .any(|offered| offered.alternative == *chosen)
                })
                .unwrap_or(seed);
            self.choice_sites.push(ChoiceSite {
                key,
                name: self.bindings[binding.index()].spelling.clone(),
                alternatives,
                seed,
                applied,
            });
            if let Some((_, encoding)) = plans
                .into_iter()
                .find(|(alternative, _)| *alternative == applied)
            {
                encodings.push((*index, encoding));
            }
        }
        if encodings.is_empty() {
            return Ok(0);
        }
        let count = encodings.len();
        let mut functions = Vec::new();
        let mut shared = None;
        // One decoder per schema: a table whose schema an earlier table has
        // calls that table's decoder.
        let mut decoders: Vec<(Schema, BindingId)> = Vec::new();
        for (index, encoding) in encodings {
            let call = match encoding {
                Encoding::FrontCoded(front) => {
                    let decoder = match shared {
                        Some(decoder) => decoder,
                        None => {
                            let (decoder, function) = self.table_decoder(budget)?;
                            functions.push(function);
                            shared = Some(decoder);
                            decoder
                        }
                    };
                    let mut emit = Emit {
                        module: self,
                        budget,
                    };
                    let keys = emit.string(&front.keys)?;
                    let values = emit.string(&front.values)?;
                    let separator = emit.string(&front.separator.to_string())?;
                    let callee = emit.read(decoder)?;
                    emit.node(Expr::Call {
                        callee,
                        arguments: vec![keys, values, separator],
                        invocation: Invocation::Value,
                    })?
                }
                Encoding::Columns(columns) => {
                    let schema = columns.schema();
                    budget.work(Analysis, decoders.len() as u64 + 1)?;
                    let decoder = match decoders.iter().find(|(known, _)| *known == schema) {
                        Some(&(_, decoder)) => decoder,
                        None => {
                            let (decoder, function) = self.columns_decoder(&schema, budget)?;
                            functions.push(function);
                            decoders.push((schema, decoder));
                            decoder
                        }
                    };
                    self.columns_call(decoder, &columns, budget)?
                }
            };
            if let Statement::Let { value, .. } = &mut self.statements_mut(root)[index] {
                *value = Some(call);
            }
        }
        // The decoders are declared ahead of the root, in its first
        // statement's module, as the string tables always were.
        budget.reserve_vec(
            AllocationClass::Retained,
            &mut self.regions[root].statements,
            functions.len(),
        )?;
        // Decoders are a rule's definitions, beside the first module
        // (design §6).
        let first = self.root_rows.first().map_or(0, |row| row.module);
        if !self.root_rows.is_empty() {
            budget.reserve_vec(
                AllocationClass::Retained,
                &mut self.root_rows,
                functions.len(),
            )?;
        }
        let rows = functions.len();
        self.prepend_roots(
            functions,
            std::iter::repeat_n(RootRow::synthetic(first), rows),
        );
        Ok(count)
    }

    /// The shared front-coded decoder: its binding and declaration.
    fn table_decoder(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(BindingId, Statement), AllocationError> {
        let root_scope = self.regions[self.root.index()].scope;
        let binding = |module: &mut Self,
                       scope: ScopeId,
                       spelling: &str,
                       budget: &mut AllocationBudget<'_>| {
            module.binding_in(
                Binding {
                    source_symbol: None,
                    scope,
                    spelling: spelling.into(),
                    pinned: false,
                    class: None,
                    defined: false,
                },
                budget,
            )
        };
        let decoder = binding(self, root_scope, "decode", budget)?;
        let body = self.region_in(root_scope, budget)?;
        let scope = self.regions[body.index()].scope;
        let keys = binding(self, scope, "keys", budget)?;
        let values = binding(self, scope, "values", budget)?;
        let separator = binding(self, scope, "separator", budget)?;
        let object = binding(self, scope, "table", budget)?;
        let prefix = binding(self, scope, "key", budget)?;
        let index = binding(self, scope, "index", budget)?;
        let mut emit = Emit {
            module: self,
            budget,
        };
        emit.numeric(index)?;
        // k=k.split(s); v=v.split(s)
        let split_keys = emit.split(keys, separator)?;
        let split_values = emit.split(values, separator)?;
        let empty_object = emit.node(Expr::Object(vec![]))?;
        let empty_string = emit.string("")?;
        let zero = emit.number(0.0)?;
        // for(let e of k){…}
        let (loop_body, loop_scope) = emit.region(scope)?;
        let entry = emit.binding(loop_scope, "entry")?;
        // p=p.slice(0,+e[0])+e.slice(1)
        let head = {
            let object = emit.read(prefix)?;
            let start = emit.number(0.0)?;
            let digit = emit.read(entry)?;
            let first = emit.number(0.0)?;
            let digit = emit.at(digit, first)?;
            let length = emit.node(Expr::Unary {
                op: Unary::Plus,
                value: digit,
            })?;
            emit.method(object, "slice", vec![start, length])?
        };
        let tail = {
            let object = emit.read(entry)?;
            let one = emit.number(1.0)?;
            emit.method(object, "slice", vec![one])?
        };
        let joined = emit.binary(Binary::Add, head, tail)?;
        let prefix_target = emit.read(prefix)?;
        let rebuild = emit.assign(prefix_target, joined)?;
        // o[p]=v[i]
        let store = {
            let table = emit.read(object)?;
            let key = emit.read(prefix)?;
            let target = emit.at(table, key)?;
            let value = emit.cell(values, index)?;
            emit.assign(target, value)?
        };
        // i=i+1
        let step = emit.step(index)?;
        emit.module.regions[loop_body.index()].statements = vec![
            Statement::Evaluate(rebuild),
            Statement::Evaluate(store),
            Statement::Evaluate(step),
        ];
        let iterable = emit.read(keys)?;
        let result = emit.read(object)?;
        emit.module.regions[body.index()].statements = vec![
            split_keys,
            split_values,
            Statement::Let {
                binding: object,
                value: Some(empty_object),
            },
            Statement::Let {
                binding: prefix,
                value: Some(empty_string),
            },
            Statement::Let {
                binding: index,
                value: Some(zero),
            },
            Statement::ForOf {
                binding: entry,
                iterable,
                body: loop_body,
            },
            Statement::Return(Some(result)),
        ];
        let function = emit.function(vec![keys, values, separator], body)?;
        Ok((
            decoder,
            Statement::Function {
                binding: decoder,
                function,
            },
        ))
    }

    /// The decoder specialized to one schema, a function of the schema
    /// alone: its binding and declaration. `function d(K,C,I,x,…){
    /// K=K.split("|");let a=0,b=0,o={};for(;a<K.length;a=a+1){let m={},k=0,
    /// e=b+C[a];for(;b<e;b=b+1)m[k=k+I[b]]=[x[b]/1e5,…];o[K[a]]=m}return o}`.
    fn columns_decoder(
        &mut self,
        plan: &Schema,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(BindingId, Statement), AllocationError> {
        let root_scope = self.regions[self.root.index()].scope;
        let mut emit = Emit {
            module: self,
            budget,
        };
        let decoder = emit.binding(root_scope, "decode")?;
        let (body, scope) = emit.region(root_scope)?;
        let mut columns = Vec::with_capacity(plan.columns.len());
        for _ in &plan.columns {
            columns.push(emit.binding(scope, "column")?);
        }
        let numbers = match plan.numbers {
            true => Some(emit.binding(scope, "numbers")?),
            false => None,
        };
        let strings = match plan.strings {
            Some(_) => Some(emit.binding(scope, "strings")?),
            None => None,
        };
        let mut cursors = Vec::with_capacity(plan.collections.len());
        for _ in &plan.collections {
            let cursor = emit.binding(scope, "row")?;
            emit.numeric(cursor)?;
            cursors.push(cursor);
        }
        let mut statements = Vec::new();
        for (&(_, _, split), &binding) in plan.columns.iter().zip(&columns) {
            if let Some(separator) = split {
                statements.push(emit.split_on(binding, separator)?);
            }
        }
        if let (Some(Some(separator)), Some(binding)) = (plan.strings, strings) {
            statements.push(emit.split_on(binding, separator)?);
        }
        for &cursor in &cursors {
            let zero = emit.number(0.0)?;
            statements.push(Statement::Let {
                binding: cursor,
                value: Some(zero),
            });
        }
        let context = Context {
            plan,
            columns: &columns,
            numbers,
            strings,
            cursors: &cursors,
        };
        let result = emit.collection(&context, 0, None, scope, &mut statements)?;
        let value = emit.read(result)?;
        statements.push(Statement::Return(Some(value)));
        emit.module.regions[body.index()].statements = statements;
        let mut parameters = columns.clone();
        parameters.extend(numbers);
        parameters.extend(strings);
        let function = emit.function(parameters, body)?;
        Ok((
            decoder,
            Statement::Function {
                binding: decoder,
                function,
            },
        ))
    }

    /// One table's call of its schema's decoder, over the table's streams.
    fn columns_call(
        &mut self,
        decoder: BindingId,
        plan: &Columns<'_>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<ExprId, AllocationError> {
        let mut emit = Emit {
            module: self,
            budget,
        };
        let mut arguments = Vec::with_capacity(plan.columns.len() + 2);
        for column in &plan.columns {
            arguments.push(emit.stream(column)?);
        }
        for dictionary in [&plan.numbers, &plan.strings].into_iter().flatten() {
            arguments.push(emit.dictionary(dictionary)?);
        }
        let callee = emit.read(decoder)?;
        emit.node(Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })
    }
}

/// The bindings a columns decoder reads its streams through.
struct Context<'p> {
    plan: &'p Schema,
    columns: &'p [BindingId],
    numbers: Option<BindingId>,
    strings: Option<BindingId>,
    cursors: &'p [BindingId],
}

/// Decoder construction on the tree.
struct Emit<'m, 'b, 'c> {
    module: &'m mut Module,
    budget: &'b mut AllocationBudget<'c>,
}

impl Emit<'_, '_, '_> {
    fn node(&mut self, expression: Expr) -> Result<ExprId, AllocationError> {
        self.module.expression_in(expression, None, self.budget)
    }

    fn number(&mut self, value: f64) -> Result<ExprId, AllocationError> {
        self.node(Expr::Literal(Literal::Number(value)))
    }

    fn string(&mut self, text: &str) -> Result<ExprId, AllocationError> {
        self.node(Expr::Literal(Literal::String(StringValue::from(text))))
    }

    fn read(&mut self, binding: BindingId) -> Result<ExprId, AllocationError> {
        self.node(Expr::Binding(binding))
    }

    fn at(&mut self, object: ExprId, key: ExprId) -> Result<ExprId, AllocationError> {
        self.node(Expr::Member {
            object,
            property: Property::Computed(key),
        })
    }

    /// `list[row]`.
    fn cell(&mut self, list: BindingId, row: BindingId) -> Result<ExprId, AllocationError> {
        let object = self.read(list)?;
        let key = self.read(row)?;
        self.at(object, key)
    }

    fn method(
        &mut self,
        receiver: ExprId,
        name: &str,
        arguments: Vec<ExprId>,
    ) -> Result<ExprId, AllocationError> {
        let callee = self.node(Expr::Member {
            object: receiver,
            property: Property::Named(name.into()),
        })?;
        self.node(Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Reference,
        })
    }

    fn binary(
        &mut self,
        op: Binary,
        left: ExprId,
        right: ExprId,
    ) -> Result<ExprId, AllocationError> {
        self.node(Expr::Binary { op, left, right })
    }

    fn assign(&mut self, target: ExprId, value: ExprId) -> Result<ExprId, AllocationError> {
        self.node(Expr::Assign { target, value })
    }

    fn binding(&mut self, scope: ScopeId, spelling: &str) -> Result<BindingId, AllocationError> {
        self.module.binding_in(
            Binding {
                source_symbol: None,
                scope,
                spelling: spelling.into(),
                pinned: false,
                class: None,
                defined: false,
            },
            self.budget,
        )
    }

    /// A binding that only ever holds a number: the printer spells its
    /// `x=x+1` as `x++`.
    fn numeric(&mut self, binding: BindingId) -> Result<(), AllocationError> {
        self.module.binding_mut(binding).class = Some(ValueClass::Number);
        Ok(())
    }

    fn region(&mut self, parent: ScopeId) -> Result<(RegionId, ScopeId), AllocationError> {
        let region = self.module.region_in(parent, self.budget)?;
        Ok((region, self.module.regions[region.index()].scope))
    }

    /// `counter=counter+1`.
    fn step(&mut self, counter: BindingId) -> Result<ExprId, AllocationError> {
        let target = self.read(counter)?;
        let current = self.read(counter)?;
        let one = self.number(1.0)?;
        let next = self.binary(Binary::Add, current, one)?;
        self.assign(target, next)
    }

    /// `list=list.split(separator)`, the separator a binding.
    fn split(
        &mut self,
        list: BindingId,
        separator: BindingId,
    ) -> Result<Statement, AllocationError> {
        let separator = self.read(separator)?;
        self.split_with(list, separator)
    }

    /// `list=list.split("|")`.
    fn split_on(&mut self, list: BindingId, separator: char) -> Result<Statement, AllocationError> {
        let separator = self.string(&separator.to_string())?;
        self.split_with(list, separator)
    }

    fn split_with(
        &mut self,
        list: BindingId,
        separator: ExprId,
    ) -> Result<Statement, AllocationError> {
        let target = self.read(list)?;
        let object = self.read(list)?;
        let call = self.method(object, "split", vec![separator])?;
        let assign = self.assign(target, call)?;
        Ok(Statement::Evaluate(assign))
    }

    fn function(
        &mut self,
        parameters: Vec<BindingId>,
        body: RegionId,
    ) -> Result<FunctionId, AllocationError> {
        let function =
            FunctionId::try_new(self.module.functions.len()).ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            Function {
                rest: false,
                parameters,
                body,
                arrow: false,
                name: FunctionName::Unobserved,
                strict: false,
                length: None,
                suspension: Suspension::None,
            },
        )?;
        Ok(function)
    }

    /// One collection's loop, appended to `out`: the binding holding it.
    fn collection(
        &mut self,
        context: &Context<'_>,
        index: usize,
        parent: Option<BindingId>,
        scope: ScopeId,
        out: &mut Vec<Statement>,
    ) -> Result<BindingId, AllocationError> {
        let collection = &context.plan.collections[index];
        let cursor = context.cursors[index];
        let result = self.binding(scope, "table")?;
        let initial = if collection.array {
            Expr::Array(vec![])
        } else if collection.null_proto {
            let null = self.node(Expr::Literal(Literal::Null))?;
            Expr::Object(vec![(Property::Named("__proto__".into()), null)])
        } else {
            Expr::Object(vec![])
        };
        let initial = self.node(initial)?;
        out.push(Statement::Let {
            binding: result,
            value: Some(initial),
        });
        let accumulator = if collection.delta {
            let key = self.binding(scope, "key")?;
            self.numeric(key)?;
            let zero = self.number(0.0)?;
            out.push(Statement::Let {
                binding: key,
                value: Some(zero),
            });
            Some(key)
        } else {
            None
        };
        let condition = match (parent, collection.counts) {
            (Some(parent), Some(counts)) => {
                let end = self.binding(scope, "end")?;
                self.numeric(end)?;
                let start = self.read(cursor)?;
                let count = self.cell(context.columns[counts], parent)?;
                let last = self.binary(Binary::Add, start, count)?;
                out.push(Statement::Let {
                    binding: end,
                    value: Some(last),
                });
                let row = self.read(cursor)?;
                let end = self.read(end)?;
                self.binary(Binary::Less, row, end)?
            }
            _ => {
                let bound = context
                    .plan
                    .bound()
                    .expect("a planned table has a bound column");
                let row = self.read(cursor)?;
                let list = self.read(context.columns[bound])?;
                let length = self.node(Expr::Member {
                    object: list,
                    property: Property::Named("length".into()),
                })?;
                self.binary(Binary::Less, row, length)?
            }
        };
        let (body, body_scope) = self.region(scope)?;
        let mut inner = Vec::new();
        let value = self.value(context, &collection.item, cursor, body_scope, &mut inner)?;
        let target = self.read(result)?;
        let store = if collection.array {
            self.method(target, "push", vec![value])?
        } else {
            let keys = context.columns[collection
                .keys
                .expect("an object collection has a key column")];
            let name = match accumulator {
                Some(key) => {
                    let delta = self.cell(keys, cursor)?;
                    let current = self.read(key)?;
                    let sum = self.binary(Binary::Add, current, delta)?;
                    let key = self.read(key)?;
                    self.assign(key, sum)?
                }
                None => self.cell(keys, cursor)?,
            };
            let member = self.at(target, name)?;
            self.assign(member, value)?
        };
        inner.push(Statement::Evaluate(store));
        self.module.regions[body.index()].statements = inner;
        let update = self.step(cursor)?;
        out.push(Statement::Loop {
            condition: Some(condition),
            update: Some(update),
            body,
        });
        Ok(result)
    }

    /// The expression one row's value at `shape` reads; nested collections
    /// append their loops to `out` first.
    fn value(
        &mut self,
        context: &Context<'_>,
        shape: &Shape,
        row: BindingId,
        scope: ScopeId,
        out: &mut Vec<Statement>,
    ) -> Result<ExprId, AllocationError> {
        match shape {
            Shape::Leaf(index) => {
                let (scale, dictionary, _) = context.plan.columns[*index];
                let read = self.cell(context.columns[*index], row)?;
                match dictionary {
                    Some(kind) => {
                        let dictionary = match kind {
                            Kind::Numbers => context.numbers,
                            Kind::Strings => context.strings,
                        }
                        .expect("a dictionary column has its dictionary");
                        let dictionary = self.read(dictionary)?;
                        self.at(dictionary, read)
                    }
                    None if scale > 0 => {
                        let power = self.number(POWERS[scale])?;
                        self.binary(Binary::Divide, read, power)
                    }
                    None => Ok(read),
                }
            }
            Shape::Tuple(fields) => {
                let mut items = Vec::with_capacity(fields.len());
                for field in fields {
                    items.push(self.value(context, field, row, scope, out)?);
                }
                self.node(Expr::Array(items))
            }
            Shape::Record {
                null_proto,
                keys,
                fields,
            } => {
                let mut entries = Vec::with_capacity(fields.len() + 1);
                if *null_proto {
                    let null = self.node(Expr::Literal(Literal::Null))?;
                    entries.push((Property::Named("__proto__".into()), null));
                }
                for (key, field) in keys.iter().zip(fields) {
                    let value = self.value(context, field, row, scope, out)?;
                    let name = self.string(key)?;
                    entries.push((Property::Computed(name), value));
                }
                self.node(Expr::Object(entries))
            }
            Shape::Collection(index) => {
                let nested = self.collection(context, *index, Some(row), scope, out)?;
                self.read(nested)
            }
        }
    }

    /// One column's stream literal.
    fn stream(&mut self, column: &Column<'_>) -> Result<ExprId, AllocationError> {
        let mut items = Vec::new();
        match &column.data {
            Data::Numbers(values) => {
                for &value in values {
                    items.push(self.number(scaled_or_plain(value, column.scale))?);
                }
            }
            Data::Strings(values) => match column.split {
                Some(separator) => return self.string(&values.join(&separator.to_string())),
                None => {
                    for value in values {
                        items.push(self.string(value)?);
                    }
                }
            },
            // Kept literals move as they are.
            Data::Nodes(values) => items.extend(values.iter().map(|node| node.expr)),
        }
        self.node(Expr::Array(items))
    }

    fn dictionary(&mut self, dictionary: &Dictionary<'_>) -> Result<ExprId, AllocationError> {
        let mut items = Vec::new();
        match dictionary {
            Dictionary::Numbers(values) => {
                for &value in values {
                    items.push(self.number(value)?);
                }
            }
            Dictionary::Strings(values, Some(separator)) => {
                return self.string(&values.join(&separator.to_string()));
            }
            Dictionary::Strings(values, None) => {
                for value in values {
                    items.push(self.string(value)?);
                }
            }
        }
        self.node(Expr::Array(items))
    }
}

#[cfg(test)]
#[path = "tables_tests.rs"]
mod tests;
