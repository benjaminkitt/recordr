import { invoke } from '@tauri-apps/api/tauri';
import { readTextFile, writeTextFile } from '@tauri-apps/api/fs';
import { readBinaryFile, BaseDirectory } from '@tauri-apps/api/fs';
import { open } from '@tauri-apps/api/dialog';
import { join, homeDir } from '@tauri-apps/api/path';
import { get } from 'svelte/store';
import { 
  sentences 
  , projectName
  , projectDirectory
  , isProjectLoaded
  , selectedSentence
  , isRecording 
} from '../stores/projectStore';
import type { Sentence } from '../types';

export async function newProject() {
  const homePath = await homeDir();
  const selected = await open({
    directory: true,
    defaultPath: homePath,
    multiple: false,
    title: 'Select Project Save Location (Home Directory)',
  }) as string;  // Cast as string since multiple is false

  if (selected) {
    projectDirectory.set(selected);
    const name = prompt('Enter a name for your project:');
    projectName.set(name || 'MyProject');
    const path = await join(selected, `${name}.json`);
    await writeTextFile(path, JSON.stringify({ sentences: [] }));
    isProjectLoaded.set(true);
  }
}

export async function openProject() {
  const homePath = await homeDir();
  const selected = await open({
    filters: [{ name: 'Project Files', extensions: ['json'] }],
    defaultPath: homePath,
    multiple: false,
    title: 'Select Project File',
  }) as string;  // Cast as string since multiple is false

  if (selected) {
    const content = await readTextFile(selected);
    const data = JSON.parse(content);
    sentences.set(data.sentences);
    projectName.set(selected.split('/').pop()?.replace('.json', '') || '');
    projectDirectory.set(selected.substring(0, selected.lastIndexOf('/')));
    isProjectLoaded.set(true);
  }
}

export async function saveProject() {
  const homePath = await homeDir();
  const path = await join(homePath, 'recordr_projects', `${projectName}.json`);
  await writeTextFile(path, JSON.stringify({ sentences: get(sentences) }));
}

export async function toggleRecording() {
  const sentence = get(selectedSentence);  // Use 'get' to retrieve the value
  if (!sentence) {
    alert('Select a sentence to record.');
    return;
  }

  const filename = await generateFilename(sentence);
  if (get(isRecording)) {  // Retrieve the current value of isRecording
    invoke('stop_recording').then(() => {
      sentence.recorded = true;
      saveProject();  // Pass current sentences as argument
    });    
  } else {
    invoke('start_recording', { filename });
  }
}

export async function generateFilename(sentence: Sentence) {
  const homePath = await homeDir();
  const projectDir = get(projectDirectory);  // Retrieve the current project directory
  return await join(projectDir, `${sentence.text.trim().replace(/\s+/g, '_')}.wav`);
}

export async function playSentence(sentence: Sentence) {
  const fullPath = await generateFilename(sentence);
  const audioData = await readBinaryFile(fullPath, { dir: BaseDirectory.Home });
  const blob = new Blob([audioData], { type: 'audio/wav' });
  const audio = new Audio();
  audio.src = URL.createObjectURL(blob);
  audio.play();
}

export async function handleFileImport() {
  try {
    const selected = await open({
      multiple: false,
      filters: [
        { name: 'Text Files', extensions: ['txt', 'csv', 'tsv'] },
      ],
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

    const newSentences:[Sentence] = await invoke('import_sentences', { 
      filePath: selected, 
      projectDir 
    });

    sentences.update(currentSentences => [...currentSentences, ...newSentences]);
  } catch (error) {
    console.error('Error importing sentences:', error);
  }
}