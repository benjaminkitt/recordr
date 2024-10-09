<script lang="ts">
  import ProjectManager from '../components/ProjectManager.svelte';
  import SentenceList from '../components/SentenceList.svelte';
</script>

<div class="flex flex-col h-screen p-4">
  <div class="card p-4 mb-2">
    <ProjectManager />
  </div>
  <div class="flex-grow overflow-hidden">
    <div class="card p-4 h-full overflow-hidden flex flex-col">
      <SentenceList />
    </div>
  </div>
</div>
