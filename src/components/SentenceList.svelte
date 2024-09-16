<script lang="ts">
  import { sentences, selectedSentence, isRecording, isProjectLoaded, projectDirectory } from '../stores/projectStore'; // import projectDirectory
  import { playSentence, toggleRecording } from '../utils/fileUtils';
  import type { Sentence } from '../types';
  import { startAutoRecord as autoRecord } from '../utils/autoRecord';
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { listen } from '@tauri-apps/api/event';

  let silenceThreshold = 0.5; // Default value
  let silenceDuration = 2000; // In milliseconds
  let silencePadding = 300; // In milliseconds

  let isAutoRecording = false;
  let currentSentenceIndex = -1;

  function startAutoRecord() {
    isAutoRecording = true;
    const sentenceTexts = get(sentences).map((s) => s.text);
    const projectDir = get(projectDirectory); // Get the project directory

    if (!projectDir) {
      console.error('Project directory is not set');
      isAutoRecording = false;
      return;
    }

    autoRecord(sentenceTexts, projectDir, silenceThreshold, silenceDuration, silencePadding)
      .catch((error) => {
        console.error('Error starting auto-record:', error);
        isAutoRecording = false;
      });
  }

  onMount(() => {
    // Listen for events from the backend
    const unlistenStart = listen('auto-record-start-sentence', (event) => {
      currentSentenceIndex = event.payload;
    });

    const unlistenFinish = listen('auto-record-finish-sentence', (event) => {
      const index = event.payload;
      $sentences[index].recorded = true;
    });

    const unlistenComplete = listen('auto-record-complete', () => {
      isAutoRecording = false;
      currentSentenceIndex = -1;
    });

    return () => {
      unlistenStart.then((unlisten) => unlisten());
      unlistenFinish.then((unlisten) => unlisten());
      unlistenComplete.then((unlisten) => unlisten());
    };
  });

  let newSentence = '';

  $: $sentences as Sentence[];
  $: $selectedSentence as Sentence | null;

  function addSentence() {
    const trimmedSentence = newSentence.trim();
    if (trimmedSentence === '') {
      alert('Please enter a sentence.');
    } else if ($sentences.some((s) => s.text === trimmedSentence)) {
      alert('This sentence is already in the list.');
    } else {
      $sentences = [...$sentences, { text: trimmedSentence, recorded: false }];
      newSentence = '';
    }
  }

  function removeSentence(index: number) {
    if (confirm('Are you sure you want to remove this sentence?')) {
      $sentences = $sentences.filter((_, i) => i !== index);
    }
  }

  function selectSentence(sentence: Sentence) {
    selectedSentence.set(sentence);
  }
</script>

<div>
  <div>
    <label>
      Silence Threshold:
      <input type="number" bind:value={silenceThreshold} min="0" max="1" step="0.01" />
    </label>
    <label>
      Silence Duration (ms):
      <input type="number" bind:value={silenceDuration} min="100" step="100" />
    </label>
    <label>
      Silence Padding (ms):
      <input type="number" bind:value={silencePadding} min="0" step="50" />
    </label>
  </div>
  
  <input
    type="text"
    bind:value={newSentence}
    placeholder="Enter a new sentence"
    on:keydown={(e) => e.key === 'Enter' && addSentence()}
  />
  <button on:click={addSentence}>Add Sentence</button>

  <div>
    {#each $sentences as sentence, index}
      <div
        class="sentence-item {$selectedSentence === sentence ? 'selected' : ''}"
        role="button"
        tabindex="0"
        on:click={() => selectSentence(sentence)}
        on:keydown={(e) => e.key === 'Enter' && selectSentence(sentence)}
      >
        <span class="sentence-text">{sentence.text}</span>
        {#if sentence.recorded}
          <span class="recorded">[Recorded]</span>
          <button on:click={() => playSentence(sentence)}>Play</button>
        {/if}
        <button on:click={() => removeSentence(index)}>Remove</button>
      </div>
    {/each}
  </div>

  <div>
    <button on:click={() => toggleRecording()} disabled={!$selectedSentence}>
      {$isRecording ? 'Stop Recording' : 'Start Recording'}
    </button>
    {#if $isRecording}
      <span class="recording-indicator"></span>
    {/if}
  </div>
  <button on:click={startAutoRecord} disabled={!$isProjectLoaded || !$sentences.length || isAutoRecording}>
    {isAutoRecording ? 'Auto-Recording...' : 'Start Auto-Record'}
  </button>
  {#if isAutoRecording}
    <p>Recording sentence {currentSentenceIndex + 1} of {$sentences.length}</p>
  {/if}
  
</div>

<style>
  .sentence-item {
    display: flex;
    align-items: center;
    padding: 8px;
    border: 1px solid #ddd;
    margin-bottom: 4px;
    cursor: pointer;
  }

  .sentence-item:hover {
    background-color: #f9f9f9;
  }

  .selected {
    background-color: #d0f0fd;
  }

  .sentence-text {
    flex-grow: 1;
  }

  .recorded {
    color: green;
    margin-left: 10px;
  }

  input[type='text'] {
    padding: 8px;
    margin-right: 8px;
    width: 300px;
  }

  button {
    padding: 8px 16px;
    cursor: pointer;
  }

  button:disabled {
    background-color: #ccc;
    cursor: not-allowed;
  }

  .recording-indicator {
    display: inline-block;
    width: 12px;
    height: 12px;
    background-color: red;
    border-radius: 50%;
    margin-left: 8px;
  }
</style>
