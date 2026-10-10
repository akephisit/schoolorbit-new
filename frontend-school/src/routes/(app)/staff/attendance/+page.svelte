<script lang="ts">
	import { UserCheck } from '@lucide/svelte';
	import AttendanceNavigation from '#lib/features/attendance/AttendanceNavigation.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { attendanceWorkspace } from '#lib/api/attendance.js';
	import AttendanceRouteRegion from '#lib/features/attendance/AttendanceRouteRegion.svelte';
	import AttendanceWorkspace from '#lib/features/attendance/AttendanceWorkspace.svelte';
	import { attendanceIdentity } from '#lib/features/attendance/attendance-access.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
</script>

<PageShell
	title={data.title}
	description="เลือกวันที่และรอบเช็คชื่อ บันทึกผลและติดตามนักเรียนที่ได้รับมอบหมาย"
	icon={UserCheck}
>
	{#if data.term}<AttendanceNavigation term={data.term} date={data.date} current="workspace" />{/if}
	{#if !data.term}<PageState title="เลือกภาคเรียน" description="กรุณาเลือกภาคเรียนจากแถบด้านบน" />
	{:else if data.initial}{#key $attendanceIdentity + data.term + data.date}<AttendanceRouteRegion
				initial={data.initial}
				retry={(signal) => attendanceWorkspace(data.term!, data.date, { signal })}
				variant="table"
			>
				{#snippet children(result)}<AttendanceWorkspace
						term={data.term!}
						date={data.date}
						initial={result}
					/>{/snippet}
			</AttendanceRouteRegion>{/key}{/if}
</PageShell>
