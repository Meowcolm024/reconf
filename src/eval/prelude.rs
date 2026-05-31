use crate::lower::lower_file;
use crate::resolve::modules::{Loader, Module, eval_file_without_prelude};
use crate::syntax::parser::parse;

const SOURCE: &str = include_str!("prelude.reconf");

pub fn module() -> Module {
    let ast = lower_file(parse(SOURCE).expect("bundled prelude must parse"));
    let mut loader = Loader::default();
    eval_file_without_prelude(&mut loader, std::path::Path::new("."), ast)
        .expect("bundled prelude must evaluate")
}
