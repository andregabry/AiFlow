import {
  useEffect,
  useRef,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";
import { AnimatePresence, motion } from "motion/react";
import {
  X,
  Circle,
  CircleCheck,
  LoaderCircle,
  Clock3,
  TriangleAlert,
  OctagonX,
} from "lucide-react";
import { motionTokens } from "../design-system/branding";
import { providerNames } from "../core/types";

export function AiButton({
  children,
  variant = "secondary",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost" | "danger";
}) {
  return (
    <button className={`button ${variant}`} {...props}>
      {children}
    </button>
  );
}
export function AiStatusBadge({ state }: { state: string }) {
  const active = [
    "RUNNING",
    "IN_PROGRESS",
    "PREPARING",
    "VALIDATING",
    "COMMITTING",
  ].includes(state);
  const done = ["SUCCESS", "DONE", "CONNECTED", "AVAILABLE"].includes(state);
  const wait = state.includes("WAIT") || state === "LIMITED";
  const error = ["FAILED", "INCOMPATIBLE", "DISCONNECTED"].includes(state);
  const warn = [
    "BLOCKED",
    "HUMAN_REVIEW",
    "AUTH_REQUIRED",
    "CANCELLED",
  ].includes(state);
  const Icon = active
    ? LoaderCircle
    : done
      ? CircleCheck
      : wait
        ? Clock3
        : error
          ? OctagonX
          : warn
            ? TriangleAlert
            : Circle;
  return (
    <span
      className={`status ${active ? "active" : done ? "done" : wait ? "waiting" : error ? "failed" : warn ? "blocked" : ""}`}
    >
      <Icon size={12} className={active ? "spin-slow" : ""} />
      {state.replaceAll("_", " ")}
    </span>
  );
}
export function AiProviderBadge({ id }: { id: string }) {
  return (
    <span className={`provider-badge ${id}`}>
      <span className="provider-glyph">
        {id === "codex"
          ? "◎"
          : id === "claude"
            ? "✳"
            : id === "gemini"
              ? "✧"
              : "↗"}
      </span>
      {providerNames[id] ?? id}
    </span>
  );
}
export function AiSegmentedControl<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: T[];
  onChange: (value: T) => void;
}) {
  return (
    <div className="segmented" role="group">
      {options.map((o) => (
        <button key={o} aria-pressed={value === o} onClick={() => onChange(o)}>
          {value === o && (
            <motion.span
              className="segment-indicator"
              layoutId={`segment-${options.join("-")}`}
              transition={motionTokens.soft}
            />
          )}
          <span>{o}</span>
        </button>
      ))}
    </div>
  );
}
export function AiSheet({
  open,
  title,
  children,
  onClose,
  wide = false,
  modal = false,
}: {
  open: boolean;
  title: string;
  children: ReactNode;
  onClose: () => void;
  wide?: boolean;
  modal?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const previous = document.activeElement as HTMLElement;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "Tab") {
        const elements = ref.current?.querySelectorAll<HTMLElement>(
          'button:not(:disabled),input,select,textarea,a[href],[tabindex="0"]',
        );
        if (!elements?.length) return;
        const first = elements[0],
          last = elements[elements.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    const timer = setTimeout(
      () => ref.current?.querySelector<HTMLElement>("button,input")?.focus(),
      20,
    );
    document.addEventListener("keydown", handler);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("keydown", handler);
      previous?.focus();
    };
  }, [open, onClose]);
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          className={`scrim ${modal ? "modal-scrim" : ""}`}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onMouseDown={(e) => {
            if (e.target === e.currentTarget) onClose();
          }}
        >
          <motion.div
            ref={ref}
            role="dialog"
            aria-modal="true"
            aria-label={title}
            className={`sheet ${wide ? "wide" : ""} ${modal ? "centered-modal" : ""}`}
            initial={{ x: 70, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: 70, opacity: 0 }}
            transition={motionTokens.soft}
          >
            <div className="sheet-heading">
              <h2>{title}</h2>
              <AiButton
                variant="ghost"
                onClick={onClose}
                aria-label="Close panel"
              >
                <X size={19} />
              </AiButton>
            </div>
            {children}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
export function Empty({
  title,
  children,
  action,
}: {
  title: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="empty">
      <div className="empty-orbit">↗</div>
      <h2>{title}</h2>
      <p>{children}</p>
      {action}
    </div>
  );
}
