use kube::Resource;
use kube::{client::Client, runtime::controller::Action};
use std::path::Path;
use std::sync::Arc;
use tokio::time::Duration;

use crate::finalizer;
use crate::gitlab::Gitlab;
use crate::namespace::{create_namespace, delete_namespace};
use crate::project::{create_project, delete_project};
use crate::project_crd::Project;
use crate::rbacs::{add_rbacs, remove_rbacs};
use crate::secret::{create_secret, delete_secret};

pub struct ContextData {
    pub client: Client,
    pub gitlab: Gitlab,
}

impl ContextData {
    #[allow(dead_code)]
    pub fn new(client: Client, gitlab: Gitlab) -> Self {
        Self { client, gitlab }
    }
}

enum ProjectAction {
    Create,
    Delete,
    NoOp,
}

pub async fn reconcile(project: Arc<Project>, context: Arc<ContextData>) -> Result<Action, Error> {
    let client: Client = context.client.clone();
    let gitlab = context.gitlab.clone();
    let project_name = project.metadata.name.clone().unwrap();
    let google_group = project.spec.googleGroup.clone();
    let repo_root = std::env::var("DEPLOY_ROOT").expect("DEPLOY_ROOT not set");
    let flux_root = std::env::var("FLUX_ROOT").expect("FLUX_ROOT not set");
    let repo_root = Path::new(repo_root.as_str());
    let flux_root = Path::new(flux_root.as_str());
    let namespace: String = match project.metadata.namespace.clone() {
        None => {
            return Err(Error::UserInputError(
                "Project CRD must have a namespace".to_owned(),
            ));
        }
        Some(namespace) => namespace,
    };

    #[allow(clippy::needless_return)]
    return match determine_action(&project) {
        ProjectAction::Create => {
            finalizer::add(client.clone(), &project_name, &namespace).await?;

            match create_namespace(client.clone(), &project_name).await {
                Ok(_) => {}
                Err(e) => {
                    log::error!("Failed to create namespace: {:?}", e);
                }
            }

            let group_id = gitlab.create_group(&project_name).await.unwrap();

            //check if pull token exists
            let pull_token = match gitlab
                .get_group_access_token_id(&format!("{}-image-puller", project_name), &group_id)
                .await
                .unwrap()
            {
                None => {
                    gitlab
                        .create_group_access_token(
                            &format!("{}-image-puller", project_name),
                            &group_id,
                        )
                        .await
                }
                Some(_) => {
                    gitlab
                        .rotate_group_access_token(
                            &format!("{}-image-puller", project_name),
                            &group_id,
                        )
                        .await
                }
            };
            create_secret(client.clone(), &project_name, &pull_token.unwrap())
                .await
                .unwrap();
            create_project(&project_name, repo_root).await.unwrap();
            add_rbacs(&project_name, flux_root, &google_group)
                .await
                .unwrap();

            Ok(Action::requeue(Duration::from_secs(10)))
        }
        ProjectAction::Delete => {
            remove_rbacs(&project_name, flux_root, &google_group)
                .await
                .unwrap();
            match delete_project(&project_name, repo_root).await {
                Ok(_) => {}
                Err(e) => {
                    log::error!("Failed to delete project: {:?}", e);
                }
            }
            delete_secret(client.clone(), &project_name).await.unwrap();
            delete_namespace(client.clone(), &project_name)
                .await
                .unwrap();
            finalizer::delete(client, &project_name, &namespace).await?;
            Ok(Action::await_change())
        }
        ProjectAction::NoOp => Ok(Action::requeue(Duration::from_secs(10))),
    };
}

#[allow(clippy::needless_return)]
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
