use async_graphql::*;
use crate::db::operations::BlockRepository;
use crate::db::address_analytics::AddressAnalyticsRepository;
use crate::graphql::resolvers::QueryResolver;

pub type TaikoSchema = Schema<QueryResolver, EmptyMutation, EmptySubscription>;

pub fn create_schema(block_repo: BlockRepository, analytics_repo: AddressAnalyticsRepository) -> TaikoSchema {
    Schema::build(QueryResolver::new(block_repo, analytics_repo), EmptyMutation, EmptySubscription)
        .finish()
}