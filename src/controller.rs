use chrono::{DateTime, Utc};
use futures::StreamExt;
use kube::{
    api::Api,
    client::Client,
    runtime::{
        controller::{Action, Controller},
        events::{Event, EventType},
        events::{Recorder, Reporter},
        watcher::Config,
    },
    Resource,
};
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use tokio::{sync::RwLock, time::Duration};
use tracing::info;

use crate::finalizer;
use crate::gitlab::Gitlab;
use crate::namespace::{create_namespace, delete_namespace};
use crate::project::{create_project, delete_project};
use crate::project_crd::Project;
use crate::rbacs::{add_rbacs, remove_rbacs};
use crate::secret::{create_secret, delete_secret};
use crate::{Error, Metrics, Result};

#[derive(Clone)]
pub struct Context {
    pub client: Client,
    pub gitlab: Gitlab,
    /// Diagnostics read by the web server
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    /// Prometheus metrics
    pub metrics: Metrics,
}

enum ProjectAction {
    Create,
    Delete,
    NoOp,
}

pub async fn reconcile(project: Arc<Project>, context: Arc<Context>) -> Result<Action> {
    let _timer = context.metrics.count_and_measure();
    context.diagnostics.write().await.last_event = Utc::now();

    let client: Client = context.client.clone();
    let gitlab = context.gitlab.clone();

    let project_id = project.spec.project_id.clone();
    let google_group = project.spec.google_group.clone();
    let environment_type = project.spec.environment_type.clone();
    let project_name = format!("{}-{}", project_id, environment_type);

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
            let recorder = context
                .diagnostics
                .read()
                .await
                .recorder(client.clone(), &project);
            finalizer::add(
                client.clone(),
                project.metadata.name.as_ref().unwrap(),
                &namespace,
            )
            .await
            .unwrap();

            match create_namespace(client.clone(), &project_name).await {
                Ok(_) => {}
                Err(e) => {
                    log::error!("Failed to create namespace: {:?}", e);
                }
            }

            let group_id = gitlab.create_group(&project_id).await.unwrap();

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

            recorder
                .publish(Event {
                    type_: EventType::Normal,
                    reason: "Create".into(),
                    note: Some(format!("Creating `{project_name}`")),
                    action: "Creating".into(),
                    secondary: None,
                })
                .await
                .map_err(Error::KubeError)?;
            Ok(Action::requeue(Duration::from_secs(10)))
        }
        ProjectAction::Delete => {
            let recorder = context
                .diagnostics
                .read()
                .await
                .recorder(context.client.clone(), &project);
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
            finalizer::delete(client, project.metadata.name.as_ref().unwrap(), &namespace)
                .await
                .unwrap();

            recorder
                .publish(Event {
                    type_: EventType::Normal,
                    reason: "DeleteRequested".into(),
                    note: Some(format!("Delete `{}`", project_name)),
                    action: "Deleting".into(),
                    secondary: None,
                })
                .await
                .map_err(Error::KubeError)?;
            Ok(Action::await_change())
        }
        ProjectAction::NoOp => Ok(Action::requeue(Duration::from_secs(10))),
    };
}

pub async fn run(state: State) {
    let client = Client::try_default()
        .await
        .expect("Failed to create client");

    let crd_api: Api<Project> = Api::all(client.clone());

    let gitlab_url = std::env::var("GITLAB_URL").expect("GITLAB_URL not set");
    let gitlab_token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN not set");

    let gitlab = Gitlab::new(gitlab_url, gitlab_token);

    Controller::new(crd_api.clone(), Config::default().any_semantic())
        .run(reconcile, on_error, state.to_context(client, gitlab))
        .for_each(|reconciliation_result| async move {
            match reconciliation_result {
                Ok(echo_resource) => {
                    info!("Reconciliation successful. Resource: {:?}", echo_resource);
                }
                Err(reconciliation_err) => {
                    eprintln!("Reconciliation error: {:?}", reconciliation_err)
                }
            }
        })
        .await;
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
        log::info!(
            "Project {} {} is being created {}",
            project.spec.project_id,
            project.spec.environment_type,
            project.metadata.name.as_ref().unwrap()
        );
        ProjectAction::Create
    } else {
        ProjectAction::NoOp
    };
}

//error handling
pub fn on_error(proj: Arc<Project>, error: &Error, context: Arc<Context>) -> Action {
    eprintln!("Reconciliation error:\n{:?}.\n{:?}", error, proj);
    context.metrics.reconcile_failure(&proj, error);
    Action::requeue(Duration::from_secs(5))
}

/// State shared between the controller and the web server
#[derive(Clone, Default)]
pub struct State {
    /// Diagnostics populated by the reconciler
    diagnostics: Arc<RwLock<Diagnostics>>,
    /// Metrics registry
    registry: prometheus::Registry,
}

/// State wrapper around the controller outputs for the web server
impl State {
    /// Metrics getter
    pub fn metrics(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }

    /// State getter
    pub async fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.read().await.clone()
    }

    // Create a Controller Context that can update State
    pub fn to_context(&self, client: Client, gitlab: Gitlab) -> Arc<Context> {
        Arc::new(Context {
            client,
            gitlab,
            metrics: Metrics::default().register(&self.registry).unwrap(),
            diagnostics: self.diagnostics.clone(),
        })
    }
}

/// Diagnostics to be exposed by the web server
#[derive(Clone, Serialize)]
pub struct Diagnostics {
    #[serde(deserialize_with = "from_ts")]
    pub last_event: DateTime<Utc>,
    #[serde(skip)]
    pub reporter: Reporter,
}
impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            last_event: Utc::now(),
            reporter: "kyotu-project-operator".into(),
        }
    }
}
impl Diagnostics {
    fn recorder(&self, client: Client, proj: &Project) -> Recorder {
        Recorder::new(client, self.reporter.clone(), proj.object_ref(&()))
    }
}
