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

<nav aria-label="เมนูเช็คชื่อ" class="flex gap-2 overflow-x-auto pb-1">
	{#if $can.hasAny(...ATTENDANCE_STAFF_PERMISSIONS)}
		<Button
			href={destination()}
			variant={current === 'workspace' ? 'secondary' : 'outline'}
			aria-current={current === 'workspace' ? 'page' : undefined}
			aria-label="เช็คชื่อ"
			><UserCheck class="size-4" /><span class="sm:hidden" aria-hidden="true">เช็คชื่อ</span><span
				class="hidden sm:inline">เช็คชื่อ</span
			></Button
		>
		<Button
			href={destination('/report')}
			variant={current === 'report' ? 'secondary' : 'outline'}
			aria-current={current === 'report' ? 'page' : undefined}
			aria-label="สรุป / ล้างภาคเรียน"
			><ChartColumn class="size-4" /><span class="sm:hidden" aria-hidden="true">สรุป</span><span
				class="hidden sm:inline">สรุป / ล้างภาคเรียน</span
			></Button
		>
	{/if}
	{#if $can.has(PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL)}
		<Button
			href={destination('/settings')}
			variant={current === 'settings' ? 'secondary' : 'outline'}
			aria-current={current === 'settings' ? 'page' : undefined}
			aria-label="ตั้งค่าปฏิทินและรอบพิเศษ"
			><CalendarDays class="size-4" /><span class="sm:hidden" aria-hidden="true">ตั้งค่า</span><span
				class="hidden sm:inline">ตั้งค่าปฏิทินและรอบพิเศษ</span
			></Button
		>
	{/if}
	{#if $can.hasAny(...ATTENDANCE_FACE_PERMISSIONS)}
		<Button
			href={destination('/faces')}
			variant={current === 'faces' ? 'secondary' : 'outline'}
			aria-current={current === 'faces' ? 'page' : undefined}
			aria-label="เว็บแคม / ลงทะเบียนใบหน้า"
			><Camera class="size-4" /><span class="sm:hidden" aria-hidden="true">ใบหน้า</span><span
				class="hidden sm:inline">เว็บแคม / ลงทะเบียนใบหน้า</span
			></Button
		>
	{/if}
</nav>
