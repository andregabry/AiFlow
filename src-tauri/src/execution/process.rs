use crate::{
    model::*,
    providers::{AIExecutionProvider, Launch},
};
use std::{
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
};

pub struct ProcessResult {
    pub code: Option<i32>,
    pub cancelled: bool,
    pub tail: String,
    pub session_id: Option<String>,
    pub usage: Option<ProviderUsage>,
    pub reported_failure: bool,
}
pub(crate) async fn pump<R: AsyncRead + Unpin>(
    mut reader: R,
    stream: &'static str,
    tx: mpsc::Sender<(String, String)>,
) {
    let mut chunk = [0u8; 4096];
    let mut pending = vec![];
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                pending.extend_from_slice(&chunk[..n]);
                while let Some(pos) = pending.iter().position(|b| *b == b'\n') {
                    let bytes: Vec<_> = pending.drain(..=pos).collect();
                    if tx
                        .send((stream.into(), String::from_utf8_lossy(&bytes).into()))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                // Bound malformed/unbroken output. Streaming never waits for process completion.
                if pending.len() >= 16384 {
                    let text = String::from_utf8_lossy(&pending).into_owned();
                    pending.clear();
                    if tx.send((stream.into(), text)).await.is_err() {
                        return;
                    }
                }
            }
        }
    }
    if !pending.is_empty() {
        let _ = tx
            .send((stream.into(), String::from_utf8_lossy(&pending).into()))
            .await;
    }
}
pub async fn terminate(pid: u32) -> Result<()> {
    #[cfg(windows)]
    let output = crate::git::command("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output()
        .await;
    #[cfg(unix)]
    let output = crate::git::command("kill")
        .args(["-TERM", "--", &format!("-{pid}")])
        .output()
        .await;
    let o = output.map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err("Could not confirm process tree cancellation; worktree remains locked".into());
    }
    Ok(())
}
pub async fn run(
    launch: &Launch,
    args: &[String],
    path: &str,
    prompt: &str,
    provider: Arc<dyn AIExecutionProvider>,
    cancel: Arc<AtomicBool>,
    output: impl FnMut(&str, &str, &serde_json::Value) -> Result<()>,
) -> Result<ProcessResult> {
    run_inner(launch, args, path, prompt, Some(provider), cancel, output).await
}
pub async fn run_auth(
    launch: &Launch,
    args: &[String],
    cancel: Arc<AtomicBool>,
    output: impl FnMut(&str, &str, &serde_json::Value) -> Result<()>,
) -> Result<ProcessResult> {
    run_inner(
        launch,
        args,
        &std::env::temp_dir().to_string_lossy(),
        "",
        None,
        cancel,
        output,
    )
    .await
}
async fn run_inner(
    launch: &Launch,
    args: &[String],
    path: &str,
    prompt: &str,
    provider: Option<Arc<dyn AIExecutionProvider>>,
    cancel: Arc<AtomicBool>,
    mut output: impl FnMut(&str, &str, &serde_json::Value) -> Result<()>,
) -> Result<ProcessResult> {
    let mut command = launch.command();
    if provider.is_none() {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let executable = executable.to_string_lossy().replace('\\', "/");
        command.env(
            "GH_BROWSER",
            format!("\"{executable}\" --aiflow-managed-browser"),
        );
    }
    let mut child = command
        .args(args)
        .env_remove("GH_DEBUG")
        .current_dir(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let pid = child.id().ok_or("Process did not start")?;
    #[cfg(windows)]
    let mut job = Some(match crate::system::ProcessJob::attach(pid) {
        Ok(j) => j,
        Err(e) => {
            let _ = child.kill().await;
            return Err(e);
        }
    });
    let _awake = crate::system::AwakeGuard::running();
    let mut stdin = child.stdin.take().ok_or("stdin unavailable")?;
    let prompt = prompt.as_bytes().to_vec();
    let writer = tokio::spawn(async move {
        stdin.write_all(&prompt).await?;
        stdin.shutdown().await
    });
    let (tx, mut rx) = mpsc::channel(256);
    let stdout = tokio::spawn(pump(
        child.stdout.take().ok_or("stdout unavailable")?,
        "stdout",
        tx.clone(),
    ));
    let stderr = tokio::spawn(pump(
        child.stderr.take().ok_or("stderr unavailable")?,
        "stderr",
        tx.clone(),
    ));
    drop(tx);
    let mut tail = String::new();
    let mut session_id = None;
    let mut usage = None;
    let mut cancelled = false;
    let mut reported_failure = false;
    let mut streams_closed = false;
    let mut exit_status = None;
    let mut drain_started = None;
    let mut stdout_redactor = crate::security::StreamRedactor::default();
    let mut stderr_redactor = crate::security::StreamRedactor::default();
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(150));
    loop {
        tokio::select! {
            line=rx.recv(), if !streams_closed=>{ match line {
                None=>streams_closed=true,
                Some((stream,line))=>{
                    let clean=if stream=="stderr" {stderr_redactor.apply(&line)} else {stdout_redactor.apply(&line)};
                    let parsed=provider.as_ref().map(|p|p.parse_output(clean.trim_end())).unwrap_or_else(||serde_json::json!({}));
                    if let Some(s)=parsed["sessionId"].as_str() { if crate::security::valid_session_id(s) {session_id=Some(s.into());} }
                    if let Some(u)=provider.as_ref().and_then(|p|p.parse_quota_information(clean.trim())) { usage=Some(u); }
                    if parsed["failed"]==true { reported_failure=true; }
                    if let Err(e)=output(&stream,&clean,&parsed) { let _=terminate(pid).await; let _=child.wait().await; return Err(e); }
                    tail.push_str(&clean);
                    if tail.len()>65536 { let boundary=tail.char_indices().find(|(i,_)|*i>=tail.len()-32768).map(|(i,_)|i).unwrap_or(0); tail.drain(..boundary); }
                }
            } },
            _=interval.tick()=>{
                if cancel.load(Ordering::SeqCst) && !cancelled {
                    if child.try_wait().map_err(|e|e.to_string())?.is_none() { terminate(pid).await?; }
                    cancelled=true;
                }
                if exit_status.is_none() { exit_status=child.try_wait().map_err(|e|e.to_string())?; }
                if exit_status.is_some() {
                    if streams_closed { break; }
                    let since=drain_started.get_or_insert_with(std::time::Instant::now);
                    if since.elapsed()>std::time::Duration::from_secs(2) {
                        // A descendant may still own a pipe after its parent exited.
                        #[cfg(windows)] drop(job.take());
                        #[cfg(unix)] { let _=terminate(pid).await; }
                    }
                }
            }
        }
    }
    let status = if let Some(status) = exit_status {
        status
    } else {
        child.wait().await.map_err(|e| e.to_string())?
    };
    stdout.await.map_err(|e| e.to_string())?;
    stderr.await.map_err(|e| e.to_string())?;
    let _ = writer.await;
    Ok(ProcessResult {
        code: status.code(),
        cancelled,
        tail,
        session_id,
        usage,
        reported_failure,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn stdout_and_stderr_arrive_before_eof() {
        let (mut writer, reader) = tokio::io::duplex(1024);
        let (tx, mut rx) = mpsc::channel(8);
        let reader_task = tokio::spawn(pump(reader, "stdout", tx));
        writer.write_all(b"line 1\n").await.unwrap();
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
                .await
                .unwrap()
                .unwrap()
                .1,
            "line 1\n"
        );
        assert!(!reader_task.is_finished());
        writer.write_all(b"line 2\n").await.unwrap();
        assert_eq!(rx.recv().await.unwrap().1, "line 2\n");
        drop(writer);
        reader_task.await.unwrap();
    }
    #[tokio::test]
    async fn fake_process_streams_before_exit_and_is_persisted() {
        let temp = tempfile::tempdir().unwrap();
        let store = crate::persistence::Store::open(&temp.path().join("test.db")).unwrap();
        #[cfg(windows)]
        let launch = Launch {
            program: std::path::PathBuf::from(std::env::var("SystemRoot").unwrap())
                .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
            prefix: vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
            ],
        };
        #[cfg(windows)] let args=vec!["[Console]::Out.WriteLine('line 1'); Start-Sleep -Milliseconds 500; [Console]::Out.WriteLine('line 2'); [Console]::Error.WriteLine('stderr'); Start-Sleep -Milliseconds 500; [Console]::Out.WriteLine('line 3')".into()];
        #[cfg(unix)]
        let launch = Launch {
            program: "/bin/sh".into(),
            prefix: vec!["-c".into()],
        };
        #[cfg(unix)] let args=vec!["printf 'line 1\n'; sleep 0.5; printf 'line 2\n'; printf 'stderr\n' >&2; sleep 0.5; printf 'line 3\n'".into()];
        let start = std::time::Instant::now();
        let mut first_time = None;
        let result = run(
            &launch,
            &args,
            temp.path().to_str().unwrap(),
            "",
            Arc::new(crate::providers::codex::CodexCliProvider),
            Arc::new(AtomicBool::new(false)),
            |stream, raw, _| {
                if raw.contains("line 1") {
                    first_time = Some(start.elapsed());
                }
                store.event(
                    "fake-process",
                    1,
                    "OUTPUT_RECEIVED",
                    serde_json::json!({"stream":stream,"content":raw}),
                )?;
                Ok(())
            },
        )
        .await
        .unwrap();
        assert_eq!(result.code, Some(0));
        let elapsed = start.elapsed();
        assert!(
            elapsed - first_time.expect("first line delivered")
                >= std::time::Duration::from_millis(750)
        );
        let rows = store.events("fake-process", 0).unwrap();
        assert!(rows.iter().any(|e| e.payload["stream"] == "stderr"));
        assert!(rows
            .iter()
            .any(|e| e.payload["content"].as_str().unwrap().contains("line 3")));
    }
    #[tokio::test]
    async fn fake_process_cancel_preserves_output() {
        let temp = tempfile::tempdir().unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        #[cfg(windows)]
        let launch = Launch {
            program: std::path::PathBuf::from(std::env::var("SystemRoot").unwrap())
                .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
            prefix: vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
            ],
        };
        #[cfg(windows)]
        let args = vec!["[Console]::Out.WriteLine('partial work'); Start-Sleep -Seconds 30".into()];
        #[cfg(unix)]
        let launch = Launch {
            program: "/bin/sh".into(),
            prefix: vec!["-c".into()],
        };
        #[cfg(unix)]
        let args = vec!["printf 'partial work\n'; sleep 30".into()];
        let r = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            run(
                &launch,
                &args,
                temp.path().to_str().unwrap(),
                "",
                Arc::new(crate::providers::codex::CodexCliProvider),
                cancel,
                |_, raw, _| {
                    if raw.contains("partial") {
                        flag.store(true, Ordering::SeqCst);
                    }
                    Ok(())
                },
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(r.cancelled);
        assert!(r.tail.contains("partial work"));
    }
}
