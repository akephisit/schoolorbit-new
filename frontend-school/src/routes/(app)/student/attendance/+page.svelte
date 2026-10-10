<script lang="ts">
	import AttendanceSelect from '#lib/features/attendance/AttendanceSelect.svelte';
	import { goto, invalidate } from '$app/navigation';
	import { page } from '$app/state';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import AttendanceReportPanel from '#lib/features/attendance/AttendanceReportPanel.svelte';
	import { attendanceIdentity } from '#lib/features/attendance/attendance-access.js';
	import { attendanceReport } from '#lib/api/attendance.js';
	import AttendanceRouteRegion from '#lib/features/attendance/AttendanceRouteRegion.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	function select(key: string, value: string) {
		const url = new URL(page.url.href);
		url.searchParams.set(key, value);
		void goto(url.href);
	}
</script>

<PageShell title={data.title}
	>{#await data.initial}<PageSkeleton
			variant="table"
		/>{:then result}{#if result.ok && result.data.identityKey !== $attendanceIdentity}<PageSkeleton
				variant="table"
			/>{:else if result.ok && result.data.error}<PageState
				variant="error"
				title="โหลดผลเช็คชื่อไม่ได้"
				description={result.data.error}
				actionLabel="ลองโหลดข้อมูลใหม่"
				onaction={() => invalidate('school:attendance-own-context')}
			/>
		{:else if result.ok && result.data.resource}{@const context = result.data.resource}
			<div class="flex flex-wrap gap-3">
				<label
					>ภาคเรียน<AttendanceSelect
						label="ภาคเรียน"
						value={context.term ?? ''}
						onValueChange={(value) => select('academicTermId', value)}
						options={context.terms.map((t) => ({ value: t.id, label: t.name }))}
					/></label
				>{#if context.children.length}<label
						>นักเรียน<AttendanceSelect
							label="นักเรียน"
							value={context.studentId ?? ''}
							onValueChange={(value) => select('studentId', value)}
							options={context.children.map((c) => ({
								value: c.id,
								label: `${c.first_name} ${c.last_name}`
							}))}
						/></label
					>{/if}
			</div>
			{#if context.term && context.report && context.studentId}{#key $attendanceIdentity + context.term + context.studentId}<AttendanceRouteRegion
						initial={context.report}
						retry={(signal) => attendanceReport(context.term!, context.studentId!, { signal })}
					>
						{#snippet children(report)}<AttendanceReportPanel
								term={context.term!}
								date={data.date}
								initial={report}
								studentId={context.studentId!}
							/>{/snippet}
					</AttendanceRouteRegion>{/key}{:else}<PageState
					title="ยังไม่มีข้อมูลเช็คชื่อ"
					description="ยังไม่มีภาคเรียนหรือนักเรียนที่เชื่อมกับบัญชีนี้"
				/>{/if}{:else if !result.ok}<PageState
				variant="error"
				title="โหลดผลเช็คชื่อไม่ได้"
				description={result.error}
				actionLabel="ลองโหลดข้อมูลใหม่"
				onaction={() => invalidate('school:attendance-own-context')}
			/>{/if}{/await}</PageShell
>
