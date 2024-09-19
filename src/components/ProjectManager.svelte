<script lang="ts">
  import { openProject, saveProject, newProject, handleFileImport } from "../utils/fileUtils";
  import { projectName, isProjectLoaded } from "../stores/projectStore";
  import type { Sentence } from '../types';

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

<div class="flex flex-wrap gap-2">
  <button on:click={createProject} class="btn variant-filled">New Project</button>
  <button on:click={openExistingProject} class="btn variant-filled">Open Project</button>
  <button on:click={saveCurrentProject} disabled={!$isProjectLoaded} class="btn variant-filled">Save Project</button>
  <button on:click={handleFileImport} disabled={!$isProjectLoaded} class="btn variant-filled">Import Sentences</button>
</div>

{#if $isProjectLoaded}
  <p class="mt-4">Current Project: <strong>{$projectName}</strong></p>
{/if}