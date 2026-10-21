pub mod process;
pub mod prompt;
pub mod state;
#[cfg(test)]
pub(crate) mod tests;
use crate::{git, model::*, AppState};
use rusqlite::params;
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub fn check_cancelled(app: &AppState, project_id: &str) -> Result<()> {
    if app
        .active
        .lock()
        .unwrap()
        .get(project_id)
        .is_some_and(|flag| flag.load(Ordering::SeqCst))
    {
        return Err("Cancelled during validation or publication. Review remote state before retrying; the next issue was not released".into());
    }
    Ok(())
}
pub fn transition(
    app: &AppState,
    e: &mut Execution,
    to: &str,
    reason: Option<String>,
) -> Result<()> {
    if !state::allowed(&e.state, to) {
        return Err(format!("Invalid execution transition {} → {to}", e.state));
    }
    e.state = to.into();
    e.reason = reason;
    if matches!(
        to,
        "SUCCESS" | "FAILED" | "BLOCKED" | "HUMAN_REVIEW" | "CANCELLED"
    ) {
        e.finished_at = Some(now());
    }
    app.store.save_execution(e)?;
    app.emit(e, "STATE_CHANGED", json!({"state":to,"reason":e.reason}))?;
    app.changed();
    Ok(())
}
pub fn scheduler(app: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            interval.tick().await;
            if app
                .active
                .lock()
                .unwrap()
                .keys()
                .any(|key| key.starts_with("auth:"))
            {
                continue;
            }
            let Some(account) = app.github.account().await else {
                continue;
            };
            let projects = match app.store.account_projects(&account) {
                Ok(v) => v,
                Err(_) => continue,
            };
            for p in projects {
                let running = app
                    .store
                    .query(
                        "SELECT running FROM queues WHERE project_id=?",
                        [&p.id],
                        |r| r.get::<_, bool>(0),
                    )
                    .ok()
                    .and_then(|r| r.first().copied())
                    .unwrap_or(false);
                if !running {
                    continue;
                }
                let acquired = {
                    let mut active = app.active.lock().unwrap();
                    if active.contains_key(&p.id)
                        || active.keys().any(|key| key.starts_with("auth:"))
                    {
                        false
                    } else {
                        active.insert(p.id.clone(), Arc::new(AtomicBool::new(false)));
                        true
                    }
                };
                if !acquired {
                    continue;
                }
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let result = match crate::cloud::runtime(&app, &p.id) {
                        Ok(runtime) if runtime.mode == "cloud" => {
                            crate::cloud::step(app.clone(), p.clone(), runtime).await
                        }
                        Ok(_) => step(app.clone(), p.clone()).await,
                        Err(err) => Err(err),
                    };
                    if let Err(err) = result {
                        if let Ok(executions) = app.store.executions() {
                            if let Some(mut e) = executions.into_iter().find(|e| {
                                e.project_id == p.id
                                    && matches!(
                                        e.state.as_str(),
                                        "PREPARING" | "RUNNING" | "VALIDATING" | "COMMITTING"
                                    )
                            }) {
                                let _ = transition(
                                    &app,
                                    &mut e,
                                    "HUMAN_REVIEW",
                                    Some(crate::security::redact(&err)),
                                );
                            }
                        }
                        let _ = app
                            .store
                            .audit("ORCHESTRATOR_ERROR", json!({"project":p.id,"error":err}));
                        let _ = app
                            .store
                            .sql("UPDATE queues SET running=0 WHERE project_id=?", [&p.id]);
                    }
                    app.active.lock().unwrap().remove(&p.id);
                    app.changed();
                });
            }
        }
    });
}
async fn step(app: Arc<AppState>, p: Project) -> Result<()> {
    let items: Vec<_> = app
        .store
        .queue()?
        .into_iter()
        .filter(|q| q.project_id == p.id && !matches!(q.state.as_str(), "SUCCESS" | "SKIPPED"))
        .collect();
    let Some(mut item) = items.into_iter().next() else {
        app.store
            .sql("UPDATE queues SET running=0 WHERE project_id=?", [&p.id])?;
        return Ok(());
    };
    let prior = app
        .store
        .executions()?
        .into_iter()
        .find(|e| e.item_id == item.id);
    if matches!(
        item.state.as_str(),
        "WAITING_FOR_PROVIDER" | "WAITING_FOR_QUOTA" | "RETRY_WAIT"
    ) {
        let due = prior
            .as_ref()
            .and_then(|e| e.retry_at.as_ref())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|t| t <= chrono::Utc::now())
            .unwrap_or(false);
        if !due {
            return Ok(());
        }
        app.store.sql(
            "UPDATE queue_items SET state='QUEUED' WHERE id=?",
            [&item.id],
        )?;
        item.state = "QUEUED".into();
    }
    if item.state != "QUEUED" {
        app.store
            .sql("UPDATE queues SET running=0 WHERE project_id=?", [&p.id])?;
        return Ok(());
    }
    let provider = app.providers.select(&item.provider_id, &p.policy)?;
    let attempt = prior.as_ref().map(|e| e.attempt + 1).unwrap_or(1);
    let mut e = Execution {
        id: id(),
        item_id: item.id.clone(),
        project_id: p.id.clone(),
        provider_id: provider.id().into(),
        state: "QUEUED".into(),
        started_at: now(),
        finished_at: None,
        reason: None,
        before_git: None,
        after_git: None,
        retry_at: None,
        attempt,
        session_id: None,
    };
    app.store.save_execution(&e)?;
    transition(&app, &mut e, "PREPARING", None)?;
    app.github
        .refresh_issue(&app.store, &p, item.issue_number)
        .await?;
    let approval_needed = app
        .store
        .issues(&p.id)?
        .iter()
        .find(|i| i.number == item.issue_number)
        .map(|i| {
            i.labels.iter().any(|l| {
                p.policy
                    .manual_approval_labels
                    .iter()
                    .any(|x| x.eq_ignore_ascii_case(l))
            })
        })
        .unwrap_or(true);
    if approval_needed && !item.approved {
        return transition(
            &app,
            &mut e,
            "BLOCKED",
            Some(
                "Manual approval required by project policy. Review this issue before approving."
                    .into(),
            ),
        );
    }
    if attempt > p.policy.max_retries + 1 {
        return transition(
            &app,
            &mut e,
            "HUMAN_REVIEW",
            Some("Maximum attempts reached. Review history and project policy.".into()),
        );
    }
    let p = match crate::workspace::prepare(&app, p).await {
        Ok(p) => p,
        Err(reason) => {
            return transition(
                &app,
                &mut e,
                "BLOCKED",
                Some(format!("Could not prepare managed checkout: {reason}")),
            )
        }
    };
    let p = match crate::workspace::configure_validation(&app, p) {
        Ok(p) => p,
        Err(reason) => return transition(&app, &mut e, "BLOCKED", Some(reason)),
    };
    let lease = match git::lock(&p.path).await {
        Ok(v) => v,
        Err(err) => return transition(&app, &mut e, "BLOCKED", Some(err)),
    };
    let _lease = lease;
    app.store.sql(
        "INSERT OR REPLACE INTO worktree_locks VALUES(?,?,?,?)",
        params![p.path, e.id, std::process::id(), now()],
    )?;
    let resumed = prior.as_ref().filter(|x| {
        matches!(
            x.state.as_str(),
            "WAITING_FOR_PROVIDER" | "WAITING_FOR_QUOTA" | "RETRY_WAIT"
        )
    });
    let before = match if resumed.is_some() {
        git::snapshot(&p.path).await
    } else {
        git::validate(&p).await
    } {
        Ok(v) if v.branch == p.branch => v,
        Ok(_) => {
            return transition(
                &app,
                &mut e,
                "HUMAN_REVIEW",
                Some("Branch changed unexpectedly".into()),
            )
        }
        Err(err) => return transition(&app, &mut e, "BLOCKED", Some(err)),
    };
    e.before_git = resumed
        .and_then(|e| e.before_git.clone())
        .or_else(|| Some(before.clone()));
    app.store.save_execution(&e)?;
    let health = provider.health_check().await;
    if health.status != "AVAILABLE" {
        return transition(
            &app,
            &mut e,
            "BLOCKED",
            Some(format!("{}: {}", health.status, health.diagnostics)),
        );
    }
    if let Some(u) = app
        .store
        .usage()?
        .into_iter()
        .find(|u| u.provider_id == provider.id())
    {
        let retry = u
            .reset_at
            .as_ref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .or_else(|| {
                u.retry_after.and_then(|n| {
                    chrono::DateTime::parse_from_rfc3339(&u.detected_at)
                        .ok()
                        .map(|d| d.with_timezone(&chrono::Utc) + chrono::Duration::seconds(n))
                })
            });
        if let Some(t) = retry {
            if t > chrono::Utc::now() {
                e.retry_at = Some(t.to_rfc3339());
                return transition(
                    &app,
                    &mut e,
                    "WAITING_FOR_PROVIDER",
                    Some("Provider usage limit is still active".into()),
                );
            }
        }
    }
    let mut issue = app
        .store
        .issues(&p.id)?
        .into_iter()
        .find(|i| i.number == item.issue_number)
        .ok_or("Issue cache not found; refresh GitHub")?;
    if issue.state == "CLOSED" {
        return transition(
            &app,
            &mut e,
            "HUMAN_REVIEW",
            Some(
                "Issue was closed externally. Review before execution; local history is preserved."
                    .into(),
            ),
        );
    }
    issue.comments = app.github.comments(&p, item.issue_number).await?;
    let prompt = prompt::build(&p, &issue, resumed.map(|_| &before));
    let session = resumed
        .filter(|e| e.provider_id == provider.id())
        .and_then(|e| e.session_id.as_deref());
    let args = provider.execution_args(&health.capabilities, session)?;
    let launch = provider.detect().ok_or("Provider disappeared from PATH")?;
    let cancel = app
        .active
        .lock()
        .unwrap()
        .get(&p.id)
        .cloned()
        .ok_or("Execution lock missing")?;
    if cancel.load(Ordering::SeqCst) {
        return transition(
            &app,
            &mut e,
            "CANCELLED",
            Some("Cancelled before process start".into()),
        );
    }
    transition(&app, &mut e, "RUNNING", None)?;
    app.emit(
        &e,
        "PROVIDER_STARTED",
        json!({"providerId":provider.id(),"branch":before.branch}),
    )?;
    let result = process::run(
        &launch,
        &args,
        &p.path,
        &prompt,
        provider.clone(),
        cancel,
        |stream, raw, parsed| {
            app.emit(
                &e,
                "OUTPUT_RECEIVED",
                json!({"stream":stream,"content":raw,"formatted":parsed}),
            )
            .map(|_| ())
        },
    )
    .await;
    let result = match result {
        Ok(r) => r,
        Err(err) => return transition(&app, &mut e, "HUMAN_REVIEW", Some(err)),
    };
    e.session_id = result.session_id;
    if result.cancelled {
        return transition(&app,&mut e,"CANCELLED",Some("Process tree cancelled. Partial changes were preserved; inspect worktree before retry.".into()));
    }
    if result.code != Some(0) || result.reported_failure {
        let classification = provider.classify_error(&result.tail);
        let mut kind = classification;
        if matches!(kind, ErrorKind::AuthFailure) {
            if let Some(launch) = provider.detect() {
                if provider.get_authentication_status(&launch).await != "DISCONNECTED" {
                    kind = ErrorKind::TransientProviderError;
                }
            }
        }
        if matches!(kind, ErrorKind::UsageLimit) {
            let u = result.usage.unwrap_or(ProviderUsage {
                provider_id: provider.id().into(),
                detected_at: now(),
                reset_at: None,
                retry_after: None,
                source: "CLI usage-limit classification".into(),
                confidence: "reset unknown".into(),
            });
            let retry = u.reset_at.clone().or_else(|| {
                Some(
                    (chrono::Utc::now() + chrono::Duration::seconds(u.retry_after.unwrap_or(900)))
                        .to_rfc3339(),
                )
            });
            app.store.save_usage(&u)?;
            e.retry_at = retry;
            app.emit(
                &e,
                "QUOTA_REACHED",
                json!({"usage":u,"nextRetry":e.retry_at}),
            )?;
            return transition(&app,&mut e,"WAITING_FOR_PROVIDER",Some("Usage limit reached. Reset time is shown only when reported; otherwise retry after 15 minutes. Provider remains unchanged.".into()));
        }
        if matches!(
            kind,
            ErrorKind::TransientNetworkError
                | ErrorKind::TransientProviderError
                | ErrorKind::ProviderUnavailable
        ) && attempt <= p.policy.max_retries
        {
            e.retry_at = Some(
                (chrono::Utc::now()
                    + chrono::Duration::seconds((30 * 2_i64.pow(attempt.min(6))).min(900)))
                .to_rfc3339(),
            );
            return transition(
                &app,
                &mut e,
                "RETRY_WAIT",
                Some(format!(
                    "{:?}; exit {:?}. Bounded retry scheduled.",
                    kind, result.code
                )),
            );
        }
        return transition(
            &app,
            &mut e,
            if matches!(kind, ErrorKind::PermissionRequired) {
                "HUMAN_REVIEW"
            } else {
                "FAILED"
            },
            Some(format!(
                "{:?}; exit {:?}. Inspect persistent output for details.",
                kind, result.code
            )),
        );
    }
    transition(&app, &mut e, "VALIDATING", None)?;
    let validation = validate(&app, &p, &mut e, result.code).await;
    if let Err(reason) = validation {
        let report=app.github.report(&p,item.issue_number,&format!("### AiFlow: validation blocked\n\nExecution `{}` did not pass independent validation. AiFlow has not closed this issue or released the next task.\n\n{}",e.id,crate::security::redact(&reason))).await;
        let reason = match report {
            Ok(_) => reason,
            Err(err) => format!("{reason}. Could not publish the blocked report: {err}"),
        };
        return transition(&app, &mut e, "HUMAN_REVIEW", Some(reason));
    }
    let latest = app
        .github
        .refresh_issue(&app.store, &p, item.issue_number)
        .await?;
    if latest.updated_at != issue.updated_at || latest.state != "OPEN" {
        return transition(&app,&mut e,"HUMAN_REVIEW",Some("GitHub issue changed during execution. Review the latest acceptance criteria and remote state before completing.".into()));
    }
    let head = e
        .after_git
        .as_ref()
        .ok_or("Missing validated Git snapshot")?
        .head
        .clone();
    check_cancelled(&app, &p.id)?;
    let published = crate::workspace::publish(&app, &p, &head).await?;
    if p.policy.require_ci && !app.github.ci_passed(&p, &head).await? {
        return transition(
            &app,
            &mut e,
            "HUMAN_REVIEW",
            Some(
                "Required GitHub checks have not passed for the validated commit. Queue stopped."
                    .into(),
            ),
        );
    }
    let body = completion_report(
        &p,
        &e,
        if published {
            "Published to the managed GitHub branch"
        } else {
            "Local worktree; commit has not been published by AiFlow"
        },
    );
    check_cancelled(&app, &p.id)?;
    let url = app.github.report(&p, item.issue_number, &body).await?;
    app.emit(
        &e,
        "ISSUE_REPORT_PUBLISHED",
        json!({"url":url,"head":head,"published":published}),
    )?;
    check_cancelled(&app, &p.id)?;
    if p.policy.close_issue {
        app.github.close(&p, item.issue_number).await?;
        if app
            .github
            .refresh_issue(&app.store, &p, item.issue_number)
            .await?
            .state
            != "CLOSED"
        {
            return Err("Issue closure was not confirmed by the latest GitHub state".into());
        }
        app.emit(&e, "ISSUE_STATE_CHANGED", json!({"githubState":"CLOSED"}))?;
    }
    check_cancelled(&app, &p.id)?;
    app.store.sql(
        "DELETE FROM provider_usage WHERE provider_id=?",
        [provider.id()],
    )?;
    transition(&app,&mut e,"SUCCESS",Some("Independent validation passed. Acceptance criteria still available in execution history.".into()))?;
    app.notify(
        "Issue completed",
        &format!("#{} · {}", issue.number, issue.title),
    );
    app.store
        .sql("DELETE FROM worktree_locks WHERE execution_id=?", [&e.id])?;
    app.store.sql(
        "UPDATE queues SET running=0 WHERE project_id=? AND stop_after_current=1",
        [&p.id],
    )?;
    Ok(())
}
async fn validate(app: &AppState, p: &Project, e: &mut Execution, code: Option<i32>) -> Result<()> {
    app.emit(e, "VALIDATION_STARTED", json!({}))?;
    if code != Some(0) {
        return Err("Provider did not exit successfully".into());
    }
    let before = e
        .before_git
        .as_ref()
        .ok_or("Missing pre-execution snapshot")?
        .clone();
    let first = git::snapshot(&p.path).await?;
    if first.branch != before.branch {
        return Err("Branch changed unexpectedly. Human review required.".into());
    }
    for args in &p.policy.test_commands {
        if args.is_empty() {
            return Err("Empty validation command".into());
        }
        app.emit(
            e,
            "COMMAND_STARTED",
            json!({"command":args,"stream":"validation"}),
        )?;
        let executable = which::which(&args[0])
            .map_err(|_| format!("Validation executable not found: {}", args[0]))?;
        // No cmd.exe / shell strings. Windows npm shims are deliberately not passed to a shell.
        let launch = if matches!(
            executable.extension().and_then(|x| x.to_str()),
            Some("cmd" | "bat" | "ps1")
        ) {
            if args[0] != "npm" {
                return Err(
                    "Use a native executable or node + script for validation commands".into(),
                );
            }
            let script = executable
                .parent()
                .ok_or("npm parent missing")?
                .join("node_modules/npm/bin/npm-cli.js");
            if !script.is_file() {
                return Err("Cannot resolve npm CLI without shell".into());
            }
            crate::providers::Launch {
                program: which::which("node").map_err(|e| e.to_string())?,
                prefix: vec![script.to_string_lossy().into()],
            }
        } else {
            crate::providers::Launch {
                program: executable,
                prefix: vec![],
            }
        };
        let provider = app.providers.get(&e.provider_id)?;
        let cancel = app
            .active
            .lock()
            .unwrap()
            .get(&p.id)
            .cloned()
            .ok_or("Execution lock missing")?;
        let future = process::run(
            &launch,
            &args[1..],
            &p.path,
            "",
            provider,
            cancel,
            |_, raw, _| {
                app.emit(
                    e,
                    "OUTPUT_RECEIVED",
                    json!({"stream":"validation","content":raw}),
                )
                .map(|_| ())
            },
        );
        // Process supervisor controls cancellation. Commands must finish or user can cancel.
        let r = future.await?;
        app.emit(
            e,
            "COMMAND_FINISHED",
            json!({"command":args,"exitCode":r.code}),
        )?;
        if r.cancelled || r.code != Some(0) {
            return Err(format!("TEST_FAILURE: {:?}, exit {:?}", args, r.code));
        }
    }
    let after = git::snapshot(&p.path).await?;
    e.after_git = Some(after.clone());
    app.store.save_execution(e)?;
    if after.branch != before.branch {
        return Err("Branch changed during validation".into());
    }
    if p.policy.require_clean_tree && !after.status.is_empty() {
        return Err(
            "VALIDATION_REQUIRED: working tree is dirty; partial work was preserved".into(),
        );
    }
    if p.policy.require_commit {
        if after.head == before.head {
            return Err("No new commit was produced".into());
        }
        git::git(
            &p.path,
            &["merge-base", "--is-ancestor", &before.head, &after.head],
        )
        .await
        .map_err(|_| "Commit history is not a fast-forward from the original HEAD")?;
        app.emit(e, "COMMIT_CREATED", json!({"head":after.head}))?;
    }
    app.emit(
        e,
        "VALIDATION_FINISHED",
        json!({"passed":true,"testCommands":p.policy.test_commands.len(),"head":after.head}),
    )?;
    Ok(())
}

pub fn completion_report(p: &Project, e: &Execution, publication: &str) -> String {
    let head = e
        .after_git
        .as_ref()
        .map(|s| s.head.as_str())
        .unwrap_or("not available");
    format!("### AiFlow: independently validated\n\n- Execution: `{}`\n- Provider: `{}`\n- Commit: `{}`\n- Branch: `{}`\n- Publication: {}\n- Independent validation commands passed: {}\n- Required GitHub checks: {}\n\nThe controller releases the next queued issue only after GitHub confirms this report{}.",e.id,e.provider_id,head,p.branch,publication,p.policy.test_commands.len(),if p.policy.require_ci {"passed on this exact commit"} else {"not required by policy"},if p.policy.close_issue {" and issue closure"} else {""})
}
