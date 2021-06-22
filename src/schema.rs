use anyhow::Result;
use rabbit_digger::rd_interface::schemars::{
    schema::{
        InstanceType, Metadata, ObjectValidation, RootSchema, Schema, SchemaObject,
        SubschemaValidation,
    },
    visit::{visit_root_schema, visit_schema_object, Visitor},
};
use rabbit_digger::Registry;
use serde_json::Value;
use std::iter::FromIterator;
use std::{collections::BTreeMap, path::Path};
use tokio::fs::{create_dir_all, write};

use crate::plugin_loader;

fn anyof_schema(anyof: Vec<Schema>) -> Schema {
    let mut schema = SchemaObject::default();
    schema.object().additional_properties = Some(Box::new(
        SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            subschemas: Some(
                SubschemaValidation {
                    any_of: Some(anyof),
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        }
        .into(),
    ));
    schema.into()
}

fn append_type(schema: &RootSchema, type_name: &str) -> RootSchema {
    let mut schema = schema.clone();
    schema.schema.object().properties.insert(
        "type".to_string(),
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            const_value: Some(Value::String(type_name.to_string())),
            ..Default::default()
        }
        .into(),
    );
    schema.schema.object().required.insert("type".to_string());
    schema
}

// add prefix to all $ref
struct PrefixVisitor(String);
impl PrefixVisitor {
    fn prefix(&self, key: &str) -> String {
        format!("{}{}", self.0, key)
    }
}
impl Visitor for PrefixVisitor {
    fn visit_schema_object(&mut self, schema: &mut SchemaObject) {
        if let Some(ref mut reference) = schema.reference {
            let r = reference
                .strip_prefix("#/definitions/")
                .unwrap_or(reference);
            *reference = format!("#/definitions/{}", self.prefix(r));
        }
        visit_schema_object(self, schema)
    }
    fn visit_root_schema(&mut self, root: &mut RootSchema) {
        root.definitions = root
            .definitions
            .clone()
            .into_iter()
            .map(|(k, v)| (self.prefix(&k), v))
            .collect();
        visit_root_schema(self, root)
    }
}

pub async fn write_schema(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let schema = generate_schema().await?;
    let schema = serde_json::to_string_pretty(&schema)?;
    if let Some(parent) = path.parent() {
        create_dir_all(parent).await?;
    }
    write(path, schema).await?;

    Ok(())
}

pub async fn generate_schema() -> Result<RootSchema> {
    let mut registry = Registry::new();

    rabbit_digger::builtin::load_builtin(&mut registry)?;
    plugin_loader(&Default::default(), &mut registry)?;

    let mut nets: Vec<Schema> = Vec::new();
    let mut servers: Vec<Schema> = Vec::new();
    let mut root: RootSchema = RootSchema::default();

    for (id, net) in registry.net.iter() {
        let mut schema = append_type(net.resolver.schema(), id);
        let mut visitor = PrefixVisitor(format!("net_{}_", id));

        visitor.visit_root_schema(&mut schema);

        nets.push(schema.schema.into());
        root.definitions.extend(schema.definitions);
    }
    for (id, server) in registry.server.iter() {
        let mut schema = append_type(server.resolver.schema(), id);
        let mut visitor = PrefixVisitor(format!("server_{}_", id));

        visitor.visit_root_schema(&mut schema);
        servers.push(schema.schema.into());
        root.definitions.extend(schema.definitions);
    }

    let string_schema = SchemaObject {
        instance_type: Some(InstanceType::String.into()),
        ..Default::default()
    }
    .into();
    let net_schema = anyof_schema(nets);
    let server_schema = anyof_schema(servers);

    root.schema = SchemaObject {
        instance_type: Some(InstanceType::Object.into()),
        metadata: Some(
            Metadata {
                title: Some("Config".to_string()),
                ..Default::default()
            }
            .into(),
        ),
        object: Some(
            ObjectValidation {
                properties: BTreeMap::from_iter([
                    ("id".to_string(), string_schema),
                    ("net".to_string(), net_schema),
                    ("server".to_string(), server_schema),
                ]),
                ..Default::default()
            }
            .into(),
        ),
        ..Default::default()
    };

    Ok(root)
}
