//! Location demand lives on the existing owning cell Storage row. Inline aliases borrow
//! original actual places; this module retains no alias or call-edge table.
use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::semantic_program) enum LocationDemand {
    #[default]
    None,
    /// An undischarged check or actual inline access needs this location.
    Local,
    /// A surviving shared invocation transports this bank's location.
    Shared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::semantic_program) struct PlaceLocation {
    pub context: ContextId,
    pub place: PlaceId,
}

/// Demand/stability projection ONLY. Formation must retain the original
/// PlaceLocation and InlineActual preparation identity for complete paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::semantic_program) struct ResolvedLocation {
    pub context: ContextId,
    pub cell: CellId,
    pub top_field: Option<NominalMemberId>,
    /// Full composed projection depth for common demand/placement queries.
    pub field_depth: usize,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::semantic_program) enum LocationCheck {
    Preparation { call: CallId, position: u32 },
    CheckPlace { operation: OpId },
}

#[derive(Debug, Clone, Copy)]
pub(super) enum LocationUse {
    Preparation { call: CallId, position: u32 },
    CheckPlace { operation: OpId },
    Read,
    Write,
    Shared,
}
impl LocationUse {
    fn is_check(self) -> bool {
        matches!(self, Self::Preparation { .. } | Self::CheckPlace { .. })
    }
}

