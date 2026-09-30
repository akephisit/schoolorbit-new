<script lang="ts">
	import type { PageProps } from './$types';
	import { authStore } from '$lib/stores/auth';
	let { data }: PageProps = $props();
	import { PageShell } from '$lib/components/app-layout';
	import { PageState } from '$lib/components/app-state';
	import SchoolFontLibrary from '$lib/components/school-fonts/SchoolFontLibrary.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const canManageFonts = $derived($can.has(PERMISSIONS.FONT_MANAGE_SCHOOL));
</script>

<PageShell
	title="คลังฟอนต์โรงเรียน"
	description="เก็บฟอนต์ที่ผ่านการตรวจไว้ส่วนกลาง เพื่อให้ผู้จัดทำงานของโรงเรียนเลือกใช้ร่วมกัน"
	backHref="/staff/settings"
>
	{#if canManageFonts}
		{#key $authStore.user?.id}<SchoolFontLibrary initialFonts={data.fonts} />{/key}
	{:else}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์จัดการคลังฟอนต์"
			description="บัญชีนี้ยังไม่ได้รับสิทธิ์ดู อัปโหลด หรือลบฟอนต์ส่วนกลางของโรงเรียน"
		/>
	{/if}
</PageShell>
