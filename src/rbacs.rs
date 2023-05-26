use crate::repository::Repository;
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Debug, Serialize, Deserialize, Clone)]
struct VaultConfig {
    vault: Vault,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Vault {
    external_config: ExternalConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ExternalConfig {
    policies: Vec<Policy>,
    groups: Vec<Group>,
    #[serde(rename = "group-aliases")]
    group_aliases: Vec<GroupAlias>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Policy {
    name: String,
    rules: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Group {
    name: String,
    policies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
    #[serde(rename = "type")]
    group_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Metadata {
    privileged: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GroupAlias {
    name: String,
    mountpath: String,
    group: String,
}

pub async fn add_rbacs(
    name: &str,
    repo_root: &Path,
    google_group: &str,
) -> Result<String, RbacError> {
    let repo_url = match std::env::var("FLUX_REPO") {
        Ok(url) => url,
        Err(e) => {
            log::error!("FLUX_REPO not set: {}", e);
            ::std::process::exit(1);
        }
    };
    let repo_branch = match std::env::var("REPO_BRANCH") {
        Ok(branch) => branch,
        Err(e) => {
            log::error!("REPO_BRANCH not set: {}", e);
            ::std::process::exit(1);
        }
    };

    //clear tmp dir
    if repo_root.exists() {
        std::fs::remove_dir_all(repo_root).expect("Failed to remove repo root");
    }

    let deploy_token = std::env::var("FLUX_DEPLOY_TOKEN").expect("FLUX_DEPLOY_TOKEN not set");

    //clone repo into project folder
    let flux_repository = Repository::clone(
        &repo_url,
        &repo_branch,
        &repo_root.to_string_lossy(),
        Some(&deploy_token),
    )
    .expect("Failed to clone repo");

    let vault_values = std::fs::read_to_string(format!(
        "{}/namespaces/vault/vault/rbac_values.yaml",
        repo_root.to_string_lossy()
    ))
    .expect("Something went wrong reading the file");

    let mut vault_values: VaultConfig = serde_yaml::from_str(&vault_values).unwrap();

    //create new policy
    let policy_name = format!("{}_access", name.replace('-', "_"));
    let new_policy = Policy{
        name: policy_name.clone(),
        rules: format!(
            "path \"secret/{}/*\" {{\n  capabilities = [\"create\", \"read\", \"update\", \"delete\", \"list\"]\n}}",
            name.replace('-', "_")
        ),
    };

    //check if policy already exists
    let mut policy_exists = false;
    for policy in vault_values.vault.external_config.policies.iter() {
        if policy.name == new_policy.name {
            policy_exists = true;
        }
    }
    //add new policy to vault_values
    if !policy_exists {
        vault_values.vault.external_config.policies.push(new_policy);
    }

    //check if group already exists if it does add policy to group else create new group
    let mut group_exists = false;
    for group in vault_values.vault.external_config.groups.iter_mut() {
        if group.name == google_group {
            group_exists = true;
            //add policy to group if it doesn't already exist
            if !group.policies.contains(&policy_name) {
                group.policies.push(policy_name.clone());
            }
        }
    }
    //add new group to vault_values
    if !group_exists {
        let new_group = Group {
            name: google_group.to_string(),
            policies: vec![policy_name],
            metadata: None,
            group_type: "external".to_string(),
        };
        vault_values.vault.external_config.groups.push(new_group);
    }

    //check if group alias exists if not create new group alias
    let mut group_alias_exists = false;
    for group_alias in vault_values.vault.external_config.group_aliases.iter() {
        if group_alias.name == google_group {
            group_alias_exists = true;
        }
    }
    //add new group alias to vault_values
    if !group_alias_exists {
        let new_group_alias = GroupAlias {
            name: google_group.to_string(),
            mountpath: "oidc".to_string(),
            group: google_group.to_string(),
        };
        vault_values
            .vault
            .external_config
            .group_aliases
            .push(new_group_alias);
    }

    //write vault_values yaml back to file
    std::fs::write(
        format!(
            "{}/namespaces/vault/vault/rbac_values.yaml",
            repo_root.to_string_lossy()
        ),
        serde_yaml::to_string(&vault_values).unwrap(),
    )
    .expect("Unable to write file");

    //argo rbac
    let mut argo_values = std::fs::read_to_string(format!(
        "{}/namespaces/argocd/argocd-operator/rbac.yaml",
        repo_root.to_string_lossy()
    ))
    .expect("Something went wrong reading the file");

    let template = std::fs::read_to_string("templates/rbac_tmpl.yaml")
        .expect("Something went wrong reading the file");

    let template = template.replace("{{ name }}", name);
    let template = template.replace("{{ google_group }}", google_group);

    argo_values.push_str(format!("\n{}", template).as_str());

    //write argo_values yaml back to file
    std::fs::write(
        format!(
            "{}/namespaces/argocd/argocd-operator/rbac.yaml",
            repo_root.to_string_lossy()
        ),
        argo_values,
    )
    .unwrap();

    flux_repository
        .commit(format!("Created rbac for {}", name).as_str())
        .expect("Failed to commit changes");
    flux_repository
        .push(&repo_branch)
        .expect("Failed to push changes");

    Ok(format!("Added rbacs for project {}", name))
}

pub async fn remove_rbacs(
    name: &str,
    repo_root: &Path,
    google_group: &str,
) -> Result<String, RbacError> {
    let repo_url = match std::env::var("FLUX_REPO") {
        Ok(url) => url,
        Err(e) => {
            log::error!("FLUX_REPO not set: {}", e);
            ::std::process::exit(1);
        }
    };

    let repo_branch = match std::env::var("REPO_BRANCH") {
        Ok(branch) => branch,
        Err(e) => {
            log::error!("REPO_BRANCH not set: {}", e);
            ::std::process::exit(1);
        }
    };

    //clear tmp dir
    if repo_root.exists() {
        std::fs::remove_dir_all(repo_root).expect("Failed to remove repo root");
    }

    let deploy_token = std::env::var("FLUX_DEPLOY_TOKEN").expect("FLUX_DEPLOY_TOKEN not set");

    let flux_repository = Repository::clone(
        &repo_url,
        &repo_branch,
        &repo_root.to_string_lossy(),
        Some(&deploy_token),
    )
    .expect("Failed to clone repo");

    let vault_values = std::fs::read_to_string(format!(
        "{}/namespaces/vault/vault/rbac_values.yaml",
        repo_root.to_string_lossy()
    ))
    .expect("Something went wrong reading the file");

    let mut vault_values: VaultConfig = serde_yaml::from_str(&vault_values).unwrap();

    //remove policy from vault_values

    let policy_name = format!("{}_access", name.replace('-', "_"));
    vault_values
        .vault
        .external_config
        .policies
        .retain(|policy| policy.name != policy_name);

    //remove policy from all groups
    for group in vault_values.vault.external_config.groups.iter_mut() {
        group.policies.retain(|policy| policy != &policy_name);
    }

    //remove groups with no policies
    vault_values
        .vault
        .external_config
        .groups
        .retain(|group| !group.policies.is_empty());

    //remove group alias if none group with google_group exists
    let mut group_exists = false;
    for group in vault_values.vault.external_config.groups.iter() {
        if group.name == google_group {
            group_exists = true;
        }
    }

    if !group_exists {
        vault_values
            .vault
            .external_config
            .group_aliases
            .retain(|group_alias| group_alias.name != google_group);
    }
    //write vault_values yaml back to file
    std::fs::write(
        format!(
            "{}/namespaces/vault/vault/rbac_values.yaml",
            repo_root.to_string_lossy()
        ),
        serde_yaml::to_string(&vault_values).unwrap(),
    )
    .expect("Unable to write file");

    //argo rbac

    let argo_values = std::fs::read_to_string(format!(
        "{}/namespaces/argocd/argocd-operator/rbac.yaml",
        repo_root.to_string_lossy()
    ))
    .expect("Something went wrong reading the file");

    let template = std::fs::read_to_string("templates/rbac_tmpl.yaml")
        .expect("Something went wrong reading the file");

    let template = template.replace("{{ name }}", name);
    let template = template.replace("{{ google_group }}", google_group);

    //remove template from rbac.yaml line by line
    let mut argo_values_lines = argo_values.lines().collect::<Vec<&str>>();
    let template_lines = template.lines().collect::<Vec<&str>>();

    for line in template_lines.iter() {
        argo_values_lines.retain(|argo_line| argo_line != line);
    }

    let argo_values = argo_values_lines.join("\n");

    //write argo_values yaml back to file
    std::fs::write(
        format!(
            "{}/namespaces/argocd/argocd-operator/rbac.yaml",
            repo_root.to_string_lossy()
        ),
        argo_values,
    )
    .unwrap();

    //commit and push changes
    flux_repository
        .commit(format!("Removed rbac for {}", name).as_str())
        .expect("Failed to commit changes");
    flux_repository
        .push(&repo_branch)
        .expect("Failed to push changes");

    Ok(format!("Removed rbacs for project {}", name))
}

//error enum
#[derive(Debug, thiserror::Error)]
pub enum RbacError {
    #[error("Could not create rbac: {0}")]
    _CreateRbacError(String),
    #[error("Could not delete rbac: {0}")]
    _DeleteRbactError(String),
}
