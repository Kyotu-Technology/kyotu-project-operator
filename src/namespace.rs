use k8s_openapi::api::core::v1::Namespace;
use kube::api::{DeleteParams, ObjectMeta, PostParams};
use kube::{Api, Client, Error};
use std::collections::BTreeMap;

//create namespace
pub async fn create_namespace(client: Client, name: &str) -> anyhow::Result<String> {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_string(), "kyotu-project-operator".to_string());

    let namespace = Namespace {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            labels: Some(labels),
            ..Default::default()
        },
        ..Default::default()
    };
    let ns_api: Api<Namespace> = Api::all(client);

    //check if namespace exists
    let res = ns_api.get(name).await;
    match res {
        Ok(_) => {
            log::warn!("Namespace {} already exists", name);
            return Ok(name.to_string());
        }
        Err(_) => {
            let res = ns_api.create(&PostParams::default(), &namespace).await?;
            log::info!("Created namespace {}", res.metadata.name.unwrap());
            Ok(name.to_string())
        }
    }
}

//delete namespace
pub async fn delete_namespace(client: Client, name: &str) -> anyhow::Result<String> {
    let ns_api: Api<Namespace> = Api::all(client);
    //delete only if label app=kyotu-project-operator is present
    let res = ns_api.get(name).await;

    match res {
        Ok(_) => {
            let labels = res.unwrap().metadata.labels.unwrap();
            if labels.get("app").unwrap_or(&"none".to_string()) != "kyotu-project-operator" {
                log::warn!(
                    "Namespace {} does not have label app=kyotu-project-operator",
                    name
                );
                return Ok(name.to_string());
            } else {
                let dp = DeleteParams::default();
                let _res = ns_api.delete(name, &dp).await?;
                log::info!("Deleted namespace {}", name);
                Ok(name.to_string())
            }
        }
        Err(_) => {
            log::warn!("Namespace {} does not exist", name);
            return Ok(name.to_string());
        }
    }
}
