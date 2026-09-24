//! Constant string tables as data (beyond Closure: no minifier re-encodes
//! data). A root `let t={k:"v",…}` of many string entries prints as a call to
//! one shared decoder over two strings: the keys front-coded (each key is the
//! length it shares with the one before, one digit, then the rest) and the
//! values joined, both on a separator no entry contains.
//!
//! `function d(k,v,s){k=k.split(s);v=v.split(s);let o={},p="",i=0;
//! for(let e of k){p=p.slice(0,+e[0])+e.slice(1);o[p]=v[i];i=i+1}return o}`
//!
//! The decoder builds the same plain object: the same keys stored in the same
//! order, so the same enumeration order, and the same values. It runs once,
//! where the literal was evaluated, and calls only pristine `String` and
//! `Array` methods. A key named `__proto__` (a prototype in a literal, a
//! setter in a store) keeps its table literal.
//!
//! Separating the key stream from the value stream and sharing sorted keys'
//! prefixes removes novelty a codec pays for: micromark's 2,125 named
//! character references measured −1,054 Brotli, −1,084 gzip and −8,085 raw
//! bytes, so every objective takes it where the raw text shrinks enough.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

/// Fewest entries worth a decoder call.
const MIN_ENTRIES: usize = 64;
/// The encoded strings must be at most this share of the literal's text.
const MAX_SHARE: f64 = 0.85;
/// Separators tried in order: printable, never escaped, never a digit.
const SEPARATORS: [char; 10] = ['|', '~', '^', '`', ';', '#', '@', '!', '%', '&'];

