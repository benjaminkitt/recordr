<script lang="ts">
  import { invoke } from '@tauri-apps/api/tauri';
  import { open } from '@tauri-apps/api/dialog';
  import { readTextFile, writeTextFile, BaseDirectory } from '@tauri-apps/api/fs';
  import { join } from '@tauri-apps/api/path';

  let sentences: string[] = [];
  let newSentence = '';
  let selectedSentence = '';
  let isRecording = false;
  let filename = '';
  let projectPath = '';
  let projectDirectory = '';
  let projectName = '';
  let isProjectLoaded = false;

  async function newProject() {
    const selected = await open({
      title: 'Select Project Save Location',
      directory: true,
      defaultPath: '',
    });

    if (selected && typeof selected === 'string') {
      projectDirectory = selected;
      projectName = prompt('Enter a name for your project:', 'MyProject');

      if (projectName) {
        projectPath = `${projectDirectory}/${projectName}.json`;
        sentences = [];
        isProjectLoaded = true;
        saveProject();
      } else {
        alert('Project name is required.');
      }
    }
  }

  async function openProject() {
    const selected = await open({
      title: 'Open Project',
      filters: [
        {
          name: 'Project Files',
          extensions: ['json'],
        },
      ],
    });

    if (selected && typeof selected === 'string') {
      projectPath = selected;
      projectDirectory = projectPath.substring(0, projectPath.lastIndexOf('/'));
      projectName = projectPath.substring(
        projectPath.lastIndexOf('/') + 1,
        projectPath.lastIndexOf('.')
      );
      try {
        const contents = await readTextFile(projectPath);
        const data = JSON.parse(contents);
        sentences = data.sentences || [];
        selectedSentence = data.selectedSentence || '';
        isProjectLoaded = true;
      } catch (error) {
        console.error('Error reading project file:', error);
        alert('Failed to open project.');
      }
    }
  }

  async function saveProject() {
    if (!isProjectLoaded) {
      alert('No project is currently loaded.');
      return;
    }

    const data = {
      sentences,
      selectedSentence,
    };

    try {
      await writeTextFile(projectPath, JSON.stringify(data, null, 2));
      alert('Project saved successfully.');
    } catch (error) {
      console.error('Error saving project:', error);
      alert('Failed to save project.');
    }
  }

  async function generateFilename(sentence: string): Promise<string> {
    const sanitizedSentence = sentence.trim().replace(/\s+/g, '_') + '.wav';
    return await join(projectDirectory, sanitizedSentence);
  }

  function addSentence() {
    const trimmedSentence = newSentence.trim();
    if (trimmedSentence === '') {
      alert('Please enter a sentence.');
    } else if (sentences.includes(trimmedSentence)) {
      alert('This sentence is already in the list.');
    } else {
      sentences = [...sentences, trimmedSentence];
      newSentence = '';
    }

    // Save project after adding a sentence
    saveProject();
  }

  function removeSentence(index: number) {
    if (confirm('Are you sure you want to remove this sentence?')) {
      sentences = sentences.filter((_, i) => i !== index);
    }

    // Save project after removing a sentence
    saveProject();
  }

  function selectSentence(sentence: string) {
    selectedSentence = sentence;
  }

  async function toggleRecording() {
    if (!isProjectLoaded) {
      alert('Please create or open a project first.');
      return;
    }

    if (!selectedSentence) {
      alert('Please select a sentence to record.');
      return;
    }

    if (isRecording) {
      invoke('stop_recording')
        .then(() => {
          isRecording = false;
        })
        .catch((error) => {
          console.error(error);
        });
    } else {
      filename = await generateFilename(selectedSentence);
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

  .remove-button {
    background-color: #ff6b6b;
    border: none;
    color: white;
    padding: 4px 8px;
    cursor: pointer;
  }

  .remove-button:hover {
    background-color: #ff5252;
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

<div>
  <h1>Sentence Recorder</h1>

  <!-- Project Management Buttons -->
  <div>
    <button on:click={newProject}>New Project</button>
    <button on:click={openProject}>Open Project</button>
    <button on:click={saveProject} disabled={!isProjectLoaded}>Save Project</button>
  </div>

  <!-- Show current project name -->
  {#if isProjectLoaded}
    <p>Current Project: <strong>{projectName}</strong></p>
  {/if}

  <!-- Add Sentence Input -->
  <div>
    <input
      type="text"
      bind:value={newSentence}
      placeholder="Enter a new sentence"
      on:keydown={(e) => e.key === 'Enter' && addSentence()}
      disabled={!isProjectLoaded}
    />
    <button on:click={addSentence} disabled={!isProjectLoaded}>Add Sentence</button>
  </div>

  <!-- Sentence List -->
  <div>
    {#each sentences as sentence, index}
      <div
        class="sentence-item {selectedSentence === sentence ? 'selected' : ''}"
        on:click={() => selectSentence(sentence)}
      >
        <span class="sentence-text">{sentence}</span>
        <button class="remove-button" on:click|stopPropagation={() => removeSentence(index)}>
          Remove
        </button>
      </div>
    {/each}
  </div>

  <!-- Recording Controls -->
  <div>
    <button on:click={toggleRecording} disabled={!selectedSentence}>
      {isRecording ? 'Stop Recording' : 'Start Recording'}
    </button>
    {#if isRecording}
      <span class="recording-indicator"></span>
    {/if}
  </div>
</div>

