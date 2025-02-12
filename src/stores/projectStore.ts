import { writable } from 'svelte/store';
import type { Project, Sentence } from '../types';

export const project = writable<Project | null>(null);
export const sentences = writable<Sentence[]>([]);
export const selectedSentence = writable<Sentence | null>(null);
export const isProjectLoaded = writable(false);
export const isRecording = writable(false);

export function updateProject(updatedProject: Partial<Project>) {
  project.update((currentProject) => {
    if (currentProject) {
      return { ...currentProject, ...updatedProject };
    }
    return currentProject;
  });
}
