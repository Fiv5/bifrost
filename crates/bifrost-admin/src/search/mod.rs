mod engine;
mod types;

pub use engine::SearchEngine;
pub use types::{
    FilterCondition, MatchLocation, SearchFilters, SearchRequest, SearchResponse, SearchResultItem,
    SearchScope,
};
