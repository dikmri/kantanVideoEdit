<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { t, locale, locales, setLocale, type LocaleCode } from "../i18n";
  import { theme } from "../stores";
  import Icon from "./Icon.svelte";
  import { canUndoStore, canRedoStore } from "../stores";

  const dispatch = createEventDispatcher();

  let openMenu: string | null = null;

  function toggle(name: string): void {
    openMenu = openMenu === name ? null : name;
  }

  function act(action: string): void {
    openMenu = null;
    dispatch("action", action);
  }

  function toggleTheme(): void {
    theme.update((t) => (t === "dark" ? "light" : "dark"));
  }

  function pickLocale(e: Event): void {
    setLocale((e.currentTarget as HTMLSelectElement).value as LocaleCode);
  }

  function onWindowClick(e: MouseEvent): void {
    const target = e.target as HTMLElement;
    if (!target.closest(".menu-trigger")) openMenu = null;
  }
</script>

<svelte:window on:click={onWindowClick} />

<header class="topbar">
  <div class="brand">
    <Icon name="film" size={18} />
    <span class="brand-name">{$t("app.title")}</span>
  </div>

  <nav class="menus">
    {#each ["file", "edit", "view", "help"] as m}
      <div class="menu-wrap">
        <button class="menu-trigger" on:click|stopPropagation={() => toggle(m)}>
          {$t(`menu.${m}`)}
        </button>
        {#if openMenu === m}
          <div class="dropdown" on:click|stopPropagation>
            {#if m === "file"}
              <button class="menu-item" on:click={() => act("new")}>
                <Icon name="new" size={14} /> {$t("file.new")}
                <span class="kbd">Ctrl+N</span>
              </button>
              <button class="menu-item" on:click={() => act("open")}>
                <Icon name="open" size={14} /> {$t("file.open")}
                <span class="kbd">Ctrl+O</span>
              </button>
              <button class="menu-item" on:click={() => act("save")}>
                <Icon name="save" size={14} /> {$t("file.save")}
                <span class="kbd">Ctrl+S</span>
              </button>
              <button class="menu-item" on:click={() => act("saveAs")}>
                {$t("file.saveAs")}
              </button>
              <div class="menu-divider"></div>
              <button class="menu-item" on:click={() => act("export")}>
                <Icon name="export" size={14} /> {$t("file.export")}
                <span class="kbd">Ctrl+E</span>
              </button>
            {:else if m === "edit"}
              <button class="menu-item" on:click={() => act("undo")} disabled={!$canUndoStore}>
                <Icon name="undo" size={14} /> {$t("edit.undo")}
                <span class="kbd">Ctrl+Z</span>
              </button>
              <button class="menu-item" on:click={() => act("redo")} disabled={!$canRedoStore}>
                <Icon name="redo" size={14} /> {$t("edit.redo")}
                <span class="kbd">Ctrl+Y</span>
              </button>
              <div class="menu-divider"></div>
              <button class="menu-item" on:click={() => act("split")}>
                <Icon name="scissors" size={14} /> {$t("edit.split")}
                <span class="kbd">S</span>
              </button>
              <button class="menu-item" on:click={() => act("duplicate")}>
                <Icon name="copy" size={14} /> {$t("edit.duplicate")}
                <span class="kbd">Ctrl+D</span>
              </button>
              <button class="menu-item" on:click={() => act("delete")}>
                <Icon name="trash" size={14} /> {$t("edit.delete")}
                <span class="kbd">Del</span>
              </button>
            {:else if m === "view"}
              <button class="menu-item" on:click={() => act("zoomIn")}>
                <Icon name="zoom-in" size={14} /> {$t("view.zoomIn")}
                <span class="kbd">+</span>
              </button>
              <button class="menu-item" on:click={() => act("zoomOut")}>
                <Icon name="zoom-out" size={14} /> {$t("view.zoomOut")}
                <span class="kbd">-</span>
              </button>
              <button class="menu-item" on:click={() => act("fit")}>{$t("view.fit")}</button>
              <div class="menu-divider"></div>
              <button class="menu-item" on:click={toggleTheme}>
                <Icon name={$theme === "dark" ? "sun" : "moon"} size={14} />
                {$theme === "dark" ? $t("settings.light") : $t("settings.dark")}
              </button>
            {:else if m === "help"}
              <button class="menu-item" on:click={() => act("about")}>
                <Icon name="info" size={14} /> {$t("about.title")}
              </button>
            {/if}
          </div>
        {/if}
      </div>
    {/each}
  </nav>

  <div class="spacer"></div>

  <select class="locale-select" value={$locale} on:change={pickLocale} title={$t("settings.language")}>
    {#each Object.entries(locales) as [code, label]}
      <option value={code}>{label}</option>
    {/each}
  </select>

  <button class="icon" on:click={toggleTheme} title={$t("settings.theme")}>
    <Icon name={$theme === "dark" ? "sun" : "moon"} />
  </button>

  <button on:click={() => act("import")} title={$t("toolbar.import")}>
    <Icon name="import" size={15} /> {$t("toolbar.import")}
  </button>
  <button class="primary" on:click={() => act("export")} title={$t("toolbar.export")}>
    <Icon name="export" size={15} /> {$t("toolbar.export")}
  </button>
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    gap: 4px;
    height: var(--header-h);
    padding: 0 8px;
    background: var(--bg-elevated);
    border-bottom: 1px solid var(--border-soft);
    position: relative;
    z-index: 50;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px 0 4px;
    color: var(--accent);
    font-weight: 700;
  }
  .brand-name {
    color: var(--text);
    font-size: 13px;
  }
  .menus {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .menu-wrap {
    position: relative;
  }
  .menu-trigger {
    padding: 6px 10px;
  }
  .spacer {
    flex: 1;
  }
  .locale-select {
    width: auto;
    padding: 4px 6px;
    margin-right: 4px;
  }
</style>
