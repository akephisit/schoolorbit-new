<script lang="ts">
	import { Camera } from '@lucide/svelte';
	import AttendanceNavigation from '#lib/features/attendance/AttendanceNavigation.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { attendanceOptions } from '#lib/api/attendance.js';
	import AttendanceRouteRegion from '#lib/features/attendance/AttendanceRouteRegion.svelte';
	import FaceStation from '#lib/features/attendance/FaceStation.svelte';
	import { attendanceIdentity } from '#lib/features/attendance/attendance-access.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
</script>

<PageShell
	title={data.title}
	description="เช็คชื่อด้วยเว็บแคมและจัดการใบหน้าที่ได้รับความยินยอม"
	icon={Camera}
>
	{#if data.term}<AttendanceNavigation term={data.term} date={data.date} current="faces" />{/if}
	{#if !data.term}<PageState title="เลือกภาคเรียน" description="กรุณาเลือกภาคเรียนจากแถบด้านบน" />
	{:else if data.initial}{#key $attendanceIdentity + data.term + data.date}<AttendanceRouteRegion
				initial={data.initial}
				retry={(signal) => attendanceOptions(data.term!, { signal })}
				variant="detail"
			>
				{#snippet children(result)}<FaceStation
						term={data.term!}
						date={data.date}
						initial={result}
					/>{/snippet}
			</AttendanceRouteRegion>{/key}{/if}
</PageShell>
