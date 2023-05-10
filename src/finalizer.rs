use crate::project_crd::Project;
use kube::api::{Patch, PatchParams};
use kube::{Api, Client, Error};
use serde_json::{json, Value};

//add finalizer
pub async fn add(client: Client, name: &str, namespace: &str) -> Result<Project, Error> {
    let api: Api<Project> = Api::namespaced(client, namespace);
    let finalizer: Value = json!({
        "metadata": {
            "finalizers": [
                "project.kyotu.tech/finalizer"
            ]
        }
    });

    let patch: Patch<&Value> = Patch::Merge(&finalizer);
    api.patch(name, &PatchParams::default(), &patch).await
}

//delete finalizer
pub async fn delete(client: Client, name: &str, namespace: &str) -> Result<Project, Error> {
    let api: Api<Project> = Api::namespaced(client, namespace);
    let finalizer: Value = json!({
        "metadata": {
            "finalizers": []
        }
    });
    let patch: Patch<&Value> = Patch::Merge(&finalizer);
    api.patch(name, &PatchParams::default(), &patch).await
}
