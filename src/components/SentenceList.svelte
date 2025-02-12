<script lang="ts">
  import {
    project,
    sentences,
    selectedSentence,
    isRecording,
    isProjectLoaded,
  } from '../stores/projectStore';
  import { playSentence, toggleRecording, saveProject } from '../utils/fileUtils';
  import type { Sentence, AutoRecordFinishSentenceEvent } from '../types';
  import {
    startAutoRecord as autoRecord,
    stopAutoRecord,
    pauseAutoRecord,
    resumeAutoRecord,
  } from '../utils/autoRecord';
  import { onMount, afterUpdate } from 'svelte';
  import { get } from 'svelte/store';
  import { listen } from '@tauri-apps/api/event';
  import { RangeSlider } from '@skeletonlabs/skeleton';
  import MdiRemoveBox from '~icons/mdi/remove-box';
  import MdiPlus from '~icons/mdi/plus';
  import MdiRecordRec from '~icons/mdi/record-rec';
  import MdiStop from '~icons/mdi/stop';
  import MdiPlayPause from '~icons/mdi/play-pause';
  import MdiPause from '~icons/mdi/pause';
  import MdiPlay from '~icons/mdi/play';
  import { popup } from '@skeletonlabs/skeleton';
  import type { PopupSettings } from '@skeletonlabs/skeleton';
  import { appWindow } from '@tauri-apps/api/window';

  let silenceThreshold = 0.5;
  let silenceDuration = 2000;
  let silencePadding = 300;

  let isAutoRecording = false;
  let isPaused = false;
  let currentSentenceIndex = -1;

  let sentenceListContainer: HTMLDivElement;
  let currentRecordingId: number | null = null;

  async function startAutoRecord() {
    isAutoRecording = true;
    const currentProject = get(project);

    if (!currentProject) {
      console.error('No project loaded');
      isAutoRecording = false;
      return;
    }

    try {
      await autoRecord(
        get(sentences),
        currentProject.metadata.directory,
        silenceThreshold,
        silenceDuration,
        silencePadding,
        appWindow as unknown as Window
      );
    } catch (error) {
      console.error('Error starting auto-record:', error);
      isAutoRecording = false;
    }
  }

  async function toggleAutoRecord() {
    if (!isAutoRecording) {
      await startAutoRecord();
      isAutoRecording = true;
    } else {
      await stopAutoRecord();
      isAutoRecording = false;
      isPaused = false;
    }
  }

  async function togglePauseResume() {
    if (isPaused) {
      await resumeAutoRecord();
      isPaused = false;
    } else {
      await pauseAutoRecord();
      isPaused = true;
    }
  }

  onMount(() => {
    const unlistenStart = listen('auto-record-start-sentence', (event: { payload: number }) => {
      currentRecordingId = event.payload;
      scrollToCurrentSentence();
    });

    const unlistenFinish = listen(
      'auto-record-finish-sentence',
      (event: AutoRecordFinishSentenceEvent) => {
        const sentenceIndex = $sentences.findIndex((s) => s.id === event.payload.id);
        if (sentenceIndex !== -1) {
          $sentences[sentenceIndex].recorded = true;
          $sentences[sentenceIndex].audio_file_path = event.payload.audioFilePath;
          saveProject(); // Add this function to auto-save the project
        }
        currentRecordingId = null;
      }
    );

    const unlistenComplete = listen('auto-record-complete', () => {
      isAutoRecording = false;
      currentRecordingId = null;
    });

    return () => {
      unlistenStart.then((unlisten) => unlisten());
      unlistenFinish.then((unlisten) => unlisten());
      unlistenComplete.then((unlisten) => unlisten());
    };
  });

  function scrollToCurrentSentence() {
    if (currentRecordingId !== null && sentenceListContainer) {
      const currentSentenceElement = sentenceListContainer.querySelector(
        `[data-sentence-id="${currentRecordingId}"]`
      ) as HTMLElement;
      if (currentSentenceElement) {
        const containerRect = sentenceListContainer.getBoundingClientRect();
        const elementRect = currentSentenceElement.getBoundingClientRect();

        const containerScrollTop = sentenceListContainer.scrollTop;
        const elementRelativeTop = elementRect.top - containerRect.top + containerScrollTop;

        let scrollOffset = elementRelativeTop - containerRect.height / 2 + elementRect.height / 2;
        scrollOffset = Math.max(
          0,
          Math.min(scrollOffset, sentenceListContainer.scrollHeight - containerRect.height)
        );

        sentenceListContainer.scrollTo({
          top: scrollOffset,
          behavior: 'smooth',
        });
      }
    }
  }

  afterUpdate(() => {
    scrollToCurrentSentence();
  });

  let newSentence = '';

  function addSentence() {
    if (!$isProjectLoaded || $isRecording || isAutoRecording) {
      return;
    }
    const trimmedSentence = newSentence.trim();
    if (trimmedSentence.length === 0) {
      return;
    } else if ($sentences.some((s) => s.text === trimmedSentence)) {
      alert('This sentence is already in the list.');
    } else {
      const newId = Math.max(0, ...$sentences.map((s) => s.id)) + 1;
      $sentences = [
        ...$sentences,
        { id: newId, text: trimmedSentence, recorded: false, audio_file_path: null },
      ];
      saveProject();
      newSentence = '';
    }
  }

  function handleSentenceKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      addSentence();
    }
  }

  function removeSentence(index: number) {
    if ($isRecording || isAutoRecording) {
      return; // Don't allow removal while recording
    }
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
    placement: 'top',
  };
