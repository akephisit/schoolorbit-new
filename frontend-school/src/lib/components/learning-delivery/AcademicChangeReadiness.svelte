<script lang="ts">
	import type { LearningDeliveryRefreshScope } from '#lib/academic/learning-delivery-page.js';
	import {
		cancelAcademicTermChangeSet,
		getAcademicTermChangeSet,
		previewAcademicTermChangeSet,
		publishAcademicTermChangeSet,
		type AcademicChangeFinding,
		type AcademicChangeFindingCode,
		type AcademicTermChangeSet,
		type AcademicTermChangeSetPreview
	} from '#lib/api/learning-delivery.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import {
		CheckCircle2,
		CircleAlert,
		Clock3,
		ExternalLink,
		RefreshCw,
		Send,
		TriangleAlert,
		UsersRound
	} from '@lucide/svelte';

	let {
		changeSet,
		canManage,
		onChanged
	}: {
		changeSet: AcademicTermChangeSet;
		canManage: boolean;
		onChanged: (
			changeSet: AcademicTermChangeSet,
			refreshScope?: LearningDeliveryRefreshScope
		) => void | Promise<void>;
	} = $props();

	let preview = $state.raw<AcademicTermChangeSetPreview | null>(null);
	let loadingPreview = $state(false);
	let publishing = $state(false);
	let cancelling = $state(false);
	let acknowledgedWarnings = $state<AcademicChangeFindingCode[]>([]);
	let errorMessage = $state('');

	let blockingFindings = $derived(
		preview?.findings.filter((finding) => finding.severity === 'blocking') ?? []
	);
	const teacherFindingCodes = new Set<AcademicChangeFindingCode>([
		'missing_effective_teacher',
		'missing_primary_teacher'
	]);
	let teacherFindings = $derived(
		blockingFindings.filter((finding) => teacherFindingCodes.has(finding.code))
	);
	let otherBlockingFindings = $derived(
		blockingFindings.filter((finding) => !teacherFindingCodes.has(finding.code))
	);
	let warningFindings = $derived.by(() => {
		return (preview?.findings ?? [])
			.filter((finding) => finding.severity === 'warning')
			.reduce<AcademicChangeFinding[]>((findings, finding) => {
				const existingIndex = findings.findIndex((existing) => existing.code === finding.code);
				if (existingIndex === -1) return [...findings, finding];
				return findings.map((existing, index) =>
					index === existingIndex
						? { ...existing, affectedCount: existing.affectedCount + finding.affectedCount }
						: existing
				);
			}, []);
	});
	let warningsAcknowledged = $derived(
		warningFindings.every((finding) => acknowledgedWarnings.includes(finding.code))
	);
	const impactItems = $derived(
		preview
			? ([
					['กลุ่มเรียน', preview.impactCounts.groups],
					['ห้องประจำชั้น', preview.impactCounts.homerooms],
					['รายชื่อนักเรียน', preview.impactCounts.membershipIntervals],
					['ครูผู้สอน', preview.impactCounts.teacherAssignments],
					['คาบในตารางที่อ้างอิง', preview.impactCounts.targetTimetableEntries],
					['แผนโครงสร้างคะแนน', preview.impactCounts.courseAssessmentPlans],
					['ช่วงคะแนน', preview.impactCounts.courseAssessmentPhases],
					['รายการคะแนนรายกลุ่ม', preview.impactCounts.learningGroupScoreItems],
					['คะแนนนักเรียน', preview.impactCounts.studentScores],
					['การเลือกผลการเรียน', preview.impactCounts.resultSelections],
					['รายการยืนยันผล', preview.impactCounts.resultConfirmations],
					['ผลกิจกรรมที่บันทึก', preview.impactCounts.activityEvaluations],
					['ผลประเมินผู้เรียน', preview.impactCounts.learnerEvaluations],
					['ชุดล็อกผลทางการ', preview.impactCounts.officialResultLocks],
					['ผลการเรียนทางการ', preview.impactCounts.officialResults],
					['ประวัติแก้ผลการเรียน', preview.impactCounts.resultCorrections],
					['ตารางสอบ', preview.impactCounts.examScheduleItems],
					['นิเทศการสอน', preview.impactCounts.supervisionObservations]
				] as const)
			: []
	);

	function formatDate(value: string): string {
		return new Intl.DateTimeFormat('th-TH', { dateStyle: 'medium' }).format(
			new Date(`${value}T00:00:00`)
		);
	}

	function formatDateTime(value: string): string {
		return new Intl.DateTimeFormat('th-TH', {
			dateStyle: 'medium',
			timeStyle: 'short'
		}).format(new Date(value));
	}

	async function syncCurrentChangeSet() {
		const current = await getAcademicTermChangeSet(changeSet.id);
		await onChanged(current);
	}

	async function recoverFromConflict(message: string) {
		preview = null;
		acknowledgedWarnings = [];
		errorMessage = message;
		try {
			await syncCurrentChangeSet();
		} catch (error) {
			errorMessage =
				error instanceof Error
					? `${message} (${error.message})`
					: `${message} และโหลดข้อมูลล่าสุดไม่สำเร็จ`;
		}
	}

	async function refreshPreview() {
		loadingPreview = true;
		preview = null;
		acknowledgedWarnings = [];
		errorMessage = '';
		try {
			const current = await getAcademicTermChangeSet(changeSet.id);
			if (current.rowVersion !== changeSet.rowVersion) {
				await onChanged(current);
				return;
			}
			const loadedPreview = await previewAcademicTermChangeSet(changeSet.id);
			if (loadedPreview.changeSetRowVersion !== current.rowVersion) {
				await syncCurrentChangeSet();
				return;
			}
			preview = loadedPreview;
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await recoverFromConflict('ข้อมูลเปลี่ยนระหว่างตรวจ กรุณาตรวจความพร้อมใหม่อีกครั้ง');
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'ตรวจความพร้อมไม่สำเร็จ';
		} finally {
			loadingPreview = false;
		}
	}

	function setWarningAcknowledgement(code: AcademicChangeFindingCode, checked: boolean) {
		acknowledgedWarnings = checked
			? [...new Set([...acknowledgedWarnings, code])]
			: acknowledgedWarnings.filter((item) => item !== code);
	}

	async function publishChangeSet() {
		if (!canManage || !preview || blockingFindings.length > 0 || !warningsAcknowledged) return;
		publishing = true;
		errorMessage = '';
		try {
			const updated = await publishAcademicTermChangeSet(changeSet.id, {
				rowVersion: preview.changeSetRowVersion,
				targetDeliveryVersionRowVersion: preview.targetDeliveryVersionRowVersion,
				previewHash: preview.previewHash,
				acknowledgedWarningCodes: [...new Set(warningFindings.map((finding) => finding.code))],
				idempotencyKey: crypto.randomUUID()
			});
			await onChanged(updated, 'homerooms');
			preview = null;
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await recoverFromConflict(
					'ข้อมูลเปลี่ยนหลังตรวจความพร้อม กรุณาตรวจความพร้อมใหม่ก่อนเผยแพร่'
				);
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'เผยแพร่การเปลี่ยนแปลงไม่สำเร็จ';
		} finally {
			publishing = false;
		}
	}

	async function cancelDraft() {
		if (!canManage) return;
		cancelling = true;
		errorMessage = '';
		try {
			const updated = await cancelAcademicTermChangeSet(changeSet.id, {
				rowVersion: changeSet.rowVersion
			});
			await onChanged(updated);
			preview = null;
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await recoverFromConflict('แบบร่างถูกแก้ไขจากที่อื่น กรุณาตรวจรายการล่าสุดแล้วลองใหม่');
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'ยกเลิกแบบร่างไม่สำเร็จ';
		} finally {
			cancelling = false;
		}
	}

	function findingRoute(finding: AcademicChangeFinding): string | null {
		return finding.route ?? null;
	}
