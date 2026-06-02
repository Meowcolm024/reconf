use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum DataValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    None,
    Some(Box<DataValue>),
    List(Vec<DataValue>),
    Record(BTreeMap<String, DataValue>),
}
