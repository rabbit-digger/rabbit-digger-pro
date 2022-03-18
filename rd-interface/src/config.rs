use crate::{self as rd_interface, registry::NetGetter, Address, Net};
pub use resolvable::{Resolvable, ResolvableSchema};
use schemars::{
    schema::{InstanceType, Metadata, SchemaObject, SubschemaValidation},
    JsonSchema,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Result;
pub use compact_vec_string::CompactVecString;
pub use single_or_vec::SingleOrVec;

mod compact_vec_string;
mod resolvable;
mod single_or_vec;

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

pub trait Visitor<T>
where
    T: ResolvableSchema,
{
    #[allow(unused_variables)]
    fn visit_resolvabe(
        &mut self,
        ctx: &mut VisitorContext,
        resolvable: &mut Resolvable<T>,
    ) -> Result<()> {
        Ok(())
    }
}

pub struct VisitorContext {
    path: CompactVecString,
}

impl VisitorContext {
    pub(crate) fn new() -> VisitorContext {
        VisitorContext {
            path: CompactVecString::new(),
        }
    }
    pub fn push(&mut self, field: impl AsRef<str>) -> &mut Self {
        self.path.push(field.as_ref());
        self
    }
    pub fn pop(&mut self) {
        self.path.pop();
    }
    pub fn path(&self) -> &CompactVecString {
        &self.path
    }
}

pub trait Config<R: ResolvableSchema> {
    fn visit<V>(&mut self, ctx: &mut VisitorContext, visitor: &mut V) -> Result<()>
    where
        V: Visitor<R>;
}

impl Config<NetSchema> for NetRef {
    fn visit<V>(&mut self, ctx: &mut VisitorContext, visitor: &mut V) -> Result<()>
    where
        V: Visitor<NetSchema>,
    {
        visitor.visit_resolvabe(ctx, self)
    }
}

#[macro_export]
macro_rules! impl_empty_config {
    ($($x:ident),+ $(,)?) => ($(
        impl<R: rd_interface::config::ResolvableSchema> rd_interface::config::Config<R> for $x {
            fn visit< V>(&mut self, _ctx: &mut rd_interface::config::VisitorContext, _visitor: &mut V) -> rd_interface::Result<()>
            where
                V: rd_interface::config::Visitor<R>
            {
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
    use std::path::PathBuf;

    macro_rules! impl_container_config {
        ($($x:ident),+ $(,)?) => ($(
            impl<R: rd_interface::config::ResolvableSchema, T: Config<R>> Config<R> for $x<T> {
                fn visit<V>(&mut self, ctx: &mut rd_interface::config::VisitorContext, visitor: &mut V) -> rd_interface::Result<()>
                where
                    V: rd_interface::config::Visitor<R>
                {
                    for (key, i) in self.iter_mut().enumerate() {
                        ctx.push(key.to_string());
                        i.visit(ctx, visitor)?;
                        ctx.pop();
                    }
                    Ok(())
                }
            }
        )*)
    }
    macro_rules! impl_key_container_config {
        ($($x:ident),+ $(,)?) => ($(
            impl<K: std::string::ToString, R: rd_interface::config::ResolvableSchema, T: Config<R>> Config<R> for $x<K, T> {
                fn visit<V>(&mut self, ctx: &mut rd_interface::config::VisitorContext, visitor: &mut V) -> rd_interface::Result<()>
                where
                    V: rd_interface::config::Visitor<R>
                {
                    for (key, i) in self.iter_mut() {
                        ctx.push(key.to_string());
                        i.visit(ctx, visitor)?;
                        ctx.pop();
                    }
                    Ok(())
                }
            }
        )*)
    }

    impl_empty_config! { Address }
    impl_empty_config! { String, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, bool, f32, f64 }
    impl_empty_config! { IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6 }
    impl_empty_config! { PathBuf }
    impl_container_config! { Vec, Option, VecDeque, Result, LinkedList }
    impl_key_container_config! { HashMap, BTreeMap }

    impl<T1, T2, R: rd_interface::config::ResolvableSchema> rd_interface::config::Config<R>
        for (T1, T2)
    {
        fn visit<V>(
            &mut self,
            _ctx: &mut rd_interface::config::VisitorContext,
            _visitor: &mut V,
        ) -> rd_interface::Result<()>
        where
            V: rd_interface::config::Visitor<R>,
        {
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

crate::impl_empty_config! { EmptyConfig, Value }

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

pub fn resolve_net(config: &mut impl Config<NetSchema>, getter: NetGetter) -> Result<()> {
    struct ResolveNetVisitor<'a>(NetGetter<'a>);

    impl<'a> Visitor<NetSchema> for ResolveNetVisitor<'a> {
        fn visit_resolvabe(
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

    config.visit(&mut VisitorContext::new(), &mut ResolveNetVisitor(getter))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{async_trait, rd_config, INet, IntoDyn};
    use std::collections::HashMap;

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
        resolve_net(&mut test, &|key| net_map.get(key).map(|i| i.clone())).unwrap();

        assert_eq!(test.net[0].as_ptr(), noop.as_ptr())
    }
}
