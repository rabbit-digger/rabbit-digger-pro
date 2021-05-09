use std::path::PathBuf;

pub(crate) fn local_chain() -> Vec<String> {
    vec!["local".to_string()]
}

pub(crate) fn noop_chain() -> Vec<String> {
    vec!["noop".to_string()]
}

pub(crate) fn local_string() -> String {
    "local".to_string()
}

pub(crate) fn rule() -> String {
    "rule".to_string()
}

pub(crate) fn plugins() -> PathBuf {
    PathBuf::from("plugins")
}
