<script lang="ts">
  import ProjectManager from '../components/ProjectManager.svelte';
  import SentenceList from '../components/SentenceList.svelte';
</script>

<div class="container mx-auto p-4">
  <div class="card p-4 mb-4">
    <ProjectManager />
  </div>
  <div class="card p-4">
    <SentenceList />
  </div>
</div>