<script lang="ts">
  import ProjectManager from '../components/ProjectManager.svelte';
  import SentenceList from '../components/SentenceList.svelte';
</script>

<div class="flex flex-col min-h-screen mx-auto p-4 transition-width">
  <div class="flex-grow"> <div class="card p-4 mb-4">
      <ProjectManager />
    </div>
    <div class="card p-4 flex-grow overflow-y-auto"> 
      <SentenceList />
    </div>
  </div>
</div>
