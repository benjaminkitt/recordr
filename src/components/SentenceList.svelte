<script lang="ts">
  import { 
    sentences,
    selectedSentence,
    isRecording,
    isProjectLoaded,
    projectDirectory 
  } from '../stores/projectStore';
  import { playSentence, toggleRecording } from '../utils/fileUtils';
  import type { 
    Sentence,
    AutoRecordStartSentenceEvent,
    AutoRecordFinishSentenceEvent 
  } from '../types';
  import { startAutoRecord as autoRecord } from '../utils/autoRecord';
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { listen } from '@tauri-apps/api/event';
  import { RangeSlider } from '@skeletonlabs/skeleton';
  import MdiRemoveBox from '~icons/mdi/remove-box';
  import { popup } from '@skeletonlabs/skeleton';
  import type { PopupSettings } from '@skeletonlabs/skeleton';

  let silenceThreshold = 0.5;
  let silenceDuration = 2000;
  let silencePadding = 300;

  let isAutoRecording = false;
  let currentSentenceIndex = -1;

  function startAutoRecord() {
    isAutoRecording = true;
    const sentenceTexts = get(sentences).map((s) => s.text);
    const projectDir = get(projectDirectory);

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
    const unlistenStart = listen('auto-record-start-sentence', (event: AutoRecordStartSentenceEvent) => {
      currentSentenceIndex = event.payload;
    });

    const unlistenFinish = listen('auto-record-finish-sentence', (event: AutoRecordFinishSentenceEvent) => {
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

  const removePopupHover: PopupSettings = {
    event: 'hover',
    target: 'removePopupHover',
    placement: 'top'
  };
</script>

<div class="space-y-4 flex flex-col h-full">
  <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
    <div>
      <label class="label">
        Silence Threshold:
        <div class="flex justify-between text-xs">
          <span>0</span>
          <span>{silenceThreshold}</span>
          <span>1</span>
        </div>
        <RangeSlider name="range-slider" bind:value={silenceThreshold} min={0} max={1} step={0.01} />
      </label>
    </div>
    <div>
      <label class="label">
        Silence Duration (ms):
        <div class="flex justify-between text-xs">
          <span>100</span>
          <span>{silenceDuration}</span>
          <span>5000</span>
        </div>
        <RangeSlider name="range-slider" bind:value={silenceDuration} min={100} max={5000} step={100} />
      </label>
    </div>
    <div>
      <label class="label">
        Silence Padding (ms):
        <div class="flex justify-between text-xs">
          <span>0</span>
          <span>{silencePadding}</span>
          <span>1000</span>
        </div>
        <RangeSlider name="range-slider" bind:value={silencePadding} min={0} max={1000} step={50} />
      </label>
    </div>
  </div>

  <div class="flex gap-4">
    <input 
      type="text" 
      class="input py-3 px-4 block w-full" 
      placeholder="Enter a new sentence" 
      bind:value={newSentence}
    >
    <button class="btn variant-filled shrink-0 inline-flex justify-center items-center gap-x-2" on:click={addSentence}>Add Sentence</button>
  </div>  

  <div class="flex-grow overflow-hidden mt-4 border border-surface-300-600-token rounded-container-token">
    <div class="h-full overflow-y-auto p-4">
    {#each $sentences as sentence, index}
      <div
        class="p-2 mb-2 {$selectedSentence === sentence ? 'bg-primary-500' : 'bg-surface-200-700-token'} rounded-container-token"
        on:click={() => selectSentence(sentence)}
        on:keydown={(e) => e.key === 'Enter' && selectSentence(sentence)}
      >
        <div class="flex justify-between items-center">
          <span>{sentence.text}</span>
          <div>
            {#if sentence.recorded}
              <span class="badge variant-filled-success">Recorded</span>
              <button class="btn variant-ghost" on:click={() => playSentence(sentence)}>Play</button>
            {/if}
            <button class="btn variant-filled-error" use:popup={removePopupHover} on:click={() => removeSentence(index)}>
              <MdiRemoveBox />
            </button>
            <div class="card p-4 variant-filled-secondary" data-popup="removePopupHover">
              <p>Delete</p>
              <div class="arrow variant-filled-secondary" />
            </div>
          </div>
        </div>
      </div>
    {/each}
    </div>
  </div>

  <div class="mt-4 flex gap-2">
    <button 
      class="btn variant-filled" 
      on:click={() => toggleRecording()} 
      disabled={!$selectedSentence}
    >
      {$isRecording ? 'Stop Recording' : 'Start Recording'}
    </button>
    {#if $isRecording}
      <span class="badge variant-filled-error animate-pulse">Recording</span>
    {/if}
    <button 
      class="btn variant-filled" 
      on:click={startAutoRecord} 
      disabled={!$isProjectLoaded || !$sentences.length || isAutoRecording}
    >
      {isAutoRecording ? 'Auto-Recording...' : 'Start Auto-Record'}
    </button>
  </div>
  {#if isAutoRecording}
    <p class="mt-2">Recording sentence {currentSentenceIndex + 1} of {$sentences.length}</p>
  {/if}
</div>
