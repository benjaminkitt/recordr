<script lang="ts">
  import { recentProjects, loadRecentProjects } from '../stores/recentProjectsStore';
  import { openProject, newProject } from '../utils/fileUtils';
  import { getModalStore } from '@skeletonlabs/skeleton';

  const modalStore = getModalStore();

  loadRecentProjects();

  function openRecentProject(path: string) {
    openProject(path);
    modalStore.close();
  }

  function openNewProject() {
    openProject();
    modalStore.close();
  }

  function createNewProject() {
    newProject(modalStore);
  }
</script>

<div class="card p-4">
  <h2 class="h2 mb-4">Recent Projects</h2>
  {#each $recentProjects as project}
    <button
      class="btn variant-ghost w-full text-left mb-2"
      on:click={() => openRecentProject(project)}
    >
      {project}
    </button>
  {/each}
  <div class="flex justify-between mt-4">
    <button class="btn variant-filled" on:click={openNewProject}>Open Other Project</button>
    <button class="btn variant-filled" on:click={createNewProject}>Create New Project</button>
  </div>
</div>
