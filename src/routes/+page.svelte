<script lang="ts">
  import { invoke } from '@tauri-apps/api/tauri';
  import { open } from '@tauri-apps/api/dialog';
  import { readTextFile, writeTextFile, readBinaryFile, BaseDirectory } from '@tauri-apps/api/fs';
  import { join, homeDir, dirname } from '@tauri-apps/api/path';

  type Sentence = {
    text: string;
    recorded: boolean;
    audioFile?: string;  // Relative path to the audio file within the project directory
  };

  let sentences: Sentence[] = [];
  let newSentence = '';
  let selectedSentence: Sentence | null = null;
  let isRecording = false;
  let projectPath = '';
  let projectDirectory = '';
  let projectName = '';
  let isProjectLoaded = false;
  let audioPlayer: HTMLAudioElement | null = null;

  async function newProject() {
    try {
      // Select a folder under the user's home directory
      const homeDirPath = await homeDir();
      const selected = await open({
        directory: true,
        defaultPath: await join(homeDirPath),
        title: 'Select Project Save Location (Home Directory)',
      });

      if (selected && typeof selected === 'string') {
        // Ensure the selected directory is within the user's home directory
        if (!selected.startsWith(homeDirPath)) {
          alert('Project must be created within your home directory.');
          return;
        }

        projectDirectory = selected;
        projectName = prompt('Enter a name for your project:', 'MyProject');

        if (projectName) {
          const projectPath = await join(projectDirectory, `${projectName}.json`);
          await writeTextFile(projectPath, JSON.stringify({ sentences: [] }, null, 2));
          isProjectLoaded = true;
          alert(`Project ${projectName} created successfully.`);
        } else {
          alert('Project name is required.');
        }
      }
    } catch (error) {
      console.error('Error creating project:', error);
    }
  }

  async function openProject() {
    try {
      // Get the actual home directory path
      const homePath = await homeDir();

      // Let the user select the project JSON file
      const selected = await open({
        title: 'Open Project',
        filters: [
          {
            name: 'Project Files',
            extensions: ['json'],
          },
        ],
        defaultPath: homePath,
        directory: false,
      });

      if (selected && typeof selected === 'string') {
        // Ensure that the selected file is within the home directory
        if (!selected.startsWith(homePath)) {
          alert('The selected project is not within your home directory.');
          return;
        }

        // Calculate the relative project directory using dirname
        const fullProjectDirectory = await dirname(selected);  // Get the full directory path
        projectDirectory = fullProjectDirectory.replace(homePath, '');  // Strip the home directory part

        // Extract project name from the file name
        projectName = selected.substring(
          selected.lastIndexOf('/') + 1,
          selected.lastIndexOf('.')
        );

        // Read and parse the project JSON file
        const contents = await readTextFile(selected);
        const data = JSON.parse(contents);
        sentences = data.sentences || [];

        isProjectLoaded = true;
        alert(`Project ${projectName} loaded successfully.`);
      }
    } catch (error) {
      console.error('Error opening project:', error);
      alert('Failed to open project.');
    }
  }

  async function saveProject() {
    if (!isProjectLoaded) {
      alert('No project is currently loaded.');
      return;
    }

    const data = {
      sentences,
    };

    try {
      const homeDirPath = await homeDir();
      const projectFile = await join(homeDirPath, 'recordr_projects', projectName, `${projectName}.json`);
      await writeTextFile(projectFile, JSON.stringify(data, null, 2));
      alert('Project saved successfully.');
    } catch (error) {
      console.error('Error saving project:', error);
      alert('Failed to save project.');
    }
  }


  async function generateFilename(sentence: Sentence): Promise<string> {
    // If there's no audio file, generate a relative path for the audio file
    if (!sentence.audioFile) {
      sentence.audioFile = `${sentence.text.trim().replace(/\s+/g, '_')}.wav`;
    }

    // Join the project directory with the relative audio file path
    let homeDirPath = await homeDir();
    return await join(homeDirPath, projectDirectory, sentence.audioFile);
  }

  function addSentence() {
    const trimmedSentence = newSentence.trim();
    if (trimmedSentence === '') {
      alert('Please enter a sentence.');
    } else if (sentences.some((s) => s.text === trimmedSentence)) {
      alert('This sentence is already in the list.');
    } else {
      sentences = [...sentences, { text: trimmedSentence, recorded: false }];
      newSentence = '';
      saveProject(); // Save project after adding a sentence
    }
  }

  function removeSentence(index: number) {
    if (confirm('Are you sure you want to remove this sentence?')) {
      sentences = sentences.filter((_, i) => i !== index);
    }

    // Save project after removing a sentence
    saveProject();
  }

  function selectSentence(sentence: Sentence) {
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
          selectedSentence!.recorded = true;

          // Store the relative path to the audio file in the sentence
          const relativeFilename = `${selectedSentence!.text.trim().replace(/\s+/g, '_')}.wav`;
          selectedSentence!.audioFile = relativeFilename;

          saveProject();  // Save project after recording
        })
        .catch((error) => {
          console.error(error);
        });
    } else {
      const filename = await generateFilename(selectedSentence);
      invoke('start_recording', { filename })
        .then(() => {
          isRecording = true;
        })
        .catch((error) => {
          console.error(error);
        });
    }
  }

  // TODO: Find a way to reduce the scope (currently "[**]" in tauri.conf.json)
  async function playSentence(sentence: Sentence) {
    if (!sentence.recorded || !sentence.audioFile) {
      alert('This sentence has not been recorded yet.');
      return;
    }

    const fullPath = await join(projectDirectory, sentence.audioFile);  // Construct the full path
    const dirsToLog = `
    projectDirectory: ${projectDirectory}
    audioFile: ${sentence.audioFile}
    `
    console.log(dirsToLog);
    try {
      // Read the audio file using the full path
      const audioData = await readBinaryFile(fullPath, { dir: BaseDirectory.Home });

      // Create a Blob from the audio data
      const blob = new Blob([audioData], { type: 'audio/wav' });

      // Create an object URL from the Blob and play it
      if (!audioPlayer) {
        audioPlayer = new Audio();
      }
      audioPlayer.src = URL.createObjectURL(blob);
      audioPlayer.play().catch((error) => {
        console.error('Failed to play audio:', error);
      });
    } catch (error) {
      console.error('Failed to load audio file:', error);
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

  .recorded {
    color: green;
    margin-left: 10px;
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
        <span class="sentence-text">
          {sentence.text}
          {#if sentence.recorded}
            <span class="status recorded">[Recorded]</span>
            <button on:click|stopPropagation={() => playSentence(sentence)}>Play</button>
          {/if}
        </span>
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
