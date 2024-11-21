use thiserror::Error;

pub mod label;
pub mod todo;

#[derive(Debug, Error)]
enum RepositoryError {
    #[error("Unexpected Error: [{0}]")]
    Unexpected(String),
    #[error("NotFound, id is {0}")]
    NotFound(i32),
    #[error("Duplicate data, id is {0}")]
    Duplicate(i32),
}
