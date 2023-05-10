use std::path::Path;
use tera::{Context, Tera};

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

    Ok(format!("Created project {}", name))
}

pub async fn delete_project(name: &str, repo_root: &Path) -> Result<String, ProjectError> {
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
