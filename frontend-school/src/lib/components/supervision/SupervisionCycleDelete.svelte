<script lang="ts">
	import { untrack } from 'svelte';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { Trash2 } from '@lucide/svelte';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		deleteSupervisionCycle,
		getSupervisionCycleDeletionImpact,
		type SupervisionCycle,
		type SupervisionCycleDeletionImpact
	} from '#lib/api/supervision.js';
	import { toast } from 'svelte-sonner';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';

	let {
		cycle,
		allowed,
		ondelete
	}: {
		cycle: SupervisionCycle;
		allowed: boolean;
		ondelete: (impact: SupervisionCycleDeletionImpact) => void;
	} = $props();
	let open = $state(false);
	let impact = $state<SupervisionCycleDeletionImpact | null>(null);
	let confirmation = $state('');
	let loading = $state(false);
	let saving = $state(false);
	let error = $state('');
	const request = new LatestRequest();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
	let context = '';
	function ownerKey() {
		return `${identity}|${cycle.id}|${allowed}`;
	}
	let epoch = 0;
	$effect.pre(() => {
		const key = ownerKey();
		untrack(() => {
			if (context !== key) {
				context = key;
				epoch += 1;
				request.abort();
				open = false;
				impact = null;
				confirmation = '';
				loading = false;
				saving = false;
				error = '';
			}
		});
	});
	$effect(() => () => {
		epoch += 1;
		request.abort();
	});

	async function preview() {
		if (!allowed || !open) return;
		const ticket = request.begin();
		const sourceContext = ownerKey();
		impact = null;
		loading = true;
		error = '';
		try {
			const result = await getSupervisionCycleDeletionImpact(cycle.id, { signal: ticket.signal });
			if (request.isCurrent(ticket.revision) && sourceContext === ownerKey() && open)
				impact = result;
		} catch (failure) {
			if (
				request.isCurrent(ticket.revision) &&
				sourceContext === ownerKey() &&
				!isAbortError(failure)
			)
				error = failure instanceof Error ? failure.message : 'ตรวจผลกระทบไม่สำเร็จ';
		} finally {
			if (request.isCurrent(ticket.revision) && sourceContext === ownerKey()) loading = false;
		}
	}
	function show() {
		if (!allowed || saving) return;
		open = true;
		confirmation = '';
		void preview();
	}
	function setOpen(value: boolean) {
		if (saving) return;
		if (value && !open) {
			show();
			return;
		}
		open = value;
		if (!value) {
			request.abort();
			loading = false;
		}
	}
	async function remove() {
		if (!allowed || !impact || saving || confirmation !== cycle.title) return;
		const sourceEpoch = epoch;
		const sourceContext = ownerKey();
		saving = true;
		error = '';
		try {
			const response = await deleteSupervisionCycle(cycle.id, {
				expectedRevision: impact.revision
			});
			if (sourceEpoch !== epoch || sourceContext !== ownerKey()) return;
			if (!response.success || !response.data) {
				if (response.status === 409) impact = null;
				throw new Error(response.error || 'ลบรอบนิเทศไม่สำเร็จ');
			}
			ondelete(response.data);
			open = false;
			toast.success('ลบรอบนิเทศและข้อมูลของรอบนี้ถาวรแล้ว');
		} catch (failure) {
			if (sourceEpoch === epoch && sourceContext === ownerKey())
				error = failure instanceof Error ? failure.message : 'ลบรอบนิเทศไม่สำเร็จ';
		} finally {
			if (sourceEpoch === epoch && sourceContext === ownerKey()) saving = false;
		}
	}
</script>

<AlertDialog.Root {open} onOpenChange={setOpen}>
	{#if allowed}
		<AlertDialog.Trigger>
			{#snippet child({ props })}
				<Button
					{...props}
					size="sm"
					variant="ghost"
					class="text-destructive hover:text-destructive"
					disabled={saving}
					aria-label={`ลบรอบ ${cycle.title} ถาวร`}><Trash2 class="size-4" />ลบถาวร</Button
				>
			{/snippet}
		</AlertDialog.Trigger>
	{/if}
	<AlertDialog.Content
		class="max-h-[calc(100dvh-2rem)] overflow-y-auto overscroll-contain p-4 sm:max-w-lg sm:p-6"
	>
		<AlertDialog.Header class="text-start">
			<AlertDialog.Title>ลบรอบนิเทศถาวร</AlertDialog.Title>
			<AlertDialog.Description class="wrap-anywhere">
				รอบ “{cycle.title}” พร้อมคำขอ ผลประเมิน และข้อมูลของรอบนี้จะถูกลบถาวร
			</AlertDialog.Description>
		</AlertDialog.Header>
		{#if loading}<PageSkeleton rows={3} columns={2} />{/if}
		{#if error}<PageState
				variant="error"
				title="ยังลบรอบไม่ได้"
				description={error}
				actionLabel="ตรวจผลกระทบอีกครั้ง"
				onaction={() => void preview()}
			/>{/if}
		{#if impact}
			<h3 class="text-sm font-semibold">ข้อมูลที่จะถูกลบ</h3>
			<dl
				class="grid grid-cols-2 gap-3 rounded-lg border border-destructive/20 bg-destructive/5 p-3 text-sm sm:grid-cols-3 sm:p-4"
				data-testid="supervision-delete-impact"
			>
				{#each [{ label: 'เป้าหมาย', count: impact.counts.targets }, { label: 'คำขอ/รายการนิเทศ', count: impact.counts.observations }, { label: 'คาบนิเทศ', count: impact.counts.periods }, { label: 'ผู้ประเมินที่มอบหมาย', count: impact.counts.evaluators }, { label: 'คำตอบประเมิน', count: impact.counts.responses }, { label: 'บันทึกการดำเนินการ', count: impact.counts.actions }] as item (item.label)}<div
					>
						<dt class="text-muted-foreground">{item.label}</dt>
						<dd class="text-lg font-semibold tabular-nums">{item.count}</dd>
					</div>{/each}
			</dl>
			<p class="text-sm text-muted-foreground">แบบประเมินกลาง บัญชีครู และตารางสอนยังคงอยู่</p>
			<div class="space-y-2">
				<Label for={`delete-cycle-${cycle.id}`} class="wrap-anywhere"
					>พิมพ์ชื่อรอบ “{cycle.title}” เพื่อยืนยัน</Label
				><Input
					id={`delete-cycle-${cycle.id}`}
					bind:value={confirmation}
					autocomplete="off"
					disabled={saving}
				/>
			</div>
		{/if}
		<AlertDialog.Footer class="border-t pt-4"
			><AlertDialog.Cancel disabled={saving}>ยกเลิก</AlertDialog.Cancel><LoadingButton
				variant="destructive"
				loading={saving}
				loadingLabel="กำลังลบ..."
				disabled={!impact || loading || confirmation !== cycle.title || !allowed}
				onclick={() => void remove()}>ยืนยันลบถาวร</LoadingButton
			></AlertDialog.Footer
		>
	</AlertDialog.Content>
</AlertDialog.Root>
