mod rule;

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use rd_interface::Net;

pub fn build_composite(net: HashMap<String, Net>, config: crate::config::Composite) -> Result<Net> {
    let net = match config.composite_type.as_ref() {
        "rule" => rule::Rule::new(net, serde_json::from_value(config.rest)?)?,
        _ => {
            return Err(anyhow!(
                "composite type {} is not found",
                config.composite_type
            ))
        }
    };
    Ok(net)
}