</script>

<div class="grid min-w-0 gap-5 xl:grid-cols-[minmax(0,1fr)_320px]">
	<div class="min-w-0">
		{#if changeSet.status === 'draft'}
			<section class="space-y-3">
				<div class="flex flex-wrap items-center justify-between gap-3">
					<div>
						<h3 class="font-medium">ตรวจผลกระทบและความพร้อม</h3>
						<p class="text-xs text-muted-foreground">ตรวจจากข้อมูลล่าสุดทุกครั้งก่อนเผยแพร่</p>
					</div>
					<Button variant="outline" size="sm" onclick={refreshPreview} disabled={loadingPreview}>
						<RefreshCw class={loadingPreview ? 'size-4 animate-spin' : 'size-4'} /> ตรวจความพร้อม
					</Button>
				</div>

				{#if preview}
					<div class="grid gap-3 lg:grid-cols-3">
						<div class="rounded-xl border border-sky-500/25 bg-sky-500/5 p-3">
							<p class="flex items-center gap-2 font-medium text-sky-900">
								<UsersRound class="size-4" /> ครูผู้สอน {teacherFindings.length}
							</p>
							<div class="mt-2 space-y-2">
								{#each teacherFindings as finding (`teacher:${finding.code}:${finding.resourceId ?? ''}:${finding.learningGroupId ?? ''}`)}
									<div class="rounded-lg bg-background/80 p-2 text-sm">
										<p class="font-medium">{finding.title}</p>
										<p class="text-xs text-muted-foreground">{finding.guidance}</p>
										{#if findingRoute(finding)}
											<Button
												href={findingRoute(finding) ?? undefined}
												size="sm"
												variant="link"
												class="h-auto px-0 py-1"
											>
												ไปแก้ไข
												<ExternalLink class="size-3" />
											</Button>
										{/if}
									</div>
								{:else}
									<p class="text-sm text-emerald-700">ครูพร้อมตามวันที่เริ่มใช้</p>
								{/each}
							</div>
						</div>
						<div class="rounded-xl border border-destructive/25 bg-destructive/5 p-3">
							<p class="flex items-center gap-2 font-medium text-destructive">
								<CircleAlert class="size-4" /> ข้อมูลเปิดสอน {otherBlockingFindings.length}
							</p>
							<div class="mt-2 space-y-2">
								{#each otherBlockingFindings as finding (`${finding.code}:${finding.resourceId ?? ''}:${finding.learningGroupId ?? ''}`)}
									<div class="rounded-lg bg-background/80 p-2 text-sm">
										<p class="font-medium">{finding.title}</p>
										<p class="text-xs text-muted-foreground">{finding.guidance}</p>
										{#if findingRoute(finding)}
											<Button
												href={findingRoute(finding) ?? undefined}
												size="sm"
												variant="link"
												class="h-auto px-0 py-1"
											>
												ไปแก้ไข <ExternalLink class="size-3" />
											</Button>
										{/if}
									</div>
								{:else}
									<p class="text-sm text-emerald-700">ไม่มีจุดบล็อกการเผยแพร่</p>
								{/each}
							</div>
						</div>
						<div class="rounded-xl border border-amber-500/25 bg-amber-500/5 p-3">
							<p class="flex items-center gap-2 font-medium text-amber-900">
								<TriangleAlert class="size-4" /> คำเตือน {warningFindings.length}
							</p>
							<div class="mt-2 space-y-2">
								{#each warningFindings as finding (finding.code)}
									<label
										class="flex cursor-pointer items-start gap-2 rounded-lg bg-background/80 p-2 text-sm"
									>
										<Checkbox
											checked={acknowledgedWarnings.includes(finding.code)}
											onCheckedChange={(checked) =>
												setWarningAcknowledgement(finding.code, checked)}
											aria-label={`รับทราบ ${finding.title}`}
										/>
										<span>
											<span class="font-medium">{finding.title}</span>
											<span class="block text-xs text-muted-foreground">{finding.guidance}</span>
										</span>
									</label>
								{:else}
									<p class="text-sm text-muted-foreground">ไม่มีคำเตือนที่ต้องรับทราบ</p>
								{/each}
							</div>
						</div>
					</div>
				{:else if loadingPreview}
					<div class="h-32 animate-pulse rounded-xl bg-muted"></div>
				{:else}
					<PageState
						variant="empty"
						title="ยังไม่ได้ตรวจความพร้อม"
						description="บันทึกสิ่งที่ต้องการ แล้วตรวจข้อมูลเปิดสอนก่อนเผยแพร่"
					/>
				{/if}
			</section>
		{/if}
	</div>

	<aside class="space-y-4 xl:sticky xl:top-4 xl:self-start">
		{#if changeSet.status === 'published'}<div class="rounded-xl border p-4">
				<h3 class="font-medium">รุ่นเปิดสอนนี้เผยแพร่แล้ว</h3>
				<p class="mt-1 text-sm text-muted-foreground">
					เริ่มใช้ {formatDate(changeSet.effectiveFrom)}{changeSet.publishedAt
						? ` · เผยแพร่ ${formatDateTime(changeSet.publishedAt)}`
						: ''}
				</p>
				<p class="mt-2 text-sm">
					นำข้อมูลรุ่นนี้ไปจัดตารางสอนได้ ตารางเดิมยังคงข้อมูลของรุ่นที่อ้างอิง
				</p>
			</div>
		{:else if changeSet.status === 'cancelled'}
			<div class="rounded-xl border p-4">
				<h3 class="font-medium">แบบร่างนี้ยกเลิกแล้ว</h3>
				<p class="mt-1 text-sm text-muted-foreground">
					รุ่นเปิดสอนที่เผยแพร่แล้วและตารางสอนเดิมยังใช้งานได้
				</p>
			</div>
		{/if}

		{#if preview}
			<div class="rounded-xl border p-4">
				<h3 class="font-medium">ข้อมูลที่เกี่ยวข้อง</h3>
				<p class="mt-1 text-xs text-muted-foreground">
					จำนวนอ้างอิงเพื่อประเมินผลกระทบ ไม่ได้ลบข้อมูลเดิม
				</p>
				<dl class="mt-3 grid grid-cols-2 gap-x-3 gap-y-2 text-sm">
					{#each impactItems as [label, value] (label)}
						<div class="rounded-lg bg-muted/40 px-2.5 py-2">
							<dt class="text-[11px] text-muted-foreground">{label}</dt>
							<dd class="font-mono font-semibold tabular-nums">{value}</dd>
						</div>
					{/each}
				</dl>
				<p class="mt-3 text-xs leading-relaxed text-muted-foreground">
					ข้อมูลเดิมยังคงอยู่ รวมถึงโครงสร้างคะแนน ผลการเรียน ตารางสอบ และประวัตินิเทศ
				</p>
			</div>
		{/if}

		{#if canManage && changeSet.status === 'draft'}
			<div class="space-y-2 rounded-xl border border-primary/20 bg-primary/[0.025] p-4">
				<div class="flex items-center gap-2">
					{#if preview && blockingFindings.length === 0 && warningsAcknowledged}
						<CheckCircle2 class="size-4 text-emerald-600" />
					{:else}
						<Clock3 class="size-4 text-muted-foreground" />
					{/if}
					<h3 class="font-medium">เผยแพร่รุ่นเปิดสอน</h3>
				</div>
				<p class="text-xs text-muted-foreground">
					ต้องไม่มีจุดบล็อก และรับทราบคำเตือนปัจจุบันทุกข้อ
				</p>
				<LoadingButton
					class="w-full"
					loading={publishing}
					loadingLabel="กำลังเผยแพร่"
					disabled={!preview || blockingFindings.length > 0 || !warningsAcknowledged}
					onclick={publishChangeSet}
				>
					<Send class="size-4" /> เผยแพร่ตั้งแต่ {formatDate(changeSet.effectiveFrom)}
				</LoadingButton>
				<LoadingButton
					class="w-full"
					variant="ghost"
					loading={cancelling}
					loadingLabel="กำลังยกเลิก"
					onclick={cancelDraft}
				>
					ยกเลิกแบบร่าง
				</LoadingButton>
			</div>
		{/if}

		{#if errorMessage}
			<p
				role="alert"
				class="rounded-lg border border-destructive/25 bg-destructive/5 px-3 py-2 text-sm text-destructive"
			>
				{errorMessage}
			</p>
		{/if}
	</aside>
</div>
