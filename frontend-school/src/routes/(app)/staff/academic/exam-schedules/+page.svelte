<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { toast } from 'svelte-sonner';
	import type { PageProps } from './$types';
	import {
		getAcademicStructure,
		type AcademicStructureData,
		type AcademicYear,
		type Semester
	} from '$lib/api/academic';
	import {
		createExamRound,
		listExamRounds,
		type CreateExamRoundInput,
		type ExamRound,
		type ExamRoundStatus
	} from '$lib/api/examSchedule';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { PageShell } from '$lib/components/app-layout';
	import ExamRoundDialog from '$lib/components/academic/exam-schedule/ExamRoundDialog.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
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

	let loading = $state(true);
	let roundsLoading = $state(false);
	let error = $state('');
	let structure = $state<AcademicStructureData | null>(null);
	let rounds = $state<ExamRound[]>([]);
	let selectedYearId = $state('');
	let selectedSemesterId = $state('');
	let createDialogOpen = $state(false);
	let creatingRound = $state(false);

	const canManageExamSchedules = $derived(
		$can.has(PERMISSIONS.ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)
	);
	const years = $derived<AcademicYear[]>(structure?.years ?? []);
	const semesters = $derived<Semester[]>(structure?.semesters ?? []);
	const filteredSemesters = $derived(
		semesters.filter((semester) => semester.academic_year_id === selectedYearId)
	);
	const selectedYearLabel = $derived(
		years.find((year) => year.id === selectedYearId)?.name ?? 'เลือกปีการศึกษา'
	);
	const selectedSemesterLabel = $derived(
		semesters.find((semester) => semester.id === selectedSemesterId)?.name ?? 'เลือกภาคเรียน'
	);

	function semesterYearId(semesterId: string): string {
		return semesters.find((semester) => semester.id === semesterId)?.academic_year_id ?? '';
	}

	function selectYear(yearId: string) {
		selectedYearId = yearId;
		const semester =
			semesters.find((item) => item.academic_year_id === yearId && item.is_active) ??
			semesters.find((item) => item.academic_year_id === yearId);
		selectedSemesterId = semester?.id ?? '';
		if (selectedSemesterId) {
			loadRounds();
		} else {
			rounds = [];
		}
	}

	function selectSemester(semesterId: string) {
		selectedSemesterId = semesterId;
		const yearId = semesterYearId(semesterId);
		if (yearId) selectedYearId = yearId;
		loadRounds();
	}

	async function loadRounds() {
		if (!selectedSemesterId) {
			rounds = [];
			return;
		}

		roundsLoading = true;
		error = '';
		try {
			rounds = await listExamRounds({ academicSemesterId: selectedSemesterId });
		} catch (loadError) {
			error =
				loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดรายการรอบตารางสอบได้';
			rounds = [];
		} finally {
			roundsLoading = false;
		}
	}

	async function loadInitialData() {
		loading = true;
		error = '';
		try {
			const academic = await getAcademicStructure();
			structure = academic.data;

			const activeYear = academic.data.years.find((year) => year.is_active) ?? academic.data.years[0];
			const yearSemesters = activeYear
				? academic.data.semesters.filter((semester) => semester.academic_year_id === activeYear.id)
				: academic.data.semesters;
			const activeSemester =
				yearSemesters.find((semester) => semester.is_active) ?? yearSemesters[0] ?? academic.data.semesters[0];

			selectedYearId = activeSemester?.academic_year_id ?? activeYear?.id ?? '';
			selectedSemesterId = activeSemester?.id ?? '';
			await loadRounds();
		} catch (loadError) {
			error = loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดข้อมูลตารางสอบได้';
			rounds = [];
		} finally {
			loading = false;
		}
	}

	async function handleCreateRound(input: CreateExamRoundInput): Promise<boolean> {
		creatingRound = true;
		try {
			const round = await createExamRound(input);
			if (input.academicSemesterId === selectedSemesterId) {
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

	function statusLabel(status: ExamRoundStatus): string {
		return status === 'published' ? 'เผยแพร่แล้ว' : 'ฉบับร่าง';
	}

	function statusVariant(status: ExamRoundStatus): 'default' | 'secondary' | 'outline' {
		return status === 'published' ? 'default' : 'secondary';
	}

	function formatDate(value: string | null | undefined): string {
		if (!value) return '-';
		return new Date(value).toLocaleDateString('th-TH', {
			year: 'numeric',
			month: 'short',
			day: 'numeric'
		});
	}

	onMount(loadInitialData);
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<PageShell title={data.title} description="จัดการรอบสอบประจำภาคเรียน">
	{#snippet actions()}
		<div class="flex flex-wrap items-center gap-2">
			<Button
				variant="outline"
				size="sm"
				onclick={loadRounds}
				disabled={loading || roundsLoading || !selectedSemesterId}
			>
				<RefreshCw class="h-4 w-4" />
				รีเฟรช
			</Button>
			{#if canManageExamSchedules}
				<Button
					size="sm"
					onclick={() => (createDialogOpen = true)}
					disabled={!selectedSemesterId || semesters.length === 0}
				>
					<Plus class="h-4 w-4" />
					สร้างรอบสอบ
				</Button>
			{/if}
		</div>
	{/snippet}

	<div class="flex flex-col gap-3 rounded-md border bg-background p-3 md:flex-row md:items-end md:justify-between">
		<div class="grid gap-3 sm:grid-cols-2">
			<div class="grid gap-2">
				<span class="text-xs font-medium text-muted-foreground">ปีการศึกษา</span>
				<Select.Root type="single" value={selectedYearId} onValueChange={(value) => value && selectYear(value)}>
					<Select.Trigger class="w-full sm:w-56">{selectedYearLabel}</Select.Trigger>
					<Select.Content>
						{#each years as year (year.id)}
							<Select.Item value={year.id}>{year.name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
			<div class="grid gap-2">
				<span class="text-xs font-medium text-muted-foreground">ภาคเรียน</span>
				<Select.Root
					type="single"
					value={selectedSemesterId}
					onValueChange={(value) => value && selectSemester(value)}
				>
					<Select.Trigger class="w-full sm:w-56">{selectedSemesterLabel}</Select.Trigger>
					<Select.Content>
						{#each filteredSemesters as semester (semester.id)}
							<Select.Item value={semester.id}>{semester.name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
		</div>
		<div class="text-sm text-muted-foreground">
			{rounds.length} รอบสอบ
		</div>
	</div>

	{#if loading}
		<PageSkeleton variant="table" rows={6} columns={5} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดตารางสอบไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadInitialData}
		/>
	{:else if rounds.length === 0}
		<PageState title="ยังไม่มีรอบตารางสอบ" description="ไม่พบรอบสอบในภาคเรียนที่เลือก">
			{#snippet action()}
				{#if canManageExamSchedules}
					<Button onclick={() => (createDialogOpen = true)} disabled={!selectedSemesterId}>
						<Plus class="h-4 w-4" />
						สร้างรอบสอบ
					</Button>
				{/if}
			{/snippet}
		</PageState>
	{:else}
		<Card.Root class="overflow-hidden p-0">
			<div class="overflow-x-auto">
				<Table class="min-w-[760px]">
					<TableHeader>
						<TableRow>
							<TableHead>รอบสอบ</TableHead>
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
										<div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border bg-muted/40">
											<CalendarClock class="h-4 w-4 text-muted-foreground" />
										</div>
										<div class="min-w-0">
											<div class="truncate font-medium">{round.name}</div>
											{#if round.description}
												<div class="truncate text-xs text-muted-foreground">{round.description}</div>
											{/if}
										</div>
									</div>
								</TableCell>
								<TableCell>
									<Badge variant={statusVariant(round.status)}>{statusLabel(round.status)}</Badge>
								</TableCell>
								<TableCell class="text-sm text-muted-foreground">
									{semesters.find((semester) => semester.id === round.academicSemesterId)?.name ?? '-'}
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
		semesters={filteredSemesters}
		defaultSemesterId={selectedSemesterId}
		saving={creatingRound}
		onCreate={handleCreateRound}
	/>
</PageShell>
