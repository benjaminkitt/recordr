<script lang="ts">
  import { openProject, saveProject, newProject, handleFileImport } from "../utils/fileUtils";
  import { projectName, isProjectLoaded } from "../stores/projectStore";
  import type { Sentence } from '../types';  // Import the Sentence type

  function createProject() {
    newProject();
  }

  function openExistingProject() {
    openProject();
  }

  function saveCurrentProject() {
    saveProject();
  }
</script>

<style>
  button {
    padding: 8px 16px;
    cursor: pointer;
  }

  button:disabled {
    background-color: #ccc;
    cursor: not-allowed;
  }
</style>

<div>
  <button on:click={createProject}>New Project</button>
  <button on:click={openExistingProject}>Open Project</button>
  <button on:click={saveCurrentProject} disabled={!$isProjectLoaded}
    >Save Project</button
  >
  <button on:click={handleFileImport} disabled={!$isProjectLoaded}>
    Import Sentences
  </button>

  {#if $isProjectLoaded}
    <p>Current Project: <strong>{$projectName}</strong></p>
  {/if}
</div>
