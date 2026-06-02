use crate::core::{CoreType, TypeAliasRef};
use crate::error::{Error, ErrorCode, Result};

pub trait CoreTypeContext {
    fn alias(&self, name: &str) -> Option<&CoreType>;

    fn alias_by_ref(&self, _: TypeAliasRef) -> Option<&CoreType> {
        None
    }
}

#[derive(Default)]
pub struct EmptyCoreTypeContext;

impl CoreTypeContext for EmptyCoreTypeContext {
    fn alias(&self, _: &str) -> Option<&CoreType> {
        None
    }
}

#[derive(Clone, Default)]
pub struct CoreTypeEnv {
    alias_ids: std::collections::BTreeMap<String, TypeAliasRef>,
    aliases_by_id: std::collections::BTreeMap<TypeAliasRef, CoreType>,
    aliases: std::collections::BTreeMap<String, CoreType>,
    next_alias_id: usize,
}

impl CoreTypeEnv {
    pub fn define(&mut self, name: String, ty: CoreType) -> Option<CoreType> {
        let alias = self.alias_ids.get(&name).copied().unwrap_or_else(|| {
            let alias = TypeAliasRef::new(self.next_alias_id);
            self.next_alias_id += 1;
            self.alias_ids.insert(name.clone(), alias);
            alias
        });
        self.aliases_by_id.insert(alias, ty.clone());
        self.aliases.insert(name, ty)
    }

    pub fn define_with_ref(
        &mut self,
        name: String,
        alias: TypeAliasRef,
        ty: CoreType,
    ) -> Option<CoreType> {
        self.next_alias_id = self.next_alias_id.max(alias.id() + 1);
        self.alias_ids.insert(name.clone(), alias);
        self.aliases_by_id.insert(alias, ty.clone());
        self.aliases.insert(name, ty)
    }

    pub fn define_names_with(&self, mut define: impl FnMut(String)) {
        for name in self.aliases.keys() {
            define(name.clone());
        }
    }

    pub fn alias_ref(&self, name: &str) -> Option<TypeAliasRef> {
        self.alias_ids.get(name).copied()
    }
}

impl CoreTypeContext for CoreTypeEnv {
    fn alias(&self, name: &str) -> Option<&CoreType> {
        self.aliases.get(name)
    }

    fn alias_by_ref(&self, alias: TypeAliasRef) -> Option<&CoreType> {
        self.aliases_by_id.get(&alias)
    }
}

pub struct CoreTypeEquivalence;

impl CoreTypeEquivalence {
    pub fn equivalent(left: &CoreType, right: &CoreType) -> bool {
        match (left.as_unspanned(), right.as_unspanned()) {
            (CoreType::Int, CoreType::Int)
            | (CoreType::Float, CoreType::Float)
            | (CoreType::Bool, CoreType::Bool)
            | (CoreType::String, CoreType::String) => true,
            (CoreType::LiteralUnion(left), CoreType::LiteralUnion(right)) => left == right,
            (CoreType::Option(left), CoreType::Option(right))
            | (CoreType::List(left), CoreType::List(right)) => Self::equivalent(left, right),
            (CoreType::Record(left), CoreType::Record(right)) => {
                left.len() == right.len()
                    && left.iter().all(|(name, left_ty)| {
                        right
                            .get(name)
                            .is_some_and(|right_ty| Self::equivalent(left_ty, right_ty))
                    })
            }
            (
                CoreType::Refinement {
                    binder: left_binder,
                    base: left_base,
                    pred: left_pred,
                },
                CoreType::Refinement {
                    binder: right_binder,
                    base: right_base,
                    pred: right_pred,
                },
            ) => {
                left_binder == right_binder
                    && Self::equivalent(left_base, right_base)
                    && left_pred == right_pred
            }
            (CoreType::Function(left_in, left_out), CoreType::Function(right_in, right_out)) => {
                Self::equivalent(left_in, right_in) && Self::equivalent(left_out, right_out)
            }
            (CoreType::Alias(left), CoreType::Alias(right)) => left == right,
            (CoreType::ResolvedAlias(left), CoreType::ResolvedAlias(right)) => left == right,
            _ => false,
        }
    }
}

