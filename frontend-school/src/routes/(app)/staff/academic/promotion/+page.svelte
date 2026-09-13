<script lang="ts">
	import { onMount } from 'svelte';
	import { get } from 'svelte/store';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { Plus, RefreshCw, ArrowRight } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { PageShell } from '$lib/components/app-layout';
	import { PageState, PageSkeleton, LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Label } from '$lib/components/ui/label';
	import * as Table from '$lib/components/ui/table';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { can } from '$lib/stores/permissions';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import {
		getAcademicContextStore,
		registerAcademicContextDirtySource
	} from '$lib/academic-context/store';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import {
		listPromotionRuns,
		listPromotionPolicies,
		createPromotionRun,
		type PromotionRun,
		type PromotionPolicy,
		type PromotionRunCreateInput
	} from '$lib/api/academic-promotion';
	import { runStatusLabels } from '$lib/academic/lifecycle/promotion-presentation';

	const context = getAcademicContextStore();
	const requests = new LatestRequest();
	let runs = $state.raw<PromotionRun[]>([]);
	let policies = $state.raw<PromotionPolicy[]>([]);
	let loading = $state(false);
	let loadingPolicies = $state(false);
	let error = $state('');
	let formError = $state('');
	let cursor = $state<string | null>(null);
	let createOpen = $state(false);
	let saving = $state(false);
	let targetYear = $state('');
	let policyId = $state('');
	let loadedYear = '';
	let alive = true;
	let pending: { key: string; input: PromotionRunCreateInput } | null = null;
	const year = $derived($context.selected.academicYearId);
	const years = $derived($context.options?.years ?? []);
	const source = $derived(years.find((row) => row.id === year));
	const targets = $derived(
		years.filter(
			(row) =>
				row.status === 'planning' &&
				!!source &&
				row.year > source.year &&
				row.startDate > source.endDate
		)
	);
	const canRead = $derived($can.has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL));
	const canCreate = $derived(
		$can.hasAll(
			PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_PROMOTION_MANAGE_SCHOOL
		)
	);
	const sourceEligible = $derived(
		!!source && ['active', 'closing', 'closed'].includes(source.status)
	);
	const canSave = $derived(
		canCreate &&
			sourceEligible &&
			targets.some((row) => row.id === targetYear) &&
			policies.some((row) => row.id === policyId) &&
			!loadingPolicies
	);
	function yearName(id: string) {
		return years.find((row) => row.id === id)?.name ?? 'ไม่พบชื่อปีการศึกษา';
	}
	async function loadRuns(more = false) {
		const selected = get(context).selected.academicYearId;
		if (!selected || !get(can).has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL)) return;
		const { revision, signal } = requests.begin();
		loading = true;
		error = '';
		try {
			const data = await listPromotionRuns(
				{ sourceYearId: selected, ...(more && cursor ? { beforeId: cursor } : {}) },
				{ signal }
			);
			if (!requests.isCurrent(revision) || !alive) return;
			runs = more
				? [...runs, ...data.runs.filter((row) => !runs.some((existing) => existing.id === row.id))]
				: data.runs;
			cursor = data.nextCursor ?? null;
		} catch (cause) {
			if (!isAbortError(cause) && requests.isCurrent(revision))
				error = cause instanceof Error ? cause.message : 'โหลดรอบไม่สำเร็จ';
		} finally {
			if (requests.isCurrent(revision)) loading = false;
		}
	}
	async function openCreate() {
		if (!canCreate || !sourceEligible || saving) return;
		targetYear = '';
		policyId = '';
		formError = '';
		pending = null;
		createOpen = true;
		loadingPolicies = true;
		try {
			const data = await listPromotionPolicies();
			if (alive) {
				policies = data;
				if (data.length === 1) policyId = data[0].id;
			}
		} catch (cause) {
			if (alive) formError = cause instanceof Error ? cause.message : 'โหลดเกณฑ์ไม่สำเร็จ';
		} finally {
			if (alive) loadingPolicies = false;
		}
	}
	async function save() {
		if (!canSave || !year || saving) return;
		const values = { sourceYearId: year, targetYearId: targetYear, policyId };
		const key = JSON.stringify(values);
		if (pending?.key !== key)
			pending = { key, input: { ...values, requestId: crypto.randomUUID() } };
		saving = true;
		formError = '';
		try {
			const created = await createPromotionRun(pending.input);
			if (!alive) return;
			runs = [created, ...runs.filter((row) => row.id !== created.id)];
			createOpen = false;
			pending = null;
			toast.success('สร้างรอบเลื่อนชั้นแล้ว');
			await goto(resolve(`/staff/academic/promotion/${created.id}`));
		} catch (cause) {
			if (alive) formError = cause instanceof Error ? cause.message : 'สร้างรอบไม่สำเร็จ';
		} finally {
			if (alive) saving = false;
		}
	}
	onMount(() => {
		const refresh = () => {
			const selected = get(context).selected.academicYearId;
			if (
				selected &&
				get(can).has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL) &&
				loadedYear !== selected
			) {
				loadedYear = selected;
				runs = [];
				cursor = null;
				void loadRuns();
			}
		};
		const unsubscribeContext = context.subscribe(refresh);
		const unsubscribePermissions = can.subscribe(refresh);
		const unregister = registerAcademicContextDirtySource(
			'promotion-create',
			() => createOpen || saving
		);
		return () => {
			alive = false;
			requests.abort();
			unsubscribeContext();
			unsubscribePermissions();
			unregister();
		};
	});
