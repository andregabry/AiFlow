use crate::{git, model::*, AppState};
use std::path::Path;

pub fn branch(project_id: &str) -> String {
    format!("work/aiflow-{project_id}")
}
fn destination(root: &Path, id: &str) -> Result<std::path::PathBuf> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("Invalid managed workspace identifier".into());
    }
    Ok(root.join(id))
}
pub fn is_managed(app: &AppState, p: &Project) -> Result<bool> {
    let root = app.store.workspace_root()?;
    if p.path.is_empty() || !root.exists() {
        return Ok(false);
    }
    let expected = destination(&root.canonicalize().map_err(|e| e.to_string())?, &p.id)?;
    if !expected.exists() {
        return Ok(false);
    }
    if expected.canonicalize().map_err(|e| e.to_string())? != expected {
        return Err("Managed workspace directory was redirected".into());
    }
    Ok(Path::new(&p.path)
        .canonicalize()
        .map_err(|e| e.to_string())?
        == expected)
}
fn helper() -> Result<String> {
    let gh = crate::github::GitHubService::cli_launch()?;
    let executable = gh.program.to_string_lossy().replace('\\', "/");
    if executable.contains(['"', '\n', '\r']) {
        return Err("Unsupported GitHub CLI installation path".into());
    }
    Ok(format!("!\"{executable}\" auth git-credential"))
}
async fn authenticated(path: &str, args: &[&str]) -> Result<String> {
    git::git(
        path,
        &[
            &[
                "-c",
                "credential.helper=",
                "-c",
                &format!("credential.helper={}", helper()?),
            ][..],
            args,
        ]
        .concat(),
    )
    .await
}
async fn repository_git(path: &str, args: &[&str], use_auth: bool) -> Result<String> {
    if use_auth {
        authenticated(path, args).await
    } else {
        git::git(path, args).await
    }
}
async fn checkout(
    root: &Path,
    p: &Project,
    remote: &str,
    use_auth: bool,
) -> Result<(String, String)> {
    if p.branch.is_empty() || p.branch.starts_with('-') {
        return Err("Missing valid default branch".into());
    }

    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let target = destination(&root, &p.id)?;

    let local = git::portable_path(&target)?;
    if !target.exists() {
        repository_git(
            root.to_str().ok_or("Invalid workspace root")?,
            &[
                "clone",
                "--single-branch",
                "--no-recurse-submodules",
                "--branch",
                &p.branch,
                "--",
                remote,
                &local,
            ],
            use_auth,
        )
        .await?;
    }
    if target.canonicalize().map_err(|e| e.to_string())? != target {
        return Err("Managed workspace must not redirect outside its assigned directory".into());
    }
    if git::git(&local, &["remote", "get-url", "origin"]).await? != remote {
        return Err("Managed workspace origin changed; review it before continuing".into());
    }
    let expected = branch(&p.id);
    let snapshot = git::snapshot(&local).await?;
    if snapshot.branch != expected {
        if snapshot.branch != p.branch || !snapshot.status.is_empty() {
            return Err("Managed workspace contains unexpected work; preserved for review".into());
        }
        git::git(&local, &["checkout", "-b", &expected]).await?;
    }
    Ok((local, expected))
}
pub async fn prepare(app: &AppState, mut p: Project) -> Result<Project> {
    if !p.path.trim().is_empty() {
        return Ok(p);
    }
    if !crate::security::safe_repository(&p.repository) {
        return Err("Invalid repository".into());
    }
    let root = app.store.workspace_root()?;
    let remote = format!("https://github.com/{}.git", p.repository);
    let (local, expected) = checkout(&root, &p, &remote, true).await?;
    let (name, email) = app.github.git_identity().await?;
    git::git(&local, &["config", "--local", "user.name", &name]).await?;
    git::git(&local, &["config", "--local", "user.email", &email]).await?;
    p.path = local;
    p.branch = expected;
    app.store.save_project(&p)?;
    app.store.audit(
        "MANAGED_WORKSPACE_PREPARED",
        serde_json::json!({"projectId":p.id,"branch":p.branch}),
    )?;
    app.changed();
    Ok(p)
}
pub fn detect_validation(path: &Path) -> Vec<Vec<String>> {
    let package = std::fs::read(path.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    if let Some(package) = package {
        let scripts = package["scripts"].as_object();
        let usable = |value: &serde_json::Value| {
            value
                .as_str()
                .is_some_and(|s| !s.trim().is_empty() && !s.contains("no test specified"))
        };
        let mut commands = vec![];
        if scripts.and_then(|s| s.get("test")).is_some_and(usable) {
            commands.push(vec!["npm".into(), "test".into()]);
        } else if let Some(scripts) = scripts {
            for (name, value) in scripts {
                let modifiers = name.to_ascii_lowercase();
                let excluded = modifiers
                    .split(':')
                    .any(|part| matches!(part, "watch" | "dev" | "update" | "interactive"));
                if name.starts_with("test:") && !excluded && usable(value) {
                    commands.push(vec!["npm".into(), "run".into(), name.clone()]);
                }
            }
        }
        if !commands.is_empty() {
            if scripts.and_then(|s| s.get("build")).is_some_and(usable) {
                commands.push(vec!["npm".into(), "run".into(), "build".into()]);
            }
            return commands;
        }
    }
    if path.join("Cargo.toml").is_file() {
        return vec![vec!["cargo".into(), "test".into(), "--all-targets".into()]];
    }
    if path.join("go.mod").is_file() {
        return vec![vec!["go".into(), "test".into(), "./...".into()]];
    }
    if path.join("pytest.ini").is_file()
        || (path.join("pyproject.toml").is_file() && path.join("tests").is_dir())
    {
        return vec![vec!["python".into(), "-m".into(), "pytest".into()]];
    }
    vec![]
}
pub fn configure_validation(app: &AppState, mut p: Project) -> Result<Project> {
    if p.policy.require_tests && p.policy.test_commands.is_empty() {
        let commands = detect_validation(Path::new(&p.path));
        if commands.is_empty() {
            return Err("Validation setup: no supported test suite was detected. Click the (!) queue tutorial and choose Configure validation to set this repository's test commands".into());
        }
        p.policy.test_commands = commands;
        app.store.save_project(&p)?;
        app.store.audit(
            "VALIDATION_AUTO_DETECTED",
            serde_json::json!({"projectId":p.id,"commands":p.policy.test_commands}),
        )?;
        app.changed();
    }
    Ok(p)
}
pub async fn publish(app: &AppState, p: &Project, head: &str) -> Result<bool> {
    if !is_managed(app, p)? {
        return Ok(false);
    }
    let remote = format!("https://github.com/{}.git", p.repository);
    publish_to(p, head, &remote, true).await?;
    Ok(true)
}
async fn publish_to(p: &Project, head: &str, remote: &str, use_auth: bool) -> Result<()> {
    if p.branch != branch(&p.id) {
        return Err("Unexpected managed branch; refusing publication".into());
    }
    let current = git::snapshot(&p.path).await?;
    if current.head != head || current.branch != p.branch || !current.status.is_empty() {
        return Err("Worktree changed after validation; publication stopped".into());
    }
    let refname = format!("refs/heads/{}", p.branch);
    repository_git(
        &p.path,
        &[
            "push",
            "--porcelain",
            "--",
            remote,
            &format!("{head}:{refname}"),
        ],
        use_auth,
    )
    .await?;
    let actual = repository_git(
        &p.path,
        &["ls-remote", "--exit-code", "--", remote, &refname],
        use_auth,
    )
    .await?;
    if actual.split_whitespace().next() != Some(head) {
        return Err("GitHub did not confirm the validated commit".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn managed_checkout_and_publication_preserve_work_and_verify_exact_remote_head() {
        let (_fixture, app, mut p) = crate::execution::tests::fixture(false, false).await;
        let temp = tempfile::tempdir().unwrap();
        let remote = temp.path().join("origin.git");
        git::git(
            temp.path().to_str().unwrap(),
            &["clone", "--bare", "--", &p.path, remote.to_str().unwrap()],
        )
        .await
        .unwrap();
        p.path.clear();
        let root = temp.path().join("managed");
        let (local, expected) = checkout(&root, &p, remote.to_str().unwrap(), false)
            .await
            .unwrap();
        p.path = local;
        p.branch = expected;
        std::fs::write(Path::new(&p.path).join("result.txt"), "validated result").unwrap();
        git::git(&p.path, &["add", "result.txt"]).await.unwrap();
        git::git(
            &p.path,
            &[
                "-c",
                "user.name=AiFlow Test",
                "-c",
                "user.email=test@localhost",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "completed issue",
            ],
        )
        .await
        .unwrap();
        let head = git::snapshot(&p.path).await.unwrap().head;
        publish_to(&p, &head, remote.to_str().unwrap(), false)
            .await
            .unwrap();
        assert_eq!(
            git::git(
                remote.to_str().unwrap(),
                &["rev-parse", &format!("refs/heads/{}", p.branch)]
            )
            .await
            .unwrap(),
            head
        );
        std::fs::write(Path::new(&p.path).join("partial.txt"), "preserve this").unwrap();
        assert!(publish_to(&p, &head, remote.to_str().unwrap(), false)
            .await
            .is_err());
        let again = checkout(&root, &p, remote.to_str().unwrap(), false)
            .await
            .unwrap();
        assert_eq!(again.0, p.path);
        assert_eq!(
            std::fs::read_to_string(Path::new(&p.path).join("partial.txt")).unwrap(),
            "preserve this"
        );
        assert_eq!(
            git::git(remote.to_str().unwrap(), &["rev-parse", "refs/heads/main"])
                .await
                .unwrap(),
            git::snapshot(&app.store.project("p").unwrap().path)
                .await
                .unwrap()
                .head
        );
    }
    #[tokio::test]
    async fn auto_validation_is_saved_once_and_never_replaces_explicit_commands() {
        let (_temp, app, mut p) = crate::execution::tests::fixture(false, false).await;
        std::fs::write(
            Path::new(&p.path).join("package.json"),
            r#"{"scripts":{"test":"node --test"}}"#,
        )
        .unwrap();
        p.policy.test_commands.clear();
        let detected = configure_validation(&app, p).unwrap();
        assert_eq!(detected.policy.test_commands, vec![vec!["npm", "test"]]);
        assert_eq!(
            app.store
                .project(&detected.id)
                .unwrap()
                .policy
                .test_commands,
            detected.policy.test_commands
        );
        let mut explicit = detected;
        explicit.policy.test_commands = vec![vec!["custom-validator".into()]];
        assert_eq!(
            configure_validation(&app, explicit)
                .unwrap()
                .policy
                .test_commands,
            vec![vec!["custom-validator"]]
        );
    }
    #[test]
    fn validation_detection_requires_real_tests_and_preserves_configured_policy() {
        let temp = tempfile::tempdir().unwrap();
        assert!(detect_validation(temp.path()).is_empty());
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"scripts":{"test":"echo Error: no test specified && exit 1"}}"#,
        )
        .unwrap();
        assert!(detect_validation(temp.path()).is_empty());
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"scripts":{"test":"node --test","build":"tsc"}}"#,
        )
        .unwrap();
        assert_eq!(
            detect_validation(temp.path()),
            vec![vec!["npm", "test"], vec!["npm", "run", "build"]]
        );
    }
    #[test]
    fn namespaced_npm_tests_are_detected_without_selecting_maintenance_or_watch_scripts() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("package.json"),r#"{"scripts":{"test:parser":"tsx --test tests/report-parser.test.ts","test:watch":"vitest --watch","test:update":"vitest -u","dev":"next dev","build":"next build","db:reset":"tsx scripts/db-reset.ts"}}"#).unwrap();
        assert_eq!(
            detect_validation(temp.path()),
            vec![
                vec!["npm", "run", "test:parser"],
                vec!["npm", "run", "build"]
            ]
        );
    }
    #[test]
    fn destinations_are_confined_and_branches_are_scoped() {
        assert!(destination(Path::new("root"), "../outside").is_err());
        assert!(destination(Path::new("root"), "C:/other").is_err());
        assert_eq!(
            destination(Path::new("root"), "project-123").unwrap(),
            Path::new("root").join("project-123")
        );
        assert_eq!(branch("project-123"), "work/aiflow-project-123");
    }
}
