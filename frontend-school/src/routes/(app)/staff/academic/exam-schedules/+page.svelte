<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { toast } from 'svelte-sonner';
	import type { PageProps } from './$types';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import {
		createExamRound,
		deleteExamRound,
		listExamRounds,
		type CreateExamRoundInput,
		type ExamRound,
		type ExamRoundKind
	} from '$lib/api/examSchedule';
	import { PageSkeleton, PageState, RegionUpdatingState } from '$lib/components/app-state';
	import { PageShell } from '$lib/components/app-layout';
	import {
		AcademicPrerequisiteNotice,
		type AcademicPrerequisite
	} from '$lib/components/academic-workflow';
	import ExamRoundDialog from '$lib/components/academic/exam-schedule/ExamRoundDialog.svelte';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { CalendarClock, Plus, RefreshCw, Trash2 } from '@lucide/svelte';

	let { data }: PageProps = $props();

	const academicContext = getAcademicContextStore();
	const academicTermId = $derived(data.academicTermId);
	const selectedTermLabel = $derived(
		$academicContext.options?.terms.find((term) => term.id === academicTermId)?.name ??
			'ภาคเรียนที่เลือก'
	);
	let roundsLoading = $state(true);
	let roundsLoaded = $state(false);
	let error = $state('');
	let rounds = $state<ExamRound[]>([]);
	const roundsRequest = new LatestRequest();
	let activeContextKey = '';
	let roundsRevision = 0;
	onDestroy(() => roundsRequest.abort());
	let createDialogOpen = $state(false);
	let creatingRound = $state(false);
	let deleteDialogOpen = $state(false);
	let deleteTarget = $state<ExamRound | null>(null);
	let deletingRoundId = $state<string | null>(null);
	const assessmentPrerequisite: AcademicPrerequisite = {
		key: 'exam-schedule-assessments',
		status: 'warning',
		title: 'เตรียมโครงสร้างคะแนนสำหรับรอบสอบ',
		description:
			'สร้างรอบสอบไว้ก่อนได้ เมื่อโครงสร้างคะแนนพร้อมและช่วงกลางภาคหรือปลายภาคเลือกสอบในตาราง จึงค่อยนำเข้ารายการสอบในรอบนั้น',
		actionLabel: 'ตรวจโครงสร้างคะแนน',
		href: '/staff/academic/assessments'
	};

	const canManageExamSchedules = $derived(
		$can.has(PERMISSIONS.ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)
	);
	const canPublishExamSchedules = $derived(
		$can.has(PERMISSIONS.ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL)
	);

	function canDeleteExamRound(round: ExamRound): boolean {
		return canManageExamSchedules && (round.status === 'draft' || canPublishExamSchedules);
	}

	function requestDeleteRound(round: ExamRound) {
		if (!canDeleteExamRound(round) || deletingRoundId) return;
		deleteTarget = round;
		deleteDialogOpen = true;
	}

	async function confirmDeleteRound() {
		const target = deleteTarget;
		if (!target || deletingRoundId || !canDeleteExamRound(target)) return;

		deletingRoundId = target.id;
		try {
			await deleteExamRound(target.id);
			if (target.academicTermId === academicTermId) {
				roundsRevision += 1;
				roundsRequest.abort();
				roundsLoading = false;
				rounds = rounds.filter((round) => round.id !== target.id);
			}
			toast.success(`ลบรอบสอบ “${target.name}” แล้ว`);
			deleteDialogOpen = false;
			deleteTarget = null;
		} catch (deleteError) {
			toast.error(deleteError instanceof Error ? deleteError.message : 'ไม่สามารถลบรอบตารางสอบได้');
		} finally {
			deletingRoundId = null;
		}
	}

	async function loadRounds() {
		const termId = academicTermId;
		if (!termId) return;
		const { revision, signal } = roundsRequest.begin();
		roundsRevision += 1;
		roundsLoading = true;
		error = '';
		try {
			const loaded = await listExamRounds(termId, { signal });
			if (roundsRequest.isCurrent(revision) && academicTermId === termId) {
				rounds = loaded;
				roundsLoaded = true;
			}
		} catch (loadError) {
			if (!isAbortError(loadError) && roundsRequest.isCurrent(revision))
				error =
					loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดรายการรอบตารางสอบได้';
		} finally {
			if (roundsRequest.isCurrent(revision)) roundsLoading = false;
		}
	}

	async function handleCreateRound(input: CreateExamRoundInput): Promise<boolean> {
		creatingRound = true;
		try {
			const round = await createExamRound(input);
			if (input.academicTermId === academicTermId) {
				roundsRevision += 1;
				roundsRequest.abort();
				roundsLoading = false;
				roundsLoaded = true;
				rounds = [round, ...rounds.filter((item) => item.id !== round.id)];
			}
			toast.success('สร้างรอบตารางสอบแล้ว');
			createDialogOpen = false;
			if (input.academicTermId === academicTermId)
				goto(resolve(`/staff/academic/exam-schedules/${round.id}`));
			return true;
		} catch (createError) {
			toast.error(createError instanceof Error ? createError.message : 'สร้างรอบตารางสอบไม่สำเร็จ');
			return false;
		} finally {
			creatingRound = false;
		}
	}

	function statusLabel(status: string): string {
		return status === 'published' ? 'เผยแพร่แล้ว' : 'ฉบับร่าง';
	}

	function statusVariant(status: string): 'default' | 'secondary' | 'outline' {
		return status === 'published' ? 'default' : 'secondary';
	}

	function examRoundKindLabel(kind: ExamRoundKind): string {
		return kind === 'final' ? 'ปลายภาค' : 'กลางภาค';
	}

	function formatDate(value: string | null | undefined): string {
		if (!value) return '-';
		return new Date(value).toLocaleDateString('th-TH', {
			year: 'numeric',
			month: 'short',
			day: 'numeric'
		});
	}

	$effect.pre(() => {
		const contextKey = `${data.academicYearId}:${data.academicTermId}`;
		const routeRounds = data.rounds;
		const initialRevision = roundsRevision;
		let current = true;
		untrack(() => {
			if (activeContextKey !== contextKey) {
				activeContextKey = contextKey;
				rounds = [];
				roundsLoaded = false;
				createDialogOpen = false;
				deleteDialogOpen = false;
				deleteTarget = null;
			}
			roundsRequest.abort();
			roundsLoading = Boolean(routeRounds);
			error = '';
		});
		if (routeRounds) {
			void routeRounds.then((result) => {
				if (!current) return;
				untrack(() => {
					if (roundsRevision === initialRevision) {
						if (result.ok) {
							rounds = result.data;
							roundsLoaded = true;
						} else error = result.error;
						roundsLoading = false;
					}
				});
			});
		}
		return () => {
			current = false;
		};
	});
