<script lang="ts">
	import AttendanceSelect from '#lib/features/attendance/AttendanceSelect.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import AttendanceReportPanel from '#lib/features/attendance/AttendanceReportPanel.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	function select(key: string, value: string) {
		const url = new URL(page.url.href);
		url.searchParams.set(key, value);
		void goto(url.href);
	}
</script>

<PageShell title={data.title}
	>{#await data.initial}<PageSkeleton variant="table" />{:then result}{#if result.ok}<div
				class="flex flex-wrap gap-3"
			>
				<label
					>ภาคเรียน<AttendanceSelect
						label="ภาคเรียน"
						value={result.data.term ?? ''}
						onValueChange={(value) => select('academicTermId', value)}
						options={result.data.terms.map((t) => ({ value: t.id, label: t.name }))}
					/></label
				>{#if result.data.children.length}<label
						>นักเรียน<AttendanceSelect
							label="นักเรียน"
							value={result.data.studentId ?? ''}
							onValueChange={(value) => select('studentId', value)}
							options={result.data.children.map((c) => ({
								value: c.id,
								label: `${c.first_name} ${c.last_name}`
							}))}
						/></label
					>{/if}
			</div>
			{#if result.data.term && result.data.report && result.data.studentId}{#key result.data.term + result.data.studentId}<AttendanceReportPanel
						term={result.data.term}
						date={data.date}
						initial={result.data.report}
						studentId={result.data.studentId}
					/>{/key}{:else}<PageState
					title="ยังไม่มีข้อมูลเช็คชื่อ"
					description="ยังไม่มีภาคเรียนหรือนักเรียนที่เชื่อมกับบัญชีนี้"
				/>{/if}{:else}<PageState
				variant="error"
				title="โหลดผลเช็คชื่อไม่ได้"
				description={result.error}
			/>{/if}{/await}</PageShell
>
