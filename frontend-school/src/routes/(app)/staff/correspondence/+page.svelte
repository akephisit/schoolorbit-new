<script lang="ts">
	import type { PageProps } from './$types';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { correspondenceSource } from '#lib/correspondence/route-data.js';
	import RegisterList from '#lib/components/correspondence/RegisterList.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
	const source = $derived(correspondenceSource(data.records, identity));
</script>

{#key `${identity}:${JSON.stringify(data.filters)}`}<RegisterList
		{source}
		filters={data.filters}
	/>{/key}
