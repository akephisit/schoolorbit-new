<script lang="ts">
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { attendanceReport } from '#lib/api/attendance.js';
	import AttendanceRouteRegion from '#lib/features/attendance/AttendanceRouteRegion.svelte';
	import AttendanceReportPanel from '#lib/features/attendance/AttendanceReportPanel.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
</script>

<PageShell title={data.title}>
	{#if !data.term}<PageState title="เลือกภาคเรียน" description="กรุณาเลือกภาคเรียนจากแถบด้านบน" />
	{:else if data.initial}{#key data.term + data.date}<AttendanceRouteRegion
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
