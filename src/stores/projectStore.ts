import { writable } from 'svelte/store';
import type { Sentence } from '../types';

export const sentences = writable<Sentence[]>([]);
export const selectedSentence = writable<Sentence | null>(null);
export const projectName = writable('');
export const projectDirectory = writable('');
export const isProjectLoaded = writable(false);
export const isRecording = writable(false);
