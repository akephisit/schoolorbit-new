<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { PERMISSION_MODULES, PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const permissions = $derived($can);

	onMount(() => {
		if (permissions.has(PERMISSIONS.CERTIFICATE_READ_OWN)) {
			void goto(resolve('staff/achievements/issued'), { replace: true });
			return;
		}
		if (permissions.hasModule(PERMISSION_MODULES.ACHIEVEMENT)) {
			void goto(resolve('staff/achievements/self-recorded'), { replace: true });
		}
	});
</script>

<PageShell title={data.title} description="รวมใบที่โรงเรียนออกให้และผลงานที่คุณบันทึกด้วยตนเอง">
	<PageState
		title="กำลังเปิดคลังของคุณ"
		description="ระบบกำลังเลือกหน้าที่บัญชีนี้มีสิทธิ์เข้าถึง"
	/>
</PageShell>
