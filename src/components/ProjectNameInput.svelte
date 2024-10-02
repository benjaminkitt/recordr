<script lang="ts">
	import type { SvelteComponent } from 'svelte';

	import { ListBox, ListBoxItem, getModalStore } from '@skeletonlabs/skeleton';

  import { createEventDispatcher } from 'svelte';

	// Props
	/** Exposes parent props to this component. */
	export let parent: SvelteComponent;

	// Local
	const modalStore = getModalStore();
  let projectName = '';

	// Handle Form Submission
	function onFormSubmit(): void {
		if ($modalStore[0].response) $modalStore[0].response(projectName);
		modalStore.close();
	}

	// Base Classes
	const cBase = 'card p-4 w-modal shadow-xl space-y-4';
	const cHeader = 'text-2xl font-bold';

  let isValid = false;
  const dispatch = createEventDispatcher();

  $: {
    isValid = /^[a-zA-Z0-9 _-]+$/.test(projectName);
    dispatch('validate', isValid);
  }
</script>

<!-- <div class="space-y-2">
  
</div> -->

{#if $modalStore[0]}
	<div class="modal-example-form {cBase}">
		<header class={cHeader}>{$modalStore[0].title ?? '(title missing)'}</header>
		<article>{$modalStore[0].body ?? '(body missing)'}</article>
		<input
      bind:value={projectName}
      class="input w-full px-4 py-2"
      placeholder="Enter project name"
      maxlength="50"
    />
    <p class="text-sm">
      {#if isValid}
        <span class="text-green-500">✓ Valid project name</span>
      {:else}
        <span class="text-red-500">Use only letters, numbers, spaces, hyphens, and underscores</span>
      {/if}
    </p>
		<!-- prettier-ignore -->
		<footer class="modal-footer {parent.regionFooter}">
        <button class="btn {parent.buttonNeutral}" on:click={parent.onClose}>{parent.buttonTextCancel}</button>
        <button class="btn {parent.buttonPositive}" on:click={onFormSubmit}>Create Project</button>
    </footer>
	</div>
{/if}