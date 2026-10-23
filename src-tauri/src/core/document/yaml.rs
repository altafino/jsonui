//! YAML syntax: lossless editing on `yaml-edit`. Implemented by the YAML lane.

use super::SecretPredicate;
use super::ops::OpEffect;
use super::scan::{Blocked, file_diag};
use super::syntax::{Edit, Syntax, apply_edit};
use super::value::Val;
use crate::core::CoreResult;
use crate::core::types::DraftOp;

pub struct YamlDoc;

pub fn parse(_text: &str) -> Result<YamlDoc, Box<Blocked>> {
    Err(file_diag(
        "UNSUPPORTED_FORMAT",
        "YAML files are not supported yet.",
    ))
}

impl Edit for YamlDoc {
    fn value(&self) -> Val {
        Val::Null
    }
    fn set(&mut self, _: &[String], _: &Val) -> CoreResult<()> {
        unimplemented!()
    }
    fn add(&mut self, _: &[String], _: &str, _: &Val) -> CoreResult<()> {
        unimplemented!()
    }
    fn remove(&mut self, _: &[String]) -> CoreResult<()> {
        unimplemented!()
    }
    fn insert(&mut self, _: &[String], _: usize, _: &Val) -> CoreResult<()> {
        unimplemented!()
    }
    fn move_item(&mut self, _: &[String], _: usize, _: usize) -> CoreResult<()> {
        unimplemented!()
    }
    fn rename(&mut self, _: &[String], _: &str) -> CoreResult<()> {
        unimplemented!()
    }
}

impl Syntax for YamlDoc {
    fn apply(&mut self, op: &DraftOp) -> CoreResult<OpEffect> {
        apply_edit(self, op)
    }
    fn render(&self) -> String {
        String::new()
    }
    fn value(&self) -> Val {
        Edit::value(self)
    }
    fn secret_lexemes(&self, _: SecretPredicate) -> Vec<String> {
        Vec::new()
    }
}
