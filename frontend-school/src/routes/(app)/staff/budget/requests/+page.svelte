<script lang="ts">
	import type { PageProps } from './$types';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { budgetSource } from '#lib/budget/route-data.js';
	import RequestList from '#lib/components/budget/RequestList.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import Navigation from '#lib/components/budget/Navigation.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
</script>

{#key `${identity}|${data.requestKey}`}<PageShell title="คำขอใช้เงิน" contentClass="space-y-4"
		><Navigation /><RequestList
			source={budgetSource(data.record, identity)}
			query={data.query}
		/></PageShell
	>{/key}
