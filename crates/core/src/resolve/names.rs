use std::collections::BTreeSet;

pub use crate::core::{GlobalRef as BindingId, TypeAliasRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingIds {
    next: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeAliasIds {
    next: usize,
}

impl BindingIds {
    pub fn new() -> Self {
        Self { next: 0 }
    }

    pub fn from_next(next: usize) -> Self {
        Self { next }
    }

    pub fn fresh(&mut self) -> BindingId {
        let id = BindingId::new(self.next);
        self.next += 1;
        id
    }

    pub fn next(&self) -> usize {
        self.next
    }

    pub fn reserve(&mut self, id: BindingId) {
        self.next = self.next.max(id.id() + 1);
    }
}

impl Default for BindingIds {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeAliasIds {
    pub fn new() -> Self {
        Self { next: 0 }
    }

    pub fn fresh(&mut self) -> TypeAliasRef {
        let id = TypeAliasRef::new(self.next);
        self.next += 1;
        id
    }
}

impl Default for TypeAliasIds {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Namespace {
    Value,
    Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameCollision {
    name: String,
    namespace: Namespace,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameScope {
    values: BTreeSet<String>,
    types: BTreeSet<String>,
}

impl NameScope {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn define(&mut self, namespace: Namespace, name: impl Into<String>) {
        match namespace {
            Namespace::Value => {
                self.values.insert(name.into());
            }
            Namespace::Type => {
                self.types.insert(name.into());
            }
        }
    }

    pub fn contains(&self, namespace: Namespace, name: &str) -> bool {
        match namespace {
            Namespace::Value => self.values.contains(name),
            Namespace::Type => self.types.contains(name),
        }
    }

    pub fn contains_any(&self, name: &str) -> bool {
        self.contains(Namespace::Value, name) || self.contains(Namespace::Type, name)
    }

    pub fn collision(&self, name: &str) -> Option<NameCollision> {
        if self.contains(Namespace::Value, name) {
            Some(NameCollision::new(name, Namespace::Value))
        } else if self.contains(Namespace::Type, name) {
            Some(NameCollision::new(name, Namespace::Type))
        } else {
            None
        }
    }

    pub fn first_collision<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Option<NameCollision> {
        for name in names {
            if let Some(collision) = self.collision(name) {
                return Some(collision);
            }
        }
        None
    }
}

impl NameCollision {
    pub fn new(name: impl Into<String>, namespace: Namespace) -> Self {
        Self {
            name: name.into(),
            namespace,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn namespace(&self) -> Namespace {
        self.namespace
    }
}
