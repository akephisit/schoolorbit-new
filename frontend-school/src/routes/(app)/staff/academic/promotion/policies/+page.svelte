<script lang="ts">
	import { onMount } from 'svelte';
	import { Plus, RefreshCw, Trash2, ArrowRight } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { PageShell } from '$lib/components/app-layout';
	import { PageState, PageSkeleton, LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import * as Dialog from '$lib/components/ui/dialog';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { can } from '$lib/stores/permissions';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import PromotionProgressionsDialog from '$lib/components/academic/lifecycle/PromotionProgressionsDialog.svelte';
	import {
		listPromotionPolicies,
		getPromotionPolicyOptions,
		createPromotionPolicy,
		type PromotionPolicy,
		type PromotionRule,
		type PromotionPolicyOptions,
		type PromotionPolicyInput
	} from '$lib/api/academic-promotion';

	type Choice = { value: string; label: string };
	type FormRow = { key: string; input: PromotionRule };
	const listRequest = new LatestRequest();
	const referenceRequest = new LatestRequest();
	let policies = $state.raw<PromotionPolicy[]>([]);
	let references = $state.raw<PromotionPolicyOptions | null>(null);
	let loading = $state(false);
	let loaded = false;
	let loadError = $state('');
	let referenceLoading = $state(false);
	let detailError = $state('');
	let dialogOpen = $state(false);
	let editing = $state(false);
	let reviewing = $state(false);
	let saving = $state(false);
	let progressionOpen = $state(false);
	let selected = $state.raw<PromotionPolicy | null>(null);
	let name = $state('');
	let rows = $state<FormRow[]>([]);
	let reviewedInput = $state.raw<PromotionPolicyInput | null>(null);
	const canRead = $derived($can.has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL));
	const canCreate = $derived(
		$can.hasAll(
			PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_PROMOTION_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_PROMOTION_APPROVE_SCHOOL
		)
	);
	const grades = $derived(
		references?.grades.map((grade) => ({
			value: grade.id,
			label: `${({ kindergarten: 'อ.', primary: 'ป.', secondary: 'ม.' } as Record<string, string>)[grade.levelType] ?? grade.levelType}${grade.year}${grade.isActive ? '' : ' (เลิกใช้)'}`
		})) ?? []
	);
	const programs = $derived(
		references?.programs.map((program) => ({
			value: program.id,
			label: `${program.code} · ${program.name} · รุ่น ${program.versionName}`
		})) ?? []
	);
	const outcomes: Choice[] = [
		{ value: 'promote', label: 'เลื่อนชั้น' },
		{ value: 'graduate', label: 'จบการศึกษา' }
	];
	const levels: Choice[] = [
		{ value: '0', label: '0 · ไม่ผ่าน' },
		{ value: '1', label: '1 · ผ่าน' },
		{ value: '2', label: '2 · ดี' },
		{ value: '3', label: '3 · ดีเยี่ยม' }
	];
	const sourceKeys = $derived(
		rows.map((row) => `${row.input.fromGradeLevelId}/${row.input.fromStudyProgramId}`)
	);
	const duplicate = $derived(new Set(sourceKeys).size !== sourceKeys.length);
	const valid = $derived(
		canCreate &&
			!referenceLoading &&
			!!references &&
			name.trim().length > 0 &&
			name.length <= 200 &&
			rows.length > 0 &&
			rows.length <= 500 &&
			!duplicate &&
			rows.every((row) => !ruleError(row.input))
	);
	const shownRules = $derived(editing ? (reviewedInput?.rules ?? []) : (selected?.rules ?? []));
	function label(choices: Choice[], value: string | null | undefined) {
		return choices.find((choice) => choice.value === value)?.label ?? 'ไม่พบข้อมูลอ้างอิง';
	}
	function mappingMatches(rule: PromotionRule) {
		const program = references?.programs.find((item) => item.id === rule.fromStudyProgramId);
		const matches =
			references?.progressionSet.progressions.filter(
				(mapping) =>
					mapping.isActive &&
					mapping.fromGradeLevelId === rule.fromGradeLevelId &&
					mapping.transitionKind === rule.successOutcome &&
					(!mapping.curriculumId || mapping.curriculumId === program?.curriculumId)
			) ?? [];
		return (
			matches.length === 1 &&
			(matches[0].toGradeLevelId ?? null) === (rule.targetGradeLevelId ?? null)
		);
	}
	function ruleError(rule: PromotionRule): string {
		if (!rule.fromGradeLevelId || !rule.fromStudyProgramId) return 'เลือกชั้นและแผนต้นทาง';
		if (
			rule.successOutcome === 'promote' &&
			(!rule.targetGradeLevelId ||
				!rule.targetStudyProgramId ||
				rule.targetGradeLevelId === rule.fromGradeLevelId)
		)
			return 'เลือกชั้นใหม่และแผนปลายทาง';
		if (
			!/^(0|[1-9]\d*)(\.\d{1,2})?$/.test(rule.minimumEarnedCredits) ||
			rule.minimumEarnedCredits.length > 16
		)
			return 'ระบุหน่วยกิตขั้นต่ำเป็นตัวเลข ทศนิยมไม่เกิน 2 ตำแหน่ง';
		if (!mappingMatches(rule))
			return 'ลำดับชั้นที่ตั้งไว้ยังไม่ตรง หรือมีกฎซ้ำ กรุณาตรวจลำดับชั้นก่อนยืนยัน';
		return '';
	}
	function addRow() {
		if (rows.length >= 500) return;
		rows.push({
			key: crypto.randomUUID(),
			input: {
				fromGradeLevelId: '',
				fromStudyProgramId: '',
				targetGradeLevelId: null,
				targetStudyProgramId: null,
				successOutcome: 'promote',
				minimumEarnedCredits: '',
				requireNoExceptionalOutcomes: true,
				requireActivitiesPassed: true,
				minimumLearnerLevel: 1
			}
		});
	}
	async function loadPolicies() {
		if (!canRead) return;
		const { revision, signal } = listRequest.begin();
		loading = true;
		loadError = '';
		try {
			const data = await listPromotionPolicies({ signal });
			if (listRequest.isCurrent(revision)) {
				policies = data;
				loaded = true;
			}
		} catch (error) {
			if (!isAbortError(error) && listRequest.isCurrent(revision))
				loadError = error instanceof Error ? error.message : 'โหลดเกณฑ์ไม่สำเร็จ';
		} finally {
			if (listRequest.isCurrent(revision)) loading = false;
		}
	}
	async function loadReferences() {
		const { revision, signal } = referenceRequest.begin();
		referenceLoading = true;
		detailError = '';
		try {
			const data = await getPromotionPolicyOptions({ signal });
			if (referenceRequest.isCurrent(revision)) references = data;
		} catch (error) {
			if (!isAbortError(error) && referenceRequest.isCurrent(revision))
				detailError = error instanceof Error ? error.message : 'โหลดชั้นและแผนไม่สำเร็จ';
		} finally {
			if (referenceRequest.isCurrent(revision)) referenceLoading = false;
		}
	}
	function openDetails(policy: PromotionPolicy) {
		selected = policy;
		editing = false;
		reviewing = false;
		dialogOpen = true;
		void loadReferences();
	}
	function openCreate() {
		if (!canCreate) return;
		selected = null;
		editing = true;
		reviewing = false;
		reviewedInput = null;
		name = '';
		rows = [];
		addRow();
		dialogOpen = true;
		void loadReferences();
	}
	function review() {
		if (!valid) return;
		reviewedInput = { name: name.trim(), rules: rows.map((row) => ({ ...row.input })) };
		reviewing = true;
	}
	async function save() {
		if (!canCreate || !reviewedInput || !reviewing || saving) return;
		saving = true;
		detailError = '';
		try {
			const policy = await createPromotionPolicy(reviewedInput);
			policies = [policy, ...policies.filter((row) => row.id !== policy.id)];
			dialogOpen = false;
			toast.success('ยืนยันเกณฑ์รุ่นใหม่แล้ว');
		} catch (error) {
			detailError = error instanceof Error ? error.message : 'ยืนยันเกณฑ์ไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
	onMount(() => {
		const unsubscribe = can.subscribe((permission) => {
			if (permission.has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL) && !loaded && !loading)
				void loadPolicies();
		});
		return () => {
			unsubscribe();
			listRequest.abort();
			referenceRequest.abort();
		};
	});
</script>

{#snippet choice(title: string, value: string, options: Choice[], change: (value: string) => void)}
	<div class="space-y-1.5">
		<Label>{title}</Label>
		<Select.Root type="single" {value} onValueChange={change}>
			<Select.Trigger aria-label={title} class="w-full"
				><span class="truncate">{value ? label(options, value) : 'เลือกข้อมูล'}</span
				></Select.Trigger
			>
			<Select.Content
				>{#each options as option (option.value)}<Select.Item
						value={option.value}
						label={option.label}>{option.label}</Select.Item
					>{/each}</Select.Content
			>
		</Select.Root>
	</div>
{/snippet}

{#snippet ruleTable(rules: PromotionRule[])}
	<div class="overflow-x-auto rounded-xl border">
		<Table.Root class="min-w-[680px]"
			><Table.Header
				><Table.Row>
					<Table.Head>ชั้นและแผนต้นทาง</Table.Head><Table.Head>ผลที่เสนอเมื่อผ่านเกณฑ์</Table.Head
					><Table.Head class="text-right">หน่วยกิตขั้นต่ำ</Table.Head><Table.Head
						>เงื่อนไขของปีนี้</Table.Head
					>
				</Table.Row></Table.Header
			><Table.Body>
				{#each rules as rule (`${rule.fromGradeLevelId}/${rule.fromStudyProgramId}`)}<Table.Row>
						<Table.Cell
							><p class="font-medium">{label(grades, rule.fromGradeLevelId)}</p>
							<p class="text-muted-foreground text-xs">
								{label(programs, rule.fromStudyProgramId)}
							</p></Table.Cell
						>
						<Table.Cell
							>{#if rule.successOutcome === 'graduate'}จบการศึกษา{:else}<p
									class="flex items-center gap-2"
								>
									<ArrowRight class="size-3.5" />{label(grades, rule.targetGradeLevelId)}
								</p>
								<p class="text-muted-foreground text-xs">
									{label(programs, rule.targetStudyProgramId)}
								</p>{/if}</Table.Cell
						>
						<Table.Cell class="text-right font-medium tabular-nums"
							>{rule.minimumEarnedCredits}</Table.Cell
						>
						<Table.Cell class="text-xs"
							><p>
								{rule.requireNoExceptionalOutcomes
									? 'ต้องไม่มี ร/มส หรือผลค้าง'
									: 'ตรวจผลค้างเป็นรายคน'}
							</p>
							<p>{rule.requireActivitiesPassed ? 'กิจกรรมต้องผ่าน' : 'ตรวจผลกิจกรรมเป็นรายคน'}</p>
							<p>ผลประเมินทั้งสองด้าน ≥ {rule.minimumLearnerLevel}</p></Table.Cell
						>
					</Table.Row>{/each}
			</Table.Body></Table.Root
		>
	</div>
{/snippet}

<PageShell
	title="เกณฑ์การเลื่อนชั้น"
	description="ยืนยันเกณฑ์ของโรงเรียนเป็นรุ่น แล้วจึงนำไปตรวจผลรายปีและพิจารณานักเรียนแต่ละคน"
>
	{#snippet actions()}
		<Button variant="outline" disabled={loading || saving} onclick={loadPolicies}
			><RefreshCw class="size-4" />รีเฟรช</Button
		>
		{#if canCreate}<Button onclick={openCreate} disabled={saving}
				><Plus class="size-4" />สร้างเกณฑ์รุ่นใหม่</Button
			>{/if}
	{/snippet}
	{#if !canRead}<PageState variant="permission" title="ไม่มีสิทธิ์ดูเกณฑ์การเลื่อนชั้น" />
	{:else if loadError}<PageState
			variant="error"
			title="โหลดเกณฑ์ไม่สำเร็จ"
			description={loadError}
			actionLabel="ลองใหม่"
			onaction={loadPolicies}
		/>
	{:else if loading && !policies.length}<PageSkeleton variant="table" rows={3} columns={4} />
	{:else if !policies.length}<PageState
			title="ยังไม่มีเกณฑ์ที่ยืนยัน"
			description="ผู้มีสิทธิ์จัดเตรียมและอนุมัติสามารถสร้างเกณฑ์รุ่นแรกได้ ระบบจะไม่กำหนดให้ทุกคนผ่านโดยอัตโนมัติ"
		/>
	{:else}
		<div class="overflow-x-auto rounded-xl border bg-card">
			<Table.Root
				><Table.Header
					><Table.Row
						><Table.Head>เกณฑ์ที่ยืนยัน</Table.Head><Table.Head class="text-right"
							>ชั้น/แผน</Table.Head
						><Table.Head>ยืนยันเมื่อ</Table.Head><Table.Head
							><span class="sr-only">รายละเอียด</span></Table.Head
						></Table.Row
					></Table.Header
				><Table.Body>
					{#each policies as policy (policy.id)}<Table.Row
							><Table.Cell class="font-medium">{policy.name}</Table.Cell><Table.Cell
								class="text-right tabular-nums">{policy.rules.length}</Table.Cell
							><Table.Cell class="whitespace-nowrap text-xs"
								>{new Date(policy.reviewedAt).toLocaleDateString('th-TH')}</Table.Cell
							><Table.Cell class="text-right"
								><Button variant="outline" size="sm" onclick={() => openDetails(policy)}
									>ดูเกณฑ์</Button
								></Table.Cell
							></Table.Row
						>{/each}
				</Table.Body></Table.Root
			>
		</div>
	{/if}
	<p class="text-muted-foreground text-xs">
		เกณฑ์แต่ละรุ่นเก็บไว้ตามที่ยืนยัน การแก้ไขใช้รุ่นใหม่ และไม่เปลี่ยนผลหรือห้องเรียนของนักเรียนเอง
	</p>
</PageShell>

<Dialog.Root
	open={dialogOpen}
	onOpenChange={(open) => {
		if (!saving) dialogOpen = open;
	}}
>
	<Dialog.Content
		class="max-h-[90dvh] overflow-y-auto sm:max-w-5xl"
		showCloseButton={!saving}
		onEscapeKeydown={(event) => {
			if (saving) event.preventDefault();
		}}
		onInteractOutside={(event) => {
			if (saving) event.preventDefault();
		}}
	>
		<Dialog.Header
			><Dialog.Title
				>{editing
					? reviewing
						? 'ตรวจทานเกณฑ์รุ่นใหม่'
						: 'สร้างเกณฑ์รุ่นใหม่'
					: (selected?.name ?? 'รายละเอียดเกณฑ์')}</Dialog.Title
			>
			<Dialog.Description
				>{editing
					? 'ระบุเกณฑ์จากผลรายปี ไม่ใช่ GPAX หรือการรับรองคุณสมบัติจบการศึกษาตามเอกสารราชการ'
					: 'เกณฑ์รุ่นนี้แก้ทับไม่ได้ ชื่อชั้นและแผนแสดงจากทะเบียนปัจจุบัน'}</Dialog.Description
			>
		</Dialog.Header>
		{#if detailError}<PageState
				variant="error"
				title="ดำเนินการไม่สำเร็จ"
				description={detailError}
			/>{/if}
		{#if referenceLoading}<PageSkeleton variant="table" rows={3} columns={2} />
		{:else if !references}<Button variant="outline" onclick={loadReferences}
				>ลองโหลดชั้นและแผนอีกครั้ง</Button
			>
		{:else if editing && !reviewing}
			<div class="space-y-1.5">
				<Label for="promotion-policy-name">ชื่อเกณฑ์</Label><Input
					id="promotion-policy-name"
					maxlength={200}
					bind:value={name}
					placeholder="เช่น เกณฑ์เลื่อนชั้นมัธยมต้น 2569"
				/>
			</div>
			{#each rows as row (row.key)}
				<fieldset class="rounded-xl border bg-card p-3 sm:p-4">
					<legend class="px-1 text-sm font-medium">เกณฑ์ของชั้นและแผนการเรียน</legend>
					<div class="grid gap-3 sm:grid-cols-2">
						{@render choice(
							'ชั้นต้นทาง',
							row.input.fromGradeLevelId,
							grades,
							(value) => (row.input.fromGradeLevelId = value)
						)}
						{@render choice(
							'แผนต้นทาง',
							row.input.fromStudyProgramId,
							programs,
							(value) => (row.input.fromStudyProgramId = value)
						)}
						{@render choice(
							'ผลที่เสนอเมื่อผ่านเกณฑ์',
							row.input.successOutcome,
							outcomes,
							(value) => {
								row.input.successOutcome = value === 'graduate' ? 'graduate' : 'promote';
								if (value === 'graduate') {
									row.input.targetGradeLevelId = null;
									row.input.targetStudyProgramId = null;
								}
							}
						)}
						<div class="space-y-1.5">
							<Label for={`credits-${row.key}`}>หน่วยกิตที่ได้ขั้นต่ำของปีนี้</Label><Input
								id={`credits-${row.key}`}
								inputmode="decimal"
								maxlength={16}
								bind:value={row.input.minimumEarnedCredits}
								placeholder="ระบุตามเกณฑ์โรงเรียน"
							/>
						</div>
						{#if row.input.successOutcome === 'promote'}
							{@render choice(
								'ชั้นปลายทาง',
								row.input.targetGradeLevelId ?? '',
								grades,
								(value) => (row.input.targetGradeLevelId = value)
							)}
							{@render choice(
								'แผนปลายทาง',
								row.input.targetStudyProgramId ?? '',
								programs,
								(value) => (row.input.targetStudyProgramId = value)
							)}
						{/if}
						{@render choice(
							'ผลประเมินขั้นต่ำทั้งสองด้าน',
							String(row.input.minimumLearnerLevel),
							levels,
							(value) => (row.input.minimumLearnerLevel = Number(value))
						)}
						<div class="flex flex-col justify-end gap-3 py-1">
							<div class="flex items-center gap-2">
								<Checkbox
									id={`exception-${row.key}`}
									bind:checked={row.input.requireNoExceptionalOutcomes}
								/><Label for={`exception-${row.key}`}>ต้องไม่มี ร/มส หรือผลค้าง</Label>
							</div>
							<div class="flex items-center gap-2">
								<Checkbox
									id={`activity-${row.key}`}
									bind:checked={row.input.requireActivitiesPassed}
								/><Label for={`activity-${row.key}`}>กิจกรรมต้องผ่าน</Label>
							</div>
						</div>
					</div>
					<div class="mt-3 flex items-start justify-between gap-3">
						<p class="text-muted-foreground text-xs">
							{ruleError(row.input) || 'ข้อมูลแถวนี้พร้อมตรวจทาน'} · ผลรายปีที่พักรอยังต้องพิจารณารายคน
						</p>
						<Button
							variant="ghost"
							size="icon"
							aria-label="ลบแถวเกณฑ์"
							disabled={rows.length === 1}
							onclick={() => (rows = rows.filter((item) => item.key !== row.key))}
							><Trash2 class="size-4" /></Button
						>
					</div>
				</fieldset>
			{/each}
			{#if duplicate}<p class="text-destructive text-sm" role="alert">
					ชั้นและแผนต้นทางซ้ำกัน ให้กำหนดเกณฑ์เดียวต่อชั้นและแผน
				</p>{/if}
			<Button variant="outline" disabled={rows.length >= 500} onclick={addRow}
				><Plus class="size-4" />เพิ่มชั้น/แผนในเกณฑ์</Button
			>
			<div class="rounded-xl border bg-muted/30 p-3 text-xs">
				<div class="flex flex-wrap items-center justify-between gap-2">
					<p class="font-medium">ลำดับชั้นที่โรงเรียนตั้งไว้</p>
					<div class="flex gap-2">
						<Button size="sm" variant="ghost" onclick={loadReferences}>โหลดลำดับชั้นล่าสุด</Button
						>{#if $can.has(PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL)}<Button
								size="sm"
								variant="outline"
								onclick={() => (progressionOpen = true)}>จัดการลำดับชั้น</Button
							>{/if}
					</div>
				</div>
				{#each references.progressionSet.progressions.filter((item) => item.isActive) as mapping (mapping.id)}<p
						class="mt-1"
					>
						{label(grades, mapping.fromGradeLevelId)} → {mapping.transitionKind === 'graduate'
							? 'จบการศึกษา'
							: label(grades, mapping.toGradeLevelId)} · {mapping.curriculumId
							? (references.programs.find(
									(program) => program.curriculumId === mapping.curriculumId
								)?.curriculumName ?? 'หลักสูตรที่กำหนด')
							: 'ทุกหลักสูตร'}
					</p>{/each}
			</div>
		{:else}
			{@render ruleTable(shownRules)}
			{#if reviewing}<p class="rounded-xl border border-primary/20 bg-primary/5 p-3 text-sm">
					การยืนยันนี้ยังไม่ย้ายห้องและไม่สร้างข้อมูลนักเรียนปีใหม่
				</p>{/if}
		{/if}
		<Dialog.Footer>
			{#if editing && reviewing}<Button
					variant="outline"
					disabled={saving}
					onclick={() => (reviewing = false)}>กลับไปแก้ไข</Button
				><LoadingButton loading={saving} disabled={!canCreate || !reviewedInput} onclick={save}
					>ยืนยันเกณฑ์รุ่นใหม่</LoadingButton
				>
			{:else if editing}<Button variant="outline" onclick={() => (dialogOpen = false)}
					>ยกเลิก</Button
				><Button disabled={!valid} onclick={review}>ตรวจทานเกณฑ์</Button>
			{:else}<Button variant="outline" onclick={() => (dialogOpen = false)}>ปิด</Button>{/if}
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

{#if progressionOpen && references}
	<PromotionProgressionsDialog
		options={references}
		{grades}
		onclose={() => (progressionOpen = false)}
		onupdated={(set) => {
			if (references) references = { ...references, progressionSet: set };
		}}
	/>
{/if}
