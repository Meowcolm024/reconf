use reconf_core::core::{CoreType, CoreTypeEnv, CoreTypeValidator, TypeAliasRef};
use reconf_core::error::ErrorCode;

#[test]
fn core_type_validator_accepts_known_aliases() {
    let mut aliases = CoreTypeEnv::default();
    aliases.define("Port".to_string(), CoreType::Int);

    let result = CoreTypeValidator::new(&aliases).well_formed(&CoreType::Alias("Port".into()));

    assert!(result.is_ok());
}

#[test]
fn core_type_validator_accepts_resolved_aliases() {
    let mut aliases = CoreTypeEnv::default();
    let port = TypeAliasRef::new(7);
    aliases.define_with_ref("Port".to_string(), port, CoreType::Int);

    let result = CoreTypeValidator::new(&aliases).well_formed(&CoreType::ResolvedAlias(port));

    assert!(result.is_ok());
}

#[test]
fn core_type_validator_rejects_unknown_aliases() {
    let aliases = CoreTypeEnv::default();

    let error = CoreTypeValidator::new(&aliases)
        .well_formed(&CoreType::Spanned(
            Box::new(CoreType::Alias("Missing".into())),
            4..11,
        ))
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::TypeUnknown);
    assert_eq!(error.diagnostic_labels()[0].span, 4..11);
}

#[test]
fn core_type_validator_rejects_recursive_aliases() {
    let mut aliases = CoreTypeEnv::default();
    aliases.define("Port".to_string(), CoreType::Alias("Port".to_string()));

    let error = CoreTypeValidator::new(&aliases)
        .well_formed(&CoreType::Spanned(
            Box::new(CoreType::Alias("Port".into())),
            12..16,
        ))
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::TypeRecursiveAlias);
    assert_eq!(error.diagnostic_labels()[0].span, 12..16);
}
