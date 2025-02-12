import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/tauri';

export interface RecentProject {
  path: string;
  name: string;
  last_accessed: string; // ISO timestamp
}

export interface RecentProjectsData {
  app_version: string;
  recent_projects: RecentProject[];
}

export const recentProjectsData = writable<RecentProjectsData>({
  app_version: '',
  recent_projects: [],
});

export async function addRecentProject(newProj: RecentProject, app_version: string) {
  // The backend now expects both the project and the current app version.
  const updatedData: RecentProjectsData = await invoke('add_recent_project', {
    new_project: newProj,
    app_version: app_version,
  });
  recentProjectsData.set(updatedData);
}

export async function loadRecentProjects() {
  const data: RecentProjectsData = await invoke('get_recent_projects');
  // Safety: sort and limit on client in case backend data is old.
  data.recent_projects.sort(
    (a, b) => new Date(b.last_accessed).getTime() - new Date(a.last_accessed).getTime()
  );
  data.recent_projects = data.recent_projects.slice(0, 10);
  recentProjectsData.set(data);
}
