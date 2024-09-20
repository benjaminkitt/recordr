export type Sentence = {
  id: number;
  text: string;
  recorded: boolean;
  audioFilePath?: string;
};

// Define the event payload types
export type AutoRecordStartSentenceEvent = {
  payload: number; // Sentence index
};

export type AutoRecordFinishSentenceEvent = {
  payload: number; // Sentence index
};