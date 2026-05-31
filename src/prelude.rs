use crate::eval::{Loader, Module, Value, eval_file_without_prelude};
use crate::parser::parse;

const SOURCE: &str = include_str!("prelude.reconf");

pub fn module() -> Module {
    let ast = parse(SOURCE).expect("bundled prelude must parse");
    let mut loader = Loader::default();
    eval_file_without_prelude(&mut loader, std::path::Path::new("."), ast)
        .expect("bundled prelude must evaluate")
}

pub fn native_value(name: &str) -> Option<Value> {
    match name {
        "showInt" | "showFloat" | "showBool" | "lengthString" | "lengthList" | "isSome"
        | "isNone" => Some(Value::Native {
            name: name.to_string(),
        }),
        "contains" | "startsWith" | "endsWith" | "unwrapOr" | "all" | "any" | "map" | "filter" => {
            Some(Value::Builtin {
                name: match name {
                    "contains" => "contains",
                    "startsWith" => "startsWith",
                    "endsWith" => "endsWith",
                    "unwrapOr" => "unwrapOr",
                    "all" => "all",
                    "any" => "any",
                    "map" => "map",
                    "filter" => "filter",
                    _ => unreachable!(),
                },
                args: Vec::new(),
            })
        }
        _ => None,
    }
}
