<script lang="ts">
	import type { PageProps } from './$types';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { correspondenceSource } from '#lib/correspondence/route-data.js';
	import Templates from '#lib/components/correspondence/Templates.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
	const source = $derived(correspondenceSource(data.templates, identity));
</script>

{#key identity}<Templates {source} />{/key}
