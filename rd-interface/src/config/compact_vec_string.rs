use crate as rd_interface;
use std::fmt;

use schemars::JsonSchema;
use serde::{
    de::{SeqAccess, Visitor},
    Deserialize, Serialize,
};

use crate::impl_empty_config;

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct CompactStringVec {
    underlying: Vec<u8>,
    index: Vec<usize>,
}

impl fmt::Debug for CompactStringVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompactDomainVec")
            .field("underlying", &self.underlying)
            .field("index", &self.index)
            .finish()
    }
}

impl CompactStringVec {
    pub fn new() -> Self {
        Self {
            underlying: Vec::new(),
            index: Vec::new(),
        }
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            underlying: Vec::with_capacity(capacity),
            index: Vec::with_capacity(capacity),
        }
    }
    pub fn push(&mut self, s: impl AsRef<str>) {
        let len = self.underlying.len();
        self.underlying.extend_from_slice(s.as_ref().as_bytes());
        self.index.push(len);
    }
    pub fn pop(&mut self) {
        if let Some(i) = self.index.pop() {
            self.underlying.truncate(i);
        }
    }
    pub fn shrink_to_fit(&mut self) {
        self.underlying.shrink_to_fit();
        self.index.shrink_to_fit();
    }
    pub fn iter(&self) -> Iter {
        Iter {
            inner: &self,
            index: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.index.len()
    }
    pub fn join(&self, sep: &str) -> String {
        self.into_iter().collect::<Vec<_>>().join(sep)
    }
}

impl Extend<String> for CompactStringVec {
    fn extend<T: IntoIterator<Item = String>>(&mut self, iter: T) {
        for s in iter {
            self.push(&s);
        }
    }
}

impl<'a> Extend<&'a str> for CompactStringVec {
    fn extend<T: IntoIterator<Item = &'a str>>(&mut self, iter: T) {
        for s in iter {
            self.push(s);
        }
    }
}

impl FromIterator<String> for CompactStringVec {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        let mut c = Self::new();
        for s in iter {
            c.push(&s);
        }
        c
    }
}

impl<'a> FromIterator<&'a str> for CompactStringVec {
    fn from_iter<I: IntoIterator<Item = &'a str>>(iter: I) -> Self {
        let mut c = Self::new();
        for s in iter {
            c.push(s);
        }
        c
    }
}

impl<'a> IntoIterator for &'a CompactStringVec {
    type Item = &'a str;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct Iter<'a> {
    inner: &'a CompactStringVec,
    index: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.inner.index.len() {
            return None;
        }
        let start = self.inner.index[self.index];
        let end = if self.index + 1 < self.inner.index.len() {
            self.inner.index[self.index + 1]
        } else {
            self.inner.underlying.len()
        };
        self.index += 1;
        Some(unsafe { std::str::from_utf8_unchecked(&self.inner.underlying[start..end]) })
    }
}

impl Serialize for CompactStringVec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(self.iter())
    }
}

impl<'de> Deserialize<'de> for CompactStringVec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StringsVisitor;

        impl<'de> Visitor<'de> for StringsVisitor {
            type Value = CompactStringVec;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "string or [string]")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let len = seq.size_hint().unwrap_or(10);

                let mut values = CompactStringVec::with_capacity(len);

                while let Some(value) = seq.next_element::<String>()? {
                    values.push(&value);
                }

                values.shrink_to_fit();

                Ok(values)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let mut values = CompactStringVec::with_capacity(1);
                values.push(value);
                Ok(values)
            }
        }

        deserializer.deserialize_seq(StringsVisitor)
    }
}

impl JsonSchema for CompactStringVec {
    fn schema_name() -> String {
        "CompactDomainVec".to_string()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        gen.subschema_for::<super::SingleOrVec<String>>()
    }
}

impl<S: AsRef<str>> From<Vec<S>> for CompactStringVec {
    fn from(v: Vec<S>) -> Self {
        let mut r = Self::new();
        for s in v.iter() {
            r.push(s.as_ref());
        }
        r.shrink_to_fit();
        return r;
    }
}

impl From<String> for CompactStringVec {
    fn from(v: String) -> Self {
        let mut r = Self::with_capacity(1);
        r.push(&v);
        return r;
    }
}

impl From<&str> for CompactStringVec {
    fn from(v: &str) -> Self {
        let mut r = Self::with_capacity(1);
        r.push(v);
        return r;
    }
}

impl_empty_config!(CompactStringVec);

impl<I> PartialEq<Vec<I>> for CompactStringVec
where
    I: AsRef<str>,
{
    fn eq(&self, other: &Vec<I>) -> bool {
        if self.len() != other.len() {
            return false;
        }
        let i1 = self.iter();
        let i2 = other.iter().map(|s| s.as_ref());
        i1.eq(i2)
    }
}
