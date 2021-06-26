use crate::{Net, Result};
use schemars::{
    schema::{InstanceType, SchemaObject},
    JsonSchema,
};
use serde::{de, ser};
use std::{fmt, ops::Deref};

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
    pub(crate) fn set_net(&mut self, net: Net) {
        self.net = Some(net);
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
