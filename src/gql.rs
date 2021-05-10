mod model;
mod query;

use anyhow::Result;
use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    EmptyMutation, EmptySubscription, Schema,
};
use async_std::task::spawn;
use query::QueryRoot;
use tide::{http::mime, Body, Response, StatusCode};

use crate::controller::Controller;

pub async fn serve(bind: String, controller: &Controller) -> Result<()> {
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(controller.clone())
        .finish();

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
