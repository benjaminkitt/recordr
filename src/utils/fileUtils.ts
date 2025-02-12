import { invoke } from '@tauri-apps/api/tauri';
import { getVersion } from '@tauri-apps/api/app';
import { open } from '@tauri-apps/api/dialog';
import { join, homeDir } from '@tauri-apps/api/path';
import { get } from 'svelte/store';
import {
  sentences,
  project,
  isProjectLoaded,
  updateProject,
  selectedSentence,
  isRecording,
} from '../stores/projectStore';
import type { Project, Sentence } from '../types';
import { appWindow } from '@tauri-apps/api/window';
import type { ModalSettings, ModalStore } from '@skeletonlabs/skeleton';

async function setWindowTitle(name: string) {
  await appWindow.setTitle(`Recordr - ${name}`);
}

export async function newProject(modalStore: ModalStore) {
  const homePath = await homeDir();
  const selected = (await open({
    directory: true,
    defaultPath: homePath,
    multiple: false,
    title: 'Select the location for your project directory',
  })) as string;

  if (selected) {
    const modal: ModalSettings = {
      type: 'component',
      component: 'projectNameInput',
      title: 'Create New Project',
      body: 'Please provide a name for your new project.',
      response: async (value: string) => {
        if (value) {
          const appVersion = await getVersion();
          const now = new Date().toISOString();
          const newProject: Project = {
            metadata: {
              name: value,
              created_version: appVersion,
              last_updated_version: appVersion,
              created_at: now,
              last_modified: now,
              directory: selected,
            },
            sentences: [],
          };
          const savedProject: Project = await invoke('create_new_project', {
            parentDir: selected,
            project: newProject,
          });
          if (savedProject) {
            project.set(savedProject);
            sentences.set([]);
            isProjectLoaded.set(true);
            await setWindowTitle(value);
            // Build a recent project object with required schema
            const projectFilePath = `${savedProject.metadata.directory}/${savedProject.metadata.name}.json`;
            const recentProject = {
              path: projectFilePath,
              name: savedProject.metadata.name,
              last_accessed: new Date().toISOString(),
            };
            // Send new recent project along with the app version
            await invoke('add_recent_project', {
              newProject: recentProject,
              appVersion: appVersion,
            });
            modalStore.close();
          }
        }
      },
    };
    modalStore.trigger(modal);
  }
}

export async function openProject(path?: string) {
  const homePath = await homeDir();
  const selected =
    path ||
    ((await open({
      filters: [{ name: 'Project Files', extensions: ['json'] }],
      defaultPath: homePath,
      multiple: false,
      title: 'Select Project File',
    })) as string);

  if (selected) {
    const loadedProject: Project = await invoke('open_project', { filePath: selected });
    project.set(loadedProject);
    sentences.set(loadedProject.sentences);
    isProjectLoaded.set(true);
    await setWindowTitle(loadedProject.metadata.name);
    // Build a recent project object and add it with the app version
    const appVersion = await getVersion();
    const recentProject = {
      path: selected,
      name: loadedProject.metadata.name,
      last_accessed: new Date().toISOString(),
    };
    await invoke('add_recent_project', {
      newProject: recentProject,
      appVersion: appVersion,
    });
  }
}

export async function saveProject() {
  const currentProject = get(project);
  if (!currentProject) {
    console.error('No project loaded');
    return;
  }
  const appVersion = await getVersion();
  const now = new Date().toISOString();

  updateProject({
    metadata: {
      ...currentProject.metadata,
      last_updated_version: appVersion,
      last_modified: now,
    },
    sentences: get(sentences),
  });

  const updatedProject = get(project);
  if (updatedProject) {
    await invoke('save_project', { project: updatedProject });
  }
}

export async function toggleRecording() {
  const sentence = get(selectedSentence);
  if (!sentence) {
    alert('Select a sentence to record.');
    return;
  }

  const filename = await generateFilename(sentence);
  if (get(isRecording)) {
    invoke('stop_recording').then(() => {
      sentence.recorded = true;
      saveProject();
    });
  } else {
    invoke('start_recording', { filename });
  }
}

export async function generateFilename(sentence: Sentence) {
  const currentProject = get(project);
  if (!currentProject) {
    throw new Error('No project loaded');
  }
  return await join(
    currentProject.metadata.directory,
    `${sentence.text.trim().replace(/\s+/g, '_')}.wav`
  );
}

export async function playSentence(sentence: Sentence) {
  const fullPath = await generateFilename(sentence);
  try {
    const audioData: number[] = await invoke('load_audio_file', { filePath: fullPath });
    const uint8Array = new Uint8Array(audioData);
    const blob = new Blob([uint8Array], { type: 'audio/wav' });
    const audio = new Audio();
    audio.src = URL.createObjectURL(blob);
    audio.play();
  } catch (error) {
    console.error('Error playing audio:', error);
  }
}

export async function handleFileImport() {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Text Files', extensions: ['txt', 'csv', 'tsv'] }],
    });

    if (Array.isArray(selected) || !selected) {
      return;
    }

    const currentProject = get(project);
    if (!currentProject) {
      console.error('No project loaded');
      return;
    }

    const newSentences: Sentence[] = await invoke('import_sentences', {
      filePath: selected,
      projectDir: currentProject.metadata.directory,
    });

    sentences.update((currentSentences) => {
      const maxId = Math.max(0, ...currentSentences.map((s) => s.id));
      return [
        ...currentSentences,
        ...newSentences.map((s, index) => ({ ...s, id: maxId + index + 1 })),
      ];
    });
  } catch (error) {
    console.error('Error importing sentences:', error);
  }
}
