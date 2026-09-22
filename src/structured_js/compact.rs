//! Remove unreachable storage using the ownership proof already produced by
//! verification. This decides no optimization policy. Target handles move;
//! source operation and symbol provenance retain their meaning.

use super::*;

#[derive(Debug, Default, Clone, Copy)]
pub struct Removed {
    pub expressions: usize,
    pub regions: usize,
    pub functions: usize,
    pub scopes: usize,
    pub bindings: usize,
}

pub(super) fn compact(module: &mut Module, structure: &mut verify::Structure) -> Removed {
    let mut live_scopes = vec![false; module.scopes.len()];
    for (region, depth) in module.regions.iter().zip(&structure.region_depths) {
        if depth.is_some() {
            live_scopes[region.scope.index()] = true;
        }
    }
    let removed = Removed {
        expressions: structure
            .live_expressions
            .iter()
            .filter(|live| !**live)
            .count(),
        regions: structure
            .region_depths
            .iter()
            .filter(|depth| depth.is_none())
            .count(),
        functions: structure
            .live_functions
            .iter()
            .filter(|live| !**live)
            .count(),
        scopes: live_scopes.iter().filter(|live| !**live).count(),
        bindings: structure
            .live_bindings
            .iter()
            .filter(|live| !**live)
            .count(),
    };
    if removed.expressions + removed.regions + removed.functions + removed.scopes + removed.bindings
        == 0
    {
        return removed;
    }
    let expressions = remapping(structure.live_expressions.iter().copied(), ExprId::new);
    let regions = remapping(
        structure.region_depths.iter().map(Option::is_some),
        RegionId::new,
    );
    let functions = remapping(structure.live_functions.iter().copied(), FunctionId::new);
    let scopes = remapping(live_scopes, ScopeId::new);
    let bindings = remapping(structure.live_bindings.iter().copied(), BindingId::new);

    // Postorder filtering preserves postorder. Every operand of a live node
    // is live by verification, so every retained reference has a mapped owner.
    retain(&mut module.expressions, &expressions, |node| {
        node.remap_children(|child| expressions[child.index()].unwrap());
        match node {
            Expr::Function(function) | Expr::Class { constructor: function, .. } => {
                *function = functions[function.index()].unwrap()
            }
            Expr::Binding(binding) => *binding = bindings[binding.index()].unwrap(),
            _ => {}
        }
    });
    retain(&mut module.origins, &expressions, |_| {});
    retain(&mut module.regions, &regions, |region| {
        region.scope = scopes[region.scope.index()].unwrap();
        for statement in &mut region.statements {
            statement.remap_expressions(|value| expressions[value.index()].unwrap());
            match statement {
                Statement::If { yes, no, .. } => {
                    *yes = regions[yes.index()].unwrap();
                    *no = no.map(|region| regions[region.index()].unwrap());
                }
                Statement::Loop { body, .. } | Statement::Block(body) => {
                    *body = regions[body.index()].unwrap();
                }
                Statement::ForIn { binding, body, .. } | Statement::ForOf { binding, body, .. } => {
                    *body = regions[body.index()].unwrap();
                    *binding = bindings[binding.index()].unwrap();
                }
                Statement::Try {
                    body,
                    catch,
                    finally,
                } => {
                    *body = regions[body.index()].unwrap();
                    if let Some(catch) = catch {
                        catch.body = regions[catch.body.index()].unwrap();
                        catch.binding = catch
                            .binding
                            .map(|binding| bindings[binding.index()].unwrap());
                    }
                    *finally = finally.map(|region| regions[region.index()].unwrap());
                }
                Statement::Function { binding, function } => {
                    *function = functions[function.index()].unwrap();
                    *binding = bindings[binding.index()].unwrap();
                }
                Statement::Let { binding, .. } => {
                    *binding = bindings[binding.index()].unwrap();
                }
                _ => {}
            }
        }
    });
    retain(&mut module.functions, &functions, |function| {
        function.body = regions[function.body.index()].unwrap();
        for parameter in &mut function.parameters {
            *parameter = bindings[parameter.index()].unwrap();
        }
    });
    retain(&mut module.scopes, &scopes, |parent| {
        *parent = parent.map(|scope| scopes[scope.index()].unwrap());
    });
    retain(&mut module.bindings, &bindings, |binding| {
        binding.scope = scopes[binding.scope.index()].unwrap();
    });
    for import in &mut module.imports {
        import.binding = bindings[import.binding.index()].unwrap();
    }
    for export in &mut module.exports {
        export.binding = bindings[export.binding.index()].unwrap();
    }
    module.root = regions[module.root.index()].unwrap();

    // No operation or containment edge changes meaning. Filter/remap the
    // existing proof instead of making the next optimizer rediscover it.
    retain(&mut structure.expression_heights, &expressions, |_| {});
    retain(&mut structure.live_expressions, &expressions, |_| {});
    retain(&mut structure.region_depths, &regions, |_| {});
    for region in &mut structure.region_postorder {
        *region = regions[region.index()].unwrap();
    }
    retain(&mut structure.live_functions, &functions, |_| {});
    retain(&mut structure.live_bindings, &bindings, |_| {});
    debug_assert_eq!(
        verify::verify(module).expect("compaction must preserve verified ownership"),
        *structure,
        "remapped structural metadata must equal fresh verification"
    );
    removed
}

fn remapping<T>(live: impl IntoIterator<Item = bool>, wrap: impl Fn(usize) -> T) -> Vec<Option<T>> {
    let mut next = 0;
    live.into_iter()
        .map(|live| {
            live.then(|| {
                let id = wrap(next);
                next += 1;
                id
            })
        })
        .collect()
}

fn retain<T, I>(values: &mut Vec<T>, ids: &[Option<I>], mut remap: impl FnMut(&mut T)) {
    let mut index = 0;
    values.retain_mut(|value| {
        let live = ids[index].is_some();
        index += 1;
        if live {
            remap(value);
        }
        live
    });
}
