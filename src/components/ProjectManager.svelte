<script lang="ts">
  import { openProject, saveProject, newProject, handleFileImport } from "../utils/fileUtils";
  import { projectName, isProjectLoaded } from "../stores/projectStore";
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

<div class="flex space-x-2">
  <button class="btn variant-filled" on:click={openExistingProject}>
    <MdiFolderOpen />
    <span class="ml-2">Open Project</span>
  </button>
  <button class="btn variant-filled" on:click={saveCurrentProject}>
    <MdiContentSave />
    <span class="ml-2">Save Project</span>
  </button>
  <button class="btn variant-filled" on:click={handleFileImport}>
    <MdiFileImport />
    <span class="ml-2">Import Sentences</span>
  </button>
  <button class="btn variant-filled" on:click={createProject}>
    <MdiPlus />
    <span class="ml-2">New Project</span>
  </button>
</div>

<style>
  :global(svg) {
    width: 1em;
    height: 1em;
    vertical-align: middle;
  }
</style>