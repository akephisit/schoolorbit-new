<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { toast } from 'svelte-sonner';
	import type { PageProps } from './$types';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import {
		createExamRound,
		listExamRounds,
		type CreateExamRoundInput,
		type ExamRound,
		type ExamRoundKind
	} from '$lib/api/examSchedule';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { PageShell } from '$lib/components/app-layout';
	import ExamRoundDialog from '$lib/components/academic/exam-schedule/ExamRoundDialog.svelte';
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
	import { CalendarClock, Plus, RefreshCw } from 'lucide-svelte';

	let { data }: PageProps = $props();

	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const selectedTermLabel = $derived(
		$academicContext.options?.terms.find((term) => term.id === academicTermId)?.name ??
			'ภาคเรียนที่เลือก'
	);
	let roundsLoading = $state(false);
	let error = $state('');
	let rounds = $state<ExamRound[]>([]);
	let createDialogOpen = $state(false);
	let creatingRound = $state(false);

	const canManageExamSchedules = $derived(
		$can.has(PERMISSIONS.ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)
	);
	async function loadRounds(termId: string | null = academicTermId) {
		if (!termId) {
			rounds = [];
			return;
		}

		roundsLoading = true;
		error = '';
		try {
			rounds = await listExamRounds(termId);
		} catch (loadError) {
			error = loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดรายการรอบตารางสอบได้';
			rounds = [];
		} finally {
			roundsLoading = false;
		}
	}

	async function handleCreateRound(input: CreateExamRoundInput): Promise<boolean> {
		creatingRound = true;
		try {
			const round = await createExamRound(input);
			if (input.academicTermId === academicTermId) {
				rounds = [round, ...rounds.filter((item) => item.id !== round.id)];
			}
			toast.success('สร้างรอบตารางสอบแล้ว');
			createDialogOpen = false;
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

	onMount(() => {
		let loadedTermId: string | null = null;
		return academicContext.subscribe((state) => {
			const termId = state.selected.academicTermId;
			if (termId && termId !== loadedTermId) {
				loadedTermId = termId;
				void loadRounds(termId);
			}
		});
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
	{:else if roundsLoading}
		<PageSkeleton variant="table" rows={6} columns={5} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดตารางสอบไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadRounds()}
		/>
	{:else if rounds.length === 0}
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
							<TableHead class="w-32 text-right">จัดการ</TableHead>
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
									<Badge variant={statusVariant(round.status)}>{statusLabel(round.status)}</Badge>
								</TableCell>
								<TableCell class="text-sm text-muted-foreground">
									{selectedTermLabel}
								</TableCell>
								<TableCell class="text-sm text-muted-foreground">
									{formatDate(round.publishedAt)}
								</TableCell>
								<TableCell class="text-right">
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
								</TableCell>
							</TableRow>
						{/each}
					</TableBody>
				</Table>
			</div>
		</Card.Root>
	{/if}

	<ExamRoundDialog
		bind:open={createDialogOpen}
		academicTermId={academicTermId ?? ''}
		termLabel={selectedTermLabel}
		saving={creatingRound}
		onCreate={handleCreateRound}
	/>
</PageShell>
