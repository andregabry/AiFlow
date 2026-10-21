#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    // GitHub CLI delegates browser opening through GH_BROWSER. The running UI
    // opens its isolated auth window when the device code arrives on stdout.
    if std::env::args().nth(1).as_deref() == Some("--aiflow-managed-browser") {
        return;
    }
    aiflow_lib::run();
}
