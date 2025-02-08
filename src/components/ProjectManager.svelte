<script lang="ts">
  import { openProject, saveProject, newProject, handleFileImport } from '../utils/fileUtils';
  import { isProjectLoaded } from '../stores/projectStore';
  import { getModalStore } from '@skeletonlabs/skeleton';
  import MdiFolderOpen from '~icons/mdi/folder-open';
  import MdiContentSave from '~icons/mdi/content-save';
  import MdiFileImport from '~icons/mdi/file-import';
  import MdiPlus from '~icons/mdi/plus';

  const modalStore = getModalStore();

  function createProject() {
    newProject(modalStore);
  }

  function openExistingProject() {
    openProject();
  }

  function saveCurrentProject() {
    saveProject();
  }
</script>

<div class="flex flex-row space-x-2">
  <div class="btn-group variant-filled h-10 flex">
    <button class="" on:click={openExistingProject}>
      <MdiFolderOpen />
      <span class="ml-2">Open Project</span>
    </button>
    <button class="" on:click={createProject}>
      <MdiPlus />
      <span class="ml-2">New Project</span>
    </button>
  </div>
  <div class="flex space-x-2">
    <button class="btn variant-filled" on:click={saveCurrentProject} disabled={!$isProjectLoaded}>
      <MdiContentSave />
      <span class="ml-2">Save Project</span>
    </button>
    <button class="btn variant-filled" on:click={handleFileImport} disabled={!$isProjectLoaded}>
      <MdiFileImport />
      <span class="ml-2">Import Sentences</span>
    </button>
  </div>
</div>

<style>
  :global(svg) {
    width: 1em;
    height: 1em;
    vertical-align: middle;
  }
</style>
