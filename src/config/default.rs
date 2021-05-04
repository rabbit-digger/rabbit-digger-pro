use std::path::PathBuf;

use super::Chain;

pub(crate) fn local_chain() -> Chain {
    Chain::One("local".to_string())
}

pub(crate) fn noop_chain() -> Chain {
    Chain::One("noop".to_string())
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
