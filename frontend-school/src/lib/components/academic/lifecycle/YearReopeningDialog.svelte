<script lang="ts">
	import { onMount } from 'svelte';
	import { ArrowRight, RotateCcw } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { registerAcademicContextDirtySource } from '$lib/academic-context/store';
	import {
		getYearReopeningWorkspace,
		reopenAcademicYear,
		type YearReopeningWorkspace,
		type YearReopeningRequest,
		type YearReopeningOutcome
	} from '$lib/api/academic-lifecycle';
	import { ApiClientError } from '$lib/api/client';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { LoadingButton, PageSkeleton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	let {
		yearId,
		yearName,
		disabled = false,
		onreopened
	}: {
		yearId: string;
		yearName: string;
		disabled?: boolean;
		onreopened: (outcome: YearReopeningOutcome) => Promise<void>;
	} = $props();
	const latest = new LatestRequest();
	let alive = false;
	let open = $state(false);
	let loading = $state(false);
	let busy = $state(false);
	let reason = $state('');
	let errorMessage = $state('');
	let needsRefresh = $state(false);
	let workspace = $state.raw<YearReopeningWorkspace | null>(null);
	let pending: { key: string; request: YearReopeningRequest } | null = null;
	const canRead = $derived($can.has(PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL));
	const canReopen = $derived(canRead && $can.has(PERMISSIONS.ACADEMIC_LIFECYCLE_REOPEN_SCHOOL));
	const reasonLength = $derived(Array.from(reason.trim()).length);
	const confirmable = $derived(
		canReopen &&
			!disabled &&
			!busy &&
			!loading &&
			!needsRefresh &&
			!!workspace?.canReopen &&
			reasonLength > 0 &&
			reasonLength <= 1000
	);

	async function load() {
		if (!canRead || !open || busy) return;
		const { revision, signal } = latest.begin();
		loading = true;
		errorMessage = '';
		try {
			const data = await getYearReopeningWorkspace(yearId, { signal });
			if (!alive || !latest.isCurrent(revision) || !canRead) return;
			workspace = data;
			pending = null;
			needsRefresh = false;
		} catch (error) {
			if (alive && latest.isCurrent(revision) && !isAbortError(error)) {
				errorMessage = error instanceof Error ? error.message : 'ตรวจเงื่อนไขไม่สำเร็จ';
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
			void load();
		} else {
			latest.abort();
			loading = false;
		}
	}
	async function confirm() {
		if (!confirmable || !workspace) return;
		const body = {
			expectedYearVersion: workspace.context.rowVersion,
			sourceChecksum: workspace.sourceChecksum,
			reason: reason.trim()
		};
		const key = JSON.stringify(body);
		if (pending?.key !== key)
			pending = { key, request: { ...body, requestId: crypto.randomUUID() } };
		busy = true;
		errorMessage = '';
		try {
			const outcome = await reopenAcademicYear(yearId, pending.request);
			if (!alive) return;
			pending = null;
			open = false;
			toast.success('เปิดปีเก่ากลับเพื่อตรวจทานแล้ว');
			await onreopened(outcome);
		} catch (error) {
			if (!alive) return;
			errorMessage = error instanceof Error ? error.message : 'เปิดปีเก่ากลับไม่สำเร็จ';
			if (error instanceof ApiClientError && error.status === 409) needsRefresh = true;
		} finally {
			busy = false;
		}
	}
	onMount(() => {
		alive = true;
		const unregister = registerAcademicContextDirtySource(
			`year-reopening:${yearId}`,
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
	variant="outline"
	class="w-full justify-start"
	disabled={disabled || !canRead || busy}
	onclick={() => changeOpen(true)}
>
	<RotateCcw class="size-4" />ตรวจการเปิดปีเก่ากลับ
</Button>
<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content
		class="max-h-[85dvh] overflow-y-auto"
		showCloseButton={!busy}
		onInteractOutside={(event) => {
			if (busy) event.preventDefault();
		}}
		onEscapeKeydown={(event) => {
			if (busy) event.preventDefault();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>ตรวจการเปิดปีเก่ากลับ</Dialog.Title>
			<Dialog.Description>เปิดกลับเพื่อทบทวนการปิดปี ไม่ใช่การเริ่มปีนี้ใหม่</Dialog.Description>
		</Dialog.Header>
		<section
			class="rounded-lg border border-primary/20 bg-primary/5 p-3"
			aria-label="ผลของการเปิดกลับ"
		>
			<p class="font-semibold">{yearName}</p>
			<p class="mt-2 flex items-center gap-2 text-sm">
				<span>ปิดแล้ว</span><ArrowRight class="size-4" /><span class="font-medium text-primary"
					>กำลังตรวจปิดปี</span
				>
			</p>
			<p class="mt-2 text-sm text-muted-foreground">
				ไม่เปลี่ยนคะแนน ผลที่ล็อก ห้องเรียน หรือช่วงเวลาที่เปิดให้กรอกคะแนน
			</p>
		</section>
		{#if loading}<PageSkeleton variant="form" rows={2} />{:else if workspace}
			{#if workspace.findings.length > 0}
				<ul class="divide-y rounded-lg border border-destructive/30">
					{#each workspace.findings as finding (finding.code)}
						<li class="p-3 text-sm">
							<p class="font-medium">{finding.message}</p>
							<p class="mt-1 text-muted-foreground">{finding.count} รายการ</p>
							{#if finding.resolutionUrl}<Button
									variant="link"
									class="h-auto px-0 pb-0"
									href={finding.resolutionUrl}>ไปตรวจสอบ</Button
								>{/if}
						</li>
					{/each}
				</ul>
			{:else}<p class="text-sm">ยังไม่มีข้อมูลปลายทางที่ขัดขวางการเปิดปีนี้กลับ</p>{/if}
		{/if}
		{#if canReopen}
			<div class="space-y-2">
				<Label for="year-reopening-reason">เหตุผลที่เปิดปีเก่ากลับ</Label>
				<Textarea
					id="year-reopening-reason"
					bind:value={reason}
					disabled={busy}
					rows={3}
					placeholder="เช่น ตรวจทานการตั้งค่าก่อนยืนยันปิดปีอีกครั้ง"
				/>
				<p class="text-xs text-muted-foreground">ไม่เกิน 1,000 ตัวอักษร ไม่ใส่เลขประจำตัวประชาชน</p>
			</div>
		{:else}<p class="text-sm text-muted-foreground">
				ดูเงื่อนไขได้ การเปิดกลับต้องใช้สิทธิ์เปิดปีหรือภาคเรียนกลับโดยเฉพาะ
			</p>{/if}
		{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		{#if needsRefresh}<LoadingButton
				variant="outline"
				{loading}
				disabled={busy}
				onclick={() => void load()}>ตรวจเงื่อนไขล่าสุด</LoadingButton
			>{/if}
		<Dialog.Footer>
			<Button variant="outline" disabled={busy} onclick={() => changeOpen(false)}
				>กลับไปตรวจสอบ</Button
			>
			{#if canReopen}<LoadingButton
					loading={busy}
					disabled={!confirmable}
					onclick={() => void confirm()}>ยืนยันเปิดกลับเพื่อตรวจทาน</LoadingButton
				>{/if}
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
