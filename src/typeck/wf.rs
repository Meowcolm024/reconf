use std::collections::BTreeMap;

use crate::error::{Error, ErrorCode, Result};
use crate::syntax::surface::Type;

pub fn well_formed_type(ty: &Type, aliases: &BTreeMap<String, Type>) -> Result<()> {
    well_formed_type_inner(ty, aliases, &mut Vec::new())
}

fn well_formed_type_inner(
    ty: &Type,
    aliases: &BTreeMap<String, Type>,
    stack: &mut Vec<String>,
) -> Result<()> {
    match ty {
        Type::Alias(name) => {
            if stack.contains(name) {
                return Err(Error::with_code(
                    ErrorCode::TypeRecursiveAlias,
                    format!("recursive type alias `{name}`"),
                ));
            }
            let Some(alias) = aliases.get(name) else {
                return Err(Error::new(format!("unknown type `{name}`")));
            };
            stack.push(name.clone());
            well_formed_type_inner(alias, aliases, stack)?;
            stack.pop();
            Ok(())
        }
        Type::Option(inner) | Type::List(inner) => well_formed_type_inner(inner, aliases, stack),
        Type::Record(fields) => {
            for field in fields.values() {
                well_formed_type_inner(field, aliases, stack)?;
            }
            Ok(())
        }
        Type::Refinement { base, .. } => well_formed_type_inner(base, aliases, stack),
        Type::Function(a, b) => {
            well_formed_type_inner(a, aliases, stack)?;
            well_formed_type_inner(b, aliases, stack)
        }
        _ => Ok(()),
    }
}
