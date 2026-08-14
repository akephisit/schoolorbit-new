<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageState } from '$lib/components/app-state';
	import { PERMISSION_MODULES, PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const permissions = $derived($can);

	onMount(() => {
		if (permissions.has(PERMISSIONS.CERTIFICATE_READ_OWN)) {
			void goto(resolve('/staff/achievements/issued'), { replaceState: true });
			return;
		}
		if (permissions.hasModule(PERMISSION_MODULES.ACHIEVEMENT)) {
			void goto(resolve('/staff/achievements/self-recorded'), { replaceState: true });
		}
	});
</script>

<PageShell title={data.title} description="รวมใบที่โรงเรียนออกให้และผลงานที่คุณบันทึกด้วยตนเอง">
	<PageState
		title="กำลังเปิดคลังของคุณ"
		description="ระบบกำลังเลือกหน้าที่บัญชีนี้มีสิทธิ์เข้าถึง"
	/>
</PageShell>
