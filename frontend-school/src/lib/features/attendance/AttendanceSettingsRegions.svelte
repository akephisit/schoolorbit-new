<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import {
		attendanceSettings,
		attendanceDays,
		attendanceOptions,
		type AttendanceSettings,
		type AttendanceDay,
		type AttendanceOptions
	} from '#lib/api/attendance.js';
	import { attendanceIdentity, type AttendanceRead } from './attendance-access.js';
	import AttendanceRouteRegion from './AttendanceRouteRegion.svelte';
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
		initialSettings: Promise<
			RouteLoadResult<AttendanceRead<[AttendanceSettings, AttendanceDay[]]>>
		>;
		initialOptions: Promise<RouteLoadResult<AttendanceRead<AttendanceOptions>>>;
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
					result.data.resource[0].archived;
		});
	});
	async function retrySettings(
		signal: AbortSignal
	): Promise<[AttendanceSettings, AttendanceDay[]]> {
		const identityKey = $attendanceIdentity;
		const end = new Date(Date.UTC(Number(date.slice(0, 4)), Number(date.slice(5, 7)), 0))
			.toISOString()
			.slice(0, 10);
		const result = await Promise.all([
			attendanceSettings(term, { signal }),
			attendanceDays(term, date.slice(0, 7) + '-01', end, { signal })
		]);
		if (!disposed && identityKey === $attendanceIdentity) archived = result[0].archived;
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
		{#snippet children(result)}<AttendanceSettingsPanel {term} {date} initial={result} />{/snippet}
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
