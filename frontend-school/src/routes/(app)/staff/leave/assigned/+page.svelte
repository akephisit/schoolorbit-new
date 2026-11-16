<script lang="ts">
	import type { PageProps } from './$types';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { leaveSource } from '#lib/staff-leave/route-data.js';
	import AssignedCoverage from '#lib/components/staff-leave/AssignedCoverage.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
	const source = $derived(leaveSource(data.tasks, identity));
</script>

{#key `${identity}|${data.offset}`}<AssignedCoverage {source} offset={data.offset} />{/key}
