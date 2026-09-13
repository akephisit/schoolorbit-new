<script lang="ts">
	import { onMount } from 'svelte';
	import { Settings2 } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { registerAcademicContextDirtySource } from '$lib/academic-context/store';
	import {
		getAcademicOpeningPolicy,
		updateAcademicOpeningPolicy,
		type OpeningPolicy
	} from '$lib/api/academic-lifecycle';
	import { ApiClientError } from '$lib/api/client';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { LoadingButton, PageSkeleton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Label } from '$lib/components/ui/label';
	import { Switch } from '$lib/components/ui/switch';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	let { disabled = false, onupdated }: { disabled?: boolean; onupdated?: () => Promise<void> } =
		$props();
	const latest = new LatestRequest();
	let alive = false;
	let open = $state(false);
	let loading = $state(false);
	let busy = $state(false);
	let errorMessage = $state('');
	let needsRefresh = $state(false);
	let policy = $state.raw<OpeningPolicy | null>(null);
	let requireHomeroomPlacements = $state(false);
	let requirePublishedOfferings = $state(false);
	let requirePublishedTimetable = $state(false);
	const canManage = $derived(
		$can.has(PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL) &&
			$can.has(PERMISSIONS.ACADEMIC_LIFECYCLE_MANAGE_SCHOOL)
	);
	const changed = $derived(
		!!policy &&
			(policy.requireHomeroomPlacements !== requireHomeroomPlacements ||
				policy.requirePublishedOfferings !== requirePublishedOfferings ||
				policy.requirePublishedTimetable !== requirePublishedTimetable)
	);

	function applyPolicy(next: OpeningPolicy) {
		policy = next;
		requireHomeroomPlacements = next.requireHomeroomPlacements;
		requirePublishedOfferings = next.requirePublishedOfferings;
		requirePublishedTimetable = next.requirePublishedTimetable;
	}

	async function load() {
		if (!canManage || !open || busy) return;
		const { revision, signal } = latest.begin();
		loading = true;
		errorMessage = '';
		try {
			const next = await getAcademicOpeningPolicy({ signal });
			if (!alive || !latest.isCurrent(revision) || !canManage) return;
			applyPolicy(next);
			needsRefresh = false;
		} catch (error) {
			if (alive && latest.isCurrent(revision) && !isAbortError(error)) {
				errorMessage = error instanceof Error ? error.message : 'โหลดเกณฑ์เปิดภาคเรียนไม่สำเร็จ';
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
			policy = null;
			needsRefresh = false;
			void load();
		} else {
			latest.abort();
			loading = false;
		}
	}

	async function save() {
		if (!canManage || !policy || !changed || busy || needsRefresh) return;
		busy = true;
		errorMessage = '';
		try {
			const saved = await updateAcademicOpeningPolicy({
				rowVersion: policy.rowVersion,
				requireHomeroomPlacements,
				requirePublishedOfferings,
				requirePublishedTimetable
			});
			if (!alive) return;
			applyPolicy(saved);
			open = false;
			toast.success('บันทึกเกณฑ์เปิดภาคเรียนแล้ว');
			try {
				await onupdated?.();
			} catch {
				toast.warning('บันทึกสำเร็จแล้ว แต่โหลดสถานะล่าสุดไม่สำเร็จ กรุณารีเฟรชหน้า');
			}
		} catch (error) {
			if (!alive) return;
			errorMessage = error instanceof Error ? error.message : 'บันทึกเกณฑ์ไม่สำเร็จ';
			if (error instanceof ApiClientError && error.status === 409) needsRefresh = true;
		} finally {
			busy = false;
		}
	}

	onMount(() => {
		alive = true;
		const unregister = registerAcademicContextDirtySource('opening-policy', () => open || busy);
		const unsubscribe = can.subscribe((permissions) => {
			if (
				!permissions.has(PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL) ||
				!permissions.has(PERMISSIONS.ACADEMIC_LIFECYCLE_MANAGE_SCHOOL)
			) {
				open = false;
				policy = null;
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

{#if canManage}
	<Button variant="outline" disabled={disabled || busy} onclick={() => changeOpen(true)}>
		<Settings2 class="size-4" />เกณฑ์เปิดภาคเรียน
	</Button>
{/if}

<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content
		class="max-h-[85dvh] overflow-y-auto sm:max-w-lg"
		showCloseButton={!busy}
		onInteractOutside={(event) => {
			if (busy) event.preventDefault();
		}}
		onEscapeKeydown={(event) => {
			if (busy) event.preventDefault();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>เกณฑ์เสริมก่อนเปิดภาคเรียน</Dialog.Title>
			<Dialog.Description>
				เลือกเฉพาะหลักฐานที่โรงเรียนต้องการบังคับเพิ่ม โครงสร้างปีและลำดับภาคเรียนยังถูกตรวจเสมอ
			</Dialog.Description>
		</Dialog.Header>
		{#if loading}
			<PageSkeleton variant="form" rows={3} />
		{:else if policy}
			<div class="divide-y overflow-hidden rounded-xl border">
				<div class="flex items-start justify-between gap-4 p-4">
					<div>
						<Label for="opening-require-homeroom">ต้องจัดห้องประจำชั้นครบ</Label>
						<p class="mt-1 text-xs text-muted-foreground">บังคับเฉพาะนักเรียนที่วางแผนเปิดปีนี้</p>
					</div>
					<Switch
						id="opening-require-homeroom"
						bind:checked={requireHomeroomPlacements}
						disabled={busy}
					/>
				</div>
				<div class="flex items-start justify-between gap-4 p-4">
					<div>
						<Label for="opening-require-offering">ต้องมีรายการเปิดสอนที่เผยแพร่</Label>
						<p class="mt-1 text-xs text-muted-foreground">
							ตรวจว่ามีอย่างน้อยหนึ่งรายการ ไม่ได้ยืนยันว่าครบทั้งหลักสูตร
						</p>
					</div>
					<Switch
						id="opening-require-offering"
						bind:checked={requirePublishedOfferings}
						disabled={busy}
					/>
				</div>
				<div class="flex items-start justify-between gap-4 p-4">
					<div>
						<Label for="opening-require-timetable">ต้องมีตารางสอนที่เผยแพร่</Label>
						<p class="mt-1 text-xs text-muted-foreground">
							ตารางต้องมีผลในวันเปิดภาคและใช้ตารางคาบของภาคนี้
						</p>
					</div>
					<Switch
						id="opening-require-timetable"
						bind:checked={requirePublishedTimetable}
						disabled={busy}
					/>
				</div>
			</div>
		{/if}
		{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		{#if needsRefresh}
			<LoadingButton variant="outline" {loading} disabled={busy} onclick={() => void load()}>
				โหลดเกณฑ์ล่าสุด
			</LoadingButton>
		{/if}
		<Dialog.Footer>
			<Button variant="outline" disabled={busy} onclick={() => changeOpen(false)}>ยกเลิก</Button>
			<LoadingButton loading={busy} disabled={!changed || needsRefresh} onclick={() => void save()}>
				บันทึกเกณฑ์
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
