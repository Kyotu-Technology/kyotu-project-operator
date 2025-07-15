use git2::CredentialType;
use std::path::{Path, PathBuf};
use tracing::log;

pub struct Repository {
    inner: git2::Repository,
    base_path: PathBuf,
    deploy_key: Option<String>,
    cred_type: CredentialType,
}

impl std::fmt::Debug for Repository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Repository")
            .field("base_path", &self.base_path)
            .finish()
    }
}

impl Repository {
    //clone repository
    pub fn clone(
        remote_url: &str,
        remote_branch: &str,
        target_path: &str,
        deploy_key: Option<&str>,
    ) -> anyhow::Result<Self> {
        let mut callbacks = git2::RemoteCallbacks::new();

        let deploy_key = deploy_key.unwrap_or("");

        //if url is ssh, use ssh key
        let cred_type = if remote_url.starts_with("https://") {
            CredentialType::USER_PASS_PLAINTEXT
        } else {
            CredentialType::SSH_KEY
        };

        match cred_type {
            CredentialType::SSH_KEY => {
                callbacks.credentials(|_url, username_from_url, _allowed_types| {
                    let user = username_from_url.unwrap_or("git");
                    git2::Cred::ssh_key_from_memory(user, None, deploy_key, None)
                });
            }
            CredentialType::USER_PASS_PLAINTEXT => {
                callbacks.credentials(|_url, username_from_url, _allowed_types| {
                    let user = username_from_url.unwrap_or("git");
                    git2::Cred::userpass_plaintext(user, deploy_key)
                });
            }
            _ => {
                log::error!("Unknown credential type");
                ::std::process::exit(1);
            }
        }

        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_options);
        builder.branch(remote_branch);

        log::info!("Cloning {} into {}", remote_url, target_path);

        let repo = builder.clone(remote_url, Path::new(target_path))?;

        Ok(Self {
            inner: repo,
            base_path: PathBuf::from(target_path),
            deploy_key: Some(deploy_key.to_string()),
            cred_type,
        })
    }

    //commit repository
    pub fn commit(&self, message: &str) -> anyhow::Result<()> {
        let mut index = self.inner.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        let oid = index.write_tree()?;
        let tree = self.inner.find_tree(oid)?;

        let sig = git2::Signature::now("kyotu-project-operator", "no-reply@kyotutechnology.com")?;

        let parent_commit = self.inner.head()?.peel_to_commit()?;
        self.inner
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent_commit])?;

        Ok(())
    }
    //push repository
    pub fn push(&self, target_branch: &str) -> anyhow::Result<()> {
        let mut remote = self.inner.find_remote("origin")?;
        let mut push_options = git2::PushOptions::new();
        let mut push_callbacks = git2::RemoteCallbacks::new();

        match self.cred_type {
            CredentialType::SSH_KEY => {
                push_callbacks.credentials(|_url, username_from_url, _allowed_types| {
                    let user = username_from_url.unwrap_or("git");
                    git2::Cred::ssh_key_from_memory(
                        user,
                        None,
                        self.deploy_key.as_ref().unwrap(),
                        None,
                    )
                });
            }
            CredentialType::USER_PASS_PLAINTEXT => {
                push_callbacks.credentials(|_url, username_from_url, _allowed_types| {
                    let user = username_from_url.unwrap_or("git");
                    git2::Cred::userpass_plaintext(user, self.deploy_key.as_ref().unwrap())
                });
            }
            _ => {
                log::error!("Unknown credential type");
                ::std::process::exit(1);
            }
        }

        push_options.remote_callbacks(push_callbacks);

        remote
            .push(
                &[&format!("refs/heads/{target_branch}")],
                Some(&mut push_options),
            )
            .expect("Could not push to remote");

        Ok(())
    }
}
