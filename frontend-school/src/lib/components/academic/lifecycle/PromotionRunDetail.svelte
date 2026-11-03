<script lang="ts">
	import { untrack } from 'svelte';
	import { resolve } from '$app/paths';
	import { getPromotionRun, type PromotionRunWorkspace } from '#lib/api/academic-promotion.js';
	import {
		outcomeLabels,
		runStatusLabels
	} from '#lib/academic/lifecycle/promotion-presentation.js';
	import { academicStatusLabel } from '#lib/academic/lifecycle/year-management.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	let {
		runId,
		workspaceLoad
	}: { runId: string; workspaceLoad: Promise<RouteLoadResult<PromotionRunWorkspace | null>> } =
		$props();
	let workspace = $state.raw<PromotionRunWorkspace | null>(null);
	let loading = $state(true);
	let error = $state('');
	const latest = new LatestRequest();
	const canRead = $derived($can.has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL));
	async function retry() {
		if (!canRead) return;
		const { revision, signal } = latest.begin();
		const identity = appIdentityKey();
		loading = true;
		error = '';
		try {
			const result = await getPromotionRun(runId, { signal });
			if (latest.isCurrent(revision) && identity === appIdentityKey()) workspace = result;
		} catch (cause) {
			if (!isAbortError(cause) && latest.isCurrent(revision))
				error = cause instanceof Error ? cause.message : 'โหลดประวัติไม่สำเร็จ';
		} finally {
			if (latest.isCurrent(revision)) loading = false;
		}
	}
	$effect.pre(() => {
		const promise = workspaceLoad;
		const allowed = canRead;
		const { revision } = latest.begin();
		untrack(() => {
			workspace = null;
			loading = allowed;
			error = '';
		});
		if (allowed)
			void promise.then((result) => {
				if (!latest.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) workspace = result.data;
					else error = result.error;
					loading = false;
				});
			});
		return () => latest.abort();
	});
</script>

<PageShell
	title="ประวัติขึ้นปีการศึกษาใหม่"
	description="หลักฐานรายชื่อและผลที่บันทึกไว้ เก็บข้อมูลเดิมเพื่อการตรวจสอบ"
>
	<Button
		variant="outline"
		href={`${resolve('staff/academic/year-lifecycle')}${workspace ? `?academicYearId=${workspace.sourceYear.academicYearId}` : ''}`}
		>กลับหน้าจัดการปีการศึกษา</Button
	>
	{#if !canRead}<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูประวัติ"
		/>{:else if loading && !workspace}<PageSkeleton
			variant="table"
			rows={4}
			columns={3}
		/>{:else if error}<PageState
			variant="error"
			title="โหลดประวัติไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={retry}
		/>{/if}
	{#if workspace}<section class="space-y-2 rounded-xl border bg-card p-4">
			<h2 class="text-lg font-semibold">
				{workspace.sourceYear.name} → {workspace.targetYear.name}
			</h2>
			<p>{runStatusLabels[workspace.run.status]}</p>
			<p class="text-sm text-muted-foreground">
				ปีปลายทาง: {academicStatusLabel(workspace.targetYear.status)}
			</p>
			{#if workspace.run.policyId}<p class="text-sm text-muted-foreground">
					รายการเดิมเก็บหลักฐานเกณฑ์และข้อเสนอที่ใช้ ณ เวลาบันทึกไว้
				</p>{/if}
		</section>
		<div class="overflow-x-auto rounded-xl border">
			<Table.Root
				><Table.Header
					><Table.Row
						><Table.Head>นักเรียน</Table.Head><Table.Head>ผลที่บันทึก</Table.Head><Table.Head
							>หลักฐาน</Table.Head
						></Table.Row
					></Table.Header
				><Table.Body
					>{#each workspace.students as student (student.item.id)}<Table.Row
							><Table.Cell
								>{student.studentName}
								<p class="text-sm text-muted-foreground">{student.studentCode ?? ''}</p></Table.Cell
							><Table.Cell
								>{student.item.decision
									? outcomeLabels[student.item.decision.outcome]
									: 'ยังไม่มีผลที่ยืนยัน'}{#if student.item.decision?.condition}<p
										class="text-sm text-muted-foreground"
									>
										เงื่อนไขที่บันทึก: {student.item.decision.condition}
									</p>{/if}{#if student.item.decision?.reason}<p
										class="text-sm text-muted-foreground"
									>
										{student.item.decision.reason}
									</p>{/if}</Table.Cell
							><Table.Cell
								>{student.receipt
									? new Date(student.receipt.executedAt).toLocaleString('th-TH', {
											timeZone: 'Asia/Bangkok'
										})
									: 'ยังไม่มีหลักฐานดำเนินการ'}</Table.Cell
							></Table.Row
						>{/each}</Table.Body
				></Table.Root
			>
		</div>{/if}
</PageShell>
