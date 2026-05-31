use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::error::{Error, Result};
use crate::eval::builtins::{self, NativeFunction};
use crate::eval::prelude;
use crate::eval::{Value, contains_function, emit, eval};
use crate::lower::lower_file;
use crate::syntax::parser::parse;
use crate::syntax::surface::{Decl, FileAst, Type};
use crate::typeck::bidir::check_expr;
use crate::typeck::wf::well_formed_type;

#[derive(Clone, Default)]
pub struct Module {
    pub values: BTreeMap<String, Value>,
    pub types: BTreeMap<String, Type>,
    pub exports: BTreeMap<String, Export>,
}

#[derive(Clone)]
pub enum Export {
    Value(Value),
    Type(Type),
}

#[derive(Default)]
pub struct Loader {
    cache: HashMap<PathBuf, Module>,
    loading: HashSet<PathBuf>,
}

impl Loader {
    pub fn load(&mut self, path: &Path) -> Result<Module> {
        let path = path
            .canonicalize()
            .map_err(|e| Error::new(format!("unknown import `{}`: {e}", path.display())))?;
        if let Some(module) = self.cache.get(&path) {
            return Ok(module.clone());
        }
        if !self.loading.insert(path.clone()) {
            return Err(Error::new(format!("cyclic import `{}`", path.display())));
        }
        let src = fs::read_to_string(&path)
            .map_err(|e| Error::new(format!("unknown import `{}`: {e}", path.display())))?;
        let ast = lower_file(parse(&src)?);
        let parent = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let module = eval_file(self, &parent, ast)?;
        self.loading.remove(&path);
        self.cache.insert(path, module.clone());
        Ok(module)
    }
}

pub fn eval_file(loader: &mut Loader, base_dir: &Path, ast: FileAst) -> Result<Module> {
    eval_file_inner(loader, base_dir, ast, true)
}

pub(crate) fn eval_file_without_prelude(
    loader: &mut Loader,
    base_dir: &Path,
    ast: FileAst,
) -> Result<Module> {
    eval_file_inner(loader, base_dir, ast, false)
}

fn eval_file_inner(
    loader: &mut Loader,
    base_dir: &Path,
    ast: FileAst,
    include_prelude: bool,
) -> Result<Module> {
    let mut module = if include_prelude {
        prelude::module()
    } else {
        Module::default()
    };

    for decl in ast.decls {
        match decl {
            Decl::Import { path, names } => {
                let imported = loader.load(&base_dir.join(path))?;
                for name in names {
                    if module.values.contains_key(&name) || module.types.contains_key(&name) {
                        return Err(Error::new(format!("duplicate import `{name}`")));
                    }
                    match imported.exports.get(&name) {
                        Some(Export::Value(value)) => {
                            module.values.insert(name, value.clone());
                        }
                        Some(Export::Type(ty)) => {
                            module.types.insert(name, ty.clone());
                        }
                        None => return Err(Error::new(format!("unexported import `{name}`"))),
                    }
                }
            }
            Decl::Native { export, name, ty } => {
                well_formed_type(&ty, &module.types)?;
                let value = builtins::declared(&name)
                    .then(|| Value::Native(NativeFunction::new(name.clone())))
                    .ok_or_else(|| Error::new(format!("unknown native `{name}`")))?;
                module.values.insert(name.clone(), value.clone());
                if export {
                    module.exports.insert(name, Export::Value(value));
                }
            }
            Decl::Type { export, name, ty } => {
                well_formed_type(&ty, &module.types)?;
                module.types.insert(name.clone(), ty.clone());
                if export {
                    module.exports.insert(name, Export::Type(ty));
                }
            }
            Decl::Let {
                export,
                name,
                annotation,
                expr,
            } => {
                let env = Rc::new(module.values.clone());
                let value = if let Some(ty) = annotation {
                    well_formed_type(&ty, &module.types)?;
                    check_expr(&expr, &ty, &env, &module.types)?
                } else {
                    eval(&expr, &env, &module.types)?
                };
                module.values.insert(name.clone(), value.clone());
                if export {
                    module.exports.insert(name, Export::Value(value));
                }
            }
        }
    }

    let env = Rc::new(module.values.clone());
    let output = eval(&ast.output, &env, &module.types)?;
    if contains_function(&output) {
        return Err(Error::new("function escaped into output"));
    }
    module.values.insert("$output".to_string(), output);
    Ok(module)
}

pub fn run(path: &Path) -> Result<String> {
    let mut loader = Loader::default();
    let module = loader.load(path)?;
    let output = module
        .values
        .get("$output")
        .ok_or_else(|| Error::new("internal error: missing output"))?;
    emit(output)
}
