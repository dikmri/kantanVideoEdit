<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import {
    project,
    selectedClipId,
    currentTime,
    duration as durationStore,
    isPlaying,
    splitAt,
    deleteClip,
    duplicateClip,
    undo,
    redo,
    commitHistory,
    zoom,
    addMediaToProject,
  } from "./stores";
  import { saveProject, openProject, newProject } from "./lib/projectIo";
  import { probeMediaBatch } from "./lib/api";
  import { stableId } from "./lib/util";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { t } from "./i18n";
  import { theme } from "./stores";

  import MenuBar from "./components/MenuBar.svelte";
  import MediaPanel from "./components/MediaPanel.svelte";
  import Preview from "./components/Preview.svelte";
  import PropertiesPanel from "./components/PropertiesPanel.svelte";
  import Timeline from "./components/Timeline.svelte";
  import StatusBar from "./components/StatusBar.svelte";
  import ExportDialog from "./components/ExportDialog.svelte";
  import AboutDialog from "./components/AboutDialog.svelte";

  let showExport = false;
  let showAbout = false;

  onMount(() => {
    document.documentElement.setAttribute("data-theme", get(theme));
  });

  async function handleAction(action: string): Promise<void> {
    switch (action) {
      case "import":
        await importMedia();
        break;
      case "new":
        if (confirm($t("dialog.replaceProjectMessage"))) {
          await newProject();
        }
        break;
      case "open":
        try {
          await openProject();
        } catch (err) {
          alert(String(err));
        }
        break;
      case "save":
        try {
          await saveProject(false);
        } catch (err) {
          alert(String(err));
        }
        break;
      case "saveAs":
        try {
          await saveProject(true);
        } catch (err) {
          alert(String(err));
        }
        break;
      case "export":
        showExport = true;
        break;
      case "undo":
        undo();
        break;
      case "redo":
        redo();
        break;
      case "split": {
        const time = get(currentTime);
        splitAt(time);
        commitHistory();
        break;
      }
      case "duplicate": {
        const id = get(selectedClipId);
        if (id) duplicateClip(id);
        break;
      }
      case "delete": {
        const id = get(selectedClipId);
        if (id) deleteClip(id);
        break;
      }
      case "zoomIn":
        zoom.update((z) => Math.min(600, z * 1.3));
        break;
      case "zoomOut":
        zoom.update((z) => Math.max(10, z / 1.3));
        break;
      case "fit":
        break;
      case "about":
        showAbout = true;
        break;
    }
  }

  async function importMedia(): Promise<void> {
    const result = await openDialog({
      multiple: true,
      filters: [
        {
          name: "Media",
          extensions: [
            "mp4", "mov", "mkv", "avi", "webm", "m4v", "mpg", "mpeg", "ts",
            "mp3", "wav", "aac", "flac", "ogg", "m4a",
            "png", "jpg", "jpeg", "bmp", "webp", "gif",
          ],
        },
      ],
    });
    if (!result) return;
    const paths = Array.isArray(result) ? result : [result];
    try {
      const results = await probeMediaBatch(paths);
      for (let i = 0; i < paths.length; i++) {
        const probe = results[i];
        if (!probe) continue;
        addMediaToProject({
          id: stableId("asset"),
          path: paths[i],
          name: paths[i].replace(/\\/g, "/").split("/").pop() || paths[i],
          type: probe.type,
          duration: probe.duration,
          width: probe.width,
          height: probe.height,
          codec: probe.codec,
          fps: probe.fps,
          thumbnail: probe.thumbnail,
        });
      }
      commitHistory();
    } catch (err) {
      alert(`${$t("status.importing")}: ${String(err)}`);
    }
  }

  function onKeydown(e: KeyboardEvent): void {
    const target = e.target as HTMLElement;
    if (target?.matches("input, textarea, select")) return;
    const mod = e.ctrlKey || e.metaKey;

    if (mod && e.key === "z" && !e.shiftKey) {
      e.preventDefault();
      undo();
    } else if (mod && (e.key === "y" || (e.key === "z" && e.shiftKey))) {
      e.preventDefault();
      redo();
    } else if (mod && e.key === "s") {
      e.preventDefault();
      handleAction("save");
    } else if (mod && e.key === "o") {
      e.preventDefault();
      handleAction("open");
    } else if (mod && e.key === "n") {
      e.preventDefault();
      handleAction("new");
    } else if (mod && e.key === "e") {
      e.preventDefault();
      handleAction("export");
    } else if (mod && e.key === "d") {
      e.preventDefault();
      handleAction("duplicate");
    } else if (mod && e.key === "i") {
      e.preventDefault();
      importMedia();
    } else if (e.key === "Delete" || e.key === "Backspace") {
      if (get(selectedClipId)) {
        e.preventDefault();
        handleAction("delete");
      }
    } else if (e.key === "s" && !mod) {
      e.preventDefault();
      handleAction("split");
    } else if (e.key === " " || e.code === "Space") {
      // handled in Preview
    } else if (e.key === "Home") {
      currentTime.set(0);
    } else if (e.key === "End") {
      currentTime.set(get(durationStore));
    }
  }
</script>

<svelte:window on:keydown={onKeydown} />

<div class="app-layout">
  <MenuBar on:action={(e) => handleAction(e.detail)} />

  <div class="main-area">
    <MediaPanel class="media" />
    <Preview />
    <PropertiesPanel />
    <Timeline />
  </div>

  <StatusBar />
</div>

{#if showExport}
  <ExportDialog on:close={() => (showExport = false)} />
{/if}
{#if showAbout}
  <AboutDialog on:close={() => (showAbout = false)} />
{/if}
