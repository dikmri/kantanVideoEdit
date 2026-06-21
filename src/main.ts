import "./styles.css";
import { invoke } from "@tauri-apps/api/core";

// ---- Global error capture → log file (must run BEFORE the app mounts) ----
// Uses invoke() directly (synchronously importable) rather than dynamic-importing
// the log plugin, so errors are captured even if they fire during app bootstrap.

function sendLog(level: string, message: string): void {
  // fire-and-forget; invoke returns a Promise we deliberately don't await so
  // the error handler stays synchronous-ish and never throws.
  invoke("write_frontend_log", { level, message }).catch(() => {
    /* if this fails too, there's nothing more we can do */
  });
}

function stringifyErr(err: unknown): string {
  if (err instanceof Error) {
    return `${err.name}: ${err.message}\n${err.stack ?? "(no stack)"}`;
  }
  if (typeof err === "object" && err !== null) {
    try {
      return JSON.stringify(err);
    } catch {
      return String(err);
    }
  }
  return String(err);
}

// Uncaught runtime errors (the ones that turn the screen black)
window.addEventListener("error", (event) => {
  const detail = event.error
    ? stringifyErr(event.error)
    : `${event.message} @ ${event.filename}:${event.lineno}:${event.colno}`;
  sendLog("error", `Uncaught error: ${detail}`);
});

// Unhandled promise rejections (async errors)
window.addEventListener("unhandledrejection", (event) => {
  sendLog("error", `Unhandled rejection: ${stringifyErr(event.reason)}`);
});

// Wrap console.error / console.warn so everything is persisted to the log file
const _origConsoleError = console.error.bind(console);
console.error = (...args: unknown[]): void => {
  _origConsoleError(...args);
  sendLog("error", `console.error: ${args.map(stringifyErr).join(" ")}`);
};

const _origConsoleWarn = console.warn.bind(console);
console.warn = (...args: unknown[]): void => {
  _origConsoleWarn(...args);
  sendLog("warn", `console.warn: ${args.map(stringifyErr).join(" ")}`);
};

// ---- Mount the app (lazy import so the error handlers above are registered first) ----
let app: unknown;
(async () => {
  try {
    const { mount } = await import("svelte");
    const { default: App } = await import("./App.svelte");
    app = mount(App, {
      target: document.getElementById("app")!,
    });
    sendLog("info", "App mounted successfully");
  } catch (err) {
    sendLog("error", `Failed to mount app: ${stringifyErr(err)}`);
    // Show a visible message so the user doesn't just see a black screen
    const root = document.getElementById("app");
    if (root) {
      root.innerHTML =
        '<div style="padding:32px;color:#ff5c5c;font-family:monospace;white-space:pre-wrap;background:#1a1b1f;min-height:100vh">Failed to start Kantan Video Edit.\nSee the log file for details.\n\n' +
        stringifyErr(err).replace(/</g, "&lt;") +
        "</div>";
    }
  }
})();

export default app;
