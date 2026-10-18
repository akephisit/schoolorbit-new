<script lang="ts">
	import type { PageProps } from './$types';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { leaveSource } from '#lib/staff-leave/route-data.js';
	import LeaveReport from '#lib/components/staff-leave/LeaveReport.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
	const source = $derived(leaveSource(data.report, identity));
</script>

{#key `${identity}|${data.requestKey}`}<LeaveReport {source} query={data.query} />{/key}
