<script lang="ts">
  import '../app.css';
  import { computePosition, autoUpdate, offset, shift, flip, arrow } from '@floating-ui/dom';
  import {
    initializeStores,
    Modal,
    type ModalComponent,
    getModalStore,
    storePopup,
  } from '@skeletonlabs/skeleton';
  import type { ModalSettings } from '@skeletonlabs/skeleton';
  import { onMount } from 'svelte';
  import ProjectNameInput from '../components/ProjectNameInput.svelte';
  import RecentProjectsModal from '../components/RecentProjectsModal.svelte';

  initializeStores();

  storePopup.set({ computePosition, autoUpdate, offset, shift, flip, arrow });

  const modalStore = getModalStore();

  const modalRegistry: Record<string, ModalComponent> = {
    // Set a unique modal ID, then pass the component reference
    projectNameInput: { ref: ProjectNameInput },
    recentProjectsModal: { ref: RecentProjectsModal },
  };

  const modal: ModalSettings = {
    type: 'component',
    component: 'recentProjectsModal',
    title: 'Recent Projects',
  };

  onMount(() => {
    modalStore.trigger(modal);
  });
</script>

<Modal components={modalRegistry} />
<slot></slot>
