<script lang="ts">
	import { ChartColumn } from '@lucide/svelte';
	import AttendanceNavigation from '#lib/features/attendance/AttendanceNavigation.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { attendanceReport } from '#lib/api/attendance.js';
	import AttendanceRouteRegion from '#lib/features/attendance/AttendanceRouteRegion.svelte';
	import AttendanceReportPanel from '#lib/features/attendance/AttendanceReportPanel.svelte';
	import { attendanceIdentity } from '#lib/features/attendance/attendance-access.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
</script>

<PageShell
	title={data.title}
	description="ดูผลรวมและรายละเอียดรายวันของภาคเรียนที่เลือก"
	icon={ChartColumn}
>
	{#if data.term}<AttendanceNavigation term={data.term} date={data.date} current="report" />{/if}
	{#if !data.term}<PageState title="เลือกภาคเรียน" description="กรุณาเลือกภาคเรียนจากแถบด้านบน" />
	{:else if data.initial}{#key $attendanceIdentity + data.term + data.date}<AttendanceRouteRegion
				initial={data.initial}
				retry={(signal) => attendanceReport(data.term!, undefined, { signal })}
				variant="table"
			>
				{#snippet children(result)}<AttendanceReportPanel
						term={data.term!}
						date={data.date}
						initial={result}
					/>{/snippet}
			</AttendanceRouteRegion>{/key}{/if}
</PageShell>
