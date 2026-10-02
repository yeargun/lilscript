//! Native arrays: one reference-counted, growable C array per element kind.
//! Typed element recipes own and trace managed fields; callbacks preserve
//! the ownership of values they keep across source-array mutation.
//! Index reads trap outside the array, as required by R11. Checked `get`
//! becomes guarded shared control flow; `pop` retains its existing contract.
//! Setting one past the end appends. Callback-created holes preserve presence
//! independently of the element payload across copying and generic views.
//!
//! Methods follow ECMA-262 step by step where it is observable: callbacks
//! see the length fixed when the method starts, and indices removed by a
//! callback are skipped, except by `findIndex`, which reads them.
use super::*;
use crate::primitive::Intrinsic;

impl Emitter<'_, '_, '_, '_, '_> {
    /// One call of a callback value with already-rendered C arguments.
    fn callback_call(
        &mut self,
        unit: UnitId,
        callback: ValueId,
        arguments: &[(&str, NativeType)],
    ) -> Result<(), NativeError> {
        self.callback_call_absence(unit,callback,arguments,None)
    }
    fn prepare_callback(&mut self,unit:UnitId,callback:ValueId,arguments:&[(&str,NativeType)],forced:Option<&str>)->Result<(),NativeError> {
        let storage=self.plan.units[unit.index()].values[callback.index()];
        let NativeType::Callable(signature)=self.plan.value_type(storage) else {unreachable!("admitted callback")};
        if matches!(storage,ValueStorage::Value(_)) {self.write(format_args!("if(!ls_v{}.code) ls_native_raise_error(\"TypeError\",\"value is not callable\");\n",callback.index()))?;}
        for (index,&(argument,from)) in arguments.iter().enumerate() {
            let to=self.plan.signatures[signature].parameters[index];
            let missing=self.absent_argument(argument,from);
            let missing=if index==0 {forced.map_or(missing.clone(),|forced|format!("({forced} || {missing})"))} else {missing};
            self.write(format_args!("{to} ls_cb_argument{index} = "))?;
            self.physical_argument(argument,from,to,self.plan.signatures[signature].source.params[index].optional.then_some(missing.as_str()))?;
            self.text(";\n")?;
        }
        Ok(())
    }
    fn callback_call_absence(&mut self,unit:UnitId,callback:ValueId,arguments:&[(&str,NativeType)],forced:Option<&str>) -> Result<(),NativeError> {
        let storage = self.plan.units[unit.index()].values[callback.index()];
        let NativeType::Callable(signature) = self.plan.value_type(storage) else {
            unreachable!("native plan admits only callable callbacks")
        };
        let returned=self.plan.signatures[signature].result;
        if returned==NativeType::Void {self.text("(ls_native_raised ? (void)0 : ")?;}
        else {self.write(format_args!("(ls_native_raised ? ({returned}){{0}} : "))?;}
        let floating = self.plan.signatures[signature].result == NativeType::F64;
        if floating {
            self.text("ls_f64(")?;
        }
        let mut leading = false;
        match storage {
            ValueStorage::Function(body) => self.write(format_args!("ls_fn{}(", body.index()))?,
            ValueStorage::Host(binding) => self.write(format_args!(
                "{}(",
                self.plan.hosts.bindings[binding].link_name
            ))?,
            ValueStorage::Value(_) => {
                self.write(format_args!(
                    "ls_v{0}.code(ls_v{0}.environment",
                    callback.index()
                ))?;
                leading = true;
            }
        }
        for index in 0..arguments.len() {
            if index!=0 || leading {self.text(",")?;}
            self.write(format_args!("ls_cb_argument{index}"))?;
        }
        if self.plan.signatures[signature].has_optional() {
            let missing=arguments.iter().enumerate().map(|(index,&(value,ty))| {
                if self.plan.signatures[signature].source.params[index].optional {
                    let missing=self.absent_argument(value,ty);
                    if index==0 {forced.map_or(missing.clone(),|forced|format!("({forced} || {missing})"))} else {missing}
                } else {"false".to_owned()}
            }).collect::<Vec<_>>();
            self.argument_presence(arguments.len(),&missing)?;
        }
        self.text(")")?;
        if floating {
            self.text(")")?;
        }
        self.text(")")
    }

    fn callback_result(&self, unit: UnitId, callback: ValueId) -> NativeType {
        let NativeType::Callable(signature) = self.plan.value_type(self.plan.units[unit.index()].values[callback.index()]) else { unreachable!("admitted callback") };
        self.plan.signatures[signature].result
    }
    fn array_callback_item(&mut self, array: usize, absent: bool) -> Result<(), NativeError> {
        let e=self.plan.arrays[array];
        self.write(format_args!("{e} ls_item = "))?;
        if absent { self.write(format_args!("ls_array_has(ls_src,ls_k) ? ls_array{array}_get(ls_src,(int32_t)ls_k,&ls_temps) : ls_array{array}_absent();\n"))?; }
        else { self.write(format_args!("ls_array{array}_get(ls_src,(int32_t)ls_k,&ls_temps);\n"))?; }
        if let Some(retain)=e.retain("ls_item") { self.text(&retain)?; }
        Ok(())
    }
    fn array_callback_cleanup(&mut self, element: NativeType) -> Result<(), NativeError> {
        if let Some(drop)=element.release("ls_item") { self.text(&drop)?; }
        self.clear_temporaries()?;
        self.text("if (ls_native_raised) break;\n")
    }
    fn array_predicate(&mut self, unit: UnitId, callback: ValueId, element: NativeType,absent:Option<&str>) -> Result<(), NativeError> {
        let returned=self.callback_result(unit,callback);
        self.prepare_callback(unit,callback,&[("ls_item",element)],absent)?;
        self.write(format_args!("{returned} ls_predicate_result = "))?;
        self.callback_call_absence(unit,callback,&[("ls_item",element)],absent)?;
        let (prefix,suffix)=Self::conversion(returned,NativeType::Bool);
        self.write(format_args!(";\nbool ls_predicate = !ls_native_raised && {prefix}ls_predicate_result{suffix};\n"))?;
        if let Some(drop)=returned.release("ls_predicate_result") { self.text(&drop)?; }
        Ok(())
    }
    pub(super) fn array_method(&mut self, unit: UnitId, call: CallId, result: Option<ValueId>, receiver: ValueId, array: usize, method: Intrinsic) -> Result<(), NativeError> {
        let data=self.plan.program.unit(unit).unwrap();
        let arguments=data.arguments(data.calls[call.index()].arguments).unwrap();
        let value=|position: usize| match arguments.get(position) { Some(CallArgument::Value(value))=>Some(*value), _=>None };
        let s=array; let e=self.plan.arrays[array]; let r=receiver.index();
        let result=result.unwrap(); let destination=Destination::Value(result);
        match method {
            Intrinsic::ArrayJoin => {
                let (prefix,suffix)=Self::conversion(e,NativeType::Dynamic(Tagged::ANY));
                self.write(format_args!("{{\nls_string_builder ls_join={{0}};\nfor(size_t ls_k=0;ls_k<ls_v{r}->length;++ls_k) {{\nif(ls_k) "))?;
                if let Some(separator)=value(0) { self.write(format_args!("ls_string_builder_text(&ls_join,ls_v{});\n",separator.index()))?; }
                else { self.text("ls_string_builder_unit(&ls_join,',');\n")?; }
                self.write(format_args!("if(ls_array_has(ls_v{r},ls_k)) ls_string_builder_value(&ls_join,{prefix}ls_array{s}_get(ls_v{r},(int32_t)ls_k,&ls_temps){suffix});\n"))?;
                self.clear_temporaries()?;
                self.text("}\n")?;
                self.assignment_start(unit,destination,true)?; self.text("ls_string_builder_finish(&ls_join)")?;
                self.assignment_end(unit,destination)?; self.text("}\n")
            }
            Intrinsic::ArrayForEach => {
                let callback=value(0).unwrap(); let returned=self.callback_result(unit,callback);
                self.write(format_args!("{{ ls_array{s} *ls_src=ls_v{r}; size_t ls_len=ls_src->length;\nfor(size_t ls_k=0;ls_k<ls_len;++ls_k) {{\nif(!ls_array_has(ls_src,ls_k)) continue;\n"))?;
                self.array_callback_item(s,false)?;
                self.prepare_callback(unit,callback,&[("ls_item",e)],None)?;
                if returned!=NativeType::Void { self.write(format_args!("{returned} ls_ignored LS_NATIVE_UNUSED = "))?; }
                self.callback_call(unit,callback,&[("ls_item",e)])?; self.text(";\n")?;
                if let Some(drop)=returned.release("ls_ignored") { self.text(&drop)?; }
                self.array_callback_cleanup(e)?; self.text("}\n}\n")
            }
            Intrinsic::ArrayMap | Intrinsic::ArrayFilter => {
                let NativeType::Array(out)=self.plan.value_type(self.plan.units[unit.index()].values[result.index()]) else { unreachable!("mapped array") };
                let callback=value(0).unwrap(); let returned=self.callback_result(unit,callback);
                let output=self.plan.arrays[out];
                self.write(format_args!("{{ ls_array{s} *ls_src=ls_v{r}; size_t ls_len=ls_src->length;\nls_array{out} *ls_out=ls_array{out}_new(ls_len);\nfor(size_t ls_k=0;ls_k<ls_len;++ls_k) {{\nif(!ls_array_has(ls_src,ls_k)) {{"))?;
                if method==Intrinsic::ArrayMap { self.write(format_args!("ls_array{out}_hole(ls_out);"))?; }
                self.text("continue;}\n")?;
                self.array_callback_item(s,false)?;
                if method==Intrinsic::ArrayFilter {
                    self.array_predicate(unit,callback,e,None)?;
                    self.write(format_args!("if(ls_predicate) ls_array{out}_push(ls_out,ls_item);\n"))?;
                } else {
                    self.prepare_callback(unit,callback,&[("ls_item",e)],None)?;
                    self.write(format_args!("{returned} ls_mapped = "))?;
                    self.callback_call(unit,callback,&[("ls_item",e)])?; self.text(";\n")?;
                    if returned==output {
                        self.write(format_args!("if (!ls_native_raised) ls_array{out}_push_owned(ls_out,ls_mapped);\n"))?;
                        if let Some(drop)=returned.release("ls_mapped") { self.text("else {\n")?; self.text(&drop)?; self.text("}\n")?; }
                    }
                    else {
                        let (prefix,suffix)=Self::conversion(returned,output);
                        self.write(format_args!("if (!ls_native_raised) ls_array{out}_push(ls_out,{prefix}ls_mapped{suffix});\n"))?;
                        if let Some(drop)=returned.release("ls_mapped") { self.text(&drop)?; }
                    }
                }
                self.array_callback_cleanup(e)?; self.text("}\n")?;
                self.assignment_start(unit,destination,true)?; self.text("ls_out")?;
                self.assignment_end(unit,destination)?; self.text("}\n")
            }
            Intrinsic::ArrayReduce => {
                let accumulator=self.plan.value_type(self.plan.units[unit.index()].values[result.index()]);
                let callback=value(0).unwrap(); let returned=self.callback_result(unit,callback);
                self.write(format_args!("{{ ls_array{s} *ls_src=ls_v{r}; size_t ls_len=ls_src->length;\n{accumulator} ls_acc = "))?;
                self.converted(unit,value(1).unwrap(),accumulator)?; self.text(";\n")?;
                if let Some(retain)=accumulator.retain("ls_acc") { self.text(&retain)?; }
                self.write(format_args!("for(size_t ls_k=0;ls_k<ls_len;++ls_k) {{\nif(!ls_array_has(ls_src,ls_k)) continue;\n"))?;
                self.array_callback_item(s,false)?;
                self.prepare_callback(unit,callback,&[("ls_acc",accumulator),("ls_item",e)],None)?;
                self.write(format_args!("{returned} ls_next = "))?;
                self.callback_call(unit,callback,&[("ls_acc",accumulator),("ls_item",e)])?; self.text(";\n")?;
                self.text("if (ls_native_raised) {\n")?;
                if let Some(drop)=returned.release("ls_next") {self.text(&drop)?;}
                self.array_callback_cleanup(e)?;
                self.text("}\n")?;
                let (prefix,suffix)=Self::conversion(returned,accumulator);
                if let Some(owner)=accumulator.owner_prefix() {
                    let operation=if returned==accumulator { "take" } else { "copy" };
                    self.write(format_args!("{owner}_{operation}(&ls_acc,{prefix}ls_next{suffix});\n"))?;
                } else { self.write(format_args!("ls_acc={prefix}ls_next{suffix};\n"))?; }
                if returned!=accumulator { if let Some(drop)=returned.release("ls_next") { self.text(&drop)?; } }
                self.array_callback_cleanup(e)?; self.text("}\n")?;
                self.assignment_start(unit,destination,true)?; self.text("ls_acc")?;
                self.assignment_end(unit,destination)?; self.text("}\n")
            }
            Intrinsic::ArraySome | Intrinsic::ArrayEvery | Intrinsic::ArrayFindIndex => {
                let find=method==Intrinsic::ArrayFindIndex; let every=method==Intrinsic::ArrayEvery;
                self.write(format_args!("{{ ls_array{s} *ls_src=ls_v{r}; size_t ls_len=ls_src->length;\n"))?;
                if find { self.text("int32_t ls_found=-1;\n")?; }
                else { self.write(format_args!("bool ls_found={every};\n"))?; }
                self.text("for(size_t ls_k=0;ls_k<ls_len;++ls_k) {\n")?;
                if !find { self.text("if(!ls_array_has(ls_src,ls_k)) continue;\n")?; }
                let callback=value(0).unwrap();
                let NativeType::Callable(signature)=self.plan.value_type(self.plan.units[unit.index()].values[callback.index()]) else {unreachable!("admitted callback")};
                let item=if find && (matches!(self.plan.signatures[signature].parameters[0],NativeType::Dynamic(_))
                    || self.plan.signatures[signature].source.params[0].optional) {
                    // findIndex observes deleted positions. Preserve absence
                    // before a typed read can erase it or reject its payload.
                    self.text("ls_value ls_item=ls_array_read(ls_src,ls_k,&ls_temps);\nls_value_retain(ls_item);\n")?;
                    NativeType::Dynamic(Tagged::ANY)
                } else {self.array_callback_item(s,find)?;e};
                self.array_predicate(unit,callback,item,find.then_some("!ls_array_has(ls_src,ls_k)"))?;
                self.array_callback_cleanup(item)?;
                if find { self.text("if(ls_predicate) { ls_found=(int32_t)ls_k; break; }\n")?; }
                else { self.write(format_args!("if({}ls_predicate) {{ ls_found={}; break; }}\n",if every {"!"} else {""},!every))?; }
                self.write(format_args!("}}\nls_v{}=ls_found;\n}}\n",result.index()))
            }
            Intrinsic::ArrayIndexOf | Intrinsic::ArrayIncludes => {
                let name=if method==Intrinsic::ArrayIndexOf {"index_of"} else {"includes"};
                self.write(format_args!("ls_v{}=ls_array{s}_{name}(ls_v{r},",result.index()))?;
                self.converted(unit,value(0).unwrap(),e)?;
                if method==Intrinsic::ArrayIncludes {
                    self.text(",")?;
                    if let Some(start)=value(1) { self.value(unit,start)?; } else { self.text("0")?; }
                }
                self.text(");\n")
            }
            Intrinsic::ArrayConcat | Intrinsic::ArraySlice | Intrinsic::ArraySplice => {
                let expression=match method {
                    Intrinsic::ArrayConcat=>format!("ls_array{s}_concat(ls_v{r},ls_v{})",value(0).unwrap().index()),
                    Intrinsic::ArraySplice=>format!("ls_array{s}_splice(ls_v{r},ls_v{},ls_v{})",value(0).unwrap().index(),value(1).unwrap().index()),
                    _=>{
                        let start=value(0).map_or("0".to_owned(),|v|format!("ls_array_relative(ls_v{},ls_v{r}->length)",v.index()));
                        let end=value(1).map_or(format!("ls_v{r}->length"),|v|format!("ls_array_relative(ls_v{},ls_v{r}->length)",v.index()));
                        format!("ls_array{s}_slice(ls_v{r},{start},{end})")
                    }
                };
                self.assignment_start(unit,destination,true)?; self.text(&expression)?; self.assignment_end(unit,destination)
            }
            Intrinsic::ArrayReverse | Intrinsic::ArrayFill | Intrinsic::ArrayCopyWithin => {
                self.assignment_start(unit,destination,false)?;
                match method {
                    Intrinsic::ArrayReverse=>self.write(format_args!("ls_array{s}_reverse(ls_v{r})"))?,
                    Intrinsic::ArrayFill=>{
                        self.write(format_args!("ls_array{s}_fill(ls_v{r},"))?;
                        self.converted(unit,value(0).unwrap(),e)?; self.text(")")?;
                    }
                    _=>{
                        self.write(format_args!("ls_array{s}_copy_within(ls_v{r},ls_v{},ls_v{},{}",value(0).unwrap().index(),value(1).unwrap().index(),value(2).is_some()))?;
                        if let Some(end)=value(2) { self.write(format_args!(",ls_v{}",end.index()))?; } else { self.text(",0")?; }
                        self.text(")")?;
                    }
                }
                self.assignment_end(unit,destination)
            }
            _=>unreachable!("admitted array method"),
        }
    }
}
