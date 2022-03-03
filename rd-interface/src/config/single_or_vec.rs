use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Config, Visitor, VisitorContext};
use crate::Result;

#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SingleOrVec<T> {
    Single(T),
    Vec(Vec<T>),
}

impl<T: Config> Config for SingleOrVec<T> {
    fn visit(&mut self, ctx: &mut VisitorContext, visitor: &mut dyn Visitor) -> Result<()> {
        match self {
            SingleOrVec::Single(x) => x.visit(ctx, visitor)?,
            SingleOrVec::Vec(x) => {
                for x in x.iter_mut() {
                    x.visit(ctx, visitor)?;
                }
            }
        }
        Ok(())
    }
}

impl<T> From<T> for SingleOrVec<T> {
    fn from(x: T) -> Self {
        SingleOrVec::Single(x)
    }
}

impl<T> From<Vec<T>> for SingleOrVec<T> {
    fn from(x: Vec<T>) -> Self {
        SingleOrVec::Vec(x)
    }
}

impl<T> SingleOrVec<T> {
    pub fn into_vec(self) -> Vec<T> {
        match self {
            SingleOrVec::Single(t) => vec![t],
            SingleOrVec::Vec(v) => v,
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        Iter {
            inner: self,
            index: 0,
        }
    }
}

pub struct Iter<'a, T> {
    inner: &'a SingleOrVec<T>,
    index: usize,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner {
            SingleOrVec::Single(x) => {
                if self.index == 0 {
                    self.index += 1;
                    Some(x)
                } else {
                    None
                }
            }
            SingleOrVec::Vec(x) => {
                let i = x.get(self.index);
                self.index += 1;
                i
            }
        }
    }
}
