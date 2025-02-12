// Define the event payload types
export type AutoRecordStartSentenceEvent = {
  payload: number; // Sentence index
};

export type AutoRecordFinishSentenceEvent = {
  payload: {
    id: number;
    audioFilePath: string;
  };
};

export interface ProjectMetadata {
  name: string;
  created_version: string;
  last_updated_version: string;
  created_at: string;
  last_modified: string;
  directory: string;
}

export interface Project {
  metadata: ProjectMetadata;
  sentences: Sentence[];
}

export interface Sentence {
  id: number;
  text: string;
  recorded: boolean;
  audio_file_path: string | null;
}
