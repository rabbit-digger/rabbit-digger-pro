use std::path::PathBuf;

pub fn local_string() -> String {
    "local".to_string()
}

pub fn rule() -> String {
    "rule".to_string()
}

pub fn plugins() -> PathBuf {
    PathBuf::from("plugins")
}
