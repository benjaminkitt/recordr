<script lang="ts">
  import '../app.css';
  import { computePosition, autoUpdate, offset, shift, flip, arrow } from '@floating-ui/dom';
  import { storePopup } from '@skeletonlabs/skeleton';
  import { initializeStores, Modal, type ModalComponent } from '@skeletonlabs/skeleton';
  import ProjectNameInput from '../components/ProjectNameInput.svelte';

  initializeStores();

  storePopup.set({ computePosition, autoUpdate, offset, shift, flip, arrow });

  const modalRegistry: Record<string, ModalComponent> = {
    // Set a unique modal ID, then pass the component reference
    projectNameInput: { ref: ProjectNameInput },
  };
</script>

<Modal components={modalRegistry} />
<slot></slot>
