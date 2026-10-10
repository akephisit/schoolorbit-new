<script lang="ts">
	import { CalendarDays } from '@lucide/svelte';
	import AttendanceNavigation from '#lib/features/attendance/AttendanceNavigation.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import AttendanceSettingsRegions from '#lib/features/attendance/AttendanceSettingsRegions.svelte';
	import { attendanceIdentity } from '#lib/features/attendance/attendance-access.js';
	import type { PageProps } from './$types';
	import { PageState } from '#lib/components/app-state/index.js';
	let { data }: PageProps = $props();
</script>

<PageShell
	title={data.title}
	description="กำหนดวันประมวลผล การแจ้งเตือน รอบพิเศษ และเครื่องเว็บแคม"
	icon={CalendarDays}
>
	{#if data.term}<AttendanceNavigation term={data.term} date={data.date} current="settings" />{/if}

	{#if !data.term}<PageState title="เลือกภาคเรียน" description="กรุณาเลือกภาคเรียนจากแถบด้านบน" />
	{:else if data.settings && data.options}{#key $attendanceIdentity + data.term + data.date}<AttendanceSettingsRegions
				term={data.term}
				date={data.date}
				initialSettings={data.settings}
				initialOptions={data.options}
			/>{/key}{/if}
</PageShell>
