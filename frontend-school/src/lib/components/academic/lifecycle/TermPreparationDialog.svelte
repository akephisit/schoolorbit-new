<script lang="ts">
	import { onMount } from 'svelte';
	import { ArrowRight, CalendarRange, CheckCircle2, Layers3, RefreshCw } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import {
		getAcademicContextStore,
		registerAcademicContextDirtySource
	} from '$lib/academic-context/store';
	import {
		applyAcademicTermPreparation,
		previewAcademicTermPreparation,
		type ApplyTermPreparationInput,
		type TermPreparationMappingKind,
		type TermPreparationMappings,
		type TermPreparationModule,
		type TermPreparationWorkspace
	} from '$lib/api/academic-lifecycle';
	import { ApiClientError } from '$lib/api/client';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { LoadingButton, PageSkeleton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	let {
		sourceTermId,
		disabled = false,
		onapplied
	}: {
		sourceTermId: string;
		disabled?: boolean;
		onapplied?: () => Promise<void> | void;
	} = $props();

	const academicContext = getAcademicContextStore();
	const latest = new LatestRequest();
	const moduleDefinitions: Array<{
		id: TermPreparationModule;
		label: string;
		description: string;
	}> = [
		{
			id: 'delivery',
			label: 'รายการเปิดสอนและกลุ่มเรียน',
			description: 'สร้างจากหลักสูตรของภาคเรียนเป้าหมาย ไม่คัดลอกรายชื่อนักเรียน'
		},
		{
			id: 'assessments',
			label: 'โครงสร้างคะแนน',
			description: 'คัดลอกเฉพาะ 4 ช่วงคะแนน ไม่คัดลอกงานย่อยหรือคะแนน'
		},
		{
			id: 'timetable',
			label: 'ตารางสอน',
			description: 'สร้างแบบร่างจากตารางที่เผยแพร่ล่าสุดและตรวจการชนกันใหม่'
		},
		{
			id: 'exams',
			label: 'ตารางสอบ',
			description: 'สร้างรอบ วัน และคาบสอบแบบร่าง โดยไม่คัดลอกกรรมการหรือผลสอบ'
		},
		{
			id: 'supervision',
			label: 'นิเทศการสอน',
			description: 'สร้างรอบและกลุ่มเป้าหมายแบบร่าง โดยไม่คัดลอกผลประเมิน'
		}
	];
	const kindLabels: Record<TermPreparationMappingKind, string> = {
		learning_offering: 'รายการเปิดสอน',
		learning_group: 'กลุ่มเรียน',
		teacher: 'ครู/บุคลากร',
		homeroom: 'ห้องประจำชั้น',
		room: 'ห้องเรียน',
		bell_period: 'คาบเรียน',
		assessment_plan: 'โครงสร้างคะแนน'
	};

	let alive = false;
	let open = $state(false);
	let loading = $state(false);
	let busy = $state(false);
	let targetTermId = $state('');
	let modules = $state<TermPreparationModule[]>(['delivery']);
	let mappings = $state<TermPreparationMappings>({ entities: [], dates: [] });
	let workspace = $state.raw<TermPreparationWorkspace | null>(null);
	let previewStale = $state(false);
	let errorMessage = $state('');
	let pending: { key: string; request: ApplyTermPreparationInput } | null = null;
	const canManage = $derived($can.has(PERMISSIONS.ACADEMIC_LIFECYCLE_MANAGE_SCHOOL));
	const sourceTerm = $derived(
		$academicContext.options?.terms.find((term) => term.id === sourceTermId) ?? null
	);
	const targetTerms = $derived(
		($academicContext.options?.terms ?? [])
			.filter((term) => term.id !== sourceTermId && term.status === 'planning')
			.toSorted((left, right) => left.startDate.localeCompare(right.startDate))
	);
	const canPreview = $derived(
		canManage && !disabled && !loading && !busy && !!targetTermId && modules.length > 0
	);
	const canApply = $derived(
		canPreview && !previewStale && !!workspace?.canApply && workspace.sourceChecksum.length === 64
	);

	function resetPreview() {
		latest.abort();
		workspace = null;
		previewStale = false;
		errorMessage = '';
		pending = null;
	}

	function changeOpen(value: boolean) {
		if (busy) return;
		open = value;
		if (value) {
			targetTermId = targetTerms[0]?.id ?? '';
			modules = ['delivery'];
			mappings = { entities: [], dates: [] };
			resetPreview();
		} else {
			latest.abort();
		}
	}

	function toggleModule(module: TermPreparationModule, checked: boolean) {
		modules = checked
			? [...modules, module].filter((item, index, values) => values.indexOf(item) === index)
			: modules.filter((item) => item !== module);
		mappings = { entities: [], dates: [] };
		resetPreview();
	}

	async function preview() {
		if (!canPreview) return;
		const { revision, signal } = latest.begin();
		loading = true;
		errorMessage = '';
		try {
			const next = await previewAcademicTermPreparation(
				{
					sourceTermId,
					targetTermId,
					modules: [...modules],
					mappings
				},
				{ signal }
			);
			if (!alive || !latest.isCurrent(revision)) return;
			workspace = next;
			previewStale = false;
			pending = null;
		} catch (error) {
			if (!isAbortError(error) && alive && latest.isCurrent(revision)) {
				errorMessage = error instanceof Error ? error.message : 'ตรวจตัวอย่างไม่สำเร็จ';
			}
		} finally {
			if (latest.isCurrent(revision)) loading = false;
		}
	}

	function entityValue(kind: TermPreparationMappingKind, sourceId: string): string {
		return (
			mappings.entities?.find((item) => item.kind === kind && item.sourceId === sourceId)
				?.targetId ??
			workspace?.mappingRequirements.find(
				(item) => item.kind === kind && item.sourceId === sourceId
			)?.selectedTargetId ??
			''
		);
	}

	function setEntityMapping(kind: TermPreparationMappingKind, sourceId: string, targetId: string) {
		mappings = {
			...mappings,
			entities: [
				...(mappings.entities ?? []).filter(
					(item) => item.kind !== kind || item.sourceId !== sourceId
				),
				...(targetId ? [{ kind, sourceId, targetId }] : [])
			]
		};
		previewStale = true;
		pending = null;
	}

	function dateValue(sourceDate: string): string | undefined {
		return (
			mappings.dates?.find((item) => item.sourceDate === sourceDate)?.targetDate ??
			workspace?.dateRequirements.find((item) => item.sourceDate === sourceDate)
				?.selectedTargetDate ??
			undefined
		);
	}

	function setDateMapping(sourceDate: string, targetDate: string | undefined) {
		mappings = {
			...mappings,
			dates: [
				...(mappings.dates ?? []).filter((item) => item.sourceDate !== sourceDate),
				...(targetDate ? [{ sourceDate, targetDate }] : [])
			]
		};
		previewStale = true;
		pending = null;
	}

	async function applyPreparation() {
		if (!canApply || !workspace) return;
		const body = {
			sourceChecksum: workspace.sourceChecksum,
			sourceTermId,
			targetTermId,
			modules: [...modules],
			mappings
		};
		const key = JSON.stringify(body);
		if (pending?.key !== key) {
			pending = { key, request: { ...body, requestId: crypto.randomUUID() } };
		}
		busy = true;
		errorMessage = '';
		try {
			const outcome = await applyAcademicTermPreparation(pending.request);
			if (!alive) return;
			pending = null;
			open = false;
			const created = outcome.modules.reduce((total, item) => total + item.createdCount, 0);
			toast.success(`สร้างแบบร่างสำหรับภาคเรียนถัดไปแล้ว ${created} รายการ`);
			await onapplied?.();
		} catch (error) {
			if (!alive) return;
			errorMessage = error instanceof Error ? error.message : 'เตรียมภาคเรียนไม่สำเร็จ';
			if (error instanceof ApiClientError && error.status === 409) previewStale = true;
		} finally {
			busy = false;
		}
	}

	onMount(() => {
		alive = true;
		const unregister = registerAcademicContextDirtySource(
			`term-preparation:${sourceTermId}`,
			() => open || busy
		);
		return () => {
			alive = false;
			latest.abort();
			unregister();
		};
	});
</script>

<Button
	variant="outline"
	class="w-full justify-start"
	disabled={disabled || !canManage || sourceTerm?.status !== 'closed' || targetTerms.length === 0}
	onclick={() => changeOpen(true)}
>
	<Layers3 class="size-4" />เตรียมภาคเรียนถัดไป
</Button>

<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content
		class="flex max-h-[92dvh] flex-col overflow-hidden p-0 sm:max-w-4xl"
		showCloseButton={!busy}
		onInteractOutside={(event) => busy && event.preventDefault()}
		onEscapeKeydown={(event) => busy && event.preventDefault()}
	>
		<Dialog.Header class="border-b px-5 py-4">
			<Dialog.Title>เตรียมข้อมูลสำหรับภาคเรียนถัดไป</Dialog.Title>
			<Dialog.Description>
				เลือกเฉพาะส่วนงานที่ต้องการ ระบบจะสร้างเป็นแบบร่างและไม่เปลี่ยนภาคเรียนที่กำลังใช้งาน
			</Dialog.Description>
		</Dialog.Header>

		<div class="min-h-0 flex-1 space-y-5 overflow-y-auto px-5 py-4">
			<section
				class="grid items-end gap-3 rounded-xl border bg-muted/20 p-4 sm:grid-cols-[1fr_auto_1fr]"
			>
				<div>
					<p class="text-xs font-medium text-muted-foreground">ต้นทางที่ปิดแล้ว</p>
					<p class="mt-1 font-semibold">{sourceTerm?.name ?? 'ภาคเรียนที่เลือก'}</p>
				</div>
				<ArrowRight class="hidden size-5 text-muted-foreground sm:block" />
				<div class="space-y-2">
					<Label for="term-preparation-target">ภาคเรียนเป้าหมายที่กำลังวางแผน</Label>
					<Select.Root
						type="single"
						value={targetTermId}
						disabled={busy || loading}
						onValueChange={(value) => {
							targetTermId = value;
							mappings = { entities: [], dates: [] };
							resetPreview();
						}}
					>
						<Select.Trigger id="term-preparation-target" class="w-full">
							{targetTerms.find((term) => term.id === targetTermId)?.name ?? 'เลือกภาคเรียน'}
						</Select.Trigger>
						<Select.Content>
							{#each targetTerms as term (term.id)}
								<Select.Item value={term.id}>{term.name} · {term.startDate}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</section>

			<section aria-labelledby="term-preparation-modules">
				<div class="mb-3 flex items-end justify-between gap-3">
					<div>
						<h3 id="term-preparation-modules" class="font-semibold">เลือกส่วนงาน</h3>
						<p class="text-sm text-muted-foreground">แต่ละส่วนถูกตรวจและสร้างแบบร่างแยกกัน</p>
					</div>
					<span class="text-sm tabular-nums text-muted-foreground">{modules.length}/5 ส่วนงาน</span>
				</div>
				<div class="grid gap-2 sm:grid-cols-2">
					{#each moduleDefinitions as item (item.id)}
						<label
							class="flex cursor-pointer items-start gap-3 rounded-lg border p-3 hover:bg-muted/30"
						>
							<Checkbox
								checked={modules.includes(item.id)}
								disabled={busy || loading}
								onCheckedChange={(checked) => toggleModule(item.id, checked)}
							/>
							<span>
								<span class="block text-sm font-medium">{item.label}</span>
								<span class="mt-1 block text-xs leading-relaxed text-muted-foreground"
									>{item.description}</span
								>
							</span>
						</label>
					{/each}
				</div>
			</section>

			<div class="flex justify-end">
				<LoadingButton {loading} disabled={!canPreview} onclick={() => void preview()}>
					<RefreshCw class="size-4" />ตรวจตัวอย่าง
				</LoadingButton>
			</div>

			{#if loading && !workspace}
				<PageSkeleton variant="form" rows={4} />
			{:else if workspace}
				<section class="space-y-3" aria-labelledby="preparation-summary">
					<div>
						<h3 id="preparation-summary" class="font-semibold">รายการแบบร่างที่จะสร้าง</h3>
						<p class="text-sm text-muted-foreground">
							{workspace.context.sourceLabel} → {workspace.context.targetLabel}
						</p>
					</div>
					<div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
						{#each workspace.modules as item (item.module)}
							<div class="rounded-lg border p-3">
								<div class="flex items-center justify-between gap-2">
									<p class="text-sm font-medium">
										{moduleDefinitions.find((definition) => definition.id === item.module)?.label}
									</p>
									<span class="rounded-full bg-primary/10 px-2 py-0.5 text-xs text-primary"
										>{item.draftCount} แบบร่าง</span
									>
								</div>
								<p class="mt-2 text-xs leading-relaxed text-muted-foreground">{item.summary}</p>
							</div>
						{/each}
					</div>
				</section>

				{#if workspace.mappingRequirements.length > 0 || workspace.dateRequirements.length > 0}
					<section class="space-y-3" aria-labelledby="preparation-mappings">
						<div>
							<h3 id="preparation-mappings" class="font-semibold">ตรวจการจับคู่ปลายทาง</h3>
							<p class="text-sm text-muted-foreground">
								ระบบแนะนำจากรหัสและชื่อที่ตรงกัน เปลี่ยนได้ก่อนตรวจตัวอย่างอีกครั้ง
							</p>
						</div>
						<div class="max-h-80 divide-y overflow-y-auto rounded-lg border">
							{#each workspace.mappingRequirements as requirement (`${requirement.kind}:${requirement.sourceId}`)}
								<div
									class="grid gap-2 p-3 sm:grid-cols-[minmax(0,1fr)_minmax(14rem,1fr)] sm:items-center"
								>
									<div class="min-w-0">
										<p class="truncate text-sm font-medium">{requirement.sourceLabel}</p>
										<p class="text-xs text-muted-foreground">{kindLabels[requirement.kind]}</p>
									</div>
									<Select.Root
										type="single"
										value={entityValue(requirement.kind, requirement.sourceId)}
										disabled={busy || requirement.targetOptions.length === 0}
										onValueChange={(value) =>
											setEntityMapping(requirement.kind, requirement.sourceId, value)}
									>
										<Select.Trigger class="w-full">
											{requirement.targetOptions.find(
												(option) =>
													option.id === entityValue(requirement.kind, requirement.sourceId)
											)?.label ?? 'เลือกปลายทาง'}
										</Select.Trigger>
										<Select.Content>
											{#each requirement.targetOptions as option (option.id)}
												<Select.Item value={option.id}>{option.label}</Select.Item>
											{/each}
										</Select.Content>
									</Select.Root>
								</div>
							{/each}
							{#each workspace.dateRequirements as requirement (requirement.sourceDate)}
								<div
									class="grid gap-2 p-3 sm:grid-cols-[minmax(0,1fr)_minmax(14rem,1fr)] sm:items-center"
								>
									<div>
										<p class="text-sm font-medium">{requirement.sourceLabel}</p>
										<p class="text-xs text-muted-foreground">วันที่ปลายทาง</p>
									</div>
									<DatePicker
										value={dateValue(requirement.sourceDate)}
										disabled={busy}
										ariaLabel={`วันที่ปลายทางของ ${requirement.sourceLabel}`}
										onValueChange={(value) => setDateMapping(requirement.sourceDate, value)}
									/>
								</div>
							{/each}
						</div>
						{#if previewStale}
							<p class="text-sm text-amber-800">แก้การจับคู่แล้ว กรุณากด “ตรวจตัวอย่าง” อีกครั้ง</p>
						{/if}
					</section>
				{/if}

				{#if workspace.findings.length === 0}
					<p
						class="flex items-center gap-2 rounded-lg border border-emerald-200 bg-emerald-50 p-3 text-sm text-emerald-900"
					>
						<CheckCircle2 class="size-5" />พร้อมสร้างแบบร่าง ข้อมูลต้นทางจะไม่ถูกแก้ไข
					</p>
				{:else}
					<ul class="divide-y rounded-lg border" aria-label="เงื่อนไขการเตรียมภาคเรียน">
						{#each workspace.findings as finding (`${finding.code}:${finding.message}`)}
							<li class="p-3 text-sm">
								<span
									class={finding.severity === 'blocking' ? 'text-destructive' : 'text-amber-800'}
								>
									{finding.severity === 'blocking' ? 'ต้องแก้ก่อน · ' : 'ควรตรวจ · '}
								</span>{finding.message}
							</li>
						{/each}
					</ul>
				{/if}
			{/if}

			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
			<div
				class="flex items-start gap-2 rounded-lg bg-muted/50 p-3 text-xs leading-relaxed text-muted-foreground"
			>
				<CalendarRange class="mt-0.5 size-4 shrink-0" />
				<span
					>การเตรียมนี้ไม่เปลี่ยนตัวเลือกปี/ภาคเรียนบนแถบด้านบน
					และไม่มีแบบร่างใดเริ่มใช้งานจนกว่าจะเผยแพร่ในหน้าของส่วนนั้น</span
				>
			</div>
		</div>

		<Dialog.Footer class="border-t bg-background px-5 py-4">
			<Button variant="outline" disabled={busy} onclick={() => changeOpen(false)}>ปิด</Button>
			<LoadingButton loading={busy} disabled={!canApply} onclick={() => void applyPreparation()}>
				สร้างแบบร่างที่เลือก
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
