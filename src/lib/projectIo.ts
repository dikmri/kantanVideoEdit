import { open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import type { Project, ProjectFile } from "../types";
import { projectStore, createEmptyProject } from "../stores";

const PROJECT_VERSION = 1;
const PROJECT_EXT = "kveproj";

const FILTERS = [
  { name: "Kantan Video Edit Project", extensions: [PROJECT_EXT] },
];

export async function saveProject(saveAs = false): Promise<boolean> {
  const p = projectStore.get();
  // Reuse stored path unless saveAs
  // (We keep it simple: always ask the first time by tracking a module-level path.)
  if (!saveAs && currentPath) {
    await writeToFile(currentPath, p);
    projectStore.markSaved();
    return true;
  }
  const path = await save({
    defaultPath: `${p.name || "untitled"}.${PROJECT_EXT}`,
    filters: FILTERS,
  });
  if (!path) return false;
  currentPath = path;
  await writeToFile(path, p);
  projectStore.markSaved();
  return true;
}

let currentPath: string | null = null;
export function getCurrentPath(): string | null {
  return currentPath;
}
export function setCurrentPath(p: string | null): void {
  currentPath = p;
}

async function writeToFile(path: string, project: Project): Promise<void> {
  const data: ProjectFile = { version: PROJECT_VERSION, project };
  await writeTextFile(path, JSON.stringify(data, null, 2));
}

export async function openProject(): Promise<boolean> {
  const path = await open({ filters: FILTERS, multiple: false });
  if (!path || Array.isArray(path)) return false;
  const text = await readTextFile(path);
  const data = JSON.parse(text) as ProjectFile;
  if (!data?.project) throw new Error("Invalid project file");
  // Basic migration / defaults
  const p = normalizeProject(data.project);
  projectStore.reset(p);
  currentPath = path;
  return true;
}

export async function newProject(): Promise<void> {
  projectStore.reset(createEmptyProject());
  currentPath = null;
}

function normalizeProject(p: Project): Project {
  return {
    name: p.name ?? "Untitled",
    width: p.width ?? 1920,
    height: p.height ?? 1080,
    fps: p.fps ?? 30,
    tracks: Array.isArray(p.tracks) ? p.tracks : [],
    assets: p.assets ?? {},
  };
}