</script>

<PageShell
	title="เลื่อนชั้นและเตรียมปีใหม่"
	description="เลือกปีต้นทาง ตรวจผลรายปี และเตรียมข้อมูลนักเรียนปีถัดไปโดยไม่ย้ายห้องของปีเดิม"
>
	{#snippet actions()}
		<Button variant="outline" href={resolve('/staff/academic/promotion/policies')}
			>เกณฑ์การเลื่อนชั้น</Button
		>
		<Button
			variant="outline"
			onclick={() => loadRuns()}
			disabled={loading || !year || saving}
			aria-label="โหลดรอบใหม่"><RefreshCw class="size-4" /></Button
		>
		{#if canCreate}<Button onclick={openCreate} disabled={!sourceEligible || saving}
				><Plus class="size-4" />สร้างรอบเลื่อนชั้น</Button
			>{/if}
	{/snippet}
	{#if !canRead}<PageState variant="permission" title="ไม่มีสิทธิ์ดูรอบเลื่อนชั้น" />
	{:else if !year}<PageState variant="empty" title="เลือกปีการศึกษาต้นทางที่แถบด้านบน" />
	{:else}
		<div class="rounded-xl border bg-card p-3 text-sm sm:p-4">
			<span class="font-semibold">ต้นทาง: {source?.name ?? 'ปีที่เลือก'}</span>
			<p class="mt-1 text-muted-foreground">
				การเลือกปีด้านบนใช้ดูข้อมูลเท่านั้น ไม่ได้เลื่อนชั้นหรือเปิดปีใหม่
			</p>
			{#if !sourceEligible}<p class="mt-2 text-amber-700 dark:text-amber-400">
					เลือกปีต้นทางที่เปิดเรียน กำลังปิด หรือปิดแล้ว เพื่อสร้างรอบ
				</p>{/if}
		</div>
		{#if error}<PageState variant="error" title="โหลดรอบไม่สำเร็จ" description={error} />{/if}
		{#if loading && !runs.length}<PageSkeleton variant="table" rows={4} columns={4} />
		{:else if !runs.length}<PageState
				variant="empty"
				title="ยังไม่มีรอบเลื่อนชั้นของปีนี้"
				description="เริ่มจากเกณฑ์ที่ฝ่ายวิชาการยืนยัน แล้วสร้างรอบเพื่อคำนวณข้อเสนอ ผลจะยังไม่ถูกนำไปใช้จนกว่าจะอนุมัติและดำเนินการ"
			/>
		{:else}
			<div class="overflow-x-auto rounded-xl border bg-card">
				<Table.Root class="min-w-[620px]">
					<Table.Header
						><Table.Row
							><Table.Head>ปีต้นทาง</Table.Head><Table.Head>ปีที่เตรียม</Table.Head><Table.Head
								>สถานะรอบ</Table.Head
							><Table.Head>สร้างเมื่อ</Table.Head><Table.Head
								><span class="sr-only">รายละเอียด</span></Table.Head
							></Table.Row
						></Table.Header
					>
					<Table.Body
						>{#each runs as run (run.id)}<Table.Row>
								<Table.Cell class="font-medium">{yearName(run.sourceYearId)}</Table.Cell>
								<Table.Cell class="bg-primary/5">{yearName(run.targetYearId)}</Table.Cell>
								<Table.Cell
									><Badge variant={run.status === 'failed' ? 'destructive' : 'outline'}
										>{runStatusLabels[run.status]}</Badge
									></Table.Cell
								>
								<Table.Cell class="text-muted-foreground"
									>{new Date(run.createdAt).toLocaleDateString('th-TH')}</Table.Cell
								>
								<Table.Cell
									><Button
										size="sm"
										variant="ghost"
										href={resolve(`/staff/academic/promotion/${run.id}`)}
										>เปิดรอบ<ArrowRight class="size-4" /></Button
									></Table.Cell
								>
							</Table.Row>{/each}</Table.Body
					>
				</Table.Root>
			</div>
			{#if cursor}<LoadingButton variant="outline" {loading} onclick={() => loadRuns(true)}
					>ดูรอบก่อนหน้า</LoadingButton
				>{/if}
		{/if}
	{/if}
</PageShell>

<Dialog.Root
	open={createOpen}
	onOpenChange={(value) => {
		if (!saving) createOpen = value;
	}}
>
	<Dialog.Content class="max-h-[85dvh] overflow-y-auto">
		<Dialog.Header
			><Dialog.Title>สร้างรอบเลื่อนชั้น</Dialog.Title><Dialog.Description
				>ต้นทาง {source?.name} · ขั้นตอนนี้ยังไม่สร้างข้อมูลนักเรียนปีใหม่</Dialog.Description
			></Dialog.Header
		>
		<div class="space-y-4">
			<div class="space-y-2">
				<Label for="promotion-target-year">ปีการศึกษาปลายทาง</Label><Select.Root
					type="single"
					bind:value={targetYear}
					><Select.Trigger id="promotion-target-year" class="w-full"
						>{targets.find((row) => row.id === targetYear)?.name ??
							'เลือกปีที่กำลังวางแผน'}</Select.Trigger
					><Select.Content
						>{#each targets as target (target.id)}<Select.Item value={target.id}
								>{target.name}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
				{#if !targets.length}<p class="text-sm text-amber-700 dark:text-amber-400">
						ยังไม่มีปีปลายทางที่ใช้ได้ ให้ผู้ดูแลเพิ่มปีที่กำลังวางแผนและตรวจช่วงวันที่ก่อน
					</p>{/if}
			</div>
			<div class="space-y-2">
				<Label for="promotion-policy">เกณฑ์ที่ยืนยันแล้ว</Label><Select.Root
					type="single"
					bind:value={policyId}
					disabled={loadingPolicies}
					><Select.Trigger id="promotion-policy" class="w-full"
						>{loadingPolicies
							? 'กำลังโหลดเกณฑ์…'
							: (policies.find((row) => row.id === policyId)?.name ?? 'เลือกเกณฑ์')}</Select.Trigger
					><Select.Content
						>{#each policies as policy (policy.id)}<Select.Item value={policy.id}
								>{policy.name}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
			</div>
			{#if !loadingPolicies && !policies.length}<p class="text-sm text-muted-foreground">
					ยังไม่มีเกณฑ์ที่ยืนยัน ให้จัดทำในหน้าเกณฑ์การเลื่อนชั้นก่อน
				</p>{/if}
			{#if formError}<p role="alert" class="text-sm text-destructive">{formError}</p>{/if}
		</div>
		<Dialog.Footer
			><Button variant="outline" disabled={saving} onclick={() => (createOpen = false)}>ปิด</Button
			><LoadingButton loading={saving} disabled={!canSave} onclick={save}>สร้างรอบ</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
