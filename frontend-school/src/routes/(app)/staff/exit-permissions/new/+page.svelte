<script lang="ts">
	import type { PageProps } from './$types';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { leaveSource } from '#lib/staff-leave/route-data.js';
	import RequestWorkflow from '#lib/components/staff-leave/RequestWorkflow.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
	const settingsSource = $derived(leaveSource(data.coverageSettings, identity));
	const optionsSource = $derived(leaveSource(data.options, identity));
</script>

{#key identity}<RequestWorkflow kind={data.kind} {optionsSource} {settingsSource} />{/key}
