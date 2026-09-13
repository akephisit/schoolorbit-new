<script lang="ts">
	import { onMount } from 'svelte';
	import { ArrowRight, ArrowUpRight, CheckCircle2, PlayCircle } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { registerAcademicContextDirtySource } from '$lib/academic-context/store';
	import {
		getTermActivationWorkspace,
		transitionAcademicTerm,
		type TermActivationWorkspace,
		type TermTransitionOutcome,
		type TermTransitionRequest
	} from '$lib/api/academic-lifecycle';
	import { ApiClientError } from '$lib/api/client';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { LoadingButton, PageSkeleton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	let {
		yearId,
		termId,
		disabled = false,
		onactivated
	}: {
		yearId: string;
		termId: string;
		disabled?: boolean;
		onactivated: (outcome: TermTransitionOutcome) => Promise<void>;
	} = $props();
	const latest = new LatestRequest();
	let alive = false;
	let open = $state(false);
	let loading = $state(false);
	let busy = $state(false);
	let errorMessage = $state('');
	let needsRefresh = $state(false);
	let workspace = $state.raw<TermActivationWorkspace | null>(null);
	let pending: { key: string; request: TermTransitionRequest } | null = null;
	const canRead = $derived($can.has(PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL));
	const canActivate = $derived(canRead && $can.has(PERMISSIONS.ACADEMIC_LIFECYCLE_ACTIVATE_SCHOOL));
	const confirmable = $derived(
		canActivate && !disabled && !busy && !loading && !needsRefresh && !!workspace?.canActivate
	);

	async function load() {
		if (!canRead || !open || busy) return;
		const { revision, signal } = latest.begin();
		loading = true;
		errorMessage = '';
		try {
			const data = await getTermActivationWorkspace(termId, { academicYearId: yearId }, { signal });
			if (!alive || !latest.isCurrent(revision) || !canRead) return;
			workspace = data;
			pending = null;
			needsRefresh = false;
		} catch (error) {
			if (alive && latest.isCurrent(revision) && !isAbortError(error)) {
				errorMessage = error instanceof Error ? error.message : 'ตรวจความพร้อมเปิดใช้ไม่สำเร็จ';
				needsRefresh = true;
			}
		} finally {
			if (latest.isCurrent(revision)) loading = false;
		}
	}

	function changeOpen(value: boolean) {
		if (busy) return;
		open = value;
		if (value) {
			workspace = null;
			pending = null;
			needsRefresh = false;
			void load();
		} else {
			latest.abort();
			loading = false;
		}
	}

	async function confirm() {
		if (!confirmable || !workspace) return;
		const body = {
			academicYearId: workspace.context.academicYearId,
			action: 'activate' as const,
			expectedYearVersion: workspace.context.yearRowVersion,
			expectedTermVersion: workspace.context.termRowVersion,
			readinessChecksum: workspace.sourceChecksum,
			acknowledgedWarningCodes: [],
			closedOn: null,
			reason: null
		};
		const key = JSON.stringify(body);
		if (pending?.key !== key)
			pending = { key, request: { ...body, requestId: crypto.randomUUID() } };
		busy = true;
		errorMessage = '';
		try {
			const outcome = await transitionAcademicTerm(termId, pending.request);
			if (!alive) return;
			pending = null;
			open = false;
			toast.success(
				workspace.opensYear ? 'เริ่มใช้ปีการศึกษาและภาคเรียนแล้ว' : 'เริ่มใช้ภาคเรียนแล้ว'
			);
			try {
				await onactivated(outcome);
			} catch {
				toast.warning('บันทึกสำเร็จแล้ว แต่โหลดบริบทล่าสุดไม่สำเร็จ กรุณารีเฟรชหน้า');
			}
		} catch (error) {
			if (!alive) return;
			errorMessage = error instanceof Error ? error.message : 'เริ่มใช้ภาคเรียนไม่สำเร็จ';
			if (error instanceof ApiClientError && error.status === 409) needsRefresh = true;
		} finally {
			busy = false;
		}
	}

	onMount(() => {
		alive = true;
		const unregister = registerAcademicContextDirtySource(
			`term-activation:${termId}`,
			() => open || busy
		);
		const unsubscribe = can.subscribe((permissions) => {
			if (!permissions.has(PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL)) {
				open = false;
				workspace = null;
				latest.abort();
			}
		});
		return () => {
			alive = false;
			latest.abort();
			unregister();
			unsubscribe();
		};
	});
</script>

<Button
	variant={canActivate ? 'default' : 'outline'}
	class="w-full justify-start"
	disabled={disabled || !canRead || busy}
	onclick={() => changeOpen(true)}
>
	<PlayCircle class="size-4" />{canActivate ? 'เริ่มใช้ภาคเรียนนี้' : 'ตรวจความพร้อมเปิดใช้'}
</Button>

<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content
		class="max-h-[88dvh] overflow-y-auto sm:max-w-2xl"
		showCloseButton={!busy}
		onInteractOutside={(event) => {
			if (busy) event.preventDefault();
		}}
		onEscapeKeydown={(event) => {
			if (busy) event.preventDefault();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>ตรวจความพร้อมก่อนเริ่มใช้</Dialog.Title>
			<Dialog.Description>
				ตรวจข้อมูลล่าสุดทุกครั้ง การเลือกดูปีและภาคเรียนบนแถบด้านบนยังไม่เปิดใช้งานจริง
			</Dialog.Description>
		</Dialog.Header>

		{#if loading}
			<PageSkeleton variant="form" rows={3} />
		{:else if workspace}
			<section class="overflow-hidden rounded-xl border" aria-label="ขอบเขตที่จะเปิดใช้">
				<div class="border-b bg-primary/5 p-4">
					<p class="text-xs font-medium tracking-wide text-primary">คำสั่งนี้จะเปลี่ยนสถานะจริง</p>
					<h3 class="mt-1 text-lg font-semibold">
						{workspace.opensYear
							? 'เปิดปีการศึกษาและภาคเรียนพร้อมกัน'
							: 'เปิดภาคเรียนถัดไปในปีการศึกษาเดิม'}
					</h3>
					<p class="mt-1 text-sm text-muted-foreground">
						{workspace.context.yearName} · {workspace.context.termName}
					</p>
				</div>
				<div class="grid gap-3 p-4 sm:grid-cols-2">
					<div class="flex items-center gap-2 text-sm">
						<span class="rounded-md bg-muted px-2 py-1">พร้อม</span><ArrowRight class="size-4" />
						<span class="rounded-md bg-primary px-2 py-1 text-primary-foreground">ใช้งาน</span>
					</div>
					{#if workspace.opensYear}
						<div class="text-sm sm:text-right">
							<p>นักเรียนที่วางแผนไว้ {workspace.plannedStudents} คน</p>
							<p class="text-muted-foreground">
								จัดเข้าห้องที่มีผลวันเปิดภาค {workspace.eligiblePlacements} คน
							</p>
						</div>
					{/if}
				</div>
			</section>

			{#if workspace.findings.length === 0}
				<p
					class="flex items-center gap-2 rounded-lg border border-emerald-200 bg-emerald-50 p-3 text-sm text-emerald-900"
				>
					<CheckCircle2 class="size-5" />ไม่พบเงื่อนไขที่ขัดขวางการเปิดใช้
				</p>
			{:else}
				<ul class="divide-y rounded-lg border" aria-label="รายการที่ต้องตรวจสอบ">
					{#each workspace.findings as finding (finding.code)}
						<li class="flex flex-wrap items-start gap-3 p-3">
							<span
								class={[
									'rounded-md px-2 py-1 text-xs font-medium',
									finding.severity === 'blocking'
										? 'bg-destructive/10 text-destructive'
										: 'bg-amber-50 text-amber-900'
								]}
							>
								{finding.severity === 'blocking' ? 'ต้องแก้ก่อน' : 'ควรตรวจสอบ'}
							</span>
							<div class="min-w-0 flex-1">
								<p class="text-sm font-medium">{finding.message}</p>
								<p class="mt-1 text-xs text-muted-foreground">{finding.count} รายการ</p>
							</div>
							{#if finding.resolutionUrl}
								<Button variant="ghost" size="sm" href={finding.resolutionUrl}>
									ไปแก้ข้อมูล<ArrowUpRight class="size-4" />
								</Button>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}

			<div class="rounded-lg bg-muted/50 p-3 text-sm">
				<p class="font-medium">เกณฑ์เสริมที่ใช้กับการตรวจครั้งนี้</p>
				<p class="mt-1 text-muted-foreground">
					ห้องประจำชั้น {workspace.policy.requireHomeroomPlacements ? 'ต้องครบ' : 'ไม่บังคับ'} · รายการเปิดสอน
					{workspace.policy.requirePublishedOfferings ? 'ต้องมีที่เผยแพร่' : 'ไม่บังคับ'}
					· ตารางสอน {workspace.policy.requirePublishedTimetable ? 'ต้องมีที่เผยแพร่' : 'ไม่บังคับ'}
				</p>
			</div>
			<p class="text-xs text-muted-foreground">
				คำสั่งนี้ไม่เปลี่ยนช่วงเปิดกรอกคะแนน ไม่ปลดล็อกผล
				และไม่สร้างรายการเปิดสอนหรือตารางสอนแทนผู้ใช้
			</p>
		{/if}

		{#if !canActivate && workspace}
			<p class="text-sm text-muted-foreground">
				บัญชีนี้ตรวจความพร้อมได้ แต่ไม่มีสิทธิ์เริ่มใช้ปีหรือภาคเรียน
			</p>
		{/if}
		{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		{#if needsRefresh}
			<LoadingButton variant="outline" {loading} disabled={busy} onclick={() => void load()}>
				ตรวจข้อมูลล่าสุด
			</LoadingButton>
		{/if}
		<Dialog.Footer>
			<Button variant="outline" disabled={busy} onclick={() => changeOpen(false)}
				>กลับไปตรวจสอบ</Button
			>
			{#if canActivate}
				<LoadingButton loading={busy} disabled={!confirmable} onclick={() => void confirm()}>
					ยืนยันเริ่มใช้ภาคเรียนนี้
				</LoadingButton>
			{/if}
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
