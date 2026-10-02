//! Plain products remain C values. A tagged boundary makes a traced immutable
//! snapshot; statement-local temporary owners make borrowed conversions safe.
use super::*;

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn temporary_declaration(&mut self) -> Result<(), NativeError> {
        if self.plan.helpers.contains(Helper::Products) {
            self.text("ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;\n")?;
        }
        Ok(())
    }
    pub(super) fn clear_temporaries(&mut self) -> Result<(), NativeError> {
        if self.plan.helpers.contains(Helper::Products) {
            self.text("ls_native_temporaries_clear(&ls_temps);\n")?;
        }
        Ok(())
    }
    /// These conversions replace one owner with another representation's
    /// owners; a call result cannot use the ordinary bitwise owner transfer.
    pub(super) fn product_conversion(from: NativeType, to: NativeType) -> bool {
        matches!((from, to), (NativeType::Struct(_), NativeType::Dynamic(_))
            | (NativeType::Dynamic(_), NativeType::Struct(_))
            | (NativeType::Dynamic(_), NativeType::Callable(_))
            | (NativeType::Callable(_), NativeType::Dynamic(_))
            | (NativeType::Callable(_), NativeType::Callable(_)))
    }
    pub(super) fn product_boxes(&mut self) -> Result<(), NativeError> {
        if !self.plan.helpers.contains(Helper::Products) { return Ok(()); }
        for &index in &self.plan.struct_order {
            self.budget.work(WorkKind::Render, 1)?;
            self.write(format_args!(
                "typedef struct {{ ls_native_object owner; ls_native_temporary temporary; ls_t{index} value; }} ls_product{index};\n\
static LS_NATIVE_UNUSED void ls_product{index}_destroy(ls_native_object *owner) {{ ls_t{index}_release(((ls_product{index} *)owner)->value); }}\n\
static LS_NATIVE_UNUSED void ls_product{index}_trace(ls_native_object *owner, ls_native_visit visit, void *context) {{ ls_t{index}_trace(((ls_product{index} *)owner)->value,visit,context); }}\n\
static LS_NATIVE_UNUSED ls_value ls_t{index}_box(ls_native_temporary **temps, ls_t{index} value) {{\n\
ls_product{index} *box=ls_native_allocate(sizeof *box,ls_product{index}_destroy,ls_product{index}_trace);\n\
ls_t{index}_retain(value); box->value=value;\n\
ls_native_temporary_push(temps,&box->temporary,&box->owner);\n\
ls_value result={{.tag=LS_PRODUCT}}; result.as.o=&box->owner; return result;\n}}\n\
static LS_NATIVE_UNUSED ls_t{index} ls_value_to_t{index}(ls_value value) {{\n\
if (value.tag!=LS_PRODUCT || !value.as.o || value.as.o->destroy!=ls_product{index}_destroy) {{ ls_value_mismatch(); return (ls_t{index}){{0}}; }}\n\
return ((ls_product{index} *)value.as.o)->value;\n}}\n"))?;
        }
        Ok(())
    }
}

impl Emitter<'_, '_, '_, '_, '_> {
    /// Rebuild a product path from the selected logical location. Array indices
    /// and reference receivers are already SSA snapshots; no reallocatable C
    /// item pointer survives the evaluation of the right-hand side.
    pub(super) fn store_product_field(&mut self, unit: UnitId, leaf: PlaceId, value: ValueId) -> Result<(), NativeError> {
        if self.plan.place_addressable(unit, leaf) {
            return self.copy_value(unit, Destination::Place(leaf), value);
        }
        let mut path = Vec::new();
        let mut root = leaf;
        while let PlaceRecipe::Field { base, slot, unbox } = self.plan.units[unit.index()].places[root.index()].recipe {
            let parent = unbox.map(NativeType::Struct).unwrap_or_else(|| self.plan.place_type(unit, base));
            self.budget.push(AllocationClass::Scratch, &mut path, (root, slot, parent))?;
            root = base;
        }
        path.reverse();
        self.text("{\n")?;
        for (index, &(_, _, parent)) in path.iter().enumerate() {
            let physical = if index == 0 { self.plan.place_type(unit, root) } else { self.plan.place_type(unit, path[index-1].0) };
            let (prefix, suffix) = Self::conversion(physical, parent);
            self.write(format_args!("{parent} ls_wb{index} = {prefix}"))?;
            if index == 0 { self.place(unit, root)?; }
            else { self.write(format_args!("ls_wb{}.ls_f{}", index-1, path[index-1].1))?; }
            self.write(format_args!("{suffix};\n"))?;
            if let Some(retain) = parent.retain(&format!("ls_wb{index}")) { self.text(&retain)?; }
        }
        let last = path.len()-1;
        let leaf_type = self.plan.place_type(unit, leaf);
        let destination = format!("ls_wb{last}.ls_f{}", path[last].1);
        if let Some(prefix) = leaf_type.owner_prefix() { self.write(format_args!("{prefix}_copy(&{destination},"))?; }
        else { self.write(format_args!("{destination} = "))?; }
        self.converted(unit, value, leaf_type)?;
        self.text(if leaf_type.managed() { ");\n" } else { ";\n" })?;
        for index in (1..path.len()).rev() {
            let parent_field = self.plan.place_type(unit, path[index-1].0);
            let from = path[index].2;
            let (prefix, suffix) = Self::conversion(from, parent_field);
            let owner = parent_field.owner_prefix().expect("a product or its tagged box owns");
            self.write(format_args!("{owner}_copy(&ls_wb{}.ls_f{},{prefix}ls_wb{index}{suffix});\n", index-1,path[index-1].1))?;
            if let Some(drop) = from.release(&format!("ls_wb{index}")) { self.text(&drop)?; }
        }
        let root_type = self.plan.place_type(unit, root);
        let (prefix, suffix) = Self::conversion(path[0].2, root_type);
        match self.plan.units[unit.index()].places[root.index()].recipe {
            PlaceRecipe::Element { receiver, index, array } => {
                self.write(format_args!("ls_array{array}_set(ls_v{},ls_v{},{prefix}ls_wb0{suffix});\n", receiver.index(),index.index()))?;
            }
            PlaceRecipe::Record { receiver, key, omit_absent } => {
                self.write(format_args!("ls_shape_set(ls_v{},", receiver.index()))?;
                self.record_key(key)?;
                self.write(format_args!(",{prefix}ls_wb0{suffix},{omit_absent});\n"))?;
            }
            _ => {
                self.assignment_start(unit, Destination::Place(root), false)?;
                self.write(format_args!("{prefix}ls_wb0{suffix}"))?;
                self.assignment_end(unit, Destination::Place(root))?;
            }
        }
        if let Some(drop) = path[0].2.release("ls_wb0") { self.text(&drop)?; }
        self.text("}\n")?;
        self.budget.release(AllocationClass::Scratch, (path.capacity()*size_of::<(PlaceId,usize,NativeType)>()) as u64)?;
        Ok(())
    }
}