impl Module {
    /// Encode large constant string tables. Returns how many.
    pub(crate) fn encode_string_tables(
        &mut self,
        protected: &[ExprId],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        if !self.pristine_builtins {
            return Ok(0);
        }
        let mut tables = Vec::new();
        for (index, statement) in self.regions[self.root.index()]
            .statements
            .iter()
            .enumerate()
        {
            budget.work(Analysis, 1)?;
            let Statement::Let {
                value: Some(value), ..
            } = *statement
            else {
                continue;
            };
            if let Some(table) = self.string_table(value, protected, budget)? {
                tables.push((index, table));
            }
        }
        if tables.is_empty() {
            return Ok(0);
        }
        let decoder = self.table_decoder(budget)?;
        let count = tables.len();
        for (index, (keys, values, separator)) in tables {
            let mut literal =
                |text: String, module: &mut Self, budget: &mut AllocationBudget<'_>| {
                    module.expression_in(
                        Expr::Literal(Literal::String(StringValue::from(text.as_str()))),
                        None,
                        budget,
                    )
                };
            let keys = literal(keys, self, budget)?;
            let values = literal(values, self, budget)?;
            let separator = literal(separator.to_string(), self, budget)?;
            let callee = self.expression_in(Expr::Binding(decoder), None, budget)?;
            let call = self.expression_in(
                Expr::Call {
                    callee,
                    arguments: vec![keys, values, separator],
                    invocation: Invocation::Value,
                },
                None,
                budget,
            )?;
            // The decoder was declared ahead of the root: one statement later.
            if let Statement::Let { value, .. } =
                &mut self.regions[self.root.index()].statements[index + 1]
            {
                *value = Some(call);
            }
        }
        Ok(count)
    }

    /// The encoded key stream, value stream and separator of a table literal
    /// worth encoding.
    fn string_table(
        &self,
        value: ExprId,
        protected: &[ExprId],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<(String, String, char)>, AllocationError> {
        let Expr::Object(entries) = &self.expressions[value.index()] else {
            return Ok(None);
        };
        if entries.len() < MIN_ENTRIES {
            return Ok(None);
        }
        budget.work(Analysis, entries.len() as u64)?;
        let mut pairs: Vec<(&str, &str)> = Vec::with_capacity(entries.len());
        let mut seen = ahash::AHashSet::with_capacity(entries.len());
        for (key, item) in entries {
            let key = match key {
                Property::Named(name) => name.as_str(),
                Property::Computed(key) => match &self.expressions[key.index()] {
                    Expr::Literal(Literal::String(text)) => match text.as_unicode() {
                        Some(text) => text,
                        None => return Ok(None),
                    },
                    _ => return Ok(None),
                },
            };
            if key == "__proto__" || !seen.insert(key) || protected.binary_search(item).is_ok() {
                return Ok(None);
            }
            let Expr::Literal(Literal::String(text)) = &self.expressions[item.index()] else {
                return Ok(None);
            };
            let Some(text) = text.as_unicode() else {
                return Ok(None);
            };
            pairs.push((key, text));
        }
        let Some(separator) = SEPARATORS.into_iter().find(|&separator| {
            pairs
                .iter()
                .all(|(key, value)| !key.contains(separator) && !value.contains(separator))
        }) else {
            return Ok(None);
        };
        let mut keys = String::new();
        let mut values = String::new();
        let mut previous = "";
        let mut literal = 2usize;
        for (position, &(key, value)) in pairs.iter().enumerate() {
            if position > 0 {
                keys.push(separator);
                values.push(separator);
            }
            let shared = previous
                .chars()
                .zip(key.chars())
                .take_while(|(a, b)| a == b)
                .take(9)
                .count();
            let rest: String = key.chars().skip(shared).collect();
            keys.push(char::from(b'0' + shared as u8));
            keys.push_str(&rest);
            values.push_str(value);
            previous = key;
            // `key:"value",` in the literal (a quoted key when not a name).
            literal += key.len() + value.len() + 4;
        }
        let encoded = keys.len() + values.len() + 12;
        if encoded as f64 > literal as f64 * MAX_SHARE {
            return Ok(None);
        }
        Ok(Some((keys, values, separator)))
    }

    /// Declare the shared decoder at the start of the root; returns its
    /// binding.
    fn table_decoder(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<BindingId, AllocationError> {
        let root_scope = self.regions[self.root.index()].scope;
        let mut binding = |module: &mut Self,
                           scope: ScopeId,
                           spelling: &str,
                           budget: &mut AllocationBudget<'_>| {
            module.binding_in(
                Binding {
                    source_symbol: None,
                    scope,
                    spelling: spelling.into(),
                    pinned: false,
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
        let mut node = |module: &mut Self, expression: Expr, budget: &mut AllocationBudget<'_>| {
            module.expression_in(expression, None, budget)
        };
        let named = |name: &str| Property::Named(name.into());
        // k=k.split(s); v=v.split(s)
        let mut split = |module: &mut Self,
                         list: BindingId,
                         budget: &mut AllocationBudget<'_>|
         -> Result<Statement, AllocationError> {
            let target = node(module, Expr::Binding(list), budget)?;
            let object = node(module, Expr::Binding(list), budget)?;
            let callee = node(
                module,
                Expr::Member {
                    object,
                    property: named("split"),
                },
                budget,
            )?;
            let argument = node(module, Expr::Binding(separator), budget)?;
            let call = node(
                module,
                Expr::Call {
                    callee,
                    arguments: vec![argument],
                    invocation: Invocation::Reference,
                },
                budget,
            )?;
            let assign = node(
                module,
                Expr::Assign {
                    target,
                    value: call,
                },
                budget,
            )?;
            Ok(Statement::Evaluate(assign))
        };
        let split_keys = split(self, keys, budget)?;
        let split_values = split(self, values, budget)?;
        let empty_object = node(self, Expr::Object(vec![]), budget)?;
        let empty_string = node(
            self,
            Expr::Literal(Literal::String(StringValue::from(""))),
            budget,
        )?;
        let zero = node(self, Expr::Literal(Literal::Number(0.0)), budget)?;
        // for(let e of k){…}
        let loop_body = self.region_in(scope, budget)?;
        let loop_scope = self.regions[loop_body.index()].scope;
        let entry = binding(self, loop_scope, "entry", budget)?;
        // p=p.slice(0,+e[0])+e.slice(1)
        let head = {
            let object = node(self, Expr::Binding(prefix), budget)?;
            let callee = node(
                self,
                Expr::Member {
                    object,
                    property: named("slice"),
                },
                budget,
            )?;
            let start = node(self, Expr::Literal(Literal::Number(0.0)), budget)?;
            let digit_object = node(self, Expr::Binding(entry), budget)?;
            let digit_index = node(self, Expr::Literal(Literal::Number(0.0)), budget)?;
            let digit = node(
                self,
                Expr::Member {
                    object: digit_object,
                    property: Property::Computed(digit_index),
                },
                budget,
            )?;
            let length = node(
                self,
                Expr::Unary {
                    op: Unary::Plus,
                    value: digit,
                },
                budget,
            )?;
            node(
                self,
                Expr::Call {
                    callee,
                    arguments: vec![start, length],
                    invocation: Invocation::Reference,
                },
                budget,
            )?
        };
        let tail = {
            let object = node(self, Expr::Binding(entry), budget)?;
            let callee = node(
                self,
                Expr::Member {
                    object,
                    property: named("slice"),
                },
                budget,
            )?;
            let one = node(self, Expr::Literal(Literal::Number(1.0)), budget)?;
            node(
                self,
                Expr::Call {
                    callee,
                    arguments: vec![one],
                    invocation: Invocation::Reference,
                },
                budget,
            )?
        };
        let joined = node(
            self,
            Expr::Binary {
                op: Binary::Add,
                left: head,
                right: tail,
            },
            budget,
        )?;
        let prefix_target = node(self, Expr::Binding(prefix), budget)?;
        let rebuild = node(
            self,
            Expr::Assign {
                target: prefix_target,
                value: joined,
            },
            budget,
        )?;
        // o[p]=v[i]
        let store = {
            let table = node(self, Expr::Binding(object), budget)?;
            let key = node(self, Expr::Binding(prefix), budget)?;
            let target = node(
                self,
                Expr::Member {
                    object: table,
                    property: Property::Computed(key),
                },
                budget,
            )?;
            let list = node(self, Expr::Binding(values), budget)?;
            let position = node(self, Expr::Binding(index), budget)?;
            let value = node(
                self,
                Expr::Member {
                    object: list,
                    property: Property::Computed(position),
                },
                budget,
            )?;
            node(self, Expr::Assign { target, value }, budget)?
        };
        // i=i+1
        let step = {
            let target = node(self, Expr::Binding(index), budget)?;
            let current = node(self, Expr::Binding(index), budget)?;
            let one = node(self, Expr::Literal(Literal::Number(1.0)), budget)?;
            let next = node(
                self,
                Expr::Binary {
                    op: Binary::Add,
                    left: current,
                    right: one,
                },
                budget,
            )?;
            node(
                self,
                Expr::Assign {
                    target,
                    value: next,
                },
                budget,
            )?
        };
        self.regions[loop_body.index()].statements = vec![
            Statement::Evaluate(rebuild),
            Statement::Evaluate(store),
            Statement::Evaluate(step),
        ];
        let iterable = node(self, Expr::Binding(keys), budget)?;
        let result = node(self, Expr::Binding(object), budget)?;
        self.regions[body.index()].statements = vec![
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
        let function =
            FunctionId::try_new(self.functions.len()).ok_or(AllocationError::Capacity)?;
        budget.push(
            AllocationClass::Retained,
            &mut self.functions,
            Function {
                parameters: vec![keys, values, separator],
                body,
                arrow: false,
                name: FunctionName::Unobserved,
                strict: false,
                length: None,
                suspension: Suspension::None,
            },
        )?;
        let root = self.root.index();
        budget.reserve_vec(
            AllocationClass::Retained,
            &mut self.regions[root].statements,
            1,
        )?;
        self.regions[root].statements.insert(
            0,
            Statement::Function {
                binding: decoder,
                function,
            },
        );
        if let Some(&first) = self.root_modules.first() {
            budget.reserve_vec(AllocationClass::Retained, &mut self.root_modules, 1)?;
            self.root_modules.insert(0, first);
        }
        Ok(decoder)
    }
}
