/// Metrics
mod metrics;
pub use metrics::*;

pub mod controller;
pub use crate::controller::*;

mod gitlab;
pub use gitlab::Gitlab;

mod project_crd;
pub use project_crd::Project;

mod namespace;
pub use namespace::{create_namespace, delete_namespace};

mod finalizer;
pub use finalizer::{add, delete};

mod project;
pub use project::{create_project, delete_project};

mod secret;
pub use secret::{create_secret, delete_secret};

mod rbacs;
pub use rbacs::{add_rbacs, remove_rbacs};

mod repository;
pub use repository::Repository;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("SerializationError: {0}")]
    SerializationError(#[source] serde_json::Error),

    #[error("Kube Error: {0}")]
    KubeError(#[source] kube::Error),

    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    FinalizerError(#[source] Box<kube::runtime::finalizer::Error<Error>>),

    #[error("IllegalDocument")]
    IllegalDocument,

    #[error("Invalid Project CRD: {0}")]
    UserInputError(String),
}

impl Error {
    pub fn metric_label(&self) -> String {
        format!("{self:?}").to_lowercase()
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
