use anyhow::Result;
use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    Context, EmptyMutation, EmptySubscription, Object, Schema, SimpleObject,
};
use async_std::task::spawn;
use tide::{http::mime, Body, Response, StatusCode};

use crate::controller::Controller;

#[derive(SimpleObject)]
pub struct Demo {
    pub id: usize,
}

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn demo(&self, _ctx: &Context<'_>) -> Demo {
        Demo { id: 42 }
    }
}

pub async fn serve(bind: String, controller: &Controller) -> Result<()> {
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription).finish();

    let mut app = tide::new();
    app.at("/graphql")
        .post(async_graphql_tide::endpoint(schema.clone()))
        .get(async_graphql_tide::Subscription::new(schema));

    app.at("/").get(|_| async move {
        Ok(Response::builder(StatusCode::Ok)
            .body(Body::from_string(playground_source(
                GraphQLPlaygroundConfig::new("/graphql").subscription_endpoint("/graphql"),
            )))
            .content_type(mime::HTML)
            .build())
    });

    spawn(async move {
        if let Err(e) = app.listen(bind).await {
            log::error!("GraphQL endpoint exited: {:?}", e);
        }
    });

    Ok(())
}
