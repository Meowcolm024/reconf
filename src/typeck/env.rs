use std::collections::BTreeMap;
use std::rc::Rc;

use crate::eval::Value;
use crate::syntax::surface::Type;

pub type ValueEnv = Rc<BTreeMap<String, Value>>;
pub type TypeEnv = BTreeMap<String, Type>;
