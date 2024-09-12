<script lang="ts">
  import { invoke } from '@tauri-apps/api/tauri';

  let isRecording = false;
  let sentence = '';
  let filename = '';

  function generateFilename(sentence: string): string {
    return sentence.trim().replace(/\s+/g, '_') + '.wav';
  }

  function toggleRecording() {
    if (isRecording) {
      invoke('stop_recording')
        .then(() => {
          isRecording = false;
        })
        .catch((error) => {
          console.error(error);
        });
    } else {
      filename = generateFilename(sentence);
      invoke('start_recording', { filename })
        .then(() => {
          isRecording = true;
        })
        .catch((error) => {
          console.error(error);
        });
    }
  }
</script>

<div>
  <h1>Sentence Recorder</h1>
  <input type="text" bind:value={sentence} placeholder="Enter sentence to record" />
  <button on:click={toggleRecording}>
    {isRecording ? 'Stop Recording' : 'Start Recording'}
  </button>
</div>
