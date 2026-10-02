//! Demand for physical views belongs to the native plan. These are pairs of
//! already admitted representations, not another source type or call graph.
use super::*;

impl NativePlan<'_, '_> {
    pub(super) fn demand_conversion(&mut self, from: NativeType, to: NativeType, budget: &mut AllocationBudget<'_>) -> Result<(), NativeError> {
        let mut pending = Vec::new();
        budget.push(Scratch, &mut pending, (from, to, false))?;
        while let Some((from, to, finish)) = pending.pop() {
            work(budget, 1)?;
            if finish {
                if let (NativeType::Callable(a),NativeType::Callable(b))=(from,to) {
                    if self.callable_view(from,to) { self.register_adapter(a,b,budget)?; continue; }
                }
                if let Some((a,b)) = self.adaptation(ValueStorage::Value(to), ValueStorage::Value(from)) {
                    self.register_adapter(a,b,budget)?;
                }
                continue;
            }
            if from == to { continue; }
            work(budget, self.conversions.len())?;
            if self.conversions.contains(&(from,to)) { continue; }
            budget.push(Scratch, &mut self.conversions, (from,to))?;
            match (from,to) {
                (NativeType::Array(a),NativeType::Array(b)) => {
                    // Views retain one mutable array. Its descriptor also needs
                    // the reverse bridge for writes through the generic view.
                    budget.push(Scratch,&mut pending,(self.arrays[a],self.arrays[b],false))?;
                    budget.push(Scratch,&mut pending,(self.arrays[b],self.arrays[a],false))?;
                }
                (NativeType::Callable(a),NativeType::Callable(b)) => {
                    let (a,b)=(&self.signatures[a],&self.signatures[b]);
                    if a.parameters.len()!=b.parameters.len() { continue; }
                    budget.push(Scratch,&mut pending,(from,to,true))?;
                    budget.push(Scratch,&mut pending,(a.result,b.result,false))?;
                    for (&a,&b) in a.parameters.iter().zip(&b.parameters) {
                        budget.push(Scratch,&mut pending,(b,a,false))?;
                    }
                }
                (NativeType::Callable(a),NativeType::Dynamic(_)) => {
                    signatures::require(&mut self.signatures,a,budget)?;
                    work(budget,self.boxed_callables.len())?;
                    if !self.boxed_callables.contains(&a) { budget.push(Scratch,&mut self.boxed_callables,a)?; }
                }
                (NativeType::Dynamic(_),NativeType::Callable(b)) => {
                    signatures::require(&mut self.signatures,b,budget)?;
                    work(budget,self.unboxed_callables.len())?;
                    if !self.unboxed_callables.contains(&b) { budget.push(Scratch,&mut self.unboxed_callables,b)?; }
                }
                _ => {}
            }
        }
        release(pending,budget)
    }

    pub(super) fn finish_conversions(&mut self,budget:&mut AllocationBudget<'_>) -> Result<(),NativeError> {
        let tagged=NativeType::Dynamic(Tagged::ANY);
        // Every emitted array descriptor has a tagged read/write interface.
        for array in 0..self.arrays.len() {
            self.demand_conversion(self.arrays[array],tagged,budget)?;
            self.demand_conversion(tagged,self.arrays[array],budget)?;
        }
        // Only callable producers crossing tagged storage and demanded typed
        // consumers participate. Newly discovered nested bridges join the same
        // paid worklist; signature IDs are never guessed from source spelling.
        let mut frontier=Vec::new();
        loop {
            let before=self.conversions.len();
            for a in 0..self.boxed_callables.len() {
                for b in 0..self.unboxed_callables.len() {
                    let pair=(self.boxed_callables[a],self.unboxed_callables[b]);
                    work(budget,frontier.len()+1)?;
                    if frontier.contains(&pair) { continue; }
                    budget.push(Scratch,&mut frontier,pair)?;
                    self.demand_conversion(NativeType::Callable(pair.0),NativeType::Callable(pair.1),budget)?;
                }
            }
            if before==self.conversions.len() { break; }
        }
        release(frontier,budget)?;
        if !self.adapters.is_empty() || !self.unboxed_callables.is_empty() || (self.helpers.contains(Helper::Dynamic) && self.signatures.iter().any(|s|s.needed)) { self.helpers.require(Helper::Products); }
        Ok(())
    }

    pub(super) fn operation_conversions(&mut self,data:&UnitData,plan:&UnitPlan,operation:&Operation,operands:&[ValueId],budget:&mut AllocationBudget<'_>) -> Result<(),NativeError> {
        let value=|v:ValueId|self.value_type(plan.values[v.index()]);
        let result=operation.result.map(value);
        let tagged=NativeType::Dynamic(Tagged::ANY);
        let mut pairs=Vec::new();
        let mut add=|from,to|budget.push(Scratch,&mut pairs,(from,to));
        match &operation.kind {
            OperationKind::Initialize(cell)=>add(value(operands[0]),self.value_type(self.cell_storage(*cell)))?,
            OperationKind::Load(place) if result.is_some()=>add(self.value_type(plan.places[place.index()].storage),result.unwrap())?,
            OperationKind::Store(place)=>add(value(operands[0]),self.value_type(plan.places[place.index()].storage))?,
            OperationKind::CopyValue=>add(value(operands[0]),result.unwrap())?,
            OperationKind::Return if !operands.is_empty()=>add(value(operands[0]),plan.return_type)?,
            OperationKind::Allocate {kind,..}=>match (kind,result) {
                (AllocationKind::Struct(_),Some(NativeType::Struct(s)))=>for (field,&v) in self.program.structs[s].fields.clone().zip(operands) { add(value(v),self.field_type(field))?; },
                (AllocationKind::Array|AllocationKind::SpreadArray(_),Some(NativeType::Array(a)))=>for (i,&v) in operands.iter().enumerate() {
                    let from=if matches!(kind,AllocationKind::SpreadArray(flags) if flags[i]) { match value(v) { NativeType::Array(a)=>self.arrays[a],other=>other } } else { value(v) };
                    add(from,self.arrays[a])?;
                },
                (AllocationKind::Instance {..},Some(NativeType::Object(c)))=>for (&v,&ty) in operands.iter().zip(&self.class_fields[c]) { add(value(v),ty)?; },
                (_,Some(NativeType::Record|NativeType::Shape))=>for &v in operands { add(value(v),tagged)?; },
                _=>{}
            },
            OperationKind::Select {yes,no}=>for r in [yes,no] { if let (Some(v),Some(to))=(data.regions[r.index()].result,result) { add(value(v),to)?; } },
            OperationKind::ShortCircuit {right,..}=>if let Some(to)=result { add(value(operands[0]),to)?; if let Some(v)=data.regions[right.index()].result { add(value(v),to)?; } },
            OperationKind::Call(call)=>{
                let args=data.arguments(data.calls[call.index()].arguments).unwrap();
                let target=plan.calls[call.index()];
                let signature=match target {
                    PreparedTarget::Function(unit)=>Some(self.signature_for_unit(unit)),
                    PreparedTarget::Callable {signature,..}|PreparedTarget::Placed {signature,..}|PreparedTarget::Host {signature,..}=>Some(signature),
                    _=>None,
                };
                if let Some(s)=signature {
                    for (argument,&ty) in args.iter().zip(&self.signatures[s].parameters) { if let CallArgument::Value(v)=argument { add(value(*v),ty)?; } }
                    if let Some(to)=result { add(self.signatures[s].result,to)?; }
                } else { match target {
                    PreparedTarget::Assume=>if let (Some(CallArgument::Value(v)),Some(to))=(args.first(),result) { add(value(*v),to)?; },
                    PreparedTarget::ArrayPush {array,..}=>for argument in args { if let CallArgument::Value(v)=argument { add(value(*v),self.arrays[array])?; } },
                    PreparedTarget::ArrayPop {array,..}=>if let Some(to)=result { add(self.arrays[array],to)?; },
                    PreparedTarget::CollectionMethod {..}|PreparedTarget::RecordBuiltin(_)=>{
                        for argument in args { if let CallArgument::Value(v)=argument { add(value(*v),tagged)?; } }
                        if let Some(to)=result { add(tagged,to)?; }
                    },
                    PreparedTarget::ArrayMethod {array,method,..}=>{
                        if let Some(CallArgument::Value(v))=args.first() {
                            if let NativeType::Callable(s)=value(*v) {
                                for (i,&to) in self.signatures[s].parameters.iter().enumerate() {
                                    let from=if method==Intrinsic::ArrayReduce && i==0 {result.unwrap()} else {self.arrays[array]};
                                    add(from,to)?;
                                }
                                let from=self.signatures[s].result;
                                if let Some(to)=result {add(from,match (method,to) {(Intrinsic::ArrayMap,NativeType::Array(a))=>self.arrays[a],_=>to})?;}
                            } else if matches!(method,Intrinsic::ArrayFill|Intrinsic::ArrayIndexOf|Intrinsic::ArrayIncludes) {add(value(*v),self.arrays[array])?;}
                        }
                    },
                    _=>{}
                }}
            },
            OperationKind::ConstructClass|OperationKind::SuperConstruct=>{
                let constructor=if matches!(operation.kind,OperationKind::ConstructClass) { match plan.values[operands[0].index()] {ValueStorage::Function(u)=>Some(u),_=>None} }
                    else {data.constructor_of.and_then(|c|self.program.class(c)).and_then(|c|c.base).and_then(|c|self.constructor_unit(c))};
                if let Some(u)=constructor {
                    let offset=usize::from(matches!(operation.kind,OperationKind::ConstructClass));
                    for (&v,&to) in operands[offset..].iter().zip(&self.signatures[self.signature_for_unit(u)].parameters[1..]) {add(value(v),to)?;}
                }
            },
            _=>{}
        }
        for &(from,to) in &pairs { self.demand_conversion(from,to,budget)?; }
        release(pairs,budget)
    }
}
