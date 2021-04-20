mod rule;
mod select;

use std::collections::HashMap;

use anyhow::Result;
use rd_interface::Net;

use crate::config::{Composite, CompositeName};

pub fn build_composite(net: HashMap<String, Net>, config: CompositeName) -> Result<Net> {
    let net = match config.composite.0 {
        Composite::Rule(rule) => rule::Rule::new(net, rule)?,
        Composite::Select => select::SelectNet::new(net, config.net_list.into_net_list()?)?,
    };
    Ok(net)
}
