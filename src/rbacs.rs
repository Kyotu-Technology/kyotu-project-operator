use git2::CredentialType;
use std::path::Path;

use crate::repository::Repository;

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
    //clone repo into project folder
    let flux_repository = Repository::clone(
        &repo_url,
        &repo_branch,
        &repo_root.to_string_lossy(),
        CredentialType::SSH_KEY,
    )
    .expect("Failed to clone repo");

    let vault_values = std::fs::read_to_string(format!(
        "{}/namespaces/vault/vault/rbac_values.yaml",
        repo_root.to_string_lossy()
    ))
    .expect("Something went wrong reading the file");

    //add value to vault_values yaml using serde_yaml
    let vault_values_yaml: serde_yaml::Value = serde_yaml::from_str(&vault_values).unwrap();

    //get array of policies that are under vault.externalConfig.policies key
    let policies = vault_values_yaml["vault"]["externalConfig"]["policies"]
        .as_sequence()
        .unwrap();

    //add new policy to with key name and value flux-<project_name>
    let mut policies = policies.to_owned();

    //check if policy already exists
    let mut policy_exists = false;
    for policy in &policies {
        if policy["name"].as_str().unwrap() == format!("{}_access", name.replace('-', "_")) {
            policy_exists = true;
        }
    }

    if !policy_exists {
        let mut new_policy = serde_yaml::mapping::Mapping::new();
        new_policy.insert(
            serde_yaml::Value::String("name".to_string()),
            serde_yaml::Value::String(format!("{}_access", name.replace('-', "_"))),
        );
        new_policy.insert(
            serde_yaml::Value::String("rules".to_string()),
            serde_yaml::Value::String(format!(
                "path \"secret/{}/*\" {{\n  capabilities = [\"create\", \"read\", \"update\", \"delete\", \"list\"]\n}}",
                name.replace('-', "_")
            )),
        );
        policies.push(serde_yaml::Value::Mapping(new_policy));
    }

    //update vault_values yaml with new policy
    let mut vault_values_yaml = vault_values_yaml.to_owned();
    vault_values_yaml["vault"]["externalConfig"]["policies"] =
        serde_yaml::Value::Sequence(policies);

    //add group
    let groups = vault_values_yaml["vault"]["externalConfig"]["groups"]
        .as_sequence()
        .unwrap();

    let mut groups = groups.to_owned();

    //check if group already exists
    let mut group_exists = false;
    for group in &groups {
        if group["name"].as_str().unwrap() == google_group {
            group_exists = true;
        }
    }

    if !group_exists {
        let mut new_group = serde_yaml::mapping::Mapping::new();
        new_group.insert(
            serde_yaml::Value::String("name".to_string()),
            serde_yaml::Value::String(google_group.to_string()),
        );
        new_group.insert(
            serde_yaml::Value::String("type".to_string()),
            serde_yaml::Value::String("external".to_string()),
        );

        //add policies as array
        let mut group_policies = Vec::new();
        group_policies.push(serde_yaml::Value::String(format!(
            "{}_access",
            name.replace('-', "_")
        )));
        new_group.insert(
            serde_yaml::Value::String("policies".to_string()),
            serde_yaml::Value::Sequence(group_policies),
        );

        groups.push(serde_yaml::Value::Mapping(new_group));
    } else {
        //add policy to existing group
        for group in &mut groups {
            if group["name"].as_str().unwrap() == google_group {
                let group_policies = group["policies"].as_sequence_mut().unwrap();
                group_policies.push(serde_yaml::Value::String(format!(
                    "{}_access",
                    name.replace('-', "_")
                )));
            }
        }
    }

    vault_values_yaml["vault"]["externalConfig"]["groups"] = serde_yaml::Value::Sequence(groups);

    //add group-aliases
    let group_aliases = vault_values_yaml["vault"]["externalConfig"]["group-aliases"]
        .as_sequence()
        .unwrap();

    let mut group_aliases = group_aliases.to_owned();

    //check if group alias already exists
    let mut group_alias_exists = false;
    for group_alias in &group_aliases {
        if group_alias["name"].as_str().unwrap() == google_group {
            group_alias_exists = true;
        }
    }

    //if group alias doesn't exist, add it
    if !group_alias_exists {
        let mut new_group_alias = serde_yaml::mapping::Mapping::new();
        new_group_alias.insert(
            serde_yaml::Value::String("name".to_string()),
            serde_yaml::Value::String(google_group.to_string()),
        );
        new_group_alias.insert(
            serde_yaml::Value::String("mountpath".to_string()),
            serde_yaml::Value::String("oidc".to_string()),
        );
        new_group_alias.insert(
            serde_yaml::Value::String("group".to_string()),
            serde_yaml::Value::String(google_group.to_string()),
        );
        group_aliases.push(serde_yaml::Value::Mapping(new_group_alias));
    }

    vault_values_yaml["vault"]["externalConfig"]["group-aliases"] =
        serde_yaml::Value::Sequence(group_aliases);

    //write vault_values yaml back to file
    let vault_values_yaml = serde_yaml::to_string(&vault_values_yaml).unwrap();
    std::fs::write(
        format!(
            "{}/namespaces/vault/vault/rbac_values.yaml",
            repo_root.to_string_lossy()
        ),
        vault_values_yaml,
    )
    .unwrap();

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

    //clone repo into project folder

    let flux_repository = Repository::clone(
        &repo_url,
        &repo_branch,
        &repo_root.to_string_lossy(),
        CredentialType::SSH_KEY,
    )
    .expect("Failed to clone repo");

    let vault_values = std::fs::read_to_string(format!(
        "{}/namespaces/vault/vault/rbac_values.yaml",
        repo_root.to_string_lossy()
    ))
    .expect("Something went wrong reading the file");

    //add value to vault_values yaml using serde_yaml
    let vault_values_yaml: serde_yaml::Value = serde_yaml::from_str(&vault_values).unwrap();

    //get array of policies that are under vault.externalConfig.policies key
    let policies = vault_values_yaml["vault"]["externalConfig"]["policies"]
        .as_sequence()
        .unwrap();

    //remove policy with key name and value <name>_access
    let mut policies = policies.to_owned();
    policies.retain(|policy| {
        policy["name"].as_str().unwrap() != format!("{}_access", name.replace('-', "_")).as_str()
    });

    //update vault_values yaml with new policy
    let mut vault_values_yaml = vault_values_yaml.to_owned();
    vault_values_yaml["vault"]["externalConfig"]["policies"] =
        serde_yaml::Value::Sequence(policies);

    //remove group
    let groups = vault_values_yaml["vault"]["externalConfig"]["groups"]
        .as_sequence()
        .unwrap();

    //initialiaze empty groups array
    let mut new_groups: Vec<serde_yaml::Value> = Vec::new();

    for group in groups {
        println!("{}", group["name"].as_str().unwrap());
        //print group policies
        println!("before: {:?}", group["policies"].as_sequence().unwrap());
        let mut group = group.to_owned();
        group["policies"]
            .as_sequence_mut()
            .unwrap()
            .retain(|policy| {
                policy.as_str().unwrap() != format!("{}_access", name.replace('-', "_")).as_str()
            });
        //push group to new_groups if it has policies
        if group["policies"].as_sequence().unwrap().is_empty() {
            new_groups.push(group);
        }
    }

    vault_values_yaml["vault"]["externalConfig"]["groups"] =
        serde_yaml::Value::Sequence(new_groups);

    //remove group-aliases

    let group_aliases = vault_values_yaml["vault"]["externalConfig"]["group-aliases"]
        .as_sequence()
        .unwrap();

    let mut group_aliases = group_aliases.to_owned();

    let new_groups = vault_values_yaml["vault"]["externalConfig"]["groups"]
        .as_sequence()
        .unwrap();
    //if group is in new_groups leave it in group_aliases
    group_aliases.retain(|group_alias| {
        new_groups
            .iter()
            .any(|group| group["name"].as_str().unwrap() == group_alias["group"].as_str().unwrap())
    });

    vault_values_yaml["vault"]["externalConfig"]["group-aliases"] =
        serde_yaml::Value::Sequence(group_aliases);

    //write vault_values yaml back to file

    let vault_values_yaml = serde_yaml::to_string(&vault_values_yaml).unwrap();
    std::fs::write(
        format!(
            "{}/namespaces/vault/vault/rbac_values.yaml",
            repo_root.to_string_lossy()
        ),
        vault_values_yaml,
    )
    .unwrap();

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
