mod any;
mod domain;
mod ip_cidr;
mod matcher;
mod rule;
mod udp;

use self::matcher::BoxMatcher;
use crate::config::Matcher;
pub use rule::RuleNet;
