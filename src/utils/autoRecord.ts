import { invoke } from '@tauri-apps/api/tauri';
import type { Sentence } from '../types';

export async function startAutoRecord(
  sentences: Sentence[],
  projectDirectory: string,
  silenceThreshold: number,
  silenceDuration: number,
  silencePadding: number,
  window: Window
) {
  await invoke('start_auto_record', {
    sentences,
    projectDirectory,
    silenceThreshold,
    silenceDuration,
    silencePadding,
    window,
  });
}

export async function stopAutoRecord() {
  await invoke('stop_auto_record');
}