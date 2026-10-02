//! Presence belongs to each call argument, independently of its physical value.
//! Defaults remain source operations evaluated by the selected callee.
use super::*;

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn absent_argument(&self, expression: &str, ty: NativeType) -> String {
        if self.plan.program.source_contract.unified_absence() && matches!(ty, NativeType::Dynamic(_)) {
            format!("({expression}).tag == LS_NULL")
        } else { "false".to_owned() }
    }
    pub(super) fn argument_presence(&mut self, count: usize, absent: &[String]) -> Result<(), NativeError> {
        self.argument_presence_expression(&count.to_string(), absent)
    }
    pub(super) fn argument_presence_expression(&mut self, count: &str, absent: &[String]) -> Result<(), NativeError> {
        self.write(format_args!(",(ls_native_arguments){{{count},"))?;
        if absent.iter().all(|value| value == "false") { self.text("NULL")?; }
        else {
            self.text("(const bool[]){")?;
            for (index,value) in absent.iter().enumerate() {
                if index!=0 { self.text(",")?; }
                self.text(value)?;
            }
            self.text("}")?;
        }
        self.text("}")
    }
    pub(super) fn physical_argument(&mut self, expression:&str, from:NativeType, to:NativeType, absent:Option<&str>) -> Result<(),NativeError> {
        if let Some(absent)=absent.filter(|value| *value!="false") {
            self.write(format_args!("({absent} ? ({to}){{0}} : "))?;
        }
        let (prefix,suffix)=Self::conversion(from,to);
        self.write(format_args!("{prefix}{expression}{suffix}"))?;
        if absent.is_some_and(|value| value!="false") { self.text(")")?; }
        Ok(())
    }
    pub(super) fn converted_argument(&mut self,unit:UnitId,value:ValueId,signature:usize,position:usize) -> Result<(),NativeError> {
        let parameter=&self.plan.signatures[signature];
        let from=self.plan.value_type(self.plan.units[unit.index()].values[value.index()]);
        let missing=self.absent_argument(&format!("ls_v{}",value.index()),from);
        let guarded=parameter.source.params[position].optional && missing!="false";
        let to=parameter.parameters[position];
        if guarded { self.write(format_args!("({missing} ? ({to}){{0}} : "))?; }
        self.converted(unit,value,to)?;
        if guarded { self.text(")")?; }
        Ok(())
    }
}
