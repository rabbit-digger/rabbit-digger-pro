use super::model;
use crate::controller::Controller;
use async_graphql::{Context, Object};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn config<'a>(
        &'a self,
        ctx: &Context<'a>,
    ) -> async_graphql::Result<Option<model::Config<'a>>> {
        let ctl = ctx.data::<Controller>()?;
        let inner = ctl.inner().await;

        if inner.config().is_none() {
            return Ok(None);
        }

        Ok(Some(model::Config(inner)))
    }
}
