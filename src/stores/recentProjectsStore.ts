import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/tauri';

export const recentProjects = writable<string[]>([]);

export async function addRecentProject(path: string) {
  const updatedList: [string] = await invoke('add_recent_project', { path });
  recentProjects.set(updatedList);
}

export async function loadRecentProjects() {
  const projects: [string] = await invoke('get_recent_projects');
  recentProjects.set(projects);
}
