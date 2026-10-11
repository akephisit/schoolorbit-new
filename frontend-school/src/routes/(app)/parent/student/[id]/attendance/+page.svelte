<script lang="ts">
	import { goto, invalidate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onDestroy } from 'svelte';
	import type { PageProps } from './$types';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import AttendanceReportPanel from '#lib/features/attendance/AttendanceReportPanel.svelte';
	import AttendanceRouteRegion from '#lib/features/attendance/AttendanceRouteRegion.svelte';
	import { attendanceReport } from '#lib/api/attendance.js';
	import { getChildProfile } from '#lib/api/parents.js';
	let { data }: PageProps = $props();
	const identityKey = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const ownerKey = $derived(`${identityKey}|${data.requestKey}`);
	const allowed = $derived(
		$authStore.user?.user_type === 'parent' && $can.has(PERMISSIONS.ATTENDANCE_READ_OWN)
	);
	let disposed = false;
	onDestroy(() => {
		disposed = true;
	});
	$effect(() => {
		const key = ownerKey,
			source = data.context,
			canRead = allowed;
		if (!canRead) return;
		void source.then((result) => {
			if (
				disposed ||
				key !== ownerKey ||
				!allowed ||
				!result.ok ||
				result.data.ownerKey !== key ||
				!result.data.replaceHref
			)
				return;
			const repaired = new URL(result.data.replaceHref);
			void goto(resolve(`parent/student/${data.studentId}/attendance`) + repaired.search, {
				shallow: true,
				replace: true,
				state: page.state
			});
		});
	});
	function select(year: string, term: string) {
		const url = new URL(data.requestHref);
		url.searchParams.set('academicYearId', year);
		if (term) url.searchParams.set('academicTermId', term);
		else url.searchParams.delete('academicTermId');
		void goto(resolve(`parent/student/${data.studentId}/attendance`) + url.search);
	}
</script>

<PageShell
	title="การเช็คชื่อ"
	description="สรุปการเข้าเรียนและประวัติรายวันของนักเรียนคนนี้"
	backHref={`${resolve(`parent/student/${data.studentId}`)}${page.url.search}`}
>
	{#if !allowed}<PageState variant="permission" title="ไม่มีสิทธิ์ดูผลเช็คชื่อ" />
	{:else}
		{#key ownerKey}
			{#await data.context}<PageSkeleton variant="form" />{:then result}
				{#if result.ok && result.data.ownerKey !== ownerKey}<PageSkeleton variant="form" />
				{:else if !result.ok}<PageState
						variant="error"
						title="โหลดประวัติปีและภาคเรียนไม่สำเร็จ"
						description={result.error}
						actionLabel="ลองบริบทอีกครั้ง"
						onaction={() => invalidate('school:attendance-own-context')}
					/>
				{:else}{@const context = result.data}{@const terms =
						context.options?.terms.filter(
							(term) => term.academicYearId === context.academicYearId
						) ?? []}
					<div class="grid gap-4 rounded-xl border bg-card p-4 sm:grid-cols-2">
						<div class="min-w-0 space-y-2">
							<Label for="child-attendance-year">ปีการศึกษา</Label><Select.Root
								type="single"
								value={context.academicYearId}
								onValueChange={(year) =>
									select(
										year,
										context.options?.terms.find(
											(term) =>
												term.academicYearId === year &&
												term.id === context.options.activeAcademicTermId
										)?.id ??
											context.options?.terms.find((term) => term.academicYearId === year)?.id ??
											''
									)}
								><Select.Trigger id="child-attendance-year" class="w-full"
									>{context.options?.years.find((year) => year.id === context.academicYearId)
										?.name ?? 'เลือกปีการศึกษา'}</Select.Trigger
								><Select.Content
									>{#each context.options?.years ?? [] as year (year.id)}<Select.Item
											value={year.id}>{year.name}</Select.Item
										>{/each}</Select.Content
								></Select.Root
							>
						</div>
						<div class="min-w-0 space-y-2">
							<Label for="child-attendance-term">ภาคเรียน</Label><Select.Root
								type="single"
								value={context.academicTermId}
								disabled={!terms.length}
								onValueChange={(term) => select(context.academicYearId, term)}
								><Select.Trigger id="child-attendance-term" class="w-full"
									>{terms.find((term) => term.id === context.academicTermId)?.name ??
										'เลือกภาคเรียน'}</Select.Trigger
								><Select.Content
									>{#each terms as term (term.id)}<Select.Item value={term.id}
											>{term.name}</Select.Item
										>{/each}</Select.Content
								></Select.Root
							>
						</div>
					</div>
					<div data-testid="child-attendance-profile">
						{#if context.academicYearId}<AttendanceRouteRegion
								initial={data.profile}
								variant="detail"
								errorTitle="โหลดข้อมูลนักเรียนไม่สำเร็จ"
								retryLabel="ลองข้อมูลนักเรียนอีกครั้ง"
								retry={(signal) =>
									getChildProfile(data.studentId, context.academicYearId, { signal })}
							>
								{#snippet children(student)}<div
										class="space-y-3 rounded-xl border border-primary/20 bg-primary/5 p-5"
									>
										<h2 class="break-words font-semibold">
											{student.first_name}
											{student.last_name}
										</h2>
										<div class="flex flex-wrap gap-2">
											<Badge variant="secondary"
												>รหัสนักเรียน {student.student_id || 'ไม่ระบุ'}</Badge
											><Badge variant="outline"
												>{student.grade_level || 'ไม่ระบุชั้น'} · ห้อง {student.homeroom ||
													'ไม่ระบุ'}</Badge
											>
										</div>
									</div>{/snippet}
							</AttendanceRouteRegion>{/if}
					</div>
					<div data-testid="child-attendance-report">
						{#if !context.academicTermId}<PageState
								title="ยังไม่มีภาคเรียนสำหรับปีการศึกษาที่เลือก"
							/>
						{:else}<AttendanceRouteRegion
								initial={data.report}
								retry={(signal) =>
									attendanceReport(context.academicTermId, data.studentId, { signal })}
							>
								{#snippet children(report)}<AttendanceReportPanel
										term={context.academicTermId}
										date={data.date}
										initial={report}
										studentId={data.studentId}
									/>{/snippet}
							</AttendanceRouteRegion>{/if}
					</div>
				{/if}
			{/await}
		{/key}
	{/if}
</PageShell>
