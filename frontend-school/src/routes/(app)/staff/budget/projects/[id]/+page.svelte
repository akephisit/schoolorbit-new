<script lang="ts">
	import type { PageProps } from './$types';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { budgetSource } from '#lib/budget/route-data.js';
	import ProjectDetail from '#lib/components/budget/ProjectDetail.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
</script>

{#key `${identity}|${data.requestKey}`}<ProjectDetail
		id={data.id}
		source={budgetSource(data.record, identity)}
		catalogsSource={budgetSource(data.catalogs, identity)}
		processSource={budgetSource(data.process, identity)}
		requestsSource={budgetSource(data.requests, identity)}
		query={data.query}
		returnTo={data.returnTo}
	/>{/key}
