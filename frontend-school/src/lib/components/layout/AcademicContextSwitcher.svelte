<script lang="ts">
	import { ArrowRight, CalendarRange, LoaderCircle, RefreshCw, TriangleAlert } from 'lucide-svelte';
	import {
		getAcademicContextStore,
		hasAcademicContextDirtySource
	} from '$lib/academic-context/store';
	import type { AcademicTermOption, AcademicYearOption } from '$lib/academic-context/types';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import * as Sheet from '$lib/components/ui/sheet';

	type ContextStatus = AcademicYearOption['status'] | AcademicTermOption['status'];
	type PendingChange =
		| { kind: 'year'; value: string; closeSheet: boolean }
		| { kind: 'term'; value: string | null; closeSheet: boolean };

	const ALL_YEAR_VALUE = '__all_year__';
	const academicContext = getAcademicContextStore();
	const contextState = $derived($academicContext);
	const yearOptions = $derived(contextState.options?.years ?? []);
	const selectedYear = $derived(
		yearOptions.find((year) => year.id === contextState.selected.academicYearId) ?? null
	);
	const termOptions = $derived.by(() => {
		const academicYearId = contextState.selected.academicYearId;
		if (!academicYearId) return [];
		return (contextState.options?.terms ?? [])
			.filter((term) => term.academicYearId === academicYearId)
			.toSorted((left, right) => left.sequence - right.sequence);
	});
	const selectedTerm = $derived(
		termOptions.find((term) => term.id === contextState.selected.academicTermId) ?? null
	);
	const showsTerm = $derived(
		contextState.requirement === 'term_required' || contextState.requirement === 'term_optional'
	);
	const mobileSummary = $derived(
		`${selectedYear?.year ?? 'เลือกปี'} · ${
			selectedTerm?.name ??
			(contextState.requirement === 'term_optional' ? 'ทั้งปี' : 'เลือกภาคเรียน')
		}`
	);

	let mobileOpen = $state(false);
	let pendingChange = $state<PendingChange | null>(null);

	const statusLabels: Record<ContextStatus, string> = {
		planning: 'กำลังวางแผน',
		ready: 'พร้อมใช้งาน',
		active: 'กำลังใช้งาน',
		closing: 'กำลังปิด',
		closed: 'ปิดแล้ว',
		archived: 'เก็บถาวร',
		cancelled: 'ยกเลิกแล้ว'
	};

	function statusClass(status: ContextStatus): string {
		switch (status) {
			case 'active':
				return 'border-emerald-300 bg-emerald-50 text-emerald-700 dark:border-emerald-800 dark:bg-emerald-950 dark:text-emerald-300';
			case 'ready':
				return 'border-sky-300 bg-sky-50 text-sky-700 dark:border-sky-800 dark:bg-sky-950 dark:text-sky-300';
			case 'planning':
				return 'border-amber-300 bg-amber-50 text-amber-700 dark:border-amber-800 dark:bg-amber-950 dark:text-amber-300';
			case 'closing':
				return 'border-orange-300 bg-orange-50 text-orange-700 dark:border-orange-800 dark:bg-orange-950 dark:text-orange-300';
			case 'closed':
			case 'archived':
			case 'cancelled':
				return 'border-border bg-muted text-muted-foreground';
		}
	}

	async function applyChange(change: PendingChange): Promise<void> {
		if (change.kind === 'year') {
			await academicContext.selectYear(change.value);
		} else {
			await academicContext.selectTerm(change.value);
		}
		if (change.closeSheet) mobileOpen = false;
	}

	function requestChange(change: PendingChange): void {
		if (change.kind === 'year' && change.value === contextState.selected.academicYearId) return;
		if (change.kind === 'term' && change.value === contextState.selected.academicTermId) return;

		if (hasAcademicContextDirtySource()) {
			pendingChange = change;
			return;
		}
		void applyChange(change);
	}

	function handleYearChange(value: string, closeSheet = false): void {
		requestChange({ kind: 'year', value, closeSheet });
	}

	function handleTermChange(value: string, closeSheet = false): void {
		requestChange({
			kind: 'term',
			value: value === ALL_YEAR_VALUE ? null : value,
			closeSheet
		});
	}

	async function confirmPendingChange(): Promise<void> {
		const change = pendingChange;
		pendingChange = null;
		if (change) await applyChange(change);
	}
