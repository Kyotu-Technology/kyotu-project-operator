use git2::CredentialType;
use std::path::{Path, PathBuf};
use tracing::log;

pub struct Repository {
    inner: git2::Repository,
    base_path: PathBuf,
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
        cred_type: CredentialType,
    ) -> anyhow::Result<Self> {
        let mut callbacks = git2::RemoteCallbacks::new();

        match cred_type {
            CredentialType::SSH_KEY => {
                callbacks.credentials(Self::credentials_cb_ssh);
            }
            CredentialType::USER_PASS_PLAINTEXT => {
                callbacks.credentials(Self::credentials_cb_pass);
            }
            _ => {
                log::error!("Unknown credential type");
                ::std::process::exit(1);
            }
        }

        //callbacks.transfer_progress(transfer_progress_cb);

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
                push_callbacks.credentials(Self::credentials_cb_ssh);
            }
            CredentialType::USER_PASS_PLAINTEXT => {
                push_callbacks.credentials(Self::credentials_cb_pass);
            }
            _ => {
                log::error!("Unknown credential type");
                ::std::process::exit(1);
            }
        }
        //push_callbacks.transfer_progress(|ref progress| transfer_progress_cb(progress));
        push_options.remote_callbacks(push_callbacks);

        remote
            .push(
                &[&format!("refs/heads/{}", target_branch)],
                Some(&mut push_options),
            )
            .expect("Could not push to remote");

        Ok(())
    }

    pub fn credentials_cb_pass(
        _user: &str,
        _user_from_url: Option<&str>,
        _cred: git2::CredentialType,
    ) -> Result<git2::Cred, git2::Error> {
        let user = _user_from_url.unwrap_or("git");

        if _cred.contains(git2::CredentialType::USERNAME) {
            return git2::Cred::username(user);
        }
        match std::env::var("DEPLOY_KEY") {
            Ok(p) => {
                log::debug!("authenticate with user {} and password", user);
                git2::Cred::userpass_plaintext(user, &p)
            }
            _ => Err(git2::Error::from_str(
                "unable to get password from DEPLOY_KEY",
            )),
        }
    }

    pub fn credentials_cb_ssh(
        _user: &str,
        _user_from_url: Option<&str>,
        _cred: git2::CredentialType,
    ) -> Result<git2::Cred, git2::Error> {
        let user = _user_from_url.unwrap_or("git");

        if _cred.contains(git2::CredentialType::USERNAME) {
            return git2::Cred::username(user);
        }
        match std::env::var("SSH_KEY_PATH") {
            Ok(p) => {
                log::debug!("authenticate with user {} and ssh key", user);
                git2::Cred::ssh_key(user, None, std::path::Path::new(&p), None)
            }
            _ => Err(git2::Error::from_str("unable to get ssh key from SSH_KEY")),
        }
    }

    #[allow(dead_code)]
    pub fn transfer_progress_cb(progress: &git2::Progress) -> bool {
        if progress.received_objects() == progress.total_objects() {
            log::info!(
                "Resolving deltas {}/{}",
                progress.indexed_deltas(),
                progress.total_deltas()
            );
        } else if progress.total_objects() > 0 {
            log::info!(
                "Received {}/{} objects ({}) in {} bytes",
                progress.received_objects(),
                progress.total_objects(),
                progress.indexed_objects(),
                progress.received_bytes()
            );
        }
        true
    }
}
