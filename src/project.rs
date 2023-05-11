use std::path::Path;
use tera::{Context, Tera};

use crate::repository::Repository;

pub async fn create_project(name: &str, repo_root: &Path) -> Result<String, ProjectError> {
    let tera = match Tera::new("templates/*.yaml") {
        Ok(t) => t,
        Err(e) => {
            log::error!("Could not create Tera template: {}", e);
            ::std::process::exit(1);
        }
    };
    let mut context = Context::new();
    context.insert("project_name", &name);

    let repo_url = std::env::var("ARGO_REPO").expect("ARGO_REPO not set");
    let repo_branch = std::env::var("REPO_BRANCH").expect("REPO_BRANCH not set");

    //clear tmp dir
    if repo_root.exists() {
        std::fs::remove_dir_all(repo_root).expect("Failed to remove repo root");
    }
    //clone repo into project folder
    let mut argo_repository =
        Repository::clone(&repo_url, &repo_branch, &repo_root.to_string_lossy())
            .expect("Failed to clone repo");

    //create project folder in repo_root
    let project_path = Path::new(&repo_root).join("manifests").join(name);
    match std::fs::create_dir_all(&project_path) {
        Ok(_) => {
            log::info!("Created project folder {}", project_path.to_string_lossy());
        }
        Err(e) => {
            log::error!(
                "Could not create project folder {}: {}",
                project_path.to_string_lossy(),
                e
            );
            return Err(ProjectError::CreateProjectError(format!(
                "Could not create project folder {}: {}",
                project_path.to_string_lossy(),
                e
            )));
        }
    }
    //create .gitkeep file in project folder
    let gitkeep_path = project_path.join(".gitkeep");
    std::fs::File::create(gitkeep_path).expect("Could not create .gitkeep file");
    //create project.yaml file in project folder
    let project_yaml_path = Path::new(&repo_root)
        .join("applications")
        .join(format!("{}.yaml", name));
    let mut file =
        std::fs::File::create(project_yaml_path).expect("Could not create project.yaml file");
    tera.render_to("argo_tmpl.yaml", &context, &mut file)
        .expect("Could not render project.yaml file");

    //commit and push changes
    argo_repository
        .commit(format!("Created project {}", name).as_str())
        .expect("Failed to commit changes");
    argo_repository
        .push(&repo_branch)
        .expect("Failed to push changes");

    Ok(format!("Created project {}", name))
}

pub async fn delete_project(name: &str, repo_root: &Path) -> Result<String, ProjectError> {
    let repo_url = std::env::var("ARGO_REPO").expect("ARGO_REPO not set");
    let repo_branch = std::env::var("REPO_BRANCH").expect("REPO_BRANCH not set");
    //clear tmp dir
    if repo_root.exists() {
        std::fs::remove_dir_all(repo_root).expect("Failed to remove repo root");
    }
    //clone repo into project folder
    let mut argo_repository =
        Repository::clone(&repo_url, &repo_branch, &repo_root.to_string_lossy())
            .expect("Failed to clone repo");

    let project_path = Path::new(&repo_root).join("manifests").join(name);
    match std::fs::remove_dir_all(&project_path) {
        Ok(_) => {
            log::info!("Deleted project folder {}", project_path.to_string_lossy());
        }
        Err(e) => {
            log::error!(
                "Could not delete project folder {}: {}",
                project_path.to_string_lossy(),
                e
            );
            return Err(ProjectError::DeleteProjectError(format!(
                "Could not delete project folder {}: {}",
                project_path.to_string_lossy(),
                e
            )));
        }
    }

    let project_yaml_path = Path::new(&repo_root)
        .join("applications")
        .join(format!("{}.yaml", name));
    std::fs::remove_file(project_yaml_path)
        .unwrap_or_else(|_| panic!("Could not delete {}.yaml file", name));

    //commit and push changes
    argo_repository
        .commit(format!("Deleted project {}", name).as_str())
        .expect("Failed to commit changes");
    argo_repository
        .push(&repo_branch)
        .expect("Failed to push changes");
    Ok(format!("Deleted project {}", name))
}

//error enum
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("Could not create project: {0}")]
    CreateProjectError(String),
    #[error("Could not delete project: {0}")]
    DeleteProjectError(String),
}
