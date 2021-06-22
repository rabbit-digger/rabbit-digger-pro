use super::NetMap;
use crate::{Net, Result};
use schemars::{
    schema::{InstanceType, SchemaObject},
    JsonSchema,
};
use serde::{de, ser};
use std::{collections::HashSet, fmt, ops::Deref};

/// `NetRef` represents a reference to another `Net`. It is a string in the configuration file.
/// The default value is `"local"`.
#[derive(Clone)]
pub struct NetRef {
    name: String,
    net: Option<Net>,
}

impl From<String> for NetRef {
    fn from(name: String) -> Self {
        NetRef { name, net: None }
    }
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

impl ser::Serialize for NetRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.name)
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

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(NetRef {
                    name: v.to_string(),
                    net: None,
                })
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

impl JsonSchema for NetRef {
    fn is_referenceable() -> bool {
        false
    }

    fn schema_name() -> String {
        "NetRef".to_string()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            format: None,
            ..Default::default()
        }
        .into()
    }
}

/// `ResolveNetRef` parses all internal `NetRef`s from strings to real `Net` values.
pub trait ResolveNetRef {
    /// After calling resolve, all internal `NetRef`s will be filled with the corresponding Net in `nets`.
    fn resolve(&mut self, _nets: &NetMap) -> Result<()>;
    /// Get all internal `NetRef`s.
    fn get_dependency_set(&mut self, nets: &mut HashSet<String>) -> Result<()>;
    fn get_dependency(&mut self) -> Result<Vec<String>> {
        let mut nets = HashSet::new();
        self.get_dependency_set(&mut nets)?;
        Ok(nets.into_iter().collect())
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
    fn get_dependency_set(&mut self, nets: &mut HashSet<String>) -> Result<()> {
        nets.insert(self.name.clone());
        Ok(())
    }
}

#[macro_export]
macro_rules! impl_empty_net_resolve {
    ($($x:ident),+ $(,)?) => ($(
        impl rd_interface::registry::ResolveNetRef for $x {
            fn resolve(&mut self, _nets: &rd_interface::registry::NetMap) -> rd_interface::Result<()> {
                Ok(())
            }
            fn get_dependency_set(&mut self, _nets: &mut std::collections::HashSet<String>) -> rd_interface::Result<()> {
                Ok(())
            }
        }
    )*)
}

mod impl_std {
    use super::{NetMap, ResolveNetRef};
    use crate as rd_interface;
    use crate::Result;
    use std::collections::{BTreeMap, HashMap, HashSet, LinkedList, VecDeque};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

    macro_rules! impl_container_resolve {
        ($($x:ident),+ $(,)?) => ($(
            impl<T: ResolveNetRef> ResolveNetRef for $x<T> {
                fn resolve(&mut self, nets: &NetMap) -> rd_interface::Result<()> {
                    for i in self.iter_mut() {
                        i.resolve(nets)?;
                    }
                    Ok(())
                }
                fn get_dependency_set(&mut self, nets: &mut HashSet<String>) -> rd_interface::Result<()> {
                    for i in self.iter_mut() {
                        i.get_dependency_set(nets)?;
                    }
                    Ok(())
                }
            }
        )*)
    }
    macro_rules! impl_key_container_resolve {
        ($($x:ident),+ $(,)?) => ($(
            impl<K, T: ResolveNetRef> ResolveNetRef for $x<K, T> {
                fn resolve(&mut self, nets: &NetMap) -> rd_interface::Result<()> {
                    for (_, i) in self.iter_mut() {
                        i.resolve(nets)?;
                    }
                    Ok(())
                }
                fn get_dependency_set(&mut self, nets: &mut HashSet<String>) -> rd_interface::Result<()> {
                    for (_, i) in self.iter_mut() {
                        i.get_dependency_set(nets)?;
                    }
                    Ok(())
                }
            }
        )*)
    }

    impl_empty_net_resolve! { String, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, bool, f32, f64 }
    impl_empty_net_resolve! { IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6 }
    impl_container_resolve! { Vec, Option, VecDeque, Result, LinkedList }
    impl_key_container_resolve! { HashMap, BTreeMap }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IntoDyn, NotImplementedNet};
    use serde::Deserialize;
    use std::sync::Arc;

    #[test]
    fn test_net_ref() {
        #[derive(Deserialize)]
        struct TestConfig {
            net: Vec<NetRef>,
        }

        let mut test: TestConfig = serde_json::from_str(r#"{ "net": ["test"] }"#).unwrap();

        assert_eq!(test.net[0].name(), "test");

        let mut net_map = NetMap::new();
        let noop = NotImplementedNet.into_dyn();

        net_map.insert("test".to_string(), noop.clone());
        test.net.resolve(&net_map).unwrap();

        assert_eq!(Arc::as_ptr(&test.net[0]), Arc::as_ptr(&noop))
    }
}
