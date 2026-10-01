//! Source map v3 input ownership. Qualified origin handles survive source and
//! target rewrites; locations are collected once from retained program origins.
//! Neither generated text nor identifier spellings reconstruct source identity.
use crate::compilation_policy::{BudgetLedger, WorkDomain, WorkKind};
use crate::output_budget::{vector_bytes, AllocationBudget, AllocationClass::{Retained, Scratch}, AllocationError};
use crate::program::{Program, SourceOriginId};

pub(crate) struct Source {
    pub name: String,
    pub text: String,
}
#[derive(Clone, Copy)]
pub(crate) struct Location {
    pub origin: SourceOriginId,
    pub source: u32,
    pub line: u32,
    pub column: u32,
}
pub(crate) struct Sources {
    pub sources: Vec<Source>,
    pub locations: Vec<Location>,
}
/// Private transfer on the frontend's compilation ledger, like PreparedProgram.
pub(crate) struct PreparedSources {
    pub sources: Sources,
    pub bytes: u64,
}
impl PreparedSources {
    pub fn discard(self, ledger: &mut BudgetLedger) {
        drop(self.sources);
        ledger.release(WorkDomain::Baseline, self.bytes).expect("owned source-map inputs");
    }
}
impl Sources {
    pub fn prepare<'a>(program: &Program<'_>, inputs: impl IntoIterator<Item=(&'a str, &'a str)>, ledger: &mut BudgetLedger) -> Result<PreparedSources, AllocationError> {
        let mut owner = AllocationBudget::new(Some((ledger, WorkDomain::Baseline)));
        let sources = owner.retained_phase(|budget| {
            let mut phase = budget.scope();
            let mut sources = phase.vector(Retained, program.modules().len())?;
            for (name, text) in inputs {
                phase.work(WorkKind::Analysis, text.len() as u64 + 1)?;
                let source = Source { name: phase.string(Retained, name)?, text: phase.string(Retained, text)? };
                phase.push(Retained, &mut sources, source)?;
            }
            if sources.len() != program.modules().len() { return Err(AllocationError::WrongOwner); }
            let mut points = Vec::new();
            for unit in program.units() {
                for operation in &unit.data().operations {
                    phase.work(WorkKind::Analysis, 1 + (usize::BITS - program.modules().len().leading_zeros()) as u64)?;
                    if let Some(origin) = operation.origin {
                        let (source, _) = program.source_origin(origin).ok_or(AllocationError::WrongOwner)?;
                        let text = &sources[source.index()].text;
                        if operation.span.start > text.len() || !text.is_char_boundary(operation.span.start) { return Err(AllocationError::WrongOwner); }
                        phase.push(Scratch, &mut points, (origin, source.index(), operation.span.start))?;
                    }
                }
            }
            phase.work(WorkKind::Analysis, (points.len() as u64).saturating_mul(2 * usize::BITS as u64))?;
            points.sort_unstable_by_key(|&(origin, _, offset)| (origin, offset));
            points.dedup_by_key(|point| point.0);
            points.sort_unstable_by_key(|&(origin, source, offset)| (source, offset, origin));
            let mut locations = phase.vector(Retained, points.len())?;
            let mut point = 0;
            for (index, source) in sources.iter().enumerate() {
                let mut position = Position::default();
                let mut chars = source.text.char_indices().peekable();
                while point < points.len() && points[point].1 == index {
                    let (origin, _, offset) = points[point];
                    while chars.peek().is_some_and(|&(at, _)| at < offset) {
                        position.advance(chars.next().unwrap().1)?;
                    }
                    locations.push(Location { origin, source: index as u32, line: position.line, column: position.column });
                    point += 1;
                }
            }
            locations.sort_unstable_by_key(|location| location.origin);
            drop(points);
            phase.finish_retained()?;
            Ok(Self { sources, locations })
        })?;
        let bytes = sources.heap_bytes()?;
        let charge = owner.detach_retained((), bytes)?;
        let (domain, transferred) = charge.into_parts(&()).unwrap_or_else(|_| unreachable!("same-factory sources"));
        debug_assert_eq!(domain, WorkDomain::Baseline);
        debug_assert_eq!(transferred, bytes);
        Ok(PreparedSources { sources, bytes })
    }
    pub fn heap_bytes(&self) -> Result<u64, AllocationError> {
        let mut bytes = vector_bytes(&self.sources)?.checked_add(vector_bytes(&self.locations)?).ok_or(AllocationError::Capacity)?;
        for source in &self.sources {
            bytes = bytes.checked_add(source.name.capacity() as u64).and_then(|sum| sum.checked_add(source.text.capacity() as u64)).ok_or(AllocationError::Capacity)?;
        }
        Ok(bytes)
    }
    pub fn location(&self, origin: SourceOriginId) -> Option<Location> {
        self.locations.binary_search_by_key(&origin, |location| location.origin).ok().map(|index| self.locations[index])
    }
}

