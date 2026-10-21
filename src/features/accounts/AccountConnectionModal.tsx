import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ArrowRight,
  ArrowUpRight,
  Check,
  CircleCheck,
  Copy,
  LoaderCircle,
  ShieldCheck,
} from "lucide-react";
import { desktop, useApp } from "../../core/store";
import {
  providerNames,
  type ProviderStatus,
  type Snapshot,
} from "../../core/types";
import { AiButton, AiSheet } from "../../components/ui";
import { githubUserCode } from "./githubSignIn";
import { officialProviderUrl, providerUserCode } from "./providerSignIn";

type Phase = "prepare" | "authorize" | "verify" | "complete";
export function AccountConnectionModal({
  accountId,
  onClose,
}: {
  accountId: string;
  onClose: () => void;
}) {
  const github = accountId === "github";
  const name = github ? "GitHub" : (providerNames[accountId] ?? accountId);
  const [executionId, setExecutionId] = useState(
    () => `auth-${crypto.randomUUID()}`,
  );
  const [phase, setPhase] = useState<Phase>("prepare");
  const [busy, setBusy] = useState(false),
    [cancelling, setCancelling] = useState(false);
  const [error, setError] = useState<string>(),
    [account, setAccount] = useState<string>();
  const [copied, setCopied] = useState(false);
  const alive = useRef(true),
    running = useRef(false),
    starting = useRef(false),
    closing = useRef(false);
  const opened = useRef<string | undefined>(undefined);
  const provider = useApp((s) =>
    s.data.providers.find((p) => p.id === accountId),
  );
  const events = useApp((s) => s.events[executionId]);
  const output = (events ?? []).map((e) => e.payload.content ?? "").join("\n");
  const code = github ? githubUserCode(output) : providerUserCode(output);
  const loginUrl = github
    ? "https://github.com/login/device"
    : officialProviderUrl(accountId, output);
  const step = ["prepare", "authorize", "verify", "complete"].indexOf(phase);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const closeWindow = useCallback(async () => {
    if (github) await invoke("github_auth_window_close");
    else await invoke("provider_auth_window_close");
  }, [github]);
  const openLogin = useCallback(
    async (external = false) => {
      if (!loginUrl)
        throw new Error(
          "The official CLI has not supplied a sign-in address yet.",
        );
      if (external || accountId === "gemini") {
        await openUrl(loginUrl);
        return;
      }
      if (github) {
        if (!code) throw new Error("Wait for the GitHub sign-in code.");
        await invoke("github_auth_window_open", { code });
      } else
        await invoke("provider_auth_window_open", {
          providerId: accountId,
          url: loginUrl,
        });
    },
    [accountId, code, github, loginUrl],
  );
  useEffect(() => {
    const key = github ? code : loginUrl;
    if (
      !desktop ||
      phase !== "authorize" ||
      !key ||
      accountId === "gemini" ||
      opened.current === key ||
      closing.current
    )
      return;
    opened.current = key;
    void openLogin().catch((e) => {
      if (alive.current && !closing.current)
        setError(
          `Could not open sign-in: ${String(e)}. Use the browser option below if required.`,
        );
    });
  }, [accountId, code, github, loginUrl, openLogin, phase]);

  const close = useCallback(() => {
    if (!desktop) {
      onClose();
      return;
    }
    if (starting.current || closing.current) return;
    closing.current = true;
    setCancelling(true);
    void (async () => {
      try {
        if (running.current)
          await invoke("cancel_account_connection", { executionId });
        await closeWindow();
        onClose();
      } catch (e) {
        if (alive.current) {
          setError(String(e));
          setCancelling(false);
          closing.current = false;
        }
      }
    })();
  }, [closeWindow, executionId, onClose]);

  const verify = useCallback(async () => {
    setPhase("verify");
    setBusy(true);
    setError(undefined);
    try {
      if (github) {
        const data = await invoke<Snapshot>("snapshot");
        if (!data.githubAccount)
          throw new Error(
            "The official GitHub CLI has not confirmed this account yet.",
          );
        if (alive.current && !closing.current) setAccount(data.githubAccount);
      } else {
        const statuses = await invoke<ProviderStatus[]>("scan_providers");
        const current = statuses.find((p) => p.id === accountId);
        if (current?.authentication !== "CONNECTED")
          throw new Error(
            current?.authentication === "UNKNOWN"
              ? "The official CLI has not provided a verifiable account status. AiFlow cannot confirm sign-in."
              : "Finish the official CLI sign-in and check the connection again.",
          );
      }
      await closeWindow();
      await useApp.getState().refresh();
      if (alive.current && !closing.current) setPhase("complete");
    } catch (e) {
      if (alive.current && !closing.current) setError(String(e));
    } finally {
      if (alive.current) setBusy(false);
    }
  }, [accountId, closeWindow, github]);

  async function start() {
    const session = `auth-${crypto.randomUUID()}`;
    setExecutionId(session);
    setCopied(false);
    opened.current = undefined;
    setError(undefined);
    if (!desktop) {
      setError("Open the AiFlow desktop application to connect an account.");
      return;
    }
    setBusy(true);
    starting.current = true;
    try {
      if (github) {
        const result = await invoke<{ method: "cli" | "existing" }>(
          "github_cli_start",
        );
        if (result.method === "existing") {
          await verify();
          return;
        }
        await useApp.getState().loadEvents(session);
        setPhase("authorize");
        running.current = true;
        starting.current = false;
        await invoke("github_cli_login", { executionId: session });
      } else {
        const statuses = await invoke<ProviderStatus[]>("scan_providers");
        await useApp.getState().refresh();
        const current = statuses.find((p) => p.id === accountId);
        if (!current || current.status === "NOT_INSTALLED")
          throw new Error(
            `Install the official ${name} CLI, then choose Continue again. The installation guide is linked below.`,
          );
        if (current.authentication === "CONNECTED") {
          await verify();
          return;
        }
        if (current.status === "INCOMPATIBLE")
          throw new Error(current.diagnostics);
        await useApp.getState().loadEvents(session);
        setPhase("authorize");
        running.current = true;
        starting.current = false;
        await invoke("provider_auth", {
          providerId: accountId,
          action: "login",
          executionId: session,
        });
      }
      running.current = false;
      if (alive.current && !closing.current) await verify();
    } catch (e) {
      running.current = false;
      void closeWindow().catch(() => undefined);
      if (alive.current && !closing.current) {
        setError(String(e));
        setPhase("prepare");
      }
    } finally {
      starting.current = false;
      if (alive.current) setBusy(false);
    }
  }
  async function copyCode() {
    try {
      if (code) {
        await navigator.clipboard.writeText(code);
        setCopied(true);
      }
    } catch (e) {
      setError(String(e));
    }
  }
  const guide = github ? "https://cli.github.com" : provider?.installationUrl;
  return (
    <AiSheet open title={`Connect ${name}`} onClose={close} modal>
      <div className="connection-wizard">
        <ol className="connection-steps" aria-label="Connection steps">
          {["Prepare", "Authorize", "Verify"].map((label, index) => (
            <li
              key={label}
              aria-current={step === index ? "step" : undefined}
              className={
                step > index ? "finished" : step === index ? "current" : ""
              }
            >
              <span>{step > index ? <Check size={13} /> : index + 1}</span>
              {label}
            </li>
          ))}
        </ol>
        {error && (
          <div className="connection-error" role="alert">
            {error}
          </div>
        )}
        {phase === "prepare" && (
          <section className="connection-step-content">
            <div className="connection-symbol">
              <ShieldCheck size={26} />
            </div>
            <h3>Connect your {name} account</h3>
            <p>
              AiFlow uses the official {name} CLI on this computer. An existing
              signed-in session is reused before starting another login.
              Credentials stay with that client.
            </p>
            <ol className="connection-instructions">
              <li>Check the installed CLI and its current account.</li>
              <li>
                If needed, sign in through the official client’s browser page.
              </li>
              <li>AiFlow verifies the CLI connection automatically.</li>
            </ol>
            {accountId === "gemini" && (
              <p>
                Google requires a secure external browser for sign-in. Gemini
                CLI opens it automatically.
              </p>
            )}
            <p>Organization access rules still apply to the official CLI.</p>
            {guide && (
              <a
                href={guide}
                target="_blank"
                rel="noreferrer"
                onClick={(e) => {
                  if (desktop) {
                    e.preventDefault();
                    void openUrl(guide).catch((e) => setError(String(e)));
                  }
                }}
              >
                Official CLI installation guide <ArrowUpRight size={12} />
              </a>
            )}
            <AiButton
              variant="primary"
              disabled={busy || cancelling}
              onClick={() => void start()}
            >
              {busy ? (
                <>
                  <LoaderCircle className="spin-slow" size={14} />
                  Preparing connection…
                </>
              ) : (
                <>
                  Continue
                  <ArrowRight size={14} />
                </>
              )}
            </AiButton>
          </section>
        )}
        {phase === "authorize" && (
          <section className="connection-step-content">
            <h3>
              {code ? `Enter your code on ${name}` : `Sign in to ${name}`}
            </h3>
            <p>
              Complete the official CLI login in the sign-in window. AiFlow
              confirms the connection when you finish.
            </p>
            {code && (
              <div className="connection-code">
                <span>YOUR ONE-TIME CODE</span>
                <strong>{code}</strong>
                <AiButton onClick={() => void copyCode()}>
                  <Copy size={13} />
                  {copied ? "Copied" : "Copy code"}
                </AiButton>
              </div>
            )}
            {accountId === "gemini" && (
              <p>
                The Google sign-in page opens in your external browser, as
                required by Google.
              </p>
            )}
            {!code && (
              <p>
                This CLI uses its normal browser login. A code is displayed only
                when the CLI supplies one.
              </p>
            )}
            {loginUrl && (!github || code) && (
              <>
                <AiButton
                  variant="primary"
                  onClick={() =>
                    void openLogin().catch((e) => setError(String(e)))
                  }
                >
                  Reopen {name} sign-in window <ArrowUpRight size={14} />
                </AiButton>
                {accountId !== "gemini" && (
                  <AiButton
                    variant="ghost"
                    onClick={() =>
                      void openLogin(true).catch((e) => setError(String(e)))
                    }
                  >
                    Use external browser if required by your sign-in method{" "}
                    <ArrowUpRight size={12} />
                  </AiButton>
                )}
              </>
            )}
            <p className="connection-wait" role="status">
              <LoaderCircle size={13} className="spin-slow" />
              {github && !code
                ? "Generating your sign-in code…"
                : "Waiting for the official CLI to confirm sign-in…"}
            </p>
          </section>
        )}
        {phase === "verify" && (
          <section className="connection-step-content">
            <h3>Confirming your connection</h3>
            <p>Checking the account through the official CLI.</p>
            {busy ? (
              <p className="connection-wait">
                <LoaderCircle size={16} className="spin-slow" />
                Checking {name}…
              </p>
            ) : (
              <AiButton variant="primary" onClick={() => void verify()}>
                Check again
              </AiButton>
            )}
          </section>
        )}
        {phase === "complete" && (
          <section className="connection-step-content connection-success">
            <CircleCheck size={42} />
            <h3>{name} is connected</h3>
            <p>
              {account
                ? `Signed in as ${account}.`
                : "The official CLI confirmed your sign-in."}{" "}
              You can now use this account in AiFlow.
            </p>
            <AiButton variant="primary" onClick={close}>
              Done
              <Check size={14} />
            </AiButton>
          </section>
        )}
        <footer className="connection-footer">
          <ShieldCheck size={13} />
          <span>
            Credentials remain with the official CLI. Never paste passwords or
            tokens here.
          </span>
          {phase !== "complete" && (
            <AiButton
              variant="ghost"
              disabled={(busy && phase === "prepare") || cancelling}
              onClick={close}
            >
              {cancelling ? "Cancelling…" : "Cancel"}
            </AiButton>
          )}
        </footer>
      </div>
    </AiSheet>
  );
}