</script>

<div class="space-y-4 flex flex-col h-full">
  <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
    <div>
      <label class="label" for="silence-threshold">
        Silence Threshold:
        <div class="flex justify-between text-xs">
          <span>0</span>
          <span>{silenceThreshold}</span>
          <span>1</span>
        </div>
        <RangeSlider
          id="silence-threshold"
          name="silence-threshold"
          bind:value={silenceThreshold}
          min={0}
          max={1}
          step={0.01}
        />
      </label>
    </div>
    <div>
      <label class="label" for="silence-duration">
        Silence Duration (ms):
        <div class="flex justify-between text-xs">
          <span>100</span>
          <span>{silenceDuration}</span>
          <span>5000</span>
        </div>
        <RangeSlider
          id="silence-duration"
          name="silence-duration"
          bind:value={silenceDuration}
          min={100}
          max={5000}
          step={100}
        />
      </label>
    </div>
    <div>
      <label class="label" for="silence-padding">
        Silence Padding (ms):
        <div class="flex justify-between text-xs">
          <span>0</span>
          <span>{silencePadding}</span>
          <span>1000</span>
        </div>
        <RangeSlider
          id="silence-padding"
          name="silence-padding"
          bind:value={silencePadding}
          min={0}
          max={1000}
          step={50}
        />
      </label>
    </div>
  </div>

  <div class="flex gap-4">
    <input
      type="text"
      class="input py-3 px-4 block w-full"
      placeholder="Enter a new sentence"
      bind:value={newSentence}
      on:keydown={handleSentenceKeydown}
    />
    <button
      class="btn variant-filled shrink-0 inline-flex justify-center items-center gap-x-2"
      on:click={addSentence}
      disabled={!$isProjectLoaded ||
        $isRecording ||
        isAutoRecording ||
        newSentence.trim().length === 0}
    >
      <MdiPlus />
      <span class="ml-2">Add Sentence</span>
    </button>
  </div>

  <div
    class="flex-grow overflow-hidden mt-4 border border-surface-300-600-token rounded-container-token"
  >
    <div bind:this={sentenceListContainer} class="h-full overflow-y-auto p-4">
      {#each $sentences as sentence, index}
        <div
          role="button"
          tabindex="0"
          data-sentence-id={sentence.id}
          class="p-2 mb-2 rounded-container-token
            {sentence.id === currentRecordingId
            ? 'bg-secondary-500 animate-pulse'
            : $selectedSentence === sentence
              ? 'bg-primary-500'
              : 'bg-surface-200-700-token'}"
          on:click={() => selectSentence(sentence)}
          on:keydown={(e) => e.key === 'Enter' && selectSentence(sentence)}
        >
          <div class="flex justify-between items-center">
            <span>{sentence.text}</span>
            <div class="flex items-center gap-2">
              {#if sentence.recorded}
                <span class="badge variant-filled-success">Recorded</span>
                <button class="btn btn-sm variant-ghost" on:click={() => playSentence(sentence)}>
                  <MdiPlay />
                </button>
              {/if}
              <button
                class="btn btn-sm variant-filled-error"
                use:popup={removePopupHover}
                on:click={() => removeSentence(index)}
                disabled={$isRecording || isAutoRecording}
              >
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
      {#if $isRecording}
        <MdiStop />
        <span class="ml-2">Stop Recording</span>
      {:else}
        <MdiRecordRec />
        <span class="ml-2">Start Recording</span>
      {/if}
    </button>
    {#if $isRecording}
      <span class="badge variant-filled-error animate-pulse">Recording</span>
    {/if}
    <button
      class="btn variant-filled"
      on:click={toggleAutoRecord}
      disabled={!$isProjectLoaded || !$sentences.length}
    >
      {#if isAutoRecording}
        <MdiStop />
        <span class="ml-2">Stop Auto-Record</span>
      {:else}
        <MdiPlayPause />
        <span class="ml-2">Start Auto-Record</span>
      {/if}
    </button>
    {#if isAutoRecording}
      <button class="btn variant-filled" on:click={togglePauseResume}>
        {#if isPaused}
          <MdiPlay />
          <span class="ml-2">Resume Auto-Record</span>
        {:else}
          <MdiPause />
          <span class="ml-2">Pause Auto-Record</span>
        {/if}
      </button>
    {/if}
  </div>
  {#if isAutoRecording}
    <p class="mt-2">
      {isPaused
        ? 'Auto-recording paused'
        : `Recording sentence ${currentSentenceIndex + 1} of ${$sentences.length}`}
    </p>
  {/if}
</div>
