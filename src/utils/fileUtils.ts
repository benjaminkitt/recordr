import { invoke } from '@tauri-apps/api/tauri';
import { open } from '@tauri-apps/api/dialog';
import { join, homeDir } from '@tauri-apps/api/path';
import { get } from 'svelte/store';
import {
  sentences,
  projectName,
  projectDirectory,
  isProjectLoaded,
  selectedSentence,
  isRecording,
} from '../stores/projectStore';
import type { Sentence } from '../types';
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
  })) as string; // Cast as string since multiple is false

  if (selected) {
    const modal: ModalSettings = {
      type: 'component',
      component: 'projectNameInput',
      title: 'Create New Project',
      body: 'Please provide a name for your new project.',
      response: async (value: string) => {
        if (value) {
          const result = await invoke('create_new_project', {
            parentDir: selected,
            projectName: value,
          });
          if (result) {
            projectName.set(value);
            projectDirectory.set(await join(selected, value));
            isProjectLoaded.set(true);
            await setWindowTitle(value);
          }
        }
      },
    };
    modalStore.trigger(modal);
  }
}

export async function openProject() {
  const homePath = await homeDir();
  const selected = (await open({
    filters: [{ name: 'Project Files', extensions: ['json'] }],
    defaultPath: homePath,
    multiple: false,
    title: 'Select Project File',
  })) as string;

  if (selected) {
    const loadedSentences: [Sentence] = await invoke('open_project', { filePath: selected });
    sentences.set(loadedSentences);
    const name = selected.split('/').pop()?.replace('.json', '') || '';
    projectName.set(name);
    projectDirectory.set(selected.substring(0, selected.lastIndexOf('/')));
    isProjectLoaded.set(true);
    await setWindowTitle(name);
  }
}

export async function saveProject() {
  const projectDir = get(projectDirectory);
  const name = get(projectName);
  if (!projectDir || !name) {
    console.error('Project directory or name is not set');
    return;
  }
  const path = await join(projectDir, `${name}.json`);
  await invoke('save_project', { filePath: path, sentences: get(sentences) });
}

export async function toggleRecording() {
  const sentence = get(selectedSentence); // Use 'get' to retrieve the value
  if (!sentence) {
    alert('Select a sentence to record.');
    return;
  }

  const filename = await generateFilename(sentence);
  if (get(isRecording)) {
    // Retrieve the current value of isRecording
    invoke('stop_recording').then(() => {
      sentence.recorded = true;
      saveProject(); // Pass current sentences as argument
    });
  } else {
    invoke('start_recording', { filename });
  }
}

export async function generateFilename(sentence: Sentence) {
  const projectDir = get(projectDirectory); // Retrieve the current project directory
  return await join(projectDir, `${sentence.text.trim().replace(/\s+/g, '_')}.wav`);
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
      // User canceled the dialog or didn't select a file
      return;
    }

    const projectDir = get(projectDirectory); // Get the project directory
    if (!projectDir) {
      console.error('Project directory is not set');
      return;
    }

    const newSentences: [Sentence] = await invoke('import_sentences', {
      filePath: selected,
      projectDir,
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
