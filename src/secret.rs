use base64::engine::general_purpose::STANDARD as BASE64;
use base64::engine::Engine as _;
use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::ByteString;
use kube::api::{DeleteParams, ObjectMeta, PostParams};
use kube::{Api, Client};
use serde_json::json;
use std::collections::BTreeMap;
//create secret

pub async fn create_secret(client: Client, namespace: &str, data: &str) -> anyhow::Result<String> {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_string(), "kyotu-project-operator".to_string());
    let mut data_map: BTreeMap<String, ByteString> = BTreeMap::new();
    let gitlab_url = std::env::var("GITLAB_URL").expect("GITLAB_URL must be set");
    let username = format!("{}-image-puller", namespace);

    let registry_url = gitlab_url.replace("https://", "https://registry.");

    let data_json = json!(
        {
            "auths": {
                registry_url: {
                    "username": username,
                    "password": data,
                    "auth": BASE64.encode(format!("{}:{}", username, data).as_bytes())
                }
            }
        }
    );

    data_map.insert(
        ".dockerconfigjson".to_string(),
        ByteString(
            serde_json::to_string_pretty(&data_json)
                .unwrap()
                .into_bytes(),
        ),
    );

    let secret = Secret {
        metadata: ObjectMeta {
            name: Some("gitlab-registry-image-pull-secret".to_string()),
            namespace: Some(namespace.to_string()),
            labels: Some(labels),
            ..Default::default()
        },
        data: Some(data_map),
        ..Default::default()
    };
    let secret_api: Api<Secret> = Api::namespaced(client, namespace);

    //check if secret exists
    let res = secret_api.get("gitlab-registry-image-pull-secret").await;
    match res {
        Ok(_) => {
            log::warn!(
                "Secret gitlab-registry-image-pull-secret already exists in namespace {}",
                namespace
            );
            Ok(namespace.to_string())
        }
        Err(_) => {
            let res = secret_api.create(&PostParams::default(), &secret).await?;
            log::info!("Created secret {}", res.metadata.name.unwrap());
            Ok(namespace.to_string())
        }
    }
}

//delete secret
pub async fn delete_secret(client: Client, namespace: &str) -> anyhow::Result<String> {
    let secret_api: Api<Secret> = Api::namespaced(client.clone(), namespace);
    //delete only if label app=kyotu-project-operator is present
    let res = secret_api.get("gitlab-registry-image-pull-secret").await;

    match res {
        Ok(_) => {
            let labels = res.unwrap().metadata.labels.unwrap();
            if labels.get("app").unwrap_or(&"none".to_string()) != "kyotu-project-operator" {
                log::warn!(
                    "Secret gitlab-registry-image-pull-secret in namspace {} does not have label app=kyotu-project-operator",
                    namespace
                );
                Ok(namespace.to_string())
            } else {
                let dp = DeleteParams::default();
                let _res = secret_api
                    .delete("gitlab-registry-image-pull-secret", &dp)
                    .await?;
                log::info!("Deleted secret {}", namespace.to_string());
                Ok(namespace.to_string())
            }
        }
        Err(_) => {
            log::warn!(
                "Secret gitlab-registry-image-pull-secret does not exist in namespace {}",
                namespace
            );
            Ok(namespace.to_string())
        }
    }
}
