use std::path::{Path, PathBuf};
use tracing::debug;
use tracing::log;

pub struct Repository {
    inner: git2::Repository,
    base_path: PathBuf,
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
    pub fn clone(remote_url: &str, remote_branch: &str, target_path: &str) -> anyhow::Result<Self> {
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(Self::credentials_cb);
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
        })
    }

    //commit repository
    pub fn commit(&self, message: &str) -> anyhow::Result<()> {
        let mut index = self.inner.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        let oid = index.write_tree()?;
        let tree = self.inner.find_tree(oid)?;

        let sig = self.inner.signature()?;
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
        push_callbacks.credentials(Self::credentials_cb);
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

    pub fn credentials_cb(
        _user: &str,
        _user_from_url: Option<&str>,
        _cred: git2::CredentialType,
    ) -> Result<git2::Cred, git2::Error> {
        let user = _user_from_url.unwrap_or("git");

        if _cred.contains(git2::CredentialType::USERNAME) {
            return git2::Cred::username(user);
        }

        match std::env::var("KEY_PATH") {
            Ok(k) => {
                debug!(
                    "authenticate with user {} and private key located in {}",
                    user, k
                );
                return git2::Cred::ssh_key(user, None, std::path::Path::new(&k), None);
            }
            Err(_) => {
                debug!("No private key found, trying to authenticate with password");
            }
        };

        match std::env::var("DEPLOY_KEY") {
            Ok(p) => {
                debug!("authenticate with user {} and password", user);
                return git2::Cred::userpass_plaintext(user, &p);
            }
            _ => Err(git2::Error::from_str(
                "unable to get password from PASSWORD",
            )),
        }
    }

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
