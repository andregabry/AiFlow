use crate::model::*;
use fs2::FileExt;
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};
use tokio::process::Command;

pub fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    cmd.kill_on_drop(true);
    cmd
}

pub async fn git(path: &str, args: &[&str]) -> Result<String> {
    let o = command("git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-C",
            path,
        ])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(crate::security::redact(&String::from_utf8_lossy(&o.stderr)));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}
pub async fn snapshot(path: &str) -> Result<GitSnapshot> {
    let canonical = Path::new(path).canonicalize().map_err(|e| e.to_string())?;
    let path = canonical.to_str().ok_or("Invalid path encoding")?;
    let root = git(path, &["rev-parse", "--show-toplevel"]).await?;
    let reported = PathBuf::from(&root)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if reported != canonical {
        return Err("Select the worktree root, not a subdirectory".into());
    }
    let branch = git(path, &["symbolic-ref", "--short", "HEAD"])
        .await
        .map_err(|_| "Detached HEAD: select a branch before execution")?;
    let head = git(path, &["rev-parse", "HEAD"]).await?;
    let status = git(
        path,
        &["status", "--porcelain=v1", "--untracked-files=normal"],
    )
    .await?;
    Ok(GitSnapshot {
        branch,
        head,
        status,
        root,
    })
}
pub async fn validate(p: &Project) -> Result<GitSnapshot> {
    let s = snapshot(&p.path).await?;
    if s.branch != p.branch {
        return Err(format!(
            "Branch mismatch: expected {}, got {}",
            p.branch, s.branch
        ));
    }
    if p.policy.require_clean_tree && !s.status.is_empty() {
        return Err("REPOSITORY_DIRTY: commit or review existing changes before starting".into());
    }
    Ok(s)
}
pub async fn lock(path: &str) -> Result<File> {
    let common = git(path, &["rev-parse", "--path-format=absolute", "--git-dir"]).await?;
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(Path::new(&common).join("aiflow.execution.lock"))
        .map_err(|e| e.to_string())?;
    f.try_lock_exclusive().map_err(|_| {
        "WORKTREE_CONFLICT: another AiFlow execution owns this worktree".to_string()
    })?;
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn fixture() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        let p = temp.path().to_str().unwrap();
        git(p, &["init", "--initial-branch=main"]).await.unwrap();
        std::fs::write(temp.path().join("readme.txt"), "base\n").unwrap();
        git(p, &["add", "readme.txt"]).await.unwrap();
        git(
            p,
            &[
                "-c",
                "user.name=AiFlow Test",
                "-c",
                "user.email=test@localhost",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "initial",
            ],
        )
        .await
        .unwrap();
        temp
    }
    #[tokio::test]
    async fn snapshots_and_dirty_branch_policies_are_independent() {
        let temp = fixture().await;
        let p = temp.path().to_str().unwrap();
        let s = snapshot(p).await.unwrap();
        assert_eq!(s.branch, "main");
        assert!(s.status.is_empty());
        let mut project = Project {
            id: id(),
            name: "test".into(),
            repository: "owner/repo".into(),
            path: p.into(),
            branch: "main".into(),
            policy: ProjectPolicy::default(),
        };
        assert!(validate(&project).await.is_ok());
        std::fs::write(temp.path().join("partial.txt"), "preserve me").unwrap();
        assert!(validate(&project)
            .await
            .unwrap_err()
            .contains("REPOSITORY_DIRTY"));
        project.policy.require_clean_tree = false;
        project.branch = "unexpected".into();
        assert!(validate(&project)
            .await
            .unwrap_err()
            .contains("Branch mismatch"));
        assert_eq!(
            std::fs::read_to_string(temp.path().join("partial.txt")).unwrap(),
            "preserve me"
        );
    }
    #[tokio::test]
    async fn lease_rejects_concurrent_execution_and_releases_on_drop() {
        let temp = fixture().await;
        let p = temp.path().to_str().unwrap();
        let first = lock(p).await.unwrap();
        assert!(lock(p).await.is_err());
        drop(first);
        assert!(lock(p).await.is_ok());
    }
    #[tokio::test]
    async fn subdirectory_and_detached_head_are_rejected() {
        let temp = fixture().await;
        let p = temp.path().to_str().unwrap();
        std::fs::create_dir(temp.path().join("nested")).unwrap();
        assert!(snapshot(temp.path().join("nested").to_str().unwrap())
            .await
            .is_err());
        git(p, &["checkout", "--detach"]).await.unwrap();
        assert!(snapshot(p).await.unwrap_err().contains("Detached"));
    }
}
