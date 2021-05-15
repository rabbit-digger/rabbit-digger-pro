use super::NetMap;
use crate::{Error, Net, NotImplementedNet, Result};
use serde::de;
use std::{fmt, ops::Deref, sync::Arc};

#[derive(Clone)]
pub struct NetRef {
    name: String,
    net: Option<Net>,
}

impl Default for NetRef {
    fn default() -> Self {
        default_net()
    }
}

impl fmt::Debug for NetRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("NetRef").field(&self.name).finish()
    }
}

fn default_net() -> NetRef {
    NetRef {
        name: "local".to_string(),
        net: None,
    }
}

impl NetRef {
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
    pub fn net(&self) -> Net {
        self.net
            .as_ref()
            .expect("Net must be resolved before used")
            .clone()
    }
}

impl Deref for NetRef {
    type Target = Net;

    fn deref(&self) -> &Self::Target {
        self.net
            .as_ref()
            .expect("Net must be resolved before Deref")
    }
}

impl<'de> de::Deserialize<'de> for NetRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FieldVisitor;
        impl<'de> de::Visitor<'de> for FieldVisitor {
            type Value = NetRef;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "Net name string")
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(NetRef { name: v, net: None })
            }
        }

        deserializer.deserialize_string(FieldVisitor)
    }
}

pub trait ResolveNetRef {
    fn resolve(&mut self, _nets: &NetMap) -> Result<()> {
        Ok(())
    }
    fn get_dependency(&mut self) -> Result<Vec<String>> {
        let noop = Arc::new(NotImplementedNet);
        let mut tmp_map = NetMap::new();
        loop {
            match self.resolve(&tmp_map) {
                Ok(_) => break,
                Err(Error::NotFound(key)) => {
                    tmp_map.insert(key, noop.clone());
                }
                Err(e) => return Err(e),
            }
        }
        Ok(tmp_map.into_iter().map(|i| i.0).collect())
    }
}

impl ResolveNetRef for NetRef {
    fn resolve(&mut self, nets: &NetMap) -> Result<()> {
        let net = nets
            .get(&self.name)
            .ok_or_else(|| crate::Error::NotFound(self.name.clone()))?
            .clone();
        self.net = Some(net);
        Ok(())
    }
}

macro_rules! impl_empty_resolve {
    ($($x:ident),+ $(,)?) => ($(
        impl ResolveNetRef for $x {
            fn resolve(&mut self, _nets: &NetMap) -> Result<()> {
                Ok(())
            }
        }
    )*)
}

impl_empty_resolve! { String, u16 }
