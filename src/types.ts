export type Sentence = {
  text: string;
  recorded: boolean;
  audioFile?: string;
};

// Define the event payload types
export type AutoRecordStartSentenceEvent = {
  payload: number; // Sentence index
};

export type AutoRecordFinishSentenceEvent = {
  payload: number; // Sentence index
};