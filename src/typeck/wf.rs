use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::syntax::surface::Type;

pub fn well_formed_type(ty: &Type, aliases: &BTreeMap<String, Type>) -> Result<()> {
    match ty {
        Type::Alias(name) if !aliases.contains_key(name) => {
            Err(Error::new(format!("unknown type `{name}`")))
        }
        Type::Option(inner) | Type::List(inner) => well_formed_type(inner, aliases),
        Type::Record(fields) => {
            for field in fields.values() {
                well_formed_type(field, aliases)?;
            }
            Ok(())
        }
        Type::Refinement { base, .. } => well_formed_type(base, aliases),
        Type::Function(a, b) => {
            well_formed_type(a, aliases)?;
            well_formed_type(b, aliases)
        }
        _ => Ok(()),
    }
}
