use std::{path::PathBuf, f32::consts::E};
use tracing::debug;

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
        _ => Err(git2::Error::from_str("unable to get password from PASSWORD")),

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