#[derive(Default)]
pub(crate) struct Position {
    pub line: u32,
    pub column: u32,
    cr: bool,
}
impl Position {
    pub fn advance(&mut self, character: char) -> Result<(), AllocationError> {
        match character {
            '\r' | '\u{2028}' | '\u{2029}' => {
                self.line = self.line.checked_add(1).ok_or(AllocationError::Capacity)?;
                self.column = 0;
            }
            '\n' => {
                if !self.cr { self.line = self.line.checked_add(1).ok_or(AllocationError::Capacity)?; }
                self.column = 0;
            }
            _ => self.column = self.column.checked_add(character.len_utf16() as u32).ok_or(AllocationError::Capacity)?,
        }
        self.cr = character == '\r';
        Ok(())
    }
}

/// Byte offsets are collected while printing and converted to UTF-16 only once.
#[derive(Clone, Copy)]
pub(crate) struct Point {
    pub offset: usize,
    pub origin: Option<SourceOriginId>,
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn vlq(value: i64, output: &mut String, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
    let mut bits = (value.unsigned_abs() << 1) | u64::from(value < 0);
    loop {
        let digit = (bits & 31) as usize;
        bits >>= 5;
        let byte = BASE64[digit | if bits != 0 { 32 } else { 0 }];
        budget.push_str(Scratch, output, std::str::from_utf8(std::slice::from_ref(&byte)).unwrap())?;
        if bits == 0 { return Ok(()); }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Map<'a> {
    version: u8,
    file: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_root: Option<&'a str>,
    sources: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sources_content: Option<Vec<&'a str>>,
    names: [(); 0],
    mappings: &'a str,
}

struct JsonWriter<'a, 'ledger> {
    text: String,
    budget: &'a mut AllocationBudget<'ledger>,
    error: Option<AllocationError>,
}
impl std::io::Write for JsonWriter<'_, '_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        // serde_json always emits UTF-8 chunks through this writer.
        let text = std::str::from_utf8(bytes).map_err(std::io::Error::other)?;
        match self.budget.push_str(Retained, &mut self.text, text) {
            Ok(()) => Ok(bytes.len()),
            Err(error) => { self.error = Some(error); Err(std::io::Error::other("source-map admission")) }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

/// The generated source is the exact final file, including its wrapper. Only
/// files with surviving mapped origins are retained in its sources table.
pub(crate) fn render(
    code: &str, points: &[Point], sources: &Sources, file: &str,
    contract: &crate::compilation_policy::ContainerContract,
    budget: &mut AllocationBudget<'_>,
) -> Result<String, AllocationError> {
    budget.retained_phase(|budget| {
        let mut phase = budget.scope();
        phase.work(WorkKind::Render, code.len() as u64 + (points.len() as u64).saturating_mul(usize::BITS as u64))?;
        let mut used = phase.vector(Scratch, points.len())?;
        for point in points {
            if let Some(location) = point.origin.and_then(|origin| sources.location(origin)) { used.push(location.source); }
        }
        used.sort_unstable(); used.dedup();
        let mut mappings = String::new();
        let mut chars = code.char_indices().peekable();
        let mut generated = Position::default();
        let mut emitted_line = 0;
        let mut previous_column = 0;
        let mut previous_source = 0;
        let mut previous_line = 0;
        let mut previous_original_column = 0;
        let mut segment = false;
        let mut last = None;
        let mut next = 0;
        while next < points.len() {
            let mut point = points[next]; next += 1;
            while next < points.len() && points[next].offset == point.offset { point = points[next]; next += 1; }
            if point.offset > code.len() || !code.is_char_boundary(point.offset) { return Err(AllocationError::WrongOwner); }
            while chars.peek().is_some_and(|&(offset, _)| offset < point.offset) { generated.advance(chars.next().unwrap().1)?; }
            while emitted_line < generated.line {
                phase.push_str(Scratch, &mut mappings, ";")?;
                emitted_line += 1; previous_column = 0; segment = false; last = None;
            }
            let location = point.origin.and_then(|origin| sources.location(origin));
            let identity = location.map(|location| (location.source, location.line, location.column));
            // No segment is needed for an already unmapped prefix or for the
            // same origin until another origin intervenes on this line.
            if identity == last { continue; }
            if segment { phase.push_str(Scratch, &mut mappings, ",")?; }
            vlq(i64::from(generated.column) - previous_column, &mut mappings, &mut phase)?;
            previous_column = i64::from(generated.column);
            if let Some(location) = location {
                let source = used.binary_search(&location.source).unwrap() as i64;
                vlq(source - previous_source, &mut mappings, &mut phase)?;
                vlq(i64::from(location.line) - previous_line, &mut mappings, &mut phase)?;
                vlq(i64::from(location.column) - previous_original_column, &mut mappings, &mut phase)?;
                previous_source = source; previous_line = i64::from(location.line); previous_original_column = i64::from(location.column);
            }
            last = identity; segment = true;
        }
        let mut names = phase.vector(Scratch, used.len())?;
        let mut content = if contract.sources_content { Some(phase.vector(Scratch, used.len())?) } else { None };
        for &index in &used {
            let source = &sources.sources[index as usize];
            names.push(source.name.as_str());
            if let Some(content) = &mut content { content.push(source.text.as_str()); }
            phase.work(WorkKind::Render, source.name.len() as u64 + if contract.sources_content { source.text.len() as u64 } else { 0 })?;
        }
        let map = Map { version: 3, file, source_root: contract.source_root.as_deref(), sources: names, sources_content: content, names: [], mappings: &mappings };
        let mut writer = JsonWriter { text: String::new(), budget: &mut phase, error: None };
        let result = serde_json::to_writer(&mut writer, &map);
        if result.is_err() { return Err(writer.error.unwrap_or(AllocationError::AllocationFailed)); }
        let text = writer.text;
        drop(map);
        drop((mappings, used));
        phase.finish_retained()?;
        Ok(text)
    })
}

pub(crate) fn inline_base64(json: &str, output: &mut String, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
    budget.work(WorkKind::Render, json.len() as u64)?;
    budget.push_str(Retained, output, "\n//# sourceMappingURL=data:application/json;charset=utf-8;base64,")?;
    for chunk in json.as_bytes().chunks(3) {
        let bits = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        let encoded = [BASE64[(bits >> 18) as usize], BASE64[((bits >> 12) & 63) as usize], if chunk.len() > 1 { BASE64[((bits >> 6) & 63) as usize] } else { b'=' }, if chunk.len() > 2 { BASE64[(bits & 63) as usize] } else { b'=' }];
        budget.push_str(Retained, output, std::str::from_utf8(&encoded).unwrap())?;
    }
    Ok(())
}