</script>

{#if contextState.status !== 'hidden'}
	<div
		class="min-w-0"
		data-testid="academic-context-switcher"
		aria-live="polite"
		aria-busy={contextState.status === 'loading'}
	>
		{#if contextState.status === 'loading' && !contextState.options}
			<div
				class="flex h-9 items-center gap-2 rounded-lg border border-border/70 bg-muted/40 px-3 text-xs text-muted-foreground"
			>
				<LoaderCircle class="size-4 animate-spin" />
				<span class="hidden sm:inline">กำลังโหลดปีการศึกษา...</span>
			</div>
		{:else if contextState.status === 'error'}
			<div
				class="flex h-9 items-center gap-2 rounded-lg border border-destructive/30 bg-destructive/5 px-2 text-xs text-destructive"
			>
				<TriangleAlert class="size-4 shrink-0" />
				<span class="hidden xl:inline">โหลดปีการศึกษาไม่สำเร็จ</span>
				<Button
					variant="ghost"
					size="sm"
					class="h-7 px-2 text-destructive hover:text-destructive"
					onclick={() => void academicContext.retry()}
				>
					<RefreshCw class="size-3.5" />
					<span class="hidden sm:inline">ลองโหลดอีกครั้ง</span>
					<span class="sm:hidden">ลองใหม่</span>
				</Button>
			</div>
		{:else if contextState.options}
			<div
				class="hidden h-11 items-center gap-2 rounded-xl border border-border/70 bg-card px-2 shadow-xs lg:flex"
				title={contextState.status === 'unavailable'
					? 'กรุณาเลือกปีการศึกษาและภาคเรียนที่ใช้ได้'
					: 'ปีการศึกษาและภาคเรียนที่เลือก'}
			>
				<div class="flex items-center px-1">
					<div
						class="flex size-7 items-center justify-center rounded-lg bg-primary/10 text-primary"
					>
						<CalendarRange class="size-4" />
					</div>
				</div>

				<Select.Root
					type="single"
					value={contextState.selected.academicYearId ?? undefined}
					onValueChange={(value) => handleYearChange(value)}
				>
					<Select.Trigger
						size="sm"
						aria-label="เลือกปีการศึกษา"
						class="min-w-32 max-w-40 border-0 bg-transparent shadow-none hover:bg-accent"
					>
						<span data-slot="select-value" class="min-w-0 truncate font-medium">
							{selectedYear?.name ?? 'เลือกปีการศึกษา'}
						</span>
					</Select.Trigger>
					<Select.Content>
						{#each yearOptions as year (year.id)}
							<Select.Item value={year.id}>
								<span class="flex min-w-0 items-center gap-2">
									<span class="truncate">{year.name}</span>
									<Badge variant="outline" class={statusClass(year.status)}>
										{statusLabels[year.status]}
									</Badge>
								</span>
							</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>

				{#if showsTerm}
					<ArrowRight class="size-3.5 shrink-0 text-muted-foreground" />
					<Select.Root
						type="single"
						value={contextState.selected.academicTermId ??
							(contextState.requirement === 'term_optional' ? ALL_YEAR_VALUE : undefined)}
						onValueChange={(value) => handleTermChange(value)}
						disabled={!contextState.selected.academicYearId}
					>
						<Select.Trigger
							size="sm"
							aria-label="เลือกภาคเรียน"
							class="min-w-28 max-w-40 border-0 bg-transparent shadow-none hover:bg-accent"
						>
							<span data-slot="select-value" class="min-w-0 truncate font-medium">
								{selectedTerm?.name ??
									(contextState.requirement === 'term_optional' ? 'ทั้งปี' : 'เลือกภาคเรียน')}
							</span>
						</Select.Trigger>
						<Select.Content>
							{#if contextState.requirement === 'term_optional'}
								<Select.Item value={ALL_YEAR_VALUE}>ทั้งปี</Select.Item>
							{/if}
							{#each termOptions as term (term.id)}
								<Select.Item value={term.id}>
									<span class="flex min-w-0 items-center gap-2">
										<span class="truncate">{term.name}</span>
										<Badge variant="outline" class={statusClass(term.status)}>
											{statusLabels[term.status]}
										</Badge>
									</span>
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				{/if}
			</div>

			<Sheet.Root bind:open={mobileOpen}>
				<Sheet.Trigger>
					{#snippet child({ props })}
						<Button
							{...props}
							variant="outline"
							size="sm"
							class="max-w-32 gap-2 sm:max-w-40 lg:hidden"
							data-testid="academic-context-mobile-trigger"
							aria-label={`ปีการศึกษาและภาคเรียน ${mobileSummary}`}
						>
							<CalendarRange class="size-4 shrink-0 text-primary" />
							<span class="truncate">{mobileSummary}</span>
							{#if contextState.status === 'unavailable'}
								<TriangleAlert class="size-3.5 shrink-0 text-amber-600" />
							{/if}
						</Button>
					{/snippet}
				</Sheet.Trigger>
				<Sheet.Content side="bottom" class="max-h-[85vh] overflow-y-auto rounded-t-2xl">
					<Sheet.Header>
						<Sheet.Title>เลือกบริบทการศึกษา</Sheet.Title>
						<Sheet.Description>
							เปลี่ยนเฉพาะข้อมูลที่กำลังดู ไม่ได้เปิดใช้งานหรือปิดปีการศึกษา
						</Sheet.Description>
					</Sheet.Header>

					<div class="grid gap-5 pb-4">
						<label class="grid gap-2 text-sm font-medium">
							<span>ปีการศึกษา</span>
							<Select.Root
								type="single"
								value={contextState.selected.academicYearId ?? undefined}
								onValueChange={(value) => handleYearChange(value)}
							>
								<Select.Trigger aria-label="เลือกปีการศึกษา (มือถือ)" class="w-full">
									<span data-slot="select-value">
										{selectedYear?.name ?? 'เลือกปีการศึกษา'}
									</span>
								</Select.Trigger>
								<Select.Content>
									{#each yearOptions as year (year.id)}
										<Select.Item value={year.id}>
											<span class="flex items-center gap-2">
												<span>{year.name}</span>
												<Badge variant="outline" class={statusClass(year.status)}>
													{statusLabels[year.status]}
												</Badge>
											</span>
										</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</label>

						{#if showsTerm}
							<label class="grid gap-2 text-sm font-medium">
								<span>ภาคเรียน</span>
								<Select.Root
									type="single"
									value={contextState.selected.academicTermId ??
										(contextState.requirement === 'term_optional' ? ALL_YEAR_VALUE : undefined)}
									onValueChange={(value) => handleTermChange(value, true)}
									disabled={!contextState.selected.academicYearId}
								>
									<Select.Trigger aria-label="เลือกภาคเรียน (มือถือ)" class="w-full">
										<span data-slot="select-value">
											{selectedTerm?.name ??
												(contextState.requirement === 'term_optional' ? 'ทั้งปี' : 'เลือกภาคเรียน')}
										</span>
									</Select.Trigger>
									<Select.Content>
										{#if contextState.requirement === 'term_optional'}
											<Select.Item value={ALL_YEAR_VALUE}>ทั้งปี</Select.Item>
										{/if}
										{#each termOptions as term (term.id)}
											<Select.Item value={term.id}>
												<span class="flex items-center gap-2">
													<span>{term.name}</span>
													<Badge variant="outline" class={statusClass(term.status)}>
														{statusLabels[term.status]}
													</Badge>
												</span>
											</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</label>
						{/if}
					</div>
				</Sheet.Content>
			</Sheet.Root>
		{/if}
	</div>
{/if}

<AlertDialog.Root
	open={pendingChange !== null}
	onOpenChange={(open) => !open && (pendingChange = null)}
>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>เปลี่ยนบริบททั้งที่ยังไม่ได้บันทึก?</AlertDialog.Title>
			<AlertDialog.Description>
				หน้านี้มีข้อมูลที่แก้ไขค้างอยู่ หากเปลี่ยนปีการศึกษาหรือภาคเรียน
				ข้อมูลที่ยังไม่ได้บันทึกอาจสูญหาย
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel>อยู่หน้านี้ต่อ</AlertDialog.Cancel>
			<AlertDialog.Action onclick={() => void confirmPendingChange()}>
				เปลี่ยนบริบท
			</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
