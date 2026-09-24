//! `Map`, `Set` and `Symbol`. A map keeps entries in insertion order and
//! finds them through an open-addressed index, comparing keys with
//! SameValueZero: numbers by value (NaN finds NaN, -0 is +0), strings by
//! code units, everything else by identity. A set is a map whose values
//! are null. Entries own their keys and values.
use super::*;
use crate::primitive::Intrinsic;

pub(in crate::program) const RUNTIME: &str = r#"typedef struct { ls_value key; ls_value value; bool live; } ls_map_entry;
typedef struct ls_map {
    ls_native_object owner;
    ls_map_entry *entries;
    size_t used, capacity, size;
    /* Entry position + 1, or 0 for an empty index slot. */
    size_t *index;
    size_t slots;
} ls_map;
static void ls_map_destroy(ls_native_object *owner) {
    ls_map *map = (ls_map *)owner;
    for (size_t position = 0; position < map->used; position++) {
        if (map->entries[position].live) {
            ls_value_release(map->entries[position].key);
            ls_value_release(map->entries[position].value);
        }
    }
    free(map->entries);
    free(map->index);
}
static ls_native_object *ls_map_new(void) {
    ls_map *map = ls_native_allocate(sizeof *map, ls_map_destroy);
    map->entries = NULL;
    map->used = map->capacity = map->size = 0;
    map->index = NULL;
    map->slots = 0;
    return &map->owner;
}
static uint64_t ls_map_mix(uint64_t bits) {
    bits ^= bits >> 33;
    bits *= UINT64_C(0xff51afd7ed558ccd);
    bits ^= bits >> 33;
    return bits;
}
static uint64_t ls_map_hash(ls_value key) {
    switch (key.tag) {
    case LS_INT: case LS_FLOAT: {
        double number = ls_value_to_number(key);
        if (number != number) return UINT64_C(0x7ff8000000000000);
        if (number == 0) number = 0;
        uint64_t bits;
        memcpy(&bits, &number, sizeof bits);
        return ls_map_mix(bits);
    }
    case LS_STRING: {
        uint64_t hash = UINT64_C(0xcbf29ce484222325);
        for (size_t index = 0; index < key.as.s.length; index++) {
            hash ^= key.as.s.data[index];
            hash *= UINT64_C(0x100000001b3);
        }
        return hash;
    }
    case LS_BOOL: return key.as.b ? 2 : 1;
    case LS_CALLABLE: return ls_map_mix(key.as.c.identity);
    case LS_OBJECT: case LS_ARRAY: case LS_SYMBOL: return ls_map_mix((uint64_t)(uintptr_t)key.as.o);
    default: return 0;
    }
}
static size_t ls_map_find(ls_map *map, ls_value key) {
    if (!map->slots) return SIZE_MAX;
    size_t mask = map->slots - 1;
    for (size_t slot = (size_t)ls_map_hash(key) & mask;; slot = (slot + 1) & mask) {
        size_t entry = map->index[slot];
        if (!entry) return SIZE_MAX;
        if (map->entries[entry - 1].live && ls_value_same_zero(map->entries[entry - 1].key, key)) return entry - 1;
    }
}
/* Compacts removed entries and rebuilds the index with room to grow. */
static void ls_map_rebuild(ls_map *map, size_t needed) {
    size_t kept = 0;
    for (size_t position = 0; position < map->used; position++) {
        if (map->entries[position].live) map->entries[kept++] = map->entries[position];
    }
    map->used = kept;
    size_t slots = 8;
    while (slots < 2 * needed) {
        if (slots > SIZE_MAX / 4) ls_native_resource_failure();
        slots *= 2;
    }
    free(map->index);
    map->index = calloc(slots, sizeof *map->index);
    if (!map->index) ls_native_resource_failure();
    map->slots = slots;
    for (size_t position = 0; position < map->used; position++) {
        size_t slot = (size_t)ls_map_hash(map->entries[position].key) & (slots - 1);
        while (map->index[slot]) slot = (slot + 1) & (slots - 1);
        map->index[slot] = position + 1;
    }
}
static int32_t ls_map_size(ls_native_object *owner) {
    return (int32_t)((ls_map *)owner)->size;
}
static ls_value ls_map_get(ls_native_object *owner, ls_value key) {
    ls_map *map = (ls_map *)owner;
    size_t position = ls_map_find(map, key);
    return position == SIZE_MAX ? (ls_value){0} : map->entries[position].value;
}
static bool ls_map_has(ls_native_object *owner, ls_value key) {
    return ls_map_find((ls_map *)owner, key) != SIZE_MAX;
}
static void ls_map_set(ls_native_object *owner, ls_value key, ls_value value) {
    ls_map *map = (ls_map *)owner;
    size_t position = ls_map_find(map, key);
    ls_value_retain(value);
    if (position != SIZE_MAX) {
        ls_value_release(map->entries[position].value);
        map->entries[position].value = value;
        return;
    }
    /* A new key: -0 is stored as +0, as JavaScript normalizes it. */
    if (key.tag == LS_FLOAT && key.as.f == 0) key.as.f = 0;
    if (map->size >= (size_t)INT32_MAX) ls_native_resource_failure();
    if (map->used == map->capacity) {
        if (map->size < map->used / 2) {
            ls_map_rebuild(map, map->size + 1);
        } else {
            size_t capacity = map->capacity ? map->capacity * 2 : 8;
            if (capacity > SIZE_MAX / sizeof *map->entries) ls_native_resource_failure();
            ls_map_entry *entries = realloc(map->entries, capacity * sizeof *entries);
            if (!entries) ls_native_resource_failure();
            map->entries = entries;
            map->capacity = capacity;
        }
    }
    if (2 * (map->used + 1) > map->slots) ls_map_rebuild(map, map->used + 1);
    ls_value_retain(key);
    map->entries[map->used] = (ls_map_entry){key, value, true};
    size_t slot = (size_t)ls_map_hash(key) & (map->slots - 1);
    while (map->index[slot]) slot = (slot + 1) & (map->slots - 1);
    map->index[slot] = ++map->used;
    map->size++;
}
static bool ls_map_delete(ls_native_object *owner, ls_value key) {
    ls_map *map = (ls_map *)owner;
    size_t position = ls_map_find(map, key);
    if (position == SIZE_MAX) return false;
    map->entries[position].live = false;
    ls_value_release(map->entries[position].key);
    ls_value_release(map->entries[position].value);
    map->size--;
    return true;
}
static void ls_map_clear(ls_native_object *owner) {
    ls_map *map = (ls_map *)owner;
    for (size_t position = 0; position < map->used; position++) {
        if (map->entries[position].live) {
            map->entries[position].live = false;
            ls_value_release(map->entries[position].key);
            ls_value_release(map->entries[position].value);
        }
    }
    map->size = 0;
    ls_map_rebuild(map, 0);
}
typedef struct { ls_native_object owner; } ls_symbol;
static ls_native_object *ls_symbol_new(void) {
    ls_symbol *symbol = ls_native_allocate(sizeof *symbol, NULL);
    return &symbol->owner;
}
"#;

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn construct(
        &mut self,
        unit: UnitId,
        result: ValueId,
        intrinsic: Intrinsic,
    ) -> Result<(), NativeError> {
        let destination = Destination::Value(result);
        self.assignment_start(unit, destination, true)?;
        // A description is observable only through `toString`, which native
        // symbols do not offer.
        self.text(match intrinsic {
            Intrinsic::MapNew | Intrinsic::SetNew => "ls_map_new()",
            _ => "ls_symbol_new()",
        })?;
        self.assignment_end(unit, destination)
    }

    pub(super) fn collection_method(
        &mut self,
        unit: UnitId,
        call: CallId,
        result: ValueId,
        receiver: ValueId,
        method: Intrinsic,
    ) -> Result<(), NativeError> {
        let data = self.plan.program.unit(unit).unwrap();
        let arguments = data.arguments(data.calls[call.index()].arguments).unwrap();
        let argument = |position: usize| match arguments[position] {
            CallArgument::Value(value) => value,
            CallArgument::Reference(_) => unreachable!("native collections take values"),
        };
        let any = NativeType::Dynamic(Tagged::ANY);
        let r = receiver.index();
        let destination = Destination::Value(result);
        match method {
            Intrinsic::MapSet | Intrinsic::SetAdd => {
                self.write(format_args!("ls_map_set(ls_v{r},"))?;
                self.converted(unit, argument(0), any)?;
                self.text(",")?;
                if method == Intrinsic::MapSet {
                    self.converted(unit, argument(1), any)?;
                } else {
                    self.text("(ls_value){0}")?;
                }
                self.text(");\n")?;
                // Both return their receiver: one more owner of it.
                self.assignment_start(unit, destination, false)?;
                self.write(format_args!("ls_v{r}"))?;
                self.assignment_end(unit, destination)
            }
            Intrinsic::MapGet => {
                // The entry lends its value; the slot takes its own owner.
                self.assignment_start(unit, destination, false)?;
                self.write(format_args!("ls_map_get(ls_v{r},"))?;
                self.converted(unit, argument(0), any)?;
                self.text(")")?;
                self.assignment_end(unit, destination)
            }
            Intrinsic::MapHas | Intrinsic::SetHas | Intrinsic::MapDelete | Intrinsic::SetDelete => {
                let operation = if matches!(method, Intrinsic::MapHas | Intrinsic::SetHas) {
                    "has"
                } else {
                    "delete"
                };
                self.write(format_args!("ls_v{} = ls_map_{operation}(ls_v{r},", result.index()))?;
                self.converted(unit, argument(0), any)?;
                self.text(");\n")
            }
            _ => self.write(format_args!("ls_map_clear(ls_v{r});\n")),
        }
    }
}
