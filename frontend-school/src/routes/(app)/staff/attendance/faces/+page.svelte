<script lang="ts">
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { attendanceOptions } from '#lib/api/attendance.js';
	import AttendanceRouteRegion from '#lib/features/attendance/AttendanceRouteRegion.svelte';
	import FaceStation from '#lib/features/attendance/FaceStation.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
</script>

<PageShell title={data.title}>
	{#if !data.term}<PageState title="เลือกภาคเรียน" description="กรุณาเลือกภาคเรียนจากแถบด้านบน" />
	{:else if data.initial}{#key data.term + data.date}<AttendanceRouteRegion
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