pub struct CoreTypeValidator<'a> {
    aliases: &'a dyn CoreTypeContext,
}

impl<'a> CoreTypeValidator<'a> {
    pub fn new(aliases: &'a dyn CoreTypeContext) -> Self {
        Self { aliases }
    }

    pub fn well_formed(&self, ty: &CoreType) -> Result<()> {
        self.well_formed_inner(ty, &mut Vec::new())
    }

    pub fn mentions_alias(
        &self,
        ty: &CoreType,
        name: &str,
        alias_ref: Option<TypeAliasRef>,
    ) -> bool {
        match ty {
            CoreType::Spanned(ty, _) => self.mentions_alias(ty, name, alias_ref),
            CoreType::Alias(alias) => alias == name,
            CoreType::ResolvedAlias(alias) => Some(*alias) == alias_ref,
            CoreType::Option(inner) | CoreType::List(inner) => {
                self.mentions_alias(inner, name, alias_ref)
            }
            CoreType::LiteralUnion(_) => false,
            CoreType::Record(fields) => fields
                .values()
                .any(|ty| self.mentions_alias(ty, name, alias_ref)),
            CoreType::Refinement { base, .. } => self.mentions_alias(base, name, alias_ref),
            CoreType::Function(input, output) => {
                self.mentions_alias(input, name, alias_ref)
                    || self.mentions_alias(output, name, alias_ref)
            }
            CoreType::Int | CoreType::Float | CoreType::Bool | CoreType::String => false,
        }
    }

    fn well_formed_inner(&self, ty: &CoreType, stack: &mut Vec<String>) -> Result<()> {
        match ty {
            CoreType::Spanned(ty, span) => self
                .well_formed_inner(ty, stack)
                .map_err(|error| label_type_error(error, span.clone())),
            CoreType::Alias(name) => {
                if stack.contains(name) {
                    return Err(Error::with_code(
                        ErrorCode::TypeRecursiveAlias,
                        format!("recursive type alias `{name}`"),
                    ));
                }
                let Some(alias) = self.aliases.alias(name) else {
                    return Err(Error::with_code(
                        ErrorCode::TypeUnknown,
                        format!("unknown type `{name}`"),
                    ));
                };
                stack.push(name.clone());
                self.well_formed_inner(alias, stack)?;
                stack.pop();
                Ok(())
            }
            CoreType::ResolvedAlias(alias) => {
                let Some(alias) = self.aliases.alias_by_ref(*alias) else {
                    return Err(Error::with_code(
                        ErrorCode::TypeUnknown,
                        "unknown type alias",
                    ));
                };
                self.well_formed_inner(alias, stack)
            }
            CoreType::Option(inner) | CoreType::List(inner) => self.well_formed_inner(inner, stack),
            CoreType::LiteralUnion(_) => Ok(()),
            CoreType::Record(fields) => {
                for field in fields.values() {
                    self.well_formed_inner(field, stack)?;
                }
                Ok(())
            }
            CoreType::Refinement { base, .. } => self.well_formed_inner(base, stack),
            CoreType::Function(input, output) => {
                self.well_formed_inner(input, stack)?;
                self.well_formed_inner(output, stack)
            }
            CoreType::Int | CoreType::Float | CoreType::Bool | CoreType::String => Ok(()),
        }
    }
}

fn label_type_error(error: Error, span: std::ops::Range<usize>) -> Error {
    if !error.diagnostic_labels().is_empty() {
        return error;
    }

    match error.code() {
        ErrorCode::TypeRecursiveAlias | ErrorCode::TypeUnknown => {
            let message = error.message().to_string();
            error.with_label(span, message)
        }
        _ => error,
    }
}
