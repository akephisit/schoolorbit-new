<script lang="ts">
	import type { PageProps } from './$types';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { correspondenceSource } from '#lib/correspondence/route-data.js';
	import RoutingSettings from '#lib/components/correspondence/RoutingSettings.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
	const source = $derived(correspondenceSource(data.settings, identity));
	const optionsSource = $derived(correspondenceSource(data.options, identity));
</script>

{#key identity}<RoutingSettings {source} {optionsSource} />{/key}
