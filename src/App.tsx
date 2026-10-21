import { useCallback, useEffect, useState, useMemo, useRef } from "react";
import { motion, AnimatePresence } from "motion/react";
import {
  Home,
  FolderGit2,
  CircleDot,
  Layers3,
  Activity,
  Cpu,
  ScrollText,
  Settings2,
  Search,
  Plus,
  ChevronDown,
  ArrowUpRight,
  Play,
  Pause,
  RefreshCw,
  GitBranch,
  ShieldCheck,
  ArrowRight,
  Check,
  X,
  GripVertical,
  StopCircle,
  Clock3,
  UserRound,
  Sun,
  Moon,
  Monitor,
  Zap,
  PanelLeftClose,
  Github,
  CircleCheck,
  ChevronRight,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { branding } from "./design-system/branding";
import { connectEvents, desktop, useApp } from "./core/store";
import {
  countdown,
  defaultPolicy,
  providerNames,
  taskState,
  type Project,
  type Issue,
  type QueueItem,
  type Execution,
  type GitSnapshot,
} from "./core/types";
import {
  AiButton,
  AiStatusBadge,
  AiProviderBadge,
  AiSegmentedControl,
  AiSheet,
  Empty,
} from "./components/ui";
import { AiExecutionConsole, AiTimeline } from "./components/Console";
import { AccountConnectionModal } from "./features/accounts/AccountConnectionModal";

const navigation = [
  { group: "", items: [{ name: "Home", icon: Home }] },
  {
    group: "WORKSPACE",
    items: [
      { name: "Projects", icon: FolderGit2 },
      { name: "Issues", icon: CircleDot },
      { name: "Queue", icon: Layers3 },
      { name: "Executions", icon: Activity },
    ],
  },
  {
    group: "INTELLIGENCE",
    items: [
      { name: "Providers", icon: Cpu },
      { name: "Usage", icon: Clock3 },
      { name: "Accounts", icon: UserRound },
    ],
  },
  {
    group: "SYSTEM",
    items: [
      { name: "Logs", icon: ScrollText },
      { name: "Settings", icon: Settings2 },
    ],
  },
];
function external(url: string) {
  if (
    !/^https:\/\/(?:github\.com|developers\.openai\.com|code\.claude\.com|geminicli\.com)(?:\/|$)/.test(
      url,
    )
  )
    return;
  if (desktop)
    void openUrl(url).catch((e) => useApp.setState({ error: String(e) }));
  else window.open(url, "_blank", "noopener,noreferrer");
}
function ProviderPicker({
  value,
  onChange,
  disabled = false,
}: {
  value: string;
  onChange: (id: string) => void;
  disabled?: boolean;
}) {
  return (
    <select
      aria-label="AI provider"
      className="provider-select"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      disabled={disabled}
    >
      {Object.entries(providerNames).map(([id, name]) => (
        <option key={id} value={id}>
          {name}
        </option>
      ))}
    </select>
  );
}
export default function App() {
  const data = useApp((s) => s.data),
    projectId = useApp((s) => s.projectId),
    loading = useApp((s) => s.loading),
    error = useApp((s) => s.error),
    notice = useApp((s) => s.notice);
  const [page, setPage] = useState("Home");
  const [projectSheet, setProjectSheet] = useState(false),
    [editProject, setEditProject] = useState<Project | undefined>();
  const [selected, setSelected] = useState<Issue | null>(null),
    [palette, setPalette] = useState(false),
    [query, setQuery] = useState("");
  const [theme, setTheme] = useState(
    localStorage.getItem("aiflow.theme") ?? "System",
  );
  const [sidebar, setSidebar] = useState(false);
  const [repositorySearch, setRepositorySearch] = useState("");
  const syncedAccount = useRef<string | undefined>(undefined);
  const project = data.projects.find((p) => p.id === projectId);
  const issues =
    data.issues.find((p) => p.projectId === projectId)?.issues ?? [];
  const queue = data.queue.filter((q) => q.projectId === projectId);
  const executions = data.executions.filter((e) => e.projectId === projectId);
  const current = executions.find((e) =>
    [
      "RUNNING",
      "PREPARING",
      "VALIDATING",
      "WAITING_FOR_PROVIDER",
      "WAITING_FOR_QUOTA",
      "RETRY_WAIT",
    ].includes(e.state),
  );
  const [selectedExecutionId, setSelectedExecutionId] = useState("");
  const execution =
    executions.find((e) => e.id === selectedExecutionId) ??
    current ??
    executions[0];
  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let stopped = false;
    void connectEvents().then((fn) => {
      if (stopped) fn();
      else cleanup = fn;
    });
    return () => {
      stopped = true;
      cleanup?.();
    };
  }, []);
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      document.documentElement.dataset.theme =
        theme === "System"
          ? media.matches
            ? "dark"
            : "light"
          : theme.toLowerCase();
    };
    apply();
    localStorage.setItem("aiflow.theme", theme);
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette((v) => !v);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);
  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => useApp.setState({ notice: null }), 3500);
    return () => clearTimeout(timer);
  }, [notice]);
  useEffect(() => {
    if (!desktop || !data.githubAccount) {
      syncedAccount.current = undefined;
      return;
    }
    if (syncedAccount.current === data.githubAccount) return;
    syncedAccount.current = data.githubAccount;
    void useApp.getState().command("sync_repositories");
  }, [data.githubAccount]);
  useEffect(() => {
    setSelected(null);
  }, [projectId]);
  useEffect(() => {
    if (!desktop || !projectId || !data.githubAccount) return;
    void useApp.getState().command("sync_issues", { projectId });
    const timer = setInterval(() => {
      void useApp.getState().command("sync_issues", { projectId });
    }, 120000);
    return () => clearInterval(timer);
  }, [projectId, data.githubAccount]);
  const closeIssue = useCallback(() => setSelected(null), []),
    closeProject = useCallback(() => {
      setProjectSheet(false);
      setEditProject(undefined);
    }, []),
    closePalette = useCallback(() => setPalette(false), []);
  const go = (name: string) => {
    setPage(name);
    setSidebar(false);
  };
  const control = (action: string) =>
    void useApp.getState().command("queue_control", { projectId, action });
  const addProject = () => go("Projects");
  const openExecution = (e?: Execution) => {
    if (e) setSelectedExecutionId(e.id);
    go("Executions");
    setSelected(null);
  };
  return (
    <div className={`app-window ${sidebar ? "sidebar-collapsed" : ""}`}>
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">
            <Zap size={21} strokeWidth={2.4} />
          </div>
          <span>{branding.name}</span>
          <small>DESKTOP</small>
        </div>
        <button className="workspace-switch" onClick={() => go("Projects")}>
          <div className="project-avatar">
            {project?.name.slice(0, 1) ?? "A"}
          </div>
          <div>
            <strong>{project?.name ?? "Your workspace"}</strong>
            <small>
              {project ? project.repository : "Let’s build something."}
            </small>
          </div>
          <ChevronDown size={14} />
        </button>
        <button className="sidebar-search" onClick={() => setPalette(true)}>
          <Search size={14} />
          <span>Jump to…</span>
          <kbd>⌘ K</kbd>
        </button>
        <nav aria-label="Main navigation">
          {navigation.map((group) => (
            <div className="nav-group" key={group.group}>
              {group.group && <div className="nav-label">{group.group}</div>}
              {group.items.map(({ name, icon: Icon }) => (
                <button
                  key={name}
                  className={page === name ? "nav-item selected" : "nav-item"}
                  onClick={() => go(name)}
                >
                  <Icon size={17} />
                  <span>{name}</span>
                  {name === "Issues" && issues.length > 0 && (
                    <small>{issues.length}</small>
                  )}
                  {name === "Queue" &&
                    queue.filter((q) => q.state === "QUEUED").length > 0 && (
                      <small>
                        {queue.filter((q) => q.state === "QUEUED").length}
                      </small>
                    )}
                  {name === "Providers" && (
                    <span className="nav-provider-dot" />
                  )}
                </button>
              ))}
            </div>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="local-label">
            <ShieldCheck size={13} /> Your workspace. Your control.
          </div>
          <button className="profile" onClick={() => go("Accounts")}>
            <div className="profile-avatar">
              {data.githubAccount?.[0]?.toUpperCase() ?? (
                <UserRound size={16} />
              )}
            </div>
            <div>
              <strong>{data.githubAccount ?? "Local workspace"}</strong>
              <small>
                {data.githubAccount
                  ? "GitHub connected"
                  : "Connect your accounts"}
              </small>
            </div>
            <Settings2 size={15} />
          </button>
        </div>
      </aside>
      <div className="main-shell">
        <header className="toolbar">
          <button
            className="icon-button"
            aria-label="Toggle sidebar"
            onClick={() => setSidebar((v) => !v)}
          >
            <PanelLeftClose size={17} />
          </button>
          <span className="breadcrumb">
            Workspace <ChevronRight size={12} /> <strong>{page}</strong>
          </span>
          <div className="toolbar-right">
            {!desktop && (
              <span className="preview-pill">
                Browser preview · backend offline
              </span>
            )}
            {project && (
              <span className="branch-pill">
                <GitBranch size={12} />
                {project.branch}
              </span>
            )}
            <button
              className="icon-button"
              aria-label="Search commands"
              onClick={() => setPalette(true)}
            >
              <Search size={17} />
            </button>
          </div>
        </header>
        {error && (
          <div className="error-banner" role="alert">
            <span>{error}</span>
            <button
              onClick={() => useApp.setState({ error: null })}
              aria-label="Dismiss error"
            >
              <X size={16} />
            </button>
          </div>
        )}
        <main>
          <AnimatePresence mode="wait">
            <motion.div
              key={page}
              className="page"
              initial={{ opacity: 0, y: 7 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -5 }}
              transition={{ duration: 0.18 }}
            >
              <div className="page-top">
                <div>
                  <div className="eyebrow">
                    {page === "Home"
                      ? "YOUR DEVELOPMENT, ORCHESTRATED"
                      : (project?.name.toUpperCase() ?? "AIFLOW WORKSPACE")}
                  </div>
                  <h1>
                    {page === "Home"
                      ? "Good work, in flow."
                      : page === "Providers"
                        ? "AI Providers"
                        : page === "Executions"
                          ? "Execution Center"
                          : page}
                  </h1>
                  <p>
                    {
                      (
                        {
                          Home: "A clear view of what’s done, what’s moving, and what comes next.",
                          Issues:
                            "From an idea to a validated commit. Every step, in view.",
                          Queue:
                            "One task at a time. The right provider for each.",
                          Executions: "Follow the work as it happens.",
                          Providers:
                            "Different strengths. One seamless workflow.",
                          Projects: "A focused space for every repository.",
                          Accounts:
                            "Official connections. Credentials stay with their owner.",
                          Usage:
                            "Independent provider limits. No invented estimates.",
                          Logs: "The complete record, from first output to final validation.",
                          Settings: "Make room for the way you work.",
                        } as Record<string, string>
                      )[page]
                    }
                  </p>
                </div>
                <div className="page-actions">
                  {["Home", "Projects"].includes(page) && (
                    <AiButton
                      disabled={loading || !data.githubAccount}
                      onClick={() =>
                        void useApp.getState().command("sync_repositories")
                      }
                    >
                      <RefreshCw size={15} />
                      Sync repositories
                    </AiButton>
                  )}
                  {["Projects", "Issues"].includes(page) && project && (
                    <AiButton
                      onClick={() => {
                        setEditProject(project);
                        setProjectSheet(true);
                      }}
                    >
                      <Settings2 size={14} />
                      {project.path ? "Local settings" : "Link local folder"}
                    </AiButton>
                  )}
                  {["Issues", "Queue"].includes(page) && project && (
                    <>
                      <AiButton
                        disabled={loading}
                        onClick={() =>
                          void useApp
                            .getState()
                            .command("sync_issues", { projectId })
                        }
                      >
                        <RefreshCw size={14} />
                        Sync issues
                      </AiButton>
                      <AiButton
                        variant="primary"
                        disabled={loading || !queue.length || !project.path}
                        onClick={() => control("start")}
                      >
                        <Play size={13} />
                        Start queue
                      </AiButton>
                    </>
                  )}
                  {page === "Providers" && (
                    <AiButton
                      disabled={loading}
                      onClick={() =>
                        void useApp.getState().command("scan_providers")
                      }
                    >
                      <RefreshCw size={14} />
                      Scan providers
                    </AiButton>
                  )}
                </div>
              </div>
              {page === "Home" && (
                <HomePage
                  project={project}
                  issues={issues}
                  queue={queue}
                  current={current}
                  go={go}
                  addProject={addProject}
                  openIssue={setSelected}
                />
              )}
              {page === "Projects" && (
                <>
                  {!data.githubAccount ? (
                    <Empty title="Your GitHub repositories">
                      <p>
                        Connect GitHub to list the repositories available to
                        your account.
                      </p>
                      <AiButton
                        variant="primary"
                        onClick={() => go("Accounts")}
                      >
                        Connect GitHub
                      </AiButton>
                    </Empty>
                  ) : (
                    <>
                      <div className="issues-subtools">
                        <label className="search-field">
                          <Search size={14} />
                          <input
                            aria-label="Search repositories"
                            placeholder="Search GitHub repositories…"
                            value={repositorySearch}
                            onChange={(e) =>
                              setRepositorySearch(e.target.value)
                            }
                          />
                        </label>
                        <span>
                          {data.projects.length} repositories ·{" "}
                          {data.githubAccount}
                        </span>
                      </div>
                      {loading && !data.projects.length && (
                        <p role="status">Loading GitHub repositories…</p>
                      )}
                      {!loading && !data.projects.length && (
                        <Empty title="No repositories to display">
                          Sync repositories to refresh the repositories
                          accessible to this GitHub account.
                        </Empty>
                      )}
                      <div className="project-grid">
                        {data.projects
                          .filter((p) =>
                            p.repository
                              .toLowerCase()
                              .includes(repositorySearch.toLowerCase()),
                          )
                          .map((p) => (
                            <button
                              key={p.id}
                              className={`project-card glass ${p.id === projectId ? "chosen" : ""}`}
                              onClick={() => {
                                useApp.getState().selectProject(p.id);
                                go("Issues");
                              }}
                            >
                              <div className="project-card-icon">
                                <FolderGit2 size={23} />
                              </div>
                              <h2>{p.name}</h2>
                              <p>{p.repository}</p>
                              <span>
                                <GitBranch size={13} />
                                {p.branch}
                              </span>
                              <footer>
                                <span>
                                  {p.path
                                    ? "Local folder linked"
                                    : "Browse issues · local folder optional"}
                                </span>
                                <ArrowUpRight size={17} />
                              </footer>
                            </button>
                          ))}
                      </div>
                    </>
                  )}
                </>
              )}
              {page === "Issues" && (
                <IssuesPage
                  key={projectId}
                  issues={issues}
                  queue={queue}
                  openIssue={setSelected}
                  hasProject={!!project}
                  addProject={addProject}
                />
              )}
              {page === "Queue" && (
                <QueuePage
                  project={project}
                  issues={issues}
                  queue={queue}
                  openIssue={setSelected}
                  control={control}
                />
              )}
              {page === "Executions" && (
                <ExecutionPage
                  execution={execution}
                  executions={executions}
                  queue={queue}
                  issues={issues}
                  onSelect={setSelectedExecutionId}
                  control={control}
                />
              )}
              {["Providers", "Usage"].includes(page) && (
                <ProvidersPage usageOnly={page === "Usage"} />
              )}
              {page === "Accounts" && <AccountsPage />}
              {page === "Logs" &&
                (execution ? (
                  <>
                    <div className="execution-selector">
                      <select
                        aria-label="Execution log"
                        value={execution.id}
                        onChange={(e) => setSelectedExecutionId(e.target.value)}
                      >
                        {executions.map((e) => (
                          <option key={e.id} value={e.id}>
                            {e.startedAt} · {e.providerId} · {e.state} · attempt{" "}
                            {e.attempt}
                          </option>
                        ))}
                      </select>
                    </div>
                    <AiExecutionConsole
                      key={execution.id}
                      executionId={execution.id}
                    />
                  </>
                ) : (
                  <Empty title="Every step leaves a trace">
                    Agent output, validation and Git events appear here after
                    your first execution.
                  </Empty>
                ))}
              {page === "Settings" && (
                <div className="settings-content">
                  <section className="glass settings-section">
                    <div>
                      <h2>Appearance</h2>
                      <p>Comfortable in every light.</p>
                    </div>
                    <div className="theme-options">
                      {[
                        { name: "Light", icon: Sun },
                        { name: "Dark", icon: Moon },
                        { name: "System", icon: Monitor },
                      ].map(({ name, icon: Icon }) => (
                        <button
                          key={name}
                          className={theme === name ? "chosen" : ""}
                          onClick={() => setTheme(name)}
                        >
                          <Icon size={24} />
                          <span>{name}</span>
                          {theme === name && <Check size={13} />}
                        </button>
                      ))}
                    </div>
                  </section>
                  <section className="glass settings-section">
                    <div>
                      <h2>Project policy</h2>
                      <p>
                        Providers, approvals, independent validation and GitHub
                        completion.
                      </p>
                    </div>
                    {project ? (
                      <AiButton
                        onClick={() => {
                          setEditProject(project);
                          setProjectSheet(true);
                        }}
                      >
                        Configure {project.name}
                        <ArrowRight size={14} />
                      </AiButton>
                    ) : (
                      <AiButton onClick={addProject}>
                        Browse repositories
                      </AiButton>
                    )}
                  </section>
                  <section className="glass settings-section">
                    <div>
                      <h2>Safety by default</h2>
                      <p>
                        Local GitHub status · No automatic failover · Production
                        approval required
                        <br />
                        Interrupted executions require review. Quota reset times
                        come from provider evidence.
                      </p>
                    </div>
                    <ShieldCheck size={24} className="sage" />
                  </section>
                  <section className="settings-about">
                    <div className="brand-mark">
                      <Zap size={24} />
                    </div>
                    <h3>{branding.name}</h3>
                    <p>0.1.0 · Development preview</p>
                    <small>
                      Built around your work. Open to every provider.
                    </small>
                  </section>
                </div>
              )}
            </motion.div>
          </AnimatePresence>
        </main>
        {current && page !== "Executions" && (
          <MiniPlayer
            execution={current}
            issue={issues.find(
              (i) =>
                i.number ===
                queue.find((q) => q.id === current.itemId)?.issueNumber,
            )}
            onOpen={() => openExecution(current)}
          />
        )}
        <footer className="statusbar">
          <span>
            <span className={`connection-dot ${desktop ? "on" : ""}`} />
            {desktop ? "Local backend connected" : "Desktop backend required"}
          </span>
          <span>
            {loading ? "Working…" : "SQLite local persistence"}
            <span className="status-separator">·</span>AiFlow 0.1.0
          </span>
        </footer>
      </div>
      <IssueDetail
        issue={projectSheet ? null : selected}
        project={project}
        item={queue.find((q) => q.issueNumber === selected?.number)}
        execution={executions.find(
          (e) =>
            e.itemId ===
            queue.find((q) => q.issueNumber === selected?.number)?.id,
        )}
        onClose={closeIssue}
        onLinkFolder={() => {
          setEditProject(project);
          setProjectSheet(true);
        }}
        onExecution={openExecution}
      />
      <AiSheet
        open={projectSheet}
        title={editProject ? "Project settings" : "New project"}
        onClose={closeProject}
      >
        <ProjectForm project={editProject} onClose={closeProject} />
      </AiSheet>
      <AiSheet open={palette} title="Find your flow" onClose={closePalette}>
        <div className="palette-search">
          <Search size={20} />
          <input
            autoFocus
            placeholder="Search issues or commands…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <kbd>ESC</kbd>
        </div>
        <div className="palette-results">
          {navigation
            .flatMap((g) => g.items)
            .filter((i) => i.name.toLowerCase().includes(query.toLowerCase()))
            .map(({ name, icon: Icon }) => (
              <button
                key={name}
                onClick={() => {
                  go(name);
                  setPalette(false);
                }}
              >
                <Icon size={18} />
                <span>Go to {name}</span>
                <ArrowRight size={13} />
              </button>
            ))}
          {[
            { name: "Start queue", action: "start" },
            { name: "Pause queue", action: "pause" },
          ]
            .filter(
              (i) =>
                project && i.name.toLowerCase().includes(query.toLowerCase()),
            )
            .map((i) => (
              <button
                key={i.name}
                onClick={() => {
                  control(i.action);
                  setPalette(false);
                }}
              >
                <Play size={17} />
                <span>{i.name}</span>
              </button>
            ))}
          {issues
            .filter((i) =>
              `${i.number} ${i.title}`
                .toLowerCase()
                .includes(query.toLowerCase()),
            )
            .map((i) => (
              <button
                key={i.number}
                onClick={() => {
                  setPalette(false);
                  setSelected(i);
                }}
              >
                <CircleDot size={17} />
                <span>
                  #{i.number} {i.title}
                </span>
              </button>
            ))}
        </div>
      </AiSheet>
      <AnimatePresence>
        {notice && (
          <motion.div
            role="status"
            className="toast"
            initial={{ y: 30, opacity: 0 }}
            animate={{ y: 0, opacity: 1 }}
            exit={{ y: 20, opacity: 0 }}
          >
            <CircleCheck size={17} />
            {notice}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function HomePage({
  project,
  issues,
  queue,
  current,
  go,
  addProject,
  openIssue,
}: {
  project?: Project;
  issues: Issue[];
  queue: QueueItem[];
  current?: Execution;
  go: (p: string) => void;
  addProject: () => void;
  openIssue: (i: Issue) => void;
}) {
  const providers = useApp((s) => s.data.providers);
  const done = queue.filter((q) => q.state === "SUCCESS").length,
    running = queue.filter((q) => taskState(q) === "IN_PROGRESS").length,
    todo = queue.filter((q) => taskState(q) === "TODO").length,
    blocked = queue.filter((q) =>
      ["BLOCKED", "FAILED"].includes(taskState(q)),
    ).length;
  const next = queue.filter((q) => q.state === "QUEUED").slice(0, 3);
  const currentIssue = issues.find(
    (i) =>
      i.number === queue.find((q) => q.id === current?.itemId)?.issueNumber,
  );
  return (
    <>
      <div className="home-grid">
        <section className="project-overview glass">
          <div className="section-kicker">
            <span>ACTIVE PROJECT</span>
            <button onClick={() => go("Projects")}>
              All projects
              <ArrowUpRight size={13} />
            </button>
          </div>
          {project ? (
            <>
              <div className="active-project-title">
                <div className="project-card-icon">
                  <FolderGit2 size={24} />
                </div>
                <div>
                  <h2>{project.name}</h2>
                  <p>{project.repository}</p>
                </div>
                <span className="local-tag">LOCAL WORKTREE</span>
              </div>
              <div className="progress-heading">
                <span>Validated queue progress</span>
                <strong>
                  {done}
                  <small> / {queue.length} complete</small>
                </strong>
              </div>
              <div className="progress-track">
                <motion.div
                  initial={{ width: 0 }}
                  animate={{
                    width: `${queue.length ? (done / queue.length) * 100 : 0}%`,
                  }}
                />
              </div>
              <div className="stats">
                <div>
                  <span className="stat-dot done-dot" />
                  <strong>{done}</strong>
                  <small>Done</small>
                </div>
                <div>
                  <span className="stat-dot active-dot" />
                  <strong>{running}</strong>
                  <small>Running</small>
                </div>
                <div>
                  <span className="stat-dot" />
                  <strong>{todo}</strong>
                  <small>To do</small>
                </div>
                <div>
                  <span className="stat-dot blocked-dot" />
                  <strong>{blocked}</strong>
                  <small>Attention</small>
                </div>
              </div>
            </>
          ) : (
            <div className="start-project">
              <div className="project-card-icon">
                <FolderGit2 size={24} />
              </div>
              <h2>A home for your next idea.</h2>
              <p>
                Connect GitHub to load your repositories,
                <br />
                and turn your issues into a clear plan.
              </p>
              <AiButton variant="primary" onClick={addProject}>
                <Plus size={15} />
                Browse GitHub repositories
              </AiButton>
            </div>
          )}
        </section>
        <section className="current-card glass">
          <div className="section-kicker">
            <span>CURRENT EXECUTION</span>
            <Activity size={15} />
          </div>
          {current ? (
            <>
              <AiStatusBadge state={current.state} />
              <small className="issue-number">
                ISSUE #{currentIssue?.number}
              </small>
              <h2>{currentIssue?.title ?? "Execution in progress"}</h2>
              <AiProviderBadge id={current.providerId} />
              <ExecutionTime execution={current} />
              <AiButton variant="primary" onClick={() => go("Executions")}>
                Open execution
                <ArrowUpRight size={14} />
              </AiButton>
            </>
          ) : (
            <div className="quiet-state">
              <div className="orbit-graphic">
                <div />
                <div />
                <Zap size={29} />
              </div>
              <h3>Ready when you are.</h3>
              <p>
                Your next execution starts with a task
                <br />
                and a provider you choose.
              </p>
              <AiButton variant="ghost" onClick={() => go("Queue")}>
                Explore your queue
                <ArrowRight size={14} />
              </AiButton>
            </div>
          )}
        </section>
      </div>
      <div className="section-heading">
        <div>
          <h2>Your AI team</h2>
          <p>Connected through their official clients.</p>
        </div>
        <AiButton variant="ghost" onClick={() => go("Providers")}>
          Manage providers
          <ArrowRight size={14} />
        </AiButton>
      </div>
      <div className="provider-grid">
        {Object.keys(providerNames)
          .filter((id) => id !== "auto")
          .map((id) => {
            const p = providers.find((p) => p.id === id);
            return (
              <button
                className="provider-card glass"
                key={id}
                onClick={() => go("Providers")}
              >
                <div className="provider-card-head">
                  <span className={`provider-logo ${id}`}>
                    {id === "codex" ? "◎" : id === "claude" ? "✳" : "✧"}
                  </span>
                  <ArrowUpRight size={15} />
                </div>
                <h3>{providerNames[id]}</h3>
                <span className="provider-description">
                  {id === "codex"
                    ? "OpenAI"
                    : id === "claude"
                      ? "Anthropic"
                      : "Google"}{" "}
                  · Official CLI
                </span>
                <footer>
                  <AiStatusBadge state={p?.status ?? "NOT_CHECKED"} />
                  <small>{p?.version ?? "Scan to detect"}</small>
                </footer>
              </button>
            );
          })}
      </div>
      <div className="bottom-grid">
        <section className="glass next-section">
          <div className="section-kicker">
            <span>UP NEXT</span>
            <button onClick={() => go("Queue")}>
              View queue
              <ArrowRight size={13} />
            </button>
          </div>
          {next.length ? (
            next.map((q, index) => {
              const i = issues.find((i) => i.number === q.issueNumber);
              return (
                <button
                  className="next-row"
                  key={q.id}
                  onClick={() => i && openIssue(i)}
                >
                  <span className="queue-position">{index + 1}</span>
                  <div>
                    <strong>
                      #{q.issueNumber} {i?.title}
                    </strong>
                    <AiProviderBadge id={q.providerId} />
                  </div>
                  <ChevronRight size={15} />
                </button>
              );
            })
          ) : (
            <div className="quiet-row">
              <Layers3 size={20} />
              <div>
                <strong>A little space for what’s next.</strong>
                <p>Add GitHub issues to build your queue.</p>
              </div>
              <button onClick={() => go("Issues")}>
                <ArrowUpRight size={17} />
              </button>
            </div>
          )}
        </section>
        <section className="safety-note">
          <ShieldCheck size={24} />
          <h3>Progress you can trust.</h3>
          <p>
            Every completed task passes independent validation. Your branch,
            commits and tests are checked before the next issue begins.
          </p>
          <span>
            Human control, built in
            <ArrowRight size={13} />
          </span>
        </section>
      </div>
    </>
  );
}
function ExecutionTime({ execution }: { execution: Execution }) {
  const now = useApp((s) => s.now);
  return (
    <div className="execution-time">
      <span>{execution.retryAt ? "Next automatic retry" : "Elapsed time"}</span>
      <strong>
        {execution.retryAt
          ? countdown(execution.retryAt, now)
          : countdown(
              new Date(
                now +
                  Math.max(
                    0,
                    (execution.finishedAt
                      ? new Date(execution.finishedAt).getTime()
                      : now) - new Date(execution.startedAt).getTime(),
                  ),
              ).toISOString(),
              now,
            )}
      </strong>
    </div>
  );
}
function MiniPlayer({
  execution,
  issue,
  onOpen,
}: {
  execution: Execution;
  issue?: Issue;
  onOpen: () => void;
}) {
  return (
    <button className="mini-player" onClick={onOpen}>
      <span className="mini-icon">
        <Activity size={17} />
      </span>
      <div>
        <strong>
          {issue ? `#${issue.number} ${issue.title}` : "Current execution"}
        </strong>
        <AiProviderBadge id={execution.providerId} />
      </div>
      <AiStatusBadge state={execution.state} />
      <ExecutionTime execution={execution} />
      <ArrowUpRight size={17} />
    </button>
  );
}
function IssuesPage({
  issues,
  queue,
  openIssue,
  hasProject,
  addProject,
}: {
  issues: Issue[];
  queue: QueueItem[];
  openIssue: (i: Issue) => void;
  hasProject: boolean;
  addProject: () => void;
}) {
  const [view, setView] = useState<"Board" | "List">("List"),
    [filter, setFilter] = useState("All"),
    [search, setSearch] = useState(""),
    [provider, setProvider] = useState("all"),
    [sort, setSort] = useState("number"),
    [remoteState, setRemoteState] = useState("all");
  const item = (i: Issue) => queue.find((q) => q.issueNumber === i.number);
  const visible = useMemo(
    () =>
      issues
        .filter((i) => remoteState === "all" || i.state === remoteState)
        .filter((i) =>
          `${i.number} ${i.title} ${i.labels.join(" ")} ${i.assignees.join(" ")} ${i.milestone ?? ""}`
            .toLowerCase()
            .includes(search.toLowerCase()),
        )
        .filter(
          (i) =>
            filter === "All" ||
            taskState(queue.find((q) => q.issueNumber === i.number)) === filter,
        )
        .filter(
          (i) =>
            provider === "all" ||
            queue.find((q) => q.issueNumber === i.number)?.providerId ===
              provider,
        )
        .sort((a, b) =>
          sort === "title"
            ? a.title.localeCompare(b.title)
            : sort === "updated"
              ? b.updatedAt.localeCompare(a.updatedAt)
              : a.number - b.number,
        ),
    [issues, queue, search, filter, provider, sort, remoteState],
  );
  if (!hasProject)
    return (
      <Empty
        title="Choose a GitHub repository"
        action={
          <AiButton variant="primary" onClick={addProject}>
            Browse repositories
          </AiButton>
        }
      >
        Select a repository to see your GitHub issues here.
      </Empty>
    );
  return (
    <>
      <div className="issues-tools">
        <div className="filter-tabs">
          {[
            "All",
            "TODO",
            "IN_PROGRESS",
            "WAITING",
            "BLOCKED",
            "FAILED",
            "DONE",
          ].map((f) => (
            <button
              key={f}
              className={filter === f ? "selected-text" : ""}
              onClick={() => setFilter(f)}
            >
              {f === "All"
                ? "All issues"
                : f === "IN_PROGRESS"
                  ? "Running"
                  : f.replaceAll("_", " ").toLowerCase()}
            </button>
          ))}
        </div>
        <AiSegmentedControl
          value={view}
          options={["Board", "List"]}
          onChange={setView}
        />
      </div>
      <div className="issues-subtools">
        <label className="search-field">
          <Search size={14} />
          <input
            aria-label="Search issues"
            placeholder="Search title, label, assignee, milestone…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </label>
        <select
          aria-label="GitHub issue state"
          value={remoteState}
          onChange={(e) => setRemoteState(e.target.value)}
        >
          <option value="all">GitHub: open and closed</option>
          <option value="OPEN">GitHub: open</option>
          <option value="CLOSED">GitHub: closed</option>
        </select>
        <select
          aria-label="Filter provider"
          value={provider}
          onChange={(e) => setProvider(e.target.value)}
        >
          <option value="all">All providers</option>
          {Object.entries(providerNames).map(([id, name]) => (
            <option key={id} value={id}>
              {name}
            </option>
          ))}
        </select>
        <select
          aria-label="Sort issues"
          value={sort}
          onChange={(e) => setSort(e.target.value)}
        >
          <option value="number">Issue number</option>
          <option value="title">Title</option>
          <option value="updated">Recently updated</option>
        </select>
        <span>{visible.length} issues</span>
      </div>
      {!issues.length ? (
        <Empty title="Your issues, right here">
          Issues are loaded from the selected GitHub repository. Use Sync issues
          to refresh its open and closed issues.
        </Empty>
      ) : view === "Board" ? (
        <div className="board">
          {[
            { name: "To do", states: ["TODO", "PAUSED"], style: "todo" },
            {
              name: "In progress",
              states: ["IN_PROGRESS", "WAITING", "BLOCKED", "FAILED"],
              style: "progress",
            },
            { name: "Done", states: ["DONE"], style: "done" },
          ].map((column) => {
            const list = visible.filter((i) =>
              column.states.includes(taskState(item(i))),
            );
            return (
              <section
                className={`board-column ${column.style}`}
                key={column.name}
              >
                <div className="column-title">
                  <span className="stat-dot" />
                  <h3>{column.name}</h3>
                  <small>{list.length}</small>
                </div>
                <div className="board-cards">
                  {list.map((i) => (
                    <IssueCard
                      key={i.number}
                      issue={i}
                      item={item(i)}
                      onClick={() => openIssue(i)}
                    />
                  ))}
                  {!list.length && (
                    <div className="column-empty">No issues here</div>
                  )}
                </div>
              </section>
            );
          })}
        </div>
      ) : (
        <div className="issue-table glass">
          <div className="issue-table-header">
            <span>STATUS</span>
            <span>ISSUE / TITLE</span>
            <span>PROVIDER</span>
            <span>GITHUB</span>
          </div>
          {visible.map((i) => (
            <button
              className="issue-table-row"
              key={i.number}
              onClick={() => openIssue(i)}
            >
              <AiStatusBadge state={taskState(item(i))} />
              <span>
                <small>#{i.number}</small>
                {i.title}
              </span>
              <AiProviderBadge id={item(i)?.providerId ?? "auto"} />
              <span className="remote-state">{i.state}</span>
            </button>
          ))}
        </div>
      )}
    </>
  );
}
function IssueCard({
  issue,
  item,
  onClick,
}: {
  issue: Issue;
  item?: QueueItem;
  onClick: () => void;
}) {
  const state = taskState(item);
  return (
    <motion.button
      layout
      transition={{ type: "spring", stiffness: 300, damping: 30 }}
      className={`issue-card glass ${state.toLowerCase()}`}
      onClick={onClick}
      onContextMenu={(e) => {
        e.preventDefault();
        onClick();
      }}
    >
      <div className="issue-card-meta">
        <span>#{issue.number}</span>
        {issue.state === "CLOSED" && state !== "DONE" ? (
          <span className="remote-state">GitHub closed · review</span>
        ) : (
          <CircleDot size={13} />
        )}
      </div>
      <h3>{issue.title}</h3>
      <div className="labels">
        {issue.labels.slice(0, 3).map((l) => (
          <span key={l}>{l}</span>
        ))}
      </div>
      <footer>
        <AiProviderBadge id={item?.providerId ?? "auto"} />
        {state === "TODO" ? (
          <span className="remote-state">
            {item ? `Position ${item.position}` : "Not queued"}
          </span>
        ) : (
          <AiStatusBadge state={state} />
        )}
      </footer>
    </motion.button>
  );
}
function QueuePage({
  project,
  issues,
  queue,
  openIssue,
  control,
}: {
  project?: Project;
  issues: Issue[];
  queue: QueueItem[];
  openIssue: (i: Issue) => void;
  control: (action: string) => void;
}) {
  const running = useApp((s) =>
    s.data.runningProjects.includes(project?.id ?? ""),
  );
  const [drag, setDrag] = useState("");
  const command = useApp((s) => s.command);
  return (
    <>
      <div className="queue-summary glass">
        <div className="queue-summary-icon">
          <Layers3 size={24} />
        </div>
        <div>
          <h2>{queue.length} tasks in your flow</h2>
          <p>
            {running ? "Queue is active" : "Queue is paused"} · Sequential per
            worktree · Failover asks first
          </p>
        </div>
        <AiButton onClick={() => control("pause")}>
          <Pause size={14} />
          Pause queue
        </AiButton>
        <AiButton onClick={() => control("stop-after")}>
          <StopCircle size={14} />
          Stop after current
        </AiButton>
      </div>
      {!queue.length ? (
        <Empty title="Make room for meaningful work">
          Open Issues and add the tasks you want your agents to tackle.
        </Empty>
      ) : (
        <div className="queue-list">
          {queue.map((q, index) => {
            const i = issues.find((i) => i.number === q.issueNumber);
            return (
              <div
                key={q.id}
                className={`queue-row glass ${drag === q.id ? "dragging" : ""}`}
                draggable={!running}
                onDragStart={() => setDrag(q.id)}
                onDragEnd={() => setDrag("")}
                onDragOver={(e) => e.preventDefault()}
                onDrop={() => {
                  if (!drag || drag === q.id) return;
                  const ids = queue
                    .map((i) => i.id)
                    .filter((id) => id !== drag);
                  ids.splice(index, 0, drag);
                  void command("reorder_queue", {
                    projectId: project?.id,
                    ids,
                  });
                  setDrag("");
                }}
              >
                <GripVertical size={15} className="muted" />
                <span className="queue-position">{index + 1}</span>
                <button
                  className="queue-title"
                  onClick={() => i && openIssue(i)}
                >
                  <small>#{q.issueNumber}</small>
                  <strong>{i?.title ?? "Cached issue unavailable"}</strong>
                  <span>
                    {q.approved
                      ? "Manual gate approved"
                      : i?.labels.some((l) =>
                            project?.policy.manualApprovalLabels.includes(l),
                          )
                        ? "Manual approval required"
                        : "Project policy applies"}
                  </span>
                </button>
                <ProviderPicker
                  value={q.providerId}
                  disabled={running}
                  onChange={(providerId) =>
                    void command("item_control", {
                      itemId: q.id,
                      action: "provider",
                      providerId,
                    })
                  }
                />
                <AiStatusBadge state={q.state} />
                <button
                  className="icon-button"
                  aria-label={`Move issue ${q.issueNumber} up`}
                  disabled={running || index === 0}
                  onClick={() => {
                    const ids = queue.map((i) => i.id);
                    [ids[index - 1], ids[index]] = [ids[index], ids[index - 1]];
                    void command("reorder_queue", {
                      projectId: project?.id,
                      ids,
                    });
                  }}
                >
                  ↑
                </button>
                <button
                  className="icon-button"
                  aria-label={`Review issue ${q.issueNumber}`}
                  onClick={() => i && openIssue(i)}
                >
                  <ChevronRight size={17} />
                </button>
              </div>
            );
          })}
        </div>
      )}
    </>
  );
}
function ExecutionPage({
  execution,
  executions,
  issues,
  queue,
  onSelect,
  control,
}: {
  execution?: Execution;
  executions: Execution[];
  issues: Issue[];
  queue: QueueItem[];
  onSelect: (id: string) => void;
  control: (action: string) => void;
}) {
  const events = useApp((s) => s.events[execution?.id ?? ""]) ?? [];
  const [cancel, setCancel] = useState(false);
  const close = useCallback(() => setCancel(false), []);
  if (!execution)
    return (
      <Empty title="The work will unfold here">
        Start your queue to follow live output, independent tests, Git evidence
        and the full timeline.
      </Empty>
    );
  const i = issues.find(
    (i) =>
      i.number === queue.find((q) => q.id === execution.itemId)?.issueNumber,
  );
  return (
    <>
      <div className="execution-header glass">
        <div>
          <div className="execution-title-line">
            <span className="issue-number">ISSUE #{i?.number}</span>
            <AiStatusBadge state={execution.state} />
            <span className="muted">Attempt {execution.attempt}</span>
          </div>
          <h2>{i?.title ?? "Execution"}</h2>
          <div className="execution-metadata">
            <AiProviderBadge id={execution.providerId} />
            <span>
              <GitBranch size={12} />
              {execution.beforeGit?.branch ?? "Preparing worktree"}
            </span>
            {execution.afterGit?.head && (
              <code>{execution.afterGit.head.slice(0, 8)}</code>
            )}
          </div>
        </div>
        <ExecutionTime execution={execution} />
      </div>
      {execution.reason && (
        <div className="execution-reason">
          <ShieldCheck size={17} />
          <span>{execution.reason}</span>
        </div>
      )}
      <div className="execution-controls">
        <AiButton onClick={() => control("pause")}>
          <Pause size={14} />
          Pause queue
        </AiButton>
        <AiButton onClick={() => control("stop-after")}>
          Stop after current
        </AiButton>
        <AiButton variant="danger" onClick={() => setCancel(true)}>
          Cancel execution
        </AiButton>
        <select
          aria-label="Execution history"
          value={execution.id}
          onChange={(e) => onSelect(e.target.value)}
        >
          {executions.map((e) => (
            <option key={e.id} value={e.id}>
              {e.startedAt.slice(0, 16)} · {e.providerId} · {e.state}
            </option>
          ))}
        </select>
      </div>
      <div className="execution-layout">
        <aside className="execution-timeline glass">
          <h3>Execution timeline</h3>
          <AiTimeline events={events} />
          {!events.length && <p className="muted">Loading persisted events…</p>}
        </aside>
        <AiExecutionConsole key={execution.id} executionId={execution.id} />
      </div>
      <AiSheet open={cancel} title="Cancel this execution?" onClose={close}>
        <div className="form-content">
          <p>
            The provider process tree will be stopped. Changes already made in
            the worktree will remain. Review them before retrying.
          </p>
          <AiButton
            variant="danger"
            onClick={() => {
              control("cancel");
              setCancel(false);
            }}
          >
            Stop provider and preserve work
          </AiButton>
          <AiButton onClick={close}>Keep working</AiButton>
        </div>
      </AiSheet>
    </>
  );
}

function ProvidersPage({ usageOnly }: { usageOnly: boolean }) {
  const providers = useApp((s) => s.data.providers),
    usage = useApp((s) => s.data.usage),
    now = useApp((s) => s.now);
  return (
    <>
      <div className="providers-intro glass">
        <div className="intro-symbol">
          <Cpu size={26} />
        </div>
        <div>
          <h2>One workspace. Your choice of intelligence.</h2>
          <p>
            AiFlow orchestrates your official clients. Each provider keeps its
            own account, capabilities and limits.
          </p>
        </div>
        <ShieldCheck size={23} />
      </div>
      <div className="provider-detail-grid">
        {Object.keys(providerNames)
          .filter((id) => id !== "auto")
          .map((id) => {
            const p = providers.find((p) => p.id === id),
              u = usage.find((u) => u.providerId === id);
            return (
              <section key={id} className="provider-detail glass">
                <div className="provider-detail-top">
                  <span className={`provider-logo ${id}`}>
                    {id === "codex" ? "◎" : id === "claude" ? "✳" : "✧"}
                  </span>
                  <AiStatusBadge
                    state={u ? "LIMITED" : (p?.status ?? "NOT_CHECKED")}
                  />
                </div>
                <h2>{providerNames[id]}</h2>
                <p className="muted">Official CLI · Local process</p>
                <dl>
                  <div>
                    <dt>Authentication</dt>
                    <dd>{p?.authentication ?? "Not checked"}</dd>
                  </div>
                  <div>
                    <dt>Version</dt>
                    <dd>{p?.version ?? "Unavailable"}</dd>
                  </div>
                  <div>
                    <dt>Usage</dt>
                    <dd>
                      {u
                        ? "Usage limit reached"
                        : "Usage information unavailable"}
                    </dd>
                  </div>
                  <div>
                    <dt>Reset time</dt>
                    <dd>
                      {u?.resetAt
                        ? new Date(u.resetAt).toLocaleString()
                        : "Unknown"}
                    </dd>
                  </div>
                  {u?.resetAt && (
                    <div>
                      <dt>Available in</dt>
                      <dd className="countdown">{countdown(u.resetAt, now)}</dd>
                    </div>
                  )}
                  <div>
                    <dt>Last check</dt>
                    <dd>
                      {p
                        ? new Date(p.checkedAt).toLocaleTimeString()
                        : "Not yet checked"}
                    </dd>
                  </div>
                </dl>
                {!usageOnly && (
                  <>
                    <div className="capabilities">
                      {[
                        {
                          key: "supportsStreaming" as const,
                          label: "Streaming",
                        },
                        { key: "supportsResume" as const, label: "Resume" },
                        {
                          key: "supportsNonInteractive" as const,
                          label: "Headless",
                        },
                      ].map((c) => (
                        <span
                          key={c.key}
                          className={p?.capabilities[c.key] ? "supported" : ""}
                        >
                          {p?.capabilities[c.key] ? (
                            <Check size={12} />
                          ) : (
                            <X size={12} />
                          )}{" "}
                          {c.label}
                        </span>
                      ))}
                    </div>
                    <div className="provider-diagnostics">
                      {p?.diagnostics ??
                        "Scan to discover the official CLI and its supported features."}
                    </div>
                    <div className="provider-detail-actions">
                      <AiButton
                        onClick={() =>
                          external(
                            p?.installationUrl ??
                              (
                                {
                                  codex:
                                    "https://developers.openai.com/codex/cli",
                                  claude:
                                    "https://code.claude.com/docs/en/setup",
                                  gemini:
                                    "https://geminicli.com/docs/get-started/installation/",
                                } as Record<string, string>
                              )[id],
                          )
                        }
                      >
                        {p?.status === "NOT_INSTALLED"
                          ? "Installation guide"
                          : "Official documentation"}
                        <ArrowUpRight size={13} />
                      </AiButton>
                    </div>
                  </>
                )}
              </section>
            );
          })}
      </div>
      <div className="note-line">
        <ShieldCheck size={15} />
        Providers are never switched automatically mid-task. Reset timestamps
        are only shown when reported by the provider.
      </div>
    </>
  );
}
function AccountsPage() {
  const command = useApp((s) => s.command),
    data = useApp((s) => s.data),
    loading = useApp((s) => s.loading);
  const [connection, setConnection] = useState<string | null>(null);
  const closeConnection = useCallback(() => setConnection(null), []);
  return (
    <div className="accounts-content">
      <section className="glass account-section">
        <div className="account-icon">
          <UserRound size={24} />
        </div>
        <div>
          <h2>AiFlow identity</h2>
          <p>
            Local workspace. Cloud identity is optional and is not configured.
          </p>
          <small>
            Signing in to Google would not authorize Gemini automatically.
          </small>
        </div>
        <span className="local-tag">LOCAL</span>
      </section>
      <section className="glass account-section">
        <div className="account-icon">
          <Github size={24} />
        </div>
        <div className="account-main">
          <h2>GitHub</h2>
          <p>
            {data.githubAccount
              ? `Connected as ${data.githubAccount}`
              : "Sign in with your GitHub account to connect your repositories."}
          </p>
          <small>
            Connect through the official GitHub CLI. Existing CLI sessions are
            reused. No AiFlow OAuth App or developer setup is required.
          </small>
          {data.githubAccount ? (
            <AiButton onClick={() => void command("github_logout")}>
              Disconnect GitHub
            </AiButton>
          ) : (
            <AiButton onClick={() => setConnection("github")}>
              Connect GitHub
              <ArrowUpRight size={13} />
            </AiButton>
          )}
        </div>
        <AiStatusBadge
          state={data.githubAccount ? "CONNECTED" : "DISCONNECTED"}
        />
      </section>
      <h2 className="account-section-title">AI provider accounts</h2>
      {Object.keys(providerNames)
        .filter((id) => id !== "auto")
        .map((id) => {
          const p = data.providers.find((p) => p.id === id);
          return (
            <section className="glass account-section" key={id}>
              <span className={`provider-logo ${id}`}>
                {id === "codex" ? "◎" : id === "claude" ? "✳" : "✧"}
              </span>
              <div className="account-main">
                <h2>{providerNames[id]}</h2>
                <p>
                  {p?.version ?? "Official client not detected"} ·{" "}
                  {p?.authentication ?? "Not checked"}
                </p>
                <small>
                  Connect using the official client. AiFlow guides you through
                  each step.
                </small>
              </div>
              <div className="account-actions">
                <AiButton onClick={() => setConnection(id)}>Connect</AiButton>
                <AiButton
                  variant="ghost"
                  disabled={
                    loading ||
                    id === "gemini" ||
                    p?.authentication !== "CONNECTED"
                  }
                  title={
                    id === "gemini"
                      ? "Manage sign-out in the official Gemini CLI; a supported logout command is not advertised."
                      : "Sign out of the shared official CLI session"
                  }
                  onClick={() =>
                    void command("provider_auth", {
                      providerId: id,
                      action: "logout",
                    }).then(() => command("scan_providers"))
                  }
                >
                  Sign out
                </AiButton>
              </div>
            </section>
          );
        })}
      {connection && (
        <AccountConnectionModal
          accountId={connection}
          onClose={closeConnection}
        />
      )}
    </div>
  );
}

function ProjectForm({
  project,
  onClose,
}: {
  project?: Project;
  onClose: () => void;
}) {
  const [form, setForm] = useState<Project>(
    project ?? {
      id: "",
      name: "",
      repository: "",
      path: "",
      branch: "",
      policy: structuredClone(defaultPolicy),
    },
  );
  const [commands, setCommands] = useState(
    JSON.stringify(form.policy.testCommands, null, 2),
  );
  const [repos, setRepos] = useState<
    { fullName: string; defaultBranch: string }[]
  >([]);
  const command = useApp((s) => s.command),
    loading = useApp((s) => s.loading);
  async function choose() {
    if (!desktop) {
      useApp.setState({
        error: "Folder selection requires the desktop application.",
      });
      return;
    }
    const path = await open({
      directory: true,
      multiple: false,
      title: "Select the Git worktree root",
    });
    if (typeof path === "string") {
      const git = await command<GitSnapshot>("inspect_repository", { path });
      setForm((f) => ({ ...f, path, branch: git?.branch ?? f.branch }));
    }
  }
  async function save() {
    let testCommands: string[][];
    try {
      const value: unknown = JSON.parse(commands);
      if (
        !Array.isArray(value) ||
        !value.every(
          (a) =>
            Array.isArray(a) &&
            a.length &&
            a.every((s) => typeof s === "string"),
        )
      )
        throw new Error(
          'Expected an array of argument arrays, e.g. [["npm","test"]]',
        );
      testCommands = value;
    } catch (e) {
      useApp.setState({ error: String(e) });
      return;
    }
    const value = await command<Project>("save_project", {
      project: { ...form, policy: { ...form.policy, testCommands } },
    });
    if (value) {
      useApp.getState().selectProject(value.id);
      onClose();
    }
  }
  return (
    <div className="form-content">
      <p className="muted">
        Repository and issues come from GitHub. Link an existing local worktree
        only when you want to run tasks. AiFlow inspects Git before saving.
      </p>
      <label className="field">
        Project name
        <input
          readOnly={!!project}
          value={form.name}
          onChange={(e) => setForm({ ...form, name: e.target.value })}
          placeholder="My project"
        />
      </label>
      <label className="field">
        GitHub repository
        <div className="field-inline">
          <input
            readOnly={!!project}
            list="github-repos"
            value={form.repository}
            onChange={(e) => setForm({ ...form, repository: e.target.value })}
            placeholder="owner/repository"
          />
          <AiButton
            onClick={() =>
              void command<typeof repos>("github_repositories").then(
                (v) => v && setRepos(v),
              )
            }
          >
            Browse
          </AiButton>
        </div>
        <datalist id="github-repos">
          {repos.map((r) => (
            <option key={r.fullName} value={r.fullName} />
          ))}
        </datalist>
      </label>
      <label className="field">
        Worktree root
        <div className="field-inline">
          <input
            value={form.path}
            onChange={(e) => setForm({ ...form, path: e.target.value })}
            placeholder="Select an existing Git worktree"
          />
          <AiButton onClick={() => void choose()}>Choose…</AiButton>
        </div>
      </label>
      <label className="field">
        Expected branch
        <input
          value={form.branch}
          onChange={(e) => setForm({ ...form, branch: e.target.value })}
          placeholder="Detected from worktree"
        />
      </label>
      <label className="field">
        Default provider
        <ProviderPicker
          value={form.policy.defaultProvider}
          onChange={(id) =>
            setForm({
              ...form,
              policy: { ...form.policy, defaultProvider: id },
            })
          }
        />
      </label>
      <fieldset>
        <legend>Allowed providers</legend>
        {Object.keys(providerNames)
          .filter((id) => id !== "auto")
          .map((id) => (
            <label className="checkbox-row" key={id}>
              <input
                type="checkbox"
                checked={form.policy.allowedProviders.includes(id)}
                onChange={(e) =>
                  setForm({
                    ...form,
                    policy: {
                      ...form.policy,
                      allowedProviders: e.target.checked
                        ? [...form.policy.allowedProviders, id]
                        : form.policy.allowedProviders.filter((p) => p !== id),
                    },
                  })
                }
              />
              {providerNames[id]}
            </label>
          ))}
      </fieldset>
      <label className="field">
        Independent validation commands{" "}
        <small>
          JSON arrays of executable + arguments. These commands run from the
          trusted project policy.
        </small>
        <textarea
          rows={4}
          value={commands}
          onChange={(e) => setCommands(e.target.value)}
          placeholder={'[["npm", "test"], ["npm", "run", "build"]]'}
        />
      </label>
      <fieldset>
        <legend>Completion policy</legend>
        {[
          {
            key: "requireCleanTree" as const,
            label: "Require clean working tree",
          },
          { key: "requireCommit" as const, label: "Require a new commit" },
          { key: "requireTests" as const, label: "Require independent tests" },
          {
            key: "requireCI" as const,
            label: "Require GitHub CI (no automatic push)",
          },
          {
            key: "closeIssue" as const,
            label: "Close GitHub issue after validation",
          },
        ].map(({ key, label }) => (
          <label className="checkbox-row" key={key}>
            <input
              type="checkbox"
              checked={form.policy[key]}
              onChange={(e) =>
                setForm({
                  ...form,
                  policy: { ...form.policy, [key]: e.target.checked },
                })
              }
            />
            {label}
          </label>
        ))}
      </fieldset>
      <label className="field">
        Manual approval labels
        <input
          value={form.policy.manualApprovalLabels.join(", ")}
          onChange={(e) =>
            setForm({
              ...form,
              policy: {
                ...form.policy,
                manualApprovalLabels: e.target.value
                  .split(",")
                  .map((l) => l.trim())
                  .filter(Boolean),
              },
            })
          }
        />
      </label>
      <div className="form-note">
        <ShieldCheck size={17} />
        Failover is ask-only in this release. Production labels require explicit
        approval. No destructive Git operations.
      </div>
      <AiButton
        variant="primary"
        disabled={loading || !form.name || !form.repository || !form.path}
        onClick={() => void save()}
      >
        {loading ? "Validating worktree…" : "Save project"}
        <ArrowRight size={14} />
      </AiButton>
    </div>
  );
}
function IssueDetail({
  issue,
  project,
  item,
  execution,
  onClose,
  onExecution,
  onLinkFolder,
}: {
  issue: Issue | null;
  project?: Project;
  item?: QueueItem;
  execution?: Execution;
  onClose: () => void;
  onExecution: (e?: Execution) => void;
  onLinkFolder: () => void;
}) {
  const [tab, setTab] = useState<"Overview" | "Execution" | "Git" | "History">(
    "Overview",
  );
  const [provider, setProvider] = useState("auto"),
    [dryRun, setDryRun] = useState<unknown>();
  const command = useApp((s) => s.command),
    executions = useApp((s) => s.data.executions),
    loading = useApp((s) => s.loading);
  useEffect(() => {
    setProvider(item?.providerId ?? "auto");
    setTab("Overview");
    setDryRun(undefined);
  }, [issue?.number, item?.providerId]);
  const act = (action: string) =>
    void command("item_control", {
      itemId: item?.id,
      action,
      providerId: provider,
    });
  return (
    <AiSheet
      open={!!issue}
      title={issue ? `Issue #${issue.number}` : "Issue"}
      onClose={onClose}
      wide
    >
      {issue && (
        <div className="issue-detail">
          <div className="detail-heading">
            <AiStatusBadge state={taskState(item)} />
            <span className="remote-state">GitHub {issue.state}</span>
            <h1>{issue.title}</h1>
            <div className="detail-provider">
              <AiProviderBadge id={item?.providerId ?? provider} />
              <button onClick={() => external(issue.url)}>
                Open on GitHub
                <ArrowUpRight size={14} />
              </button>
            </div>
          </div>
          <div className="detail-tabs">
            {(["Overview", "Execution", "Git", "History"] as const).map((t) => (
              <button
                key={t}
                className={tab === t ? "selected-text" : ""}
                onClick={() => setTab(t)}
              >
                {t}
              </button>
            ))}
          </div>
          {tab === "Overview" && (
            <>
              <div className="markdown">
                <ReactMarkdown
                  skipHtml
                  urlTransform={(url) => (/^https:\/\//.test(url) ? url : "")}
                  components={{
                    a: ({ href, children }) => (
                      <button
                        className="markdown-link"
                        onClick={() => href && external(href)}
                      >
                        {children}
                      </button>
                    ),
                    img: ({ alt }) => <span>[Image: {alt}]</span>,
                  }}
                >
                  {issue.body || "No description provided."}
                </ReactMarkdown>
              </div>
              <div className="labels">
                {issue.labels.map((l) => (
                  <span key={l}>{l}</span>
                ))}
              </div>
              <dl>
                <div>
                  <dt>Provider</dt>
                  <dd>
                    <ProviderPicker value={provider} onChange={setProvider} />
                  </dd>
                </div>
                <div>
                  <dt>Queue position</dt>
                  <dd>{item?.position ?? "Not queued"}</dd>
                </div>
                <div>
                  <dt>Assignees</dt>
                  <dd>{issue.assignees.join(", ") || "Unassigned"}</dd>
                </div>
                <div>
                  <dt>Milestone</dt>
                  <dd>{issue.milestone ?? "None"}</dd>
                </div>
                <div>
                  <dt>Updated</dt>
                  <dd>{new Date(issue.updatedAt).toLocaleString()}</dd>
                </div>
                <div>
                  <dt>Approval gate</dt>
                  <dd>
                    {item?.approved
                      ? "Approved"
                      : issue.labels.some((l) =>
                            project?.policy.manualApprovalLabels.includes(l),
                          )
                        ? "Manual approval required"
                        : "Standard policy"}
                  </dd>
                </div>
              </dl>
              {execution?.reason && (
                <div className="execution-reason">{execution.reason}</div>
              )}
              <p className="muted">
                Add to queue schedules this issue for execution when you start
                the queue. Dry run checks Git and the provider and previews the
                prompt and validation plan without running the agent.
              </p>
              {!project?.path && (
                <div className="execution-reason" role="status">
                  <p>
                    Link a local Git folder for this repository to enable Add to
                    queue and Dry run. Your tasks will run in that folder.
                  </p>
                  <AiButton onClick={onLinkFolder}>
                    <FolderGit2 size={14} />
                    Link local folder
                  </AiButton>
                </div>
              )}
              {issue.state === "CLOSED" && !item && (
                <p className="muted">
                  This issue is closed on GitHub. Reopen it to add it to the
                  queue.
                </p>
              )}
              <div className="detail-actions">
                {!item ? (
                  <AiButton
                    variant="primary"
                    title={
                      !project?.path
                        ? "Link a local folder in repository settings to run tasks"
                        : undefined
                    }
                    disabled={
                      loading || issue.state === "CLOSED" || !project?.path
                    }
                    onClick={() =>
                      void command("enqueue", {
                        projectId: project?.id,
                        number: issue.number,
                        providerId: provider,
                      })
                    }
                  >
                    <Plus size={14} />
                    Add to queue
                  </AiButton>
                ) : (
                  <>
                    <AiButton onClick={() => act("retry")}>Retry</AiButton>
                    <AiButton onClick={() => act("provider")}>
                      Apply provider
                    </AiButton>
                    <AiButton onClick={() => act("skip")}>Skip</AiButton>
                    {["BLOCKED", "HUMAN_REVIEW"].includes(item.state) && (
                      <AiButton
                        variant="primary"
                        onClick={() => act("approve")}
                      >
                        Approve after review
                      </AiButton>
                    )}
                  </>
                )}
                <AiButton
                  disabled={loading || !project?.path}
                  title="Check Git, provider health and the execution plan without running the agent"
                  onClick={() =>
                    void command("dry_run", {
                      projectId: project?.id,
                      number: issue.number,
                      providerId: provider,
                    }).then((v) => v && setDryRun(v))
                  }
                >
                  Dry run
                </AiButton>
              </div>
              {dryRun !== undefined && (
                <pre className="dry-run">{JSON.stringify(dryRun, null, 2)}</pre>
              )}
            </>
          )}
          {tab === "Execution" &&
            (execution ? (
              <>
                <AiButton onClick={() => onExecution(execution)}>
                  Open Execution Center
                  <ArrowUpRight size={14} />
                </AiButton>
                <AiExecutionConsole executionId={execution.id} />
              </>
            ) : (
              <Empty title="Not started yet">
                Add this task to your queue to follow its execution here.
              </Empty>
            ))}
          {tab === "Git" && (
            <div className="git-evidence">
              <h3>Before execution</h3>
              <pre>
                {JSON.stringify(
                  execution?.beforeGit ?? "No evidence yet",
                  null,
                  2,
                )}
              </pre>
              <h3>After independent validation</h3>
              <pre>
                {JSON.stringify(
                  execution?.afterGit ?? "No evidence yet",
                  null,
                  2,
                )}
              </pre>
            </div>
          )}
          {tab === "History" && (
            <div className="attempt-history">
              {executions
                .filter((e) => e.itemId === item?.id)
                .map((e) => (
                  <button
                    className="glass"
                    key={e.id}
                    onClick={() => onExecution(e)}
                  >
                    <span>Attempt {e.attempt}</span>
                    <AiProviderBadge id={e.providerId} />
                    <AiStatusBadge state={e.state} />
                    <small>{new Date(e.startedAt).toLocaleString()}</small>
                    {e.reason && <p>{e.reason}</p>}
                  </button>
                ))}
            </div>
          )}
        </div>
      )}
    </AiSheet>
  );
}
