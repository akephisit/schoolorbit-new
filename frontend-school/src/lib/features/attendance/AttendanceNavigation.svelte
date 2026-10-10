<script lang="ts">
	import { page } from '$app/state';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import { UserCheck, CalendarDays, ChartColumn, Camera } from '@lucide/svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import {
		ATTENDANCE_FACE_PERMISSIONS,
		ATTENDANCE_STAFF_PERMISSIONS
	} from './attendance-access.js';
	let {
		term,
		date,
		current
	}: {
		term: string;
		date: string;
		current: 'workspace' | 'settings' | 'report' | 'faces';
	} = $props();
	function destination(suffix = '') {
		const query = new SvelteURLSearchParams({ academicTermId: term, date });
		const year = page.url.searchParams.get('academicYearId');
		if (year) query.set('academicYearId', year);
		return `/staff/attendance${suffix}?${query}`;
	}
</script>

<nav aria-label="เมนูเช็คชื่อ" class="flex flex-wrap gap-2">
	{#if $can.hasAny(...ATTENDANCE_STAFF_PERMISSIONS)}
		<Button
			href={destination()}
			variant={current === 'workspace' ? 'secondary' : 'outline'}
			aria-current={current === 'workspace' ? 'page' : undefined}
			><UserCheck class="size-4" />เช็คชื่อ</Button
		>
		<Button
			href={destination('/report')}
			variant={current === 'report' ? 'secondary' : 'outline'}
			aria-current={current === 'report' ? 'page' : undefined}
			><ChartColumn class="size-4" />สรุป / ล้างภาคเรียน</Button
		>
	{/if}
	{#if $can.has(PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL)}
		<Button
			href={destination('/settings')}
			variant={current === 'settings' ? 'secondary' : 'outline'}
			aria-current={current === 'settings' ? 'page' : undefined}
			><CalendarDays class="size-4" />ตั้งค่าปฏิทินและรอบพิเศษ</Button
		>
	{/if}
	{#if $can.hasAny(...ATTENDANCE_FACE_PERMISSIONS)}
		<Button
			href={destination('/faces')}
			variant={current === 'faces' ? 'secondary' : 'outline'}
			aria-current={current === 'faces' ? 'page' : undefined}
			><Camera class="size-4" />เว็บแคม / ลงทะเบียนใบหน้า</Button
		>
	{/if}
</nav>
