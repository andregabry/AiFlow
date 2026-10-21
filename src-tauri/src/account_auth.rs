//! Authentication through the installed official CLI only. Never starts inference.
use crate::{model::Result, providers::Launch};
use serde_json::{json, Value};
use std::{
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{io::AsyncWriteExt, sync::mpsc};

pub fn authentication_error(text: &str) -> String {
    let lower = text.to_lowercase();
    if lower.contains("admin")
        || lower.contains("organization policy")
        || lower.contains("access_denied")
    {
        "Your organization must allow the official CLI for this account. AiFlow cannot change administrator policies.".into()
    } else {
        "The official CLI could not confirm sign-in. Retry the browser steps or check the CLI account settings.".into()
    }
}

fn auth_request(response: &Value) -> Result<Value> {
    if response["result"]["protocolVersion"] != 1 {
        return Err(
            "This Gemini CLI uses an unsupported authentication protocol. Update the official CLI."
                .into(),
        );
    }
    let methods = response["result"]["authMethods"]
        .as_array()
        .ok_or("Gemini CLI did not advertise authentication methods")?;
    let method = methods.iter().find(|m| m["id"] == "oauth-personal" && m["type"].as_str().unwrap_or("agent") == "agent")
        .ok_or("This Gemini CLI does not advertise Google account sign-in through ACP. Update the official CLI.")?;
    Ok(json!({"jsonrpc":"2.0","id":2,"method":"authenticate","params":{"methodId":method["id"]}}))
}

pub async fn gemini(
    launch: &Launch,
    cancel: Arc<AtomicBool>,
    mut output: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let help = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        launch.command().arg("--help").output(),
    )
    .await
    .map_err(|_| "Gemini CLI help timed out")?
    .map_err(|e| e.to_string())?;
    let help = format!(
        "{} {}",
        String::from_utf8_lossy(&help.stdout),
        String::from_utf8_lossy(&help.stderr)
    );
    let flag = if help.contains("--acp") {
        "--acp"
    } else if help.contains("--experimental-acp") {
        "--experimental-acp"
    } else {
        return Err(
            "Update Gemini CLI to a version supporting official ACP authentication.".into(),
        );
    };
    gemini_protocol(launch, flag, cancel, &mut output).await
}

async fn gemini_protocol(
    launch: &Launch,
    flag: &str,
    cancel: Arc<AtomicBool>,
    output: &mut impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let mut child = launch
        .command()
        .arg(flag)
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let pid = child.id().ok_or("Gemini CLI did not start")?;
    #[cfg(windows)]
    let _job = match crate::system::ProcessJob::attach(pid) {
        Ok(job) => job,
        Err(e) => {
            let _ = child.kill().await;
            return Err(e);
        }
    };
    let mut input = child.stdin.take().ok_or("Gemini CLI input unavailable")?;
    let (tx, mut rx) = mpsc::channel(64);
    let stdout = tokio::spawn(crate::execution::process::pump(
        child.stdout.take().ok_or("Gemini CLI output unavailable")?,
        "stdout",
        tx.clone(),
    ));
    let stderr = tokio::spawn(crate::execution::process::pump(
        child.stderr.take().ok_or("Gemini CLI errors unavailable")?,
        "stderr",
        tx,
    ));
    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{},"clientInfo":{"name":"aiflow","version":env!("CARGO_PKG_VERSION")}}});
    let mut redactor = crate::security::StreamRedactor::default();
    let mut authenticating = false;
    let result: Result<()> = async {
        input.write_all(format!("{init}\n").as_bytes()).await.map_err(|e|e.to_string())?;
        let timeout = tokio::time::sleep(std::time::Duration::from_secs(600));
        tokio::pin!(timeout);
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            tokio::select! {
                _ = &mut timeout => return Err("Google sign-in timed out. Retry to open a new login.".into()),
                _ = tick.tick() => { if cancel.load(Ordering::SeqCst) {return Err("Account connection cancelled".into());} },
                line = rx.recv() => {
                    let (stream,line) = line.ok_or("Gemini CLI ended before confirming authentication")?;
                    if stream == "stderr" { output(&redactor.apply(&line))?; continue; }
                    let Ok(v) = serde_json::from_str::<Value>(&line) else { continue; };
                    if v["id"] == 1 && !authenticating {
                        if v.get("error").is_some() { return Err(authentication_error(&v.to_string())); }
                        let request = auth_request(&v)?;
                        input.write_all(format!("{request}\n").as_bytes()).await.map_err(|e|e.to_string())?;
                        authenticating = true;
                    } else if v["id"] == 2 && authenticating {
                        if v.get("error").is_some() { return Err(authentication_error(&v.to_string())); }
                        if v.get("result").is_some() && !cancel.load(Ordering::SeqCst) { return Ok(()); }
                        return Err("Gemini CLI did not confirm Google sign-in".into());
                    } else if v.get("method").is_some() && v.get("id").is_some() {
                        let denied=json!({"jsonrpc":"2.0","id":v["id"],"error":{"code":-32601,"message":"AiFlow authentication does not expose client tools"}});
                        input.write_all(format!("{denied}\n").as_bytes()).await.map_err(|e|e.to_string())?;
                    }
                }
            }
        }
    }.await;
    let _ = crate::execution::process::terminate(pid).await;
    let _ = child.kill().await;
    let _ = child.wait().await;
    stdout.abort();
    stderr.abort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_only_advertised_agent_google_auth() {
        let response = json!({"result":{"protocolVersion":1,"authMethods":[{"id":"oauth-personal","name":"Google"}]}});
        assert_eq!(
            auth_request(&response).unwrap()["params"]["methodId"],
            "oauth-personal"
        );
        assert!(auth_request(&json!({"result":{"protocolVersion":2,"authMethods":[]}})).is_err());
        assert!(auth_request(&json!({"result":{"protocolVersion":1,"authMethods":[{"id":"oauth-personal","type":"terminal"}]}})).is_err());
        assert!(auth_request(
            &json!({"result":{"protocolVersion":1,"authMethods":[{"id":"api-key"}]}})
        )
        .is_err());
    }
    #[tokio::test]
    async fn official_protocol_confirms_auth_without_prompting_a_model() {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("agent.cjs");
        std::fs::write(&script, r#"const r=require('readline').createInterface({input:process.stdin}); r.on('line',s=>{ const v=JSON.parse(s); if(v.method==='initialize') console.log(JSON.stringify({jsonrpc:'2.0',id:v.id,result:{protocolVersion:1,authMethods:[{id:'oauth-personal'}]}})); else if(v.method==='authenticate' && v.params.methodId==='oauth-personal') console.log(JSON.stringify({jsonrpc:'2.0',id:v.id,result:{}})); else process.exit(10); });"#).unwrap();
        let launch = Launch {
            program: which::which("node").unwrap(),
            prefix: vec![script.to_string_lossy().into_owned()],
        };
        let result = gemini_protocol(
            &launch,
            "--acp",
            Arc::new(AtomicBool::new(false)),
            &mut |_| Ok(()),
        )
        .await;
        assert!(result.is_ok(), "{result:?}");
    }
}
