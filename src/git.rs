use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum GitOpsError {
    #[error("io error")]
    IOError(#[from] std::io::Error),

    #[error("git error")]
    ReturnNonZero(std::process::ExitStatus),

    #[error("utf8 error")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("failed to get git short hash")]
    GetGitHashError,

    #[error("failed to clone")]
    CloneError,

    #[error("failed to checkout")]
    CheckoutError,
}

pub fn get_branch_name(workdir: &Path) -> Result<String, GitOpsError> {
    let output = std::process::Command::new("git")
        .current_dir(workdir)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .map_err(|e| GitOpsError::IOError(e))?;
    let branch_name = String::from_utf8(output.stdout)
        .map_err(|e| GitOpsError::Utf8Error(e))?
        .trim()
        .to_string();
    Ok(branch_name)
}

pub fn get_git_short_hash(workdir: &Path) -> Result<String, GitOpsError> {
    let output = std::process::Command::new("git")
        .current_dir(workdir)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .map_err(|e| GitOpsError::IOError(e))?;
    let hash = String::from_utf8(output.stdout)
        .map_err(|e| GitOpsError::Utf8Error(e))?
        .trim()
        .to_string();
    Ok(hash)
}

pub fn git_clone_to(repo: &str, workdir: &Path, dst: &str) -> Result<(), GitOpsError> {
    let mut cmd = std::process::Command::new("git")
        .current_dir(workdir)
        .args(["clone", repo, dst])
        .spawn()
        .map_err(|e| GitOpsError::IOError(e))?;
    let result = cmd.wait().map_err(|e| GitOpsError::IOError(e))?;
    if !result.success() {
        return Err(GitOpsError::ReturnNonZero(result));
    }
    Ok(())
}

pub fn git_checkout(workdir: &Path, rev: &str) -> Result<(), GitOpsError> {
    let mut cmd = std::process::Command::new("git")
        .current_dir(workdir)
        .args(["checkout", rev])
        .spawn()
        .map_err(|e| GitOpsError::IOError(e))?;
    let result = cmd.wait().map_err(|e| GitOpsError::IOError(e))?;
    if !result.success() {
        return Err(GitOpsError::ReturnNonZero(result));
    }
    Ok(())
}
