<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import {
		attendanceSettings,
		attendanceOptions,
		type AttendanceSettings,
		type AttendanceDay,
		type AttendanceOptions
	} from '#lib/api/attendance.js';
	import { attendanceIdentity, type AttendanceRead } from './attendance-access.js';
	import type { AttendanceCalendarEvents } from './attendance-settings-calendar.js';
	import AttendanceRouteRegion from './AttendanceRouteRegion.svelte';
	import AttendanceSettingsPanel from './AttendanceSettingsPanel.svelte';
	import AttendanceManagementPanel from './AttendanceManagementPanel.svelte';
	let {
		term,
		date,
		initialSettings,
		initialOptions,
		initialDays,
		initialCalendar
	}: {
		term: string;
		date: string;
		initialSettings: Promise<RouteLoadResult<AttendanceRead<AttendanceSettings>>>;
		initialOptions: Promise<RouteLoadResult<AttendanceRead<AttendanceOptions>>>;
		initialDays: Promise<RouteLoadResult<AttendanceRead<AttendanceDay[]>>>;
		initialCalendar: Promise<RouteLoadResult<AttendanceRead<AttendanceCalendarEvents>>>;
	} = $props();
	let archived = $state(true),
		disposed = false;
	onDestroy(() => {
		disposed = true;
	});
	$effect.pre(() => {
		const source = initialSettings,
			identityKey = $attendanceIdentity;
		archived = true;
		void source.then((result) => {
			if (!disposed && source === initialSettings && identityKey === $attendanceIdentity)
				archived =
					!result.ok ||
					result.data.identityKey !== identityKey ||
					!result.data.resource ||
					result.data.resource.archived;
		});
	});
	async function retrySettings(signal: AbortSignal): Promise<AttendanceSettings> {
		const identityKey = $attendanceIdentity;
		const result = await attendanceSettings(term, { signal });
		if (!disposed && identityKey === $attendanceIdentity) archived = result.archived;
		return result;
	}
</script>

<section aria-label="เกณฑ์และปฏิทินเช็คชื่อ" class="min-w-0 space-y-6">
	<AttendanceRouteRegion
		initial={initialSettings}
		retry={retrySettings}
		variant="form"
		errorTitle="โหลดการตั้งค่าไม่ได้"
		retryLabel="ลองโหลดการตั้งค่าใหม่"
	>
		{#snippet children(result)}<AttendanceSettingsPanel
				{term}
				{date}
				initial={result}
				{initialDays}
				{initialCalendar}
			/>{/snippet}
	</AttendanceRouteRegion>
</section>
<section aria-label="กลุ่ม รอบพิเศษ และเครื่องสแกน" class="min-w-0 space-y-6">
	<AttendanceRouteRegion
		initial={initialOptions}
		errorTitle="โหลดกลุ่มและเครื่องสแกนไม่ได้"
		retryLabel="ลองโหลดกลุ่มและเครื่องสแกนใหม่"
		retry={(signal) => attendanceOptions(term, { signal })}
		variant="form"
	>
		{#snippet children(result)}<AttendanceManagementPanel
				{term}
				{date}
				{archived}
				initial={result}
			/>{/snippet}
	</AttendanceRouteRegion>
</section>
