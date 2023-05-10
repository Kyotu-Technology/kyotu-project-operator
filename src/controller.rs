use kube::Resource;
use kube::{client::Client, runtime::controller::Action};
use std::path::Path;
use std::sync::Arc;
use tokio::time::Duration;

use crate::finalizer;
use crate::project::{create_project, delete_project};
use crate::project_crd::Project;

pub struct ContextData {
    pub client: Client,
}

impl ContextData {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

enum ProjectAction {
    Create,
    Delete,
    NoOp,
}

pub async fn reconcile(project: Arc<Project>, context: Arc<ContextData>) -> Result<Action, Error> {
    let client: Client = context.client.clone();
    let project_name = project.metadata.name.clone().unwrap();
    let repo_root = std::env::var("REPO_ROOT").expect("REPO_ROOT not set");
    let namespace: String = match project.metadata.namespace.clone() {
        None => {
            return Err(Error::UserInputError(
                "Project CRD must have a namespace".to_owned(),
            ));
        }
        Some(namespace) => namespace,
    };

    return match determine_action(&project) {
        ProjectAction::Create => {
            let repo_root = Path::new(repo_root.as_str());
            finalizer::add(client.clone(), &project_name, &namespace).await?;
            create_project(&project_name, repo_root).await.unwrap();
            Ok(Action::requeue(Duration::from_secs(10)))
        }
        ProjectAction::Delete => {
            let repo_root = Path::new(repo_root.as_str());
            match delete_project(&project_name, repo_root).await {
                Ok(_) => {}
                Err(e) => {
                    log::error!("Failed to delete project: {:?}", e);
                }
            }
            finalizer::delete(client, &project_name, &namespace).await?;
            Ok(Action::await_change())
        }
        ProjectAction::NoOp => Ok(Action::requeue(Duration::from_secs(10))),
    };
}

//determine action to take based on the state of the echo CRD
fn determine_action(project: &Project) -> ProjectAction {
    return if project.meta().deletion_timestamp.is_some() {
        ProjectAction::Delete
    } else if project
        .meta()
        .finalizers
        .as_ref()
        .map_or(true, |finalizers| finalizers.is_empty())
    {
        ProjectAction::Create
    } else {
        ProjectAction::NoOp
    };
}

//error handling
pub fn on_error(echo: Arc<Project>, error: &Error, _context: Arc<ContextData>) -> Action {
    eprintln!("Reconciliation error:\n{:?}.\n{:?}", error, echo);
    Action::requeue(Duration::from_secs(5))
}

//error enum
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Any error originating from the `kube-rs` crate
    #[error("Kubernetes reported error: {source}")]
    KubeError {
        #[from]
        source: kube::Error,
    },
    /// Error in user input or Echo resource definition, typically missing fields.
    #[error("Invalid Project CRD: {0}")]
    UserInputError(String),
}