impl<'program, 'src> DemandPlan<'program, 'src> {
    /// Each iteration follows one original earlier-place edge or one actual
    /// parent inline call. A named reference formal ends this walk: it is an
    /// incoming location, never a caller-owned bank or a union of actuals.
    pub(in crate::semantic_program) fn resolve_location_visited<E>(
        &self,
        mut location: PlaceLocation,
        mut visit: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Option<ResolvedLocation>, E> {
        let mut top_field = None;
        let mut field_depth = 0usize;
        loop {
            visit(1)?;
            let context = self.context(location.context);
            let data = self.program.units[context.unit.index()].data();
            match data.places[location.place.index()] {
                Place::Field { base, field } => {
                    debug_assert!(base.index() < location.place.index());
                    field_depth = field_depth.saturating_add(1);
                    top_field = Some(field);
                    location.place = base;
                }
                Place::Cell(cell) => {
                    if self.is_reference_parameter(cell) {
                        let owner = self.cell_owner_visited(location.context, cell, || visit(1))?;
                        let Some(owner) = owner else { return Ok(None) };
                        if self.context(owner).kind.is_inline() {
                            visit(1)?;
                            let CellBinding::Parameter(position) =
                                self.program.cells[cell.index()].binding
                            else {
                                unreachable!("verified reference formal")
                            };
                            let actual = self.inline_actual(owner, position);
                            let CallArgument::Reference(place) = *actual.argument else {
                                unreachable!("verified inline reference argument")
                            };
                            location = PlaceLocation {
                                context: actual.caller,
                                place,
                            };
                            continue;
                        }
                    }
                    return Ok(Some(ResolvedLocation {
                        context: location.context,
                        cell,
                        top_field,
                        field_depth,
                    }));
                }
                Place::Value(_) | Place::Member { .. } | Place::Index { .. } => return Ok(None),
            }
        }
    }

    fn location_bank(
        &self,
        location: ResolvedLocation,
        budget: &mut Budget<'_>,
    ) -> Result<Option<(ContextId, usize, Option<u32>)>, DemandError> {
        budget.work(self.product_lookup_work())?;
        let Some(family) = self.product_for_cell(location.cell) else {
            return Ok(None);
        };
        let owner = self
            .cell_owner_visited(location.context, location.cell, || budget.work(1))?
            .ok_or_else(|| unsupported("reference product outside activation"))?;
        let banks = &self.context(owner).product_banks;
        budget.work(1 + (usize::BITS - banks.len().leading_zeros()) as usize)?;
        let bank = banks
            .binary_search_by_key(&location.cell, |bank| bank.cell)
            .map_err(|_| unsupported("reference product bank missing"))?;
        let slot = if let Some(member) = location.top_field {
            budget.work(1 + (usize::BITS - self.program.fields.len().leading_zeros()) as usize)?;
            let field = self
                .program
                .field(member)
                .ok_or_else(|| unsupported("reference product member missing"))?;
            if field.owner != self.products[family].schema() {
                return Err(unsupported("reference product member schema"));
            }
            Some(
                u32::try_from(field.index)
                    .map_err(|_| unsupported("product field slot exceeds demand capacity"))?,
            )
        } else {
            None
        };
        Ok(Some((owner, bank, slot)))
    }

    /// This is the same monotone write summary used by ordinary Product Assign
    /// and Access. Set it before need_storage or wait_for can take a shortcut.
    fn rewrite_location_slots(
        &mut self,
        owner: ContextId,
        bank: usize,
        slot: Option<u32>,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        if let Some(slot) = slot {
            budget.work(1)?;
            self.contexts[owner.index()].product_banks[bank].slots[slot as usize].rewritten = true;
        } else {
            let count = self.context(owner).product_banks[bank].slots.len();
            budget.work(count)?;
            for slot in &mut self.contexts[owner.index()].product_banks[bank].slots {
                slot.rewritten = true;
            }
        }
        Ok(())
    }

    fn need_bank_location(
        &mut self,
        owner: ContextId,
        bank: usize,
        demand: LocationDemand,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        budget.work(1)?;
        let cell = self.context(owner).product_banks[bank].cell;
        let ordinal = self.cell_ordinal(cell);
        if self.context(owner).cells[ordinal].location >= demand {
            return Ok(());
        }
        if demand == LocationDemand::Shared {
            // Shared transport is a conservative read/write boundary. Its
            // complete proof establishes shape, not absence of alias writes.
            self.rewrite_location_slots(owner, bank, None, budget)?;
        }
        self.contexts[owner.index()].cells[ordinal].location = demand;
        let unit = self.context(owner).unit;
        self.contexts[owner.index()].cells[ordinal].declaration = true;
        // A real empty product location still starts at this initialization;
        // no field loop can establish its TDZ/activation lifetime.
        match self.summaries[unit.index()].as_ref().unwrap().initializers[ordinal] {
            Initialization::Operation(operation) => {
                self.need_operation(owner, operation, budget)?
            }
            Initialization::Parameter(_) => {}
            _ => {
                return Err(unsupported(
                    "product location requires exact initialization",
                ))
            }
        }
        if demand == LocationDemand::Shared {
            let count = self.context(owner).product_banks[bank].slots.len();
            for slot in 0..count {
                self.need_storage(StorageId::Product(owner, bank, slot as u32), budget)?;
            }
        }
        Ok(())
    }

    /// Shared proof extraction for Demand and Formation; no retained flag or
    /// secondary proof algorithm. Only completed family/helper evidence admits
    /// removing this exact original preparation or body check.
    pub(in crate::semantic_program) fn location_check_proved_visited<E>(
        &self,
        original: PlaceLocation,
        check: LocationCheck,
        mut visit: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        visit(1)?;
        let context = self.context(original.context);
        let data = self.program.units[context.unit.index()].data();
        match check {
            LocationCheck::CheckPlace { operation } => {
                if !matches!(data.operations[operation.index()].kind,
                    OperationKind::CheckPlace(place) if place == original.place)
                {
                    return Ok(false);
                }
                visit(self.product_lookup_work())?;
                if self.product_operation(context.unit, operation).is_some_and(|kind|
                    matches!(kind, ProductOperationKind::Access(access) if access.place() == original.place))
                {
                    return Ok(true);
                }
                let ContextKind::Inline { helper, .. } = context.kind else {
                    return Ok(false);
                };
                Ok(self.helpers[helper]
                    .reference_access(operation)
                    .is_some_and(|access| access.place() == original.place))
            }
            LocationCheck::Preparation { call, position } => {
                let actual =
                    data.arguments(data.calls[call.index()].arguments).unwrap()[position as usize];
                if !matches!(actual, CallArgument::Reference(place) if place == original.place) {
                    return Ok(false);
                }
                let Some(location) = self.resolve_location_visited(original, &mut visit)? else {
                    return Ok(false);
                };
                if location.context != original.context {
                    return Ok(false); // incoming formal preparation needs its own scope
                }
                visit(self.product_lookup_work())?;
                Ok(self.product_for_cell(location.cell).is_some())
            }
        }
    }

    pub(super) fn need_place_location(
        &mut self,
        original: PlaceLocation,
        usage: LocationUse,
        budget: &mut Budget<'_>,
    ) -> Result<(), DemandError> {
        let check = match usage {
            LocationUse::Preparation { call, position } => {
                Some(LocationCheck::Preparation { call, position })
            }
            LocationUse::CheckPlace { operation } => Some(LocationCheck::CheckPlace { operation }),
            _ => None,
        };
        if let Some(check) = check {
            if self.location_check_proved_visited(original, check, |n| budget.work(n))? {
                return Ok(());
            }
        }
        let location = self
            .resolve_location_visited(original, |n| budget.work(n))?
            .ok_or_else(|| unsupported("reference requires original lexical place"))?;
        let Some((owner, bank, slot)) = self.location_bank(location, budget)? else {
            // Named reference formals already receive a location. They are
            // never a new owned carrier, nor a union of their callers' cells.
            if self.is_reference_parameter(location.cell) {
                if matches!(usage, LocationUse::Write) {
                    // Resolution follows every inline actual before stopping
                    // at an incoming named location. Record a real demanded
                    // write before need_cell's already-observed shortcut.
                    budget.work(1)?;
                    self.writes_incoming_references = true;
                }
                return self.need_cell(location.context, location.cell, budget);
            }
            let storage = self.cell_storage(location.context, location.cell, budget)?;
            let required = if matches!(usage, LocationUse::Shared) {
                LocationDemand::Shared
            } else {
                LocationDemand::Local
            };
            budget.work(1)?;
            // Set physical addressing before need_storage's already-needed
            // shortcut. Semantic isolation still uses complete source uses.
            if self.storage(storage).location < required {
                self.storage_mut(storage).location = required;
            }
            return self.need_storage(storage, budget);
        };
        if matches!(usage, LocationUse::Write) {
            self.rewrite_location_slots(owner, bank, slot, budget)?;
        }
        self.need_bank_location(
            owner,
            bank,
            if matches!(usage, LocationUse::Shared) {
                LocationDemand::Shared
            } else {
                LocationDemand::Local
            },
            budget,
        )?;
        if matches!(usage, LocationUse::Shared) {
            return Ok(()); // the promotion already demanded every raw field
        }
        if let Some(slot) = slot {
            // Whole/one-field checks only observe root presence. Do not demand
            // an unused field just to manufacture an address or TDZ binding.
            if !usage.is_check() || location.field_depth > 1 {
                // The composed path can require packed parents inside this top
                // component; its addressed leaf must still not be read/coerced.
                self.need_storage(StorageId::Product(owner, bank, slot), budget)?;
            }
        } else if matches!(usage, LocationUse::Read | LocationUse::Write) {
            let count = self.context(owner).product_banks[bank].slots.len();
            for slot in 0..count {
                self.need_storage(StorageId::Product(owner, bank, slot as u32), budget)?;
            }
        }
        Ok(())
    }

    /// Ordinary ref-formal Stores have no ProductOperation overlay. Resolve
    /// their actual bank during context seeding, before liveness shortcuts.
    pub(super) fn seed_location_store(
        &mut self,
        context: ContextId,
        operation: OpId,
        place: PlaceId,
        budget: &mut Budget<'_>,
    ) -> Result<bool, DemandError> {
        if self.products.is_empty() {
            return Ok(false);
        }
        let Some(location) =
            self.resolve_location_visited(PlaceLocation { context, place }, |n| budget.work(n))?
        else {
            return Ok(false);
        };
        let Some((owner, bank, slot)) = self.location_bank(location, budget)? else {
            return Ok(false);
        };
        self.rewrite_location_slots(owner, bank, slot, budget)?;
        // Raw alias writes remain observable operations. The ordinary cursor
        // retains the full RHS and any required current-parent checks. Keeping
        // this operation also handles zero-field whole replacements correctly.
        self.need_operation(context, operation, budget)?;
        Ok(true)
    }

    /// Final physical write requirement, accumulated by the existing location
    /// cursor. Formation may choose its private replacement protocol before
    /// creating any target bindings; no scan or target-use reconstruction.
    pub(in crate::semantic_program) fn writes_incoming_references_visited<E>(
        &self,
        mut visit: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        visit(1)?;
        Ok(self.writes_incoming_references)
    }

    /// One physical location requirement for every owned cell, selected
    /// product or ordinary. Borrow the actual activation, never a global bit.
    pub(in crate::semantic_program) fn cell_location_visited<E>(
        &self,
        context: ContextId,
        cell: CellId,
        mut visit: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Option<LocationDemand>, E> {
        let Some(owner) = self.cell_owner_visited(context, cell, || visit(1))? else {
            return Ok(None);
        };
        visit(1)?;
        Ok(Some(
            self.context(owner).cells[self.cell_ordinal(cell)].location,
        ))
    }

    /// Product Formation's borrowed filtered view of the same cell owner.
    /// There is no separately retained bank location flag.
    pub(in crate::semantic_program) fn product_location_visited<E>(
        &self,
        context: ContextId,
        cell: CellId,
        mut visit: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Option<LocationDemand>, E> {
        visit(self.product_lookup_work())?;
        if self.product_for_cell(cell).is_none() {
            return Ok(None);
        }
        self.cell_location_visited(context, cell, visit)
    }

    /// Formation calls this once before emitting helpers. No independently
    /// retained root list or order-dependent ABI selection is introduced.
    pub(in crate::semantic_program) fn has_shared_product_locations_visited<E>(
        &self,
        mut visit: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        if self.products.is_empty() {
            return Ok(false);
        }
        for context in &self.contexts {
            visit(1)?;
            for bank in &context.product_banks {
                visit(1)?;
                if context.cells[self.cell_ordinal(bank.cell)].location == LocationDemand::Shared {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
