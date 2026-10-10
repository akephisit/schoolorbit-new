<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import {
		attendanceSettings,
		attendanceDays,
		attendanceOptions,
		type AttendanceSettings,
		type AttendanceDay,
		type AttendanceOptions
	} from '#lib/api/attendance.js';
	import AttendanceSettingsPanel from './AttendanceSettingsPanel.svelte';
	import AttendanceManagementPanel from './AttendanceManagementPanel.svelte';
	let {
		term,
		date,
		initialSettings,
		initialOptions
	}: {
		term: string;
		date: string;
		initialSettings: Promise<RouteLoadResult<[AttendanceSettings, AttendanceDay[]] | null>>;
		initialOptions: Promise<RouteLoadResult<AttendanceOptions | null>>;
	} = $props();
	let settings = $state.raw(untrack(() => initialSettings)),
		options = $state.raw(untrack(() => initialOptions));
	let archived = $state(true),
		disposed = false;
	const settingsController = new AbortController(),
		optionsController = new AbortController();
	$effect(() => {
		const source = settings;
		void source.then((result) => {
			if (!disposed && source === settings)
				archived = !result.ok || !result.data || result.data[0].archived;
		});
	});
	onDestroy(() => {
		disposed = true;
		settingsController.abort();
		optionsController.abort();
	});
	function retrySettings() {
		const end = new Date(Date.UTC(Number(date.slice(0, 4)), Number(date.slice(5, 7)), 0))
			.toISOString()
			.slice(0, 10);
		settings = captureRouteLoad(
			Promise.all([
				attendanceSettings(term, { signal: settingsController.signal }),
				attendanceDays(term, date.slice(0, 7) + '-01', end, { signal: settingsController.signal })
			]),
			'โหลดการตั้งค่าไม่ได้'
		);
	}
	function retryOptions() {
		options = captureRouteLoad(
			attendanceOptions(term, { signal: optionsController.signal }),
			'โหลดกลุ่มและเครื่องสแกนไม่ได้'
		);
	}
</script>

<section aria-label="เกณฑ์และปฏิทินเช็คชื่อ" class="space-y-4">
	{#await settings}<PageSkeleton variant="form" rows={4} />{:then result}
		{#if result.ok && result.data}<AttendanceSettingsPanel {term} {date} initial={result.data} />
		{:else}<PageState
				variant="error"
				title="โหลดการตั้งค่าไม่ได้"
				description={result.error ?? undefined}
				actionLabel="ลองโหลดการตั้งค่าใหม่"
				onaction={retrySettings}
			/>{/if}
	{/await}
</section>
<section aria-label="กลุ่ม รอบพิเศษ และเครื่องสแกน" class="space-y-4">
	{#await options}<PageSkeleton variant="form" rows={3} />{:then result}
		{#if result.ok && result.data}<AttendanceManagementPanel
				{term}
				{date}
				{archived}
				initial={result.data}
			/>
		{:else}<PageState
				variant="error"
				title="โหลดกลุ่มและเครื่องสแกนไม่ได้"
				description={result.error ?? undefined}
				actionLabel="ลองโหลดกลุ่มและเครื่องสแกนใหม่"
				onaction={retryOptions}
			/>{/if}
	{/await}
</section>
