use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::diagnostic::{Diagnostic, SourceMap, Span};
use crate::eval::{Value, eval, reject_function_output};
use crate::syntax::{FileAst, Parser, TopDecl, Ty};
use crate::typeck::{Ctx, TypeChecker, ValueInfo, runtime_from_ctx, ty_mentions_alias};

#[derive(Clone, Debug, Default)]
struct Exports {
    types: HashMap<String, Ty>,
    values: HashMap<String, ValueInfo>,
}

#[derive(Clone, Debug)]
struct ModuleResult {
    exports: Exports,
    output_value: Value,
}

pub struct Compiler {
    sources: SourceMap,
    modules: HashMap<PathBuf, ModuleResult>,
    stack: Vec<PathBuf>,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            sources: SourceMap::default(),
            modules: HashMap::new(),
            stack: Vec::new(),
        }
    }

    pub fn check_file(&mut self, path: &Path) -> Result<(), Diagnostic> {
        self.load_module(path)?;
        Ok(())
    }

    pub fn eval_file(&mut self, path: &Path) -> Result<Value, Diagnostic> {
        let module = self.load_module(path)?;
        reject_function_output(&module.output_value, self.empty_span())?;
        Ok(module.output_value)
    }

    pub fn render(&self, diagnostic: Diagnostic) -> String {
        self.sources.render(diagnostic)
    }

    fn empty_span(&self) -> Span {
        Span::empty(0, 0)
    }

    fn load_module(&mut self, path: &Path) -> Result<ModuleResult, Diagnostic> {
        let canonical = canonicalize_existing(path)
            .map_err(|message| Diagnostic::new("E_MODULE_001", message, self.empty_span()))?;
        if let Some(module) = self.modules.get(&canonical) {
            return Ok(module.clone());
        }
        if let Some(idx) = self.stack.iter().position(|p| p == &canonical) {
            let cycle = self.stack[idx..]
                .iter()
                .chain(std::iter::once(&canonical))
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(Diagnostic::new(
                "E_MODULE_002",
                format!("cyclic import detected: {cycle}"),
                self.empty_span(),
            ));
        }

        let text = std::fs::read_to_string(&canonical).map_err(|err| {
            Diagnostic::new(
                "E_MODULE_003",
                format!("failed to read {}: {err}", canonical.display()),
                self.empty_span(),
            )
        })?;
        let file_id = self.sources.add(canonical.clone(), text.clone());
        let ast = Parser::parse_file(file_id, &text)?;

        self.stack.push(canonical.clone());
        let module = self.process_module(&canonical, ast);
        self.stack.pop();

        let module = module?;
        self.modules.insert(canonical, module.clone());
        Ok(module)
    }

    fn process_module(&mut self, path: &Path, ast: FileAst) -> Result<ModuleResult, Diagnostic> {
        let mut ctx = Ctx::default();
        let mut exports = Exports::default();
        let checker = TypeChecker;

        for import in ast.imports {
            let import_path = resolve_import_path(path, &import.path)
                .map_err(|message| Diagnostic::new("E_MODULE_004", message, import.span))?;
            let imported = self.load_module(&import_path)?;
            for name in import.names {
                let mut found = false;
                if let Some(ty) = imported.exports.types.get(&name) {
                    if ctx.types.insert(name.clone(), ty.clone()).is_some() {
                        return Err(Diagnostic::new(
                            "E_NAME_001",
                            format!("duplicate imported type `{name}`"),
                            import.span,
                        ));
                    }
                    found = true;
                }
                if let Some(value) = imported.exports.values.get(&name) {
                    if ctx.values.insert(name.clone(), value.clone()).is_some() {
                        return Err(Diagnostic::new(
                            "E_NAME_002",
                            format!("duplicate imported value `{name}`"),
                            import.span,
                        ));
                    }
                    found = true;
                }
                if !found {
                    return Err(Diagnostic::new(
                        "E_MODULE_005",
                        format!("`{name}` is not exported by {}", import_path.display()),
                        import.span,
                    ));
                }
            }
        }

        for decl in ast.decls {
            match decl {
                TopDecl::Type {
                    export,
                    name,
                    ty,
                    span,
                } => {
                    if ctx.types.contains_key(&name) {
                        return Err(Diagnostic::new(
                            "E_NAME_003",
                            format!("duplicate type `{name}`"),
                            span,
                        ));
                    }
                    if ty_mentions_alias(&ty, &name) {
                        return Err(Diagnostic::new(
                            "E_TYPE_002",
                            format!("recursive type alias `{name}`"),
                            ty.span,
                        ));
                    }
                    checker.check_well_formed_type(&ty, &ctx, &mut Vec::new())?;
                    ctx.types.insert(name.clone(), ty.clone());
                    if export {
                        exports.types.insert(name, ty);
                    }
                }
                TopDecl::Let {
                    export,
                    name,
                    ann,
                    value,
                    span,
                } => {
                    if ctx.values.contains_key(&name) {
                        return Err(Diagnostic::new(
                            "E_NAME_004",
                            format!("duplicate value `{name}`"),
                            span,
                        ));
                    }
                    let (ty, elaborated) = if let Some(ann) = ann {
                        checker.check_well_formed_type(&ann, &ctx, &mut Vec::new())?;
                        let elaborated = checker.check_expr(&value, &ann, &ctx)?;
                        (ann, elaborated)
                    } else {
                        checker.synth_expr(&value, &ctx)?
                    };
                    let value = eval(&elaborated, &runtime_from_ctx(&ctx), elaborated.span)?;
                    let info = ValueInfo {
                        ty: ty.clone(),
                        value,
                    };
                    ctx.values.insert(name.clone(), info.clone());
                    if export {
                        exports.values.insert(name, info);
                    }
                }
            }
        }

        let (_output_ty, output_expr) = checker.synth_expr(&ast.output, &ctx)?;
        let output_value = eval(&output_expr, &runtime_from_ctx(&ctx), ast.output.span)?;
        reject_function_output(&output_value, ast.output.span)?;
        Ok(ModuleResult {
            exports,
            output_value,
        })
    }
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(path)
        .map_err(|err| format!("failed to resolve {}: {err}", path.display()))
}

fn resolve_import_path(from: &Path, import: &str) -> Result<PathBuf, String> {
    let base = from.parent().unwrap_or_else(|| Path::new("."));
    let path = base.join(import);
    canonicalize_existing(&path)
}
