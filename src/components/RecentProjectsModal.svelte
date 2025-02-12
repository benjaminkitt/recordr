<script lang="ts">
  import type { RecentProject } from '../stores/recentProjectsStore';
  import { recentProjectsData, loadRecentProjects } from '../stores/recentProjectsStore';
  import { openProject, newProject } from '../utils/fileUtils';
  import { getModalStore } from '@skeletonlabs/skeleton';
  import MdiFolderOpen from '~icons/mdi/folder-open';
  import MdiPlus from '~icons/mdi/plus';

  const modalStore = getModalStore();

  loadRecentProjects();

  function openRecentProject(project: RecentProject) {
    openProject(project.path);
    modalStore.close();
  }

  function openNewProject() {
    openProject();
    modalStore.close();
  }

  function createNewProject() {
    modalStore.close();
    newProject(modalStore);
  }
</script>

<div class="card p-4 rounded-lg shadow-lg max-w-md mx-auto">
  <!-- Modal Header -->
  <header class="flex justify-between items-center border-b pb-2 mb-4">
    <h2 class="text-xl font-bold">Recent Projects</h2>
  </header>

  <!-- Modal Body -->
  <div class="max-h-60 overflow-y-auto">
    <nav class="list-nav">
      <ul>
        {#each $recentProjectsData.recent_projects as project}
          <li>
            <!-- svelte-ignore a11y-click-events-have-key-events -->
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <!-- svelte-ignore a11y-missing-attribute -->
            <a
              class="flex items-center p-2 w-full text-left cursor-pointer"
              on:click={() => openRecentProject(project)}
            >
              <span class="badge-icon p-5 variant-soft-tertiary"><i><MdiFolderOpen /></i></span>
              <div class="flex-auto">
                <p class="font-medium">{project.name}</p>
                <p class="text-xs text-gray-500">
                  Last opened: {project.last_accessed}
                </p>
              </div>
            </a>
          </li>
        {/each}
      </ul>
    </nav>
  </div>

  <!-- Modal Footer -->
  <footer class="flex justify-end pt-4">
    <div class="btn-group variant-filled">
      <button class="" on:click={createNewProject}
        ><MdiPlus />
        <span class="ml-2">Create New Project</span></button
      >
      <button class="" on:click={openNewProject}
        ><MdiFolderOpen />
        <span class="ml-2">Browse...</span></button
      >
    </div>
  </footer>
</div>
