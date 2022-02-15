use crate::{self as rd_interface, Address, Net};
use resolvable::{Resolvable, ResolvableSchema};
use schemars::{
    schema::{InstanceType, Metadata, SchemaObject, SubschemaValidation},
    JsonSchema,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{registry::NetGetter, Result};

mod resolvable;

#[derive(Clone)]
pub struct NetSchema;
impl JsonSchema for NetSchema {
    fn is_referenceable() -> bool {
        false
    }
    fn schema_name() -> String {
        "NetRef".into()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            subschemas: Some(
                SubschemaValidation {
                    any_of: Some(vec![
                        gen.subschema_for::<String>(),
                        schemars::schema::Schema::new_ref("#/definitions/Net".into()),
                    ]),
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        }
        .into()
    }
}

impl ResolvableSchema for NetSchema {
    type Represent = Value;
    type Value = Net;
}

pub type NetRef = Resolvable<NetSchema>;

impl Default for NetRef {
    fn default() -> Self {
        NetRef::new("local".into())
    }
}

pub trait Visitor {
    fn visit_net_ref(&mut self, _ctx: &mut VisitorContext, _net_ref: &mut NetRef) -> Result<()> {
        Ok(())
    }
}

pub struct VisitorContext {
    path: Vec<String>,
}

impl VisitorContext {
    pub(crate) fn new() -> VisitorContext {
        VisitorContext { path: Vec::new() }
    }
    pub fn push(&mut self, field: impl Into<String>) -> &mut Self {
        self.path.push(field.into());
        self
    }
    pub fn pop(&mut self) {
        self.path.pop();
    }
    pub(crate) fn path(&self) -> &[String] {
        &self.path
    }
}

pub trait Config {
    fn visit(&mut self, ctx: &mut VisitorContext, visitor: &mut dyn Visitor) -> Result<()>;
}

pub trait ConfigExt: Config {
    // Collect nested nets
    fn resolve_net(&mut self, getter: NetGetter) -> Result<()> {
        struct ResolveNetVisitor<'a>(NetGetter<'a>);

        impl<'a> Visitor for ResolveNetVisitor<'a> {
            fn visit_net_ref(
                &mut self,
                _ctx: &mut VisitorContext,
                net_ref: &mut NetRef,
            ) -> Result<()> {
                let name = net_ref.represent().as_str();
                let net = name
                    .map(|name| self.0(name))
                    .flatten()
                    .ok_or_else(|| crate::Error::NotFound(net_ref.represent().to_string()))?
                    .clone();
                net_ref.set_value(net);
                Ok(())
            }
        }

        self.visit(&mut VisitorContext::new(), &mut ResolveNetVisitor(getter))?;

        Ok(())
    }
}

impl<T: Config> ConfigExt for T {}

impl Config for NetRef {
    fn visit(&mut self, ctx: &mut VisitorContext, visitor: &mut dyn Visitor) -> Result<()> {
        visitor.visit_net_ref(ctx, self)
    }
}

#[macro_export]
macro_rules! impl_empty_config {
    ($($x:ident),+ $(,)?) => ($(
        impl rd_interface::config::Config for $x {
            fn visit(&mut self, _ctx: &mut rd_interface::config::VisitorContext, _visitor: &mut dyn rd_interface::config::Visitor) -> rd_interface::Result<()> {
                Ok(())
            }
        }
    )*)
}

mod impl_std {
    use super::Config;
    use crate as rd_interface;
    use crate::{Address, Result};
    use std::collections::{BTreeMap, HashMap, LinkedList, VecDeque};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

    macro_rules! impl_container_config {
        ($($x:ident),+ $(,)?) => ($(
            impl<T: Config> Config for $x<T> {
                fn visit(&mut self, ctx: &mut rd_interface::config::VisitorContext, visitor: &mut dyn rd_interface::config::Visitor) -> rd_interface::Result<()> {
                    for i in self.iter_mut() {
                        i.visit(ctx, visitor)?;
                    }
                    Ok(())
                }
            }
        )*)
    }
    macro_rules! impl_key_container_config {
        ($($x:ident),+ $(,)?) => ($(
            impl<K, T: Config> Config for $x<K, T> {
                fn visit(&mut self, ctx: &mut rd_interface::config::VisitorContext, visitor: &mut dyn rd_interface::config::Visitor) -> rd_interface::Result<()> {
                    for (_, i) in self.iter_mut() {
                        i.visit(ctx, visitor)?;
                    }
                    Ok(())
                }
            }
        )*)
    }

    impl_empty_config! { Address }
    impl_empty_config! { String, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, bool, f32, f64 }
    impl_empty_config! { IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6 }
    impl_container_config! { Vec, Option, VecDeque, Result, LinkedList }
    impl_key_container_config! { HashMap, BTreeMap }

    impl<T1, T2> rd_interface::config::Config for (T1, T2) {
        fn visit(
            &mut self,
            _ctx: &mut rd_interface::config::VisitorContext,
            _visitor: &mut dyn rd_interface::config::Visitor,
        ) -> rd_interface::Result<()> {
            Ok(())
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EmptyConfig(Value);

impl JsonSchema for EmptyConfig {
    fn schema_name() -> String {
        "EmptyConfig".to_string()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            instance_type: Some(InstanceType::Null.into()),
            format: None,
            ..Default::default()
        }
        .into()
    }
}

crate::impl_empty_config! { EmptyConfig }

impl JsonSchema for Address {
    fn is_referenceable() -> bool {
        false
    }

    fn schema_name() -> String {
        "Address".to_string()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            format: None,
            metadata: Some(
                Metadata {
                    description: Some("An address contains host and port.\nFor example: example.com:80, 1.1.1.1:53, [::1]:443".to_string()),
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{async_trait, rd_config, INet, IntoDyn};
    use std::{collections::HashMap, sync::Arc};

    struct NotImplementedNet;
    #[async_trait]
    impl INet for NotImplementedNet {}

    #[test]
    fn test_net_ref() {
        #[rd_config]
        struct TestConfig {
            net: Vec<NetRef>,
        }

        let mut test: TestConfig = serde_json::from_str(r#"{ "net": ["test"] }"#).unwrap();

        assert_eq!(test.net[0].represent(), "test");

        let mut net_map = HashMap::new();
        let noop = NotImplementedNet.into_dyn();

        net_map.insert("test".to_string(), noop.clone());
        test.resolve_net(&|key| net_map.get(key).map(|i| i.clone()))
            .unwrap();

        assert_eq!(Arc::as_ptr(&test.net[0]), Arc::as_ptr(&noop))
    }
}
