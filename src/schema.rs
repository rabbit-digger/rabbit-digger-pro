use anyhow::Result;
use rabbit_digger::Registry;
use std::path::PathBuf;

use crate::plugin_loader;

pub async fn generate_schema(path: PathBuf) -> Result<()> {
    use rabbit_digger::rd_interface::schemars::schema::{
        InstanceType, Metadata, ObjectValidation, RootSchema, Schema, SchemaObject, SingleOrVec,
        SubschemaValidation,
    };
    use serde_json::Value;
    use std::collections::BTreeMap;
    use std::iter::FromIterator;
    use tokio::fs::{create_dir_all, write};

    fn anyof_schema(anyof: Vec<Schema>) -> Schema {
        SchemaObject {
            object: Some(
                ObjectValidation {
                    additional_properties: Some(Box::new(
                        SchemaObject {
                            instance_type: Some(SingleOrVec::Single(InstanceType::Object.into())),
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
                    )),
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        }
        .into()
    }
    fn append_type(schema: &RootSchema, type_name: &str) -> RootSchema {
        let mut schema = schema.clone();
        if let Some(ref mut obj) = schema.schema.object {
            obj.properties.insert(
                "type".to_string(),
                SchemaObject {
                    instance_type: Some(SingleOrVec::Single(InstanceType::String.into())),
                    const_value: Some(Value::String(type_name.to_string())),
                    ..Default::default()
                }
                .into(),
            );
            obj.required.insert("type".to_string());
        }
        schema
    }

    let mut registry = Registry::new();

    rabbit_digger::builtin::load_builtin(&mut registry)?;
    plugin_loader(&Default::default(), &mut registry)?;

    let net_root = path.join("net");
    let server_root = path.join("server");

    create_dir_all(&net_root).await?;
    create_dir_all(&server_root).await?;

    for (id, net) in registry.net.iter() {
        let schema = net.resolver.schema();
        let schema = serde_json::to_string_pretty(&append_type(schema, id))?;
        write(net_root.join(format!("{}.json", id)), schema).await?;
    }
    for (id, server) in registry.server.iter() {
        let schema = server.resolver.schema();
        let schema = serde_json::to_string_pretty(&append_type(schema, id))?;
        write(server_root.join(format!("{}.json", id)), schema).await?;
    }

    let nets: Vec<_> = registry
        .net
        .iter()
        .map(|(k, _)| Schema::new_ref(format!("net/{}.json", k)))
        .collect();

    let servers: Vec<_> = registry
        .server
        .iter()
        .map(|(k, _)| Schema::new_ref(format!("server/{}.json", k)))
        .collect();

    let net_ref = anyof_schema(nets);
    let server_ref = anyof_schema(servers);

    let root: RootSchema = RootSchema {
        schema: SchemaObject {
            instance_type: Some(SingleOrVec::Single(InstanceType::Object.into())),
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
                        ("net".to_string(), net_ref),
                        ("server".to_string(), server_ref),
                    ]),
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        },
        ..Default::default()
    };

    let schema = serde_json::to_string_pretty(&root)?;
    write(path.join("config.json"), schema).await?;

    Ok(())
}
