//! Structured completion transport. A finally runs once on every exit, owns
//! its displaced exception, and may replace return/throw/break/continue.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Completion { Throw, Return, Break(OpId), Continue(OpId) }
impl Completion {
    fn code(self) -> usize { match self { Self::Throw=>1,Self::Return=>2,Self::Break(op)=>3+op.index()*2,Self::Continue(op)=>4+op.index()*2 } }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase { Body, Catch, Finally }
pub(super) struct TryFrame {
    op: OpId,
    parent: RegionId,
    catch: Option<(Option<CellId>,RegionId)>,
    finally: Option<RegionId>,
    enclosing_loop: Option<OpId>,
    phase: Phase,
    actions: Vec<Completion>,
}
impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn control_declarations(&mut self, unit: UnitId) -> Result<(),NativeError> {
        let data=self.plan.program.unit(unit).unwrap();
        let ty=self.plan.units[unit.index()].return_type;
        if ty!=NativeType::Void { self.write(format_args!("{ty} ls_pending_return LS_NATIVE_UNUSED{};\n",ty.empty_slot()))?; }
        for &(region,cell) in &self.plan.units[unit.index()].catch_bindings {
            let _=region;
            if self.plan.boxed_cell(cell) { self.write(format_args!("ls_box{0} *ls_c{0} = NULL;\n",cell.index()))?; }
            else { let ty=self.plan.value_type(self.plan.cell_storage(cell));self.write(format_args!("{ty} ls_c{} LS_NATIVE_UNUSED = {{0}};\n",cell.index()))?; }
        }
        for (index,operation) in data.operations.iter().enumerate() {
            self.budget.work(WorkKind::Render,1)?;
            if matches!(operation.kind,OperationKind::Try {finally:Some(_),..}) {
                self.write(format_args!("int ls_completion{index} LS_NATIVE_UNUSED=0;\nls_exception_state ls_saved{index}={{0}};\n"))?;
                if ty!=NativeType::Void { self.write(format_args!("{ty} ls_saved_return{index} LS_NATIVE_UNUSED={{0}};\n"))?; }
            }
        }
        Ok(())
    }
    pub(super) fn operation_can_raise(&self,unit:UnitId,op:OpId)->bool {
        let data=self.plan.program.unit(unit).unwrap();
        if self.plan.units[unit.index()].conversion_checks[op.index()] {return true;}
        match data.operations[op.index()].kind {
            OperationKind::Call(call)=>match self.plan.units[unit.index()].calls[call.index()] {
                PreparedTarget::Function(body)=>self.plan.units[body.index()].may_throw,
                PreparedTarget::Print | PreparedTarget::MathImul=>false,
                _=>true,
            },
            OperationKind::Load(_) | OperationKind::Store(_) | OperationKind::CheckPlace(_)
            | OperationKind::PrepareCall(_) | OperationKind::PrepareReference {..}
            | OperationKind::ConstructClass | OperationKind::SuperConstruct
            | OperationKind::Intrinsic(_) =>true,
            _=>false,
        }
    }
    pub(super) fn check_exception(&mut self,unit:UnitId)->Result<(),NativeError> {
        let data=self.plan.program.unit(unit).unwrap();
        let region=data.operations[self.operation.expect("scheduled throwing operation").index()].region;
        self.check_exception_region(region)
    }
    pub(super) fn check_exception_region(&mut self,region:RegionId)->Result<(),NativeError> {
        self.exception_regions[region.index()]=true;
        self.write(format_args!("if (ls_native_raised) goto ls_throw_region{};\n",region.index()))
    }
    pub(super) fn finish_exception_region(&mut self,unit:UnitId,region:RegionId)->Result<(),NativeError> {
        if !self.exception_regions[region.index()] { return Ok(()); }
        // All checks in one lexical region cross the same ownership scopes
        // and handlers. Emit that exit once, while its original try frames are
        // active, rather than copying the complete cleanup at every operation.
        let index=region.index();
        self.write(format_args!("goto ls_throw_done{index};\nls_throw_region{index}:;\n"))?;
        self.complete(unit,Completion::Throw)?;
        self.write(format_args!("ls_throw_done{index}:;\n"))
    }
    pub(super) fn complete(&mut self,unit:UnitId,action:Completion)->Result<(),NativeError> {
        self.complete_at(unit,action,usize::MAX)
    }
    pub(super) fn complete_at(&mut self,unit:UnitId,action:Completion,origin_depth:usize)->Result<(),NativeError> {
        let data=self.plan.program.unit(unit).unwrap();
        let stop=match action {Completion::Break(op)|Completion::Continue(op)=>Some(data.operations[op.index()].region),_=>None};
        let stop_depth=stop.and_then(|r|self.active_regions.iter().position(|&v|v==r));
        for index in (0..self.try_frames.len()).rev() {
            let frame=&self.try_frames[index];
            let depth=self.active_regions.iter().position(|&r|r==frame.parent).expect("active try parent");
            if depth>origin_depth {continue;}
            if stop_depth.is_some_and(|stop|depth<stop) { break; }
            let op=frame.op; let parent=frame.parent;
            if frame.phase==Phase::Finally {
                self.write(format_args!("ls_exception_clear(&ls_saved{});\n",op.index()))?;
                if let Some(owner)=self.plan.units[unit.index()].return_type.owner_prefix() {
                    self.write(format_args!("{owner}_clear(&ls_saved_return{});\n",op.index()))?;
                }
                continue;
            }
            if action==Completion::Throw && frame.phase==Phase::Body && frame.catch.is_some() {
                self.cleanup_path_action(unit,Some(parent),action)?; self.clear_temporaries()?;
                return self.write(format_args!("goto ls_catch{};\n",op.index()));
            }
            if frame.finally.is_some() {
                if !self.try_frames[index].actions.contains(&action) {
                    self.budget.push(AllocationClass::Scratch,&mut self.try_frames[index].actions,action)?;
                }
                self.cleanup_path_action(unit,Some(parent),action)?; self.clear_temporaries()?;
                return self.write(format_args!("ls_completion{0}={1}; goto ls_finally{0};\n",op.index(),action.code()));
            }
        }
        self.cleanup_path_action(unit,stop,action)?; self.clear_temporaries()?;
        match action {
            Completion::Throw=>{self.error_exit=true;self.text("goto ls_error;\n")},
            Completion::Return=>{self.return_exit=true;self.text("goto ls_return;\n")},
            Completion::Break(op)=>self.write(format_args!("goto ls_end{};\n",op.index())),
            Completion::Continue(op)=>self.write(format_args!("goto ls_update{};\n",op.index())),
        }
    }
    pub(super) fn defer_return(&mut self,unit:UnitId,value:Option<ValueId>)->Result<(),NativeError> {
        let ty=self.plan.units[unit.index()].return_type;
        if ty!=NativeType::Void {
            if let Some(owner)=ty.owner_prefix() {
                self.write(format_args!("{owner}_copy(&ls_pending_return,"))?;
                self.converted(unit,value.unwrap(),ty)?; self.text(");\n")?;
            } else {
                self.text("ls_pending_return=")?;self.converted(unit,value.unwrap(),ty)?;self.text(";\n")?;
            }
        }
        if let Some(value)=value {
            if self.plan.conversion_can_raise(self.plan.value_type(self.plan.units[unit.index()].values[value.index()]),ty) {self.check_exception(unit)?;}
        }
        self.complete(unit,Completion::Return)
    }
    pub(super) fn start_try(&mut self,unit:UnitId,op:OpId,body:RegionId,catch:Option<(Option<CellId>,RegionId)>,finally:Option<RegionId>,enclosing_loop:Option<OpId>)->Result<(),NativeError> {
        let parent=self.plan.program.unit(unit).unwrap().operations[op.index()].region;
        if finally.is_some() { self.write(format_args!("ls_completion{}=0;\n",op.index()))?; }
        self.budget.push(AllocationClass::Scratch,&mut self.try_frames,TryFrame {op,parent,catch,finally,enclosing_loop,phase:Phase::Body,actions:Vec::new()})?;
        self.push(Task::TryBodyEnd(op))?;self.region(body,enclosing_loop)
    }
    pub(super) fn try_body_end(&mut self,unit:UnitId,op:OpId)->Result<(),NativeError> {
        let frame=self.try_frames.last_mut().unwrap();debug_assert!(frame.op==op);
        let catch=frame.catch;let finally=frame.finally;let enclosing=frame.enclosing_loop;
        if let Some((cell,region))=catch {
            frame.phase=Phase::Catch;
            self.write(format_args!("goto ls_{}{};\nls_catch{}: LS_NATIVE_UNUSED;\n",if finally.is_some(){"finally"}else{"try_end"},op.index(),op.index()))?;
            if let Some(cell)=cell {
                if self.plan.boxed_cell(cell) {
                    self.write(format_args!("{{ ls_value ls_caught=ls_exception_catch(); ls_c{0}=ls_box_new{0}(ls_caught); ls_value_release(ls_caught); }}\n",cell.index()))?;
                } else { self.write(format_args!("ls_value_take(&ls_c{},ls_exception_catch());\n",cell.index()))?; }
            } else { self.text("ls_value_release(ls_exception_catch());\n")?; }
            self.push(Task::TryCatchEnd(op))?;self.region(region,enclosing)
        } else { self.try_catch_end(unit,op) }
    }
    pub(super) fn try_catch_end(&mut self,unit:UnitId,op:OpId)->Result<(),NativeError> {
        let frame=self.try_frames.last_mut().unwrap();debug_assert!(frame.op==op);
        let finally=frame.finally;let enclosing=frame.enclosing_loop;
        if let Some(region)=finally {
            frame.phase=Phase::Finally;
            self.write(format_args!("ls_finally{0}: LS_NATIVE_UNUSED;\nls_saved{0}=ls_exception_save();\n",op.index()))?;
            let ty=self.plan.units[unit.index()].return_type;
            if ty!=NativeType::Void { self.write(format_args!("if (ls_completion{0}==2) {{ls_saved_return{0}=ls_pending_return;ls_pending_return=({ty}){{0}};}}\n",op.index()))?; }
            self.push(Task::TryFinallyEnd(op))?;self.region(region,enclosing)
        } else {
            self.try_frames.pop();
            self.write(format_args!("ls_try_end{}: LS_NATIVE_UNUSED;\n",op.index()))
        }
    }
    pub(super) fn try_finally_end(&mut self,unit:UnitId,op:OpId)->Result<(),NativeError> {
        let frame=self.try_frames.pop().unwrap();debug_assert!(frame.op==op);
        self.write(format_args!("ls_exception_restore(&ls_saved{});\n",op.index()))?;
        let ty=self.plan.units[unit.index()].return_type;
        if ty!=NativeType::Void { self.write(format_args!("if (ls_completion{0}==2) {{ls_pending_return=ls_saved_return{0};ls_saved_return{0}=({ty}){{0}};}}\n",op.index()))?; }
        for action in frame.actions {
            self.write(format_args!("if (ls_completion{}=={}) {{\n",op.index(),action.code()))?;
            self.complete(unit,action)?;self.text("}\n")?;
        }
        Ok(())
    }
    pub(super) fn control_exits(&mut self,unit:UnitId)->Result<(),NativeError> {
        let ty=self.plan.units[unit.index()].return_type;
        if self.plan.program.unit(unit).unwrap().suspension==Suspension::Generator {
            self.text("ls_generator_finish(base);return;\n")?;
            if self.return_exit {self.text("ls_return:;ls_generator_finish(base);return;\n")?;}
            if self.error_exit {self.text("ls_error:;ls_generator_finish(base);return;\n")?;}
            return Ok(());
        }
        if self.error_exit || self.return_exit {
            // Checked nonvoid bodies end in source return/throw. Void bodies
            // may fall through normally and must not enter failure cleanup.
            if ty==NativeType::Void { self.text("return;\n")?; }
        }
        if self.return_exit {
            self.text("ls_return:;\n")?;
            self.text(if ty==NativeType::Void {"return;\n"} else {"return ls_pending_return;\n"})?;
        }
        if self.error_exit {
            self.text("ls_error:;\n")?;
            if let Some(owner)=ty.owner_prefix() { self.write(format_args!("{owner}_clear(&ls_pending_return);\n"))?; }
            if ty==NativeType::Void { self.text("return;\n")?; }
            else { self.write(format_args!("return ({ty}){{0}}; /* ignored while exception status is set */\n"))?; }
        }
        Ok(())
    }
}
