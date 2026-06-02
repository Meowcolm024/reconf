use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::compiler::module::Module;
use reconf_core::error::{Error, ErrorCode, Result};
use reconf_core::resolve::resolved::{ResolvedModule, ResolvedModuleBuilder, ResolvedProgram};

#[derive(Default)]
pub(super) struct ModuleGraph {
    cache: HashMap<PathBuf, ResolvedModule>,
    resolved_program: ResolvedProgram,
    loading: HashSet<PathBuf>,
}

impl ModuleGraph {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn resolved_program(&self) -> &ResolvedProgram {
        &self.resolved_program
    }

    pub(super) fn cached_resolved(&self, path: &Path) -> Option<ResolvedModule> {
        self.cache
            .get(path)
            .cloned()
            .or_else(|| self.resolved_program.module(path).cloned())
    }

    pub(super) fn begin_loading(
        &mut self,
        builder: ResolvedModuleBuilder,
    ) -> Result<LoadingModule> {
        let path = builder.path().to_path_buf();
        if !self.loading.insert(path.to_path_buf()) {
            return Err(Error::with_code(
                ErrorCode::ModuleCycle,
                format!("cyclic import `{}`", path.display()),
            ));
        }
        Ok(LoadingModule { path, builder })
    }

    pub(super) fn finish_loading(
        &mut self,
        loading: LoadingModule,
        module: Module,
    ) -> ResolvedModule {
        self.loading.remove(&loading.path);
        let loaded = self.record_loaded(loading.builder, module);
        self.cache.insert(loading.path, loaded.clone());
        loaded
    }

    pub(super) fn abort_loading(&mut self, loading: LoadingModule) {
        self.loading.remove(&loading.path);
    }

    fn record_loaded(&mut self, builder: ResolvedModuleBuilder, module: Module) -> ResolvedModule {
        let resolved = builder.finish(module.resolved_exports());
        self.resolved_program.insert_module(resolved.clone());
        resolved
    }
}

pub(super) struct LoadingModule {
    path: PathBuf,
    builder: ResolvedModuleBuilder,
}
