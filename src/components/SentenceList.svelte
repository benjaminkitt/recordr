<script lang="ts">
  import { sentences, selectedSentence, isRecording } from '../stores/projectStore';
  import { playSentence, toggleRecording } from '../utils/fileUtils';
  import type { Sentence } from '../types';

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
