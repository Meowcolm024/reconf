use crate::compiler::module::state::Module;
use crate::error::{Error, ErrorCode, Result};
use crate::resolve::resolved::{ResolvedImport, ResolvedModule};

pub(super) struct ModuleImportBinder<'a> {
    module: &'a mut Module,
}

impl<'a> ModuleImportBinder<'a> {
    pub(super) fn new(module: &'a mut Module) -> Self {
        Self { module }
    }

    pub(super) fn bind(
        &mut self,
        imported: &ResolvedModule,
        import: &ResolvedImport,
    ) -> Result<()> {
        self.check_available(import.names().iter().map(String::as_str))?;
        let selection = import.select_from(imported.exports())?;
        self.module.import_selection(&selection);
        Ok(())
    }

    fn check_available<'b>(&self, names: impl IntoIterator<Item = &'b str>) -> Result<()> {
        let scope = self.module.name_scope();
        if let Some(collision) = scope.first_collision(names) {
            return Err(Error::with_code(
                ErrorCode::NameDuplicateImport,
                format!("duplicate import `{}`", collision.name()),
            ));
        }
        Ok(())
    }
}