</script>

<PageShell title={data.title} description="จัดการรอบสอบประจำภาคเรียน">
	{#snippet actions()}
		<div class="flex flex-wrap items-center gap-2">
			<Button
				variant="outline"
				size="sm"
				onclick={() => loadRounds()}
				disabled={roundsLoading || !academicTermId}
			>
				<RefreshCw class="h-4 w-4" />
				รีเฟรช
			</Button>
			{#if canManageExamSchedules}
				<Button size="sm" onclick={() => (createDialogOpen = true)} disabled={!academicTermId}>
					<Plus class="h-4 w-4" />
					สร้างรอบสอบ
				</Button>
			{/if}
		</div>
	{/snippet}

	<div class="flex items-center justify-between rounded-xl border bg-card p-4 text-sm">
		<div><span class="text-muted-foreground">ภาคเรียน:</span> {selectedTermLabel}</div>
		<div class="text-muted-foreground">{rounds.length} รอบสอบ</div>
	</div>

	{#if !academicTermId}
		<PageState
			title="เลือกภาคเรียนก่อน"
			description="ใช้ตัวเลือกปีการศึกษาและภาคเรียนบนแถบด้านบน"
		/>
	{:else if roundsLoading && !roundsLoaded}
		<PageSkeleton variant="table" rows={6} columns={5} />
	{:else if error && !roundsLoaded}
		<PageState
			variant="error"
			title="โหลดตารางสอบไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadRounds()}
		/>
	{:else}
		<div class="relative space-y-4" aria-busy={roundsLoading} data-testid="exam-rounds-ready">
			{#if roundsLoading}<RegionUpdatingState label="กำลังอัปเดตรอบสอบ..." />{/if}
			{#if error}
				<div
					role="alert"
					class="flex flex-wrap items-center gap-3 rounded-xl border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
				>
					<span>{error}</span>
					<Button variant="outline" size="sm" onclick={loadRounds}>ลองใหม่</Button>
				</div>
			{/if}
			{#if rounds.length === 0}
				<div class="space-y-4">
					<AcademicPrerequisiteNotice prerequisite={assessmentPrerequisite} />
					<PageState title="ยังไม่มีรอบตารางสอบ" description="ไม่พบรอบสอบในภาคเรียนที่เลือก">
						{#snippet action()}
							{#if canManageExamSchedules}
								<Button onclick={() => (createDialogOpen = true)} disabled={!academicTermId}>
									<Plus class="h-4 w-4" />
									สร้างรอบสอบ
								</Button>
							{/if}
						{/snippet}
					</PageState>
				</div>
			{:else}
				<Card.Root class="overflow-hidden p-0">
					<div class="overflow-x-auto">
						<Table class="min-w-[840px]">
							<TableHeader>
								<TableRow>
									<TableHead>รอบสอบ</TableHead>
									<TableHead class="w-32">ชนิดรอบ</TableHead>
									<TableHead class="w-36">สถานะ</TableHead>
									<TableHead>ภาคเรียน</TableHead>
									<TableHead class="w-36">เผยแพร่</TableHead>
									<TableHead class="w-40 text-right">จัดการ</TableHead>
								</TableRow>
							</TableHeader>
							<TableBody>
								{#each rounds as round (round.id)}
									<TableRow
										class="cursor-pointer hover:bg-muted/50"
										onclick={() => goto(resolve(`/staff/academic/exam-schedules/${round.id}`))}
									>
										<TableCell>
											<div class="flex min-w-0 items-center gap-3">
												<div
													class="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border bg-muted/40"
												>
													<CalendarClock class="h-4 w-4 text-muted-foreground" />
												</div>
												<div class="min-w-0">
													<div class="truncate font-medium">{round.name}</div>
													{#if round.description}
														<div class="truncate text-xs text-muted-foreground">
															{round.description}
														</div>
													{/if}
												</div>
											</div>
										</TableCell>
										<TableCell>
											<Badge variant="outline">{examRoundKindLabel(round.examKind)}</Badge>
										</TableCell>
										<TableCell>
											<Badge variant={statusVariant(round.status)}
												>{statusLabel(round.status)}</Badge
											>
										</TableCell>
										<TableCell class="text-sm text-muted-foreground">
											{selectedTermLabel}
										</TableCell>
										<TableCell class="text-sm text-muted-foreground">
											{formatDate(round.publishedAt)}
										</TableCell>
										<TableCell>
											<div class="flex items-center justify-end gap-1">
												<Button
													variant="outline"
													size="sm"
													onclick={(event) => {
														event.stopPropagation();
														goto(resolve(`/staff/academic/exam-schedules/${round.id}`));
													}}
												>
													เปิด
												</Button>
												{#if canDeleteExamRound(round)}
													<Button
														variant="ghost"
														size="icon-sm"
														class="text-destructive hover:bg-destructive/10 hover:text-destructive"
														aria-label={`ลบรอบสอบ ${round.name}`}
														title={`ลบรอบสอบ ${round.name}`}
														disabled={deletingRoundId !== null}
														onclick={(event) => {
															event.stopPropagation();
															requestDeleteRound(round);
														}}
													>
														<Trash2 class="h-4 w-4" />
													</Button>
												{/if}
											</div>
										</TableCell>
									</TableRow>
								{/each}
							</TableBody>
						</Table>
					</div>
				</Card.Root>
			{/if}
		</div>
	{/if}

	<ExamRoundDialog
		bind:open={createDialogOpen}
		academicTermId={academicTermId ?? ''}
		termLabel={selectedTermLabel}
		saving={creatingRound}
		onCreate={handleCreateRound}
	/>

	<AlertDialog.Root bind:open={deleteDialogOpen}>
		<AlertDialog.Content>
			<AlertDialog.Header>
				<AlertDialog.Title>ลบรอบสอบ “{deleteTarget?.name ?? ''}” หรือไม่</AlertDialog.Title>
				<AlertDialog.Description>
					การดำเนินการนี้ย้อนกลับไม่ได้ และจะลบวันสอบ รายการสอบ เวลา ห้องสอบ ที่นั่ง
					และกรรมการทั้งหมดในรอบนี้ โดยไม่ลบโครงสร้างคะแนนต้นทาง
					{#if deleteTarget?.status === 'published'}
						<span class="mt-2 block font-medium text-destructive">
							รอบนี้เผยแพร่แล้ว เมื่อลบ ครูและนักเรียนจะไม่เห็นตารางสอบรอบนี้อีก
						</span>
					{/if}
				</AlertDialog.Description>
			</AlertDialog.Header>
			<AlertDialog.Footer>
				<AlertDialog.Cancel disabled={deletingRoundId !== null}>ยกเลิก</AlertDialog.Cancel>
				<AlertDialog.Action
					variant="destructive"
					disabled={deletingRoundId !== null}
					onclick={confirmDeleteRound}
				>
					{deletingRoundId ? 'กำลังลบ...' : 'ลบรอบสอบถาวร'}
				</AlertDialog.Action>
			</AlertDialog.Footer>
		</AlertDialog.Content>
	</AlertDialog.Root>
</PageShell>
