<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { attendanceDetail, attendanceWorkspace } from '#lib/api/attendance.js';
	import AttendanceRouteRegion from '#lib/features/attendance/AttendanceRouteRegion.svelte';
	import AttendanceWorkspace from '#lib/features/attendance/AttendanceWorkspace.svelte';
	import { attendanceIdentity } from '#lib/features/attendance/attendance-access.js';
	let { data }: PageProps = $props();
</script>

<PageShell
	title="เช็กชื่อต้นทาง"
	description="แก้ผลที่ระบบเช็กชื่อ คะแนนพฤติกรรมจะปรับตามผลที่บันทึก"
	>{#key $attendanceIdentity + data.id}<AttendanceRouteRegion
			initial={data.initial}
			retry={async (signal) => {
				const detail = await attendanceDetail(data.id, { signal });
				const workspace = await attendanceWorkspace(
					detail.session.academicTermId,
					detail.session.date,
					{ signal }
				);
				return { detail, workspace };
			}}
			>{#snippet children(result)}<AttendanceWorkspace
					term={result.detail.session.academicTermId}
					date={result.detail.session.date}
					initial={result.workspace}
					initialDetail={result.detail}
				/>{/snippet}</AttendanceRouteRegion
		>{/key}</PageShell
>
