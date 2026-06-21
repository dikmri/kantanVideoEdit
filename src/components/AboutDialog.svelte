<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { t } from "../i18n";
  import Icon from "./Icon.svelte";

  const dispatch = createEventDispatcher();
  export const version = "1.0.0";

  function close(): void {
    dispatch("close");
  }
</script>

<div class="overlay" on:click|self={close} on:keydown={(e) => e.key === "Escape" && close()} role="presentation">
  <div class="dialog" role="dialog" aria-modal="true">
    <div class="about-content">
      <div class="logo">
        <Icon name="film" size={48} />
      </div>
      <h2>{$t("app.title")}</h2>
      <p class="tagline">{$t("app.tagline")}</p>
      <p class="version">{$t("about.version")} {version}</p>
      <p class="desc">{$t("about.description")}</p>
      <div class="features">
        <span>✓ Fast & Lightweight</span>
        <span>✓ Cross-platform</span>
        <span>✓ Multilingual</span>
      </div>
    </div>
    <div class="dialog-footer">
      <button class="primary" on:click={close}>{$t("common.close")}</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
    backdrop-filter: blur(2px);
  }
  .dialog {
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    width: min(400px, 92vw);
    box-shadow: var(--shadow);
    overflow: hidden;
  }
  .about-content {
    padding: 32px 24px;
    text-align: center;
  }
  .logo {
    color: var(--accent);
    margin-bottom: 12px;
  }
  h2 {
    font-size: 20px;
    font-weight: 700;
    margin-bottom: 4px;
  }
  .tagline {
    color: var(--text-dim);
    font-size: 13px;
    margin-bottom: 16px;
  }
  .version {
    font-size: 12px;
    color: var(--text-faint);
    margin-bottom: 16px;
  }
  .desc {
    font-size: 13px;
    color: var(--text);
    margin-bottom: 20px;
  }
  .features {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--success);
  }
  .dialog-footer {
    display: flex;
    justify-content: center;
    padding: 12px 16px;
    border-top: 1px solid var(--border-soft);
  }
</style>
