<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { page } from '$app/state';
	import { CalendarRange } from '@lucide/svelte';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import type { CreateSupervisionCycleRequest, SupervisionCycle } from '#lib/api/supervision.js';
	import { cycleDateRangeError, cycleDateRangeLabel } from '#lib/supervision/booking-range.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';

	let {
		cycle,
		allowed,
		onsave,
		disabled = false
	}: {
		cycle: SupervisionCycle;
		allowed: boolean;
		disabled?: boolean;
		onsave: (
			dates: Pick<CreateSupervisionCycleRequest, 'observationStartDate' | 'observationEndDate'>
		) => Promise<void>;
	} = $props();
	let open = $state(false);
	let start = $state('');
	let end = $state('');
	let error = $state('');
	let saving = $state(false);
	let epoch = 0;
	let context = '';
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
	function ownerKey() {
		return `${appIdentityKey()}|${page.url.pathname}|${page.url.searchParams.get('academicYearId') ?? ''}|${page.url.searchParams.get('academicTermId') ?? ''}|${cycle.id}|${allowed}`;
	}
	$effect.pre(() => {
		void identity;
		const key = ownerKey();
		untrack(() => {
			if (context === key) return;
			context = key;
			epoch += 1;
			open = false;
			saving = false;
			error = '';
			start = '';
			end = '';
		});
	});
	onDestroy(() => {
		epoch += 1;
	});
	function setOpen(value: boolean) {
		if (saving || (value && (!allowed || disabled))) return;
		if (value && !open) {
			start = cycle.observationStartDate ?? '';
			end = cycle.observationEndDate ?? '';
			error = '';
		}
		open = value;
	}
	async function save() {
		if (!allowed || disabled || saving || !open) return;
		const invalid = cycleDateRangeError(start, end);
		if (invalid) {
			error = invalid;
			return;
		}
		const sourceEpoch = epoch;
		const sourceContext = ownerKey();
		const owns = () => sourceEpoch === epoch && sourceContext === ownerKey();
		saving = true;
		error = '';
		try {
			await onsave({ observationStartDate: start, observationEndDate: end });
			if (!owns()) return;
			open = false;
		} catch (failure) {
			if (owns()) error = failure instanceof Error ? failure.message : 'บันทึกช่วงวันที่ไม่สำเร็จ';
		} finally {
			if (owns()) saving = false;
		}
	}
</script>

<Dialog.Root {open} onOpenChange={setOpen}>
	{#if allowed}<Button
			size="sm"
			variant="outline"
			disabled={disabled || saving}
			onclick={() => setOpen(true)}
			aria-label={`กำหนดช่วงวันที่ ${cycle.title}`}
			><CalendarRange class="size-4" />ช่วงวันที่นิเทศ</Button
		>{/if}
	<Dialog.Content
		class="sm:max-w-lg"
		onEscapeKeydown={(event) => {
			if (saving) event.preventDefault();
		}}
		onInteractOutside={(event) => {
			if (saving) event.preventDefault();
		}}
	>
		<Dialog.Header
			><Dialog.Title>กำหนดช่วงวันที่นิเทศ</Dialog.Title><Dialog.Description
				>รอบ {cycle.title} · {cycleDateRangeLabel(
					cycle.observationStartDate,
					cycle.observationEndDate
				)}</Dialog.Description
			></Dialog.Header
		>
		<form
			class="space-y-4"
			onsubmit={(event) => {
				event.preventDefault();
				void save();
			}}
		>
			<p class="text-sm text-muted-foreground">
				จองได้ตั้งแต่วันเริ่มต้นถึงวันสุดท้าย รวมทั้งสองวัน ผู้ดูแลยังเปิดและปิดรับคำขอด้วยตนเอง
			</p>
			<div class="grid gap-4 sm:grid-cols-2">
				<div class="space-y-2">
					<Label for={`cycle-start-${cycle.id}`}>วันเริ่มนิเทศ</Label><Input
						id={`cycle-start-${cycle.id}`}
						type="date"
						bind:value={start}
						disabled={saving}
						required
					/>
				</div>
				<div class="space-y-2">
					<Label for={`cycle-end-${cycle.id}`}>วันสุดท้ายที่นิเทศ</Label><Input
						id={`cycle-end-${cycle.id}`}
						type="date"
						bind:value={end}
						min={start || undefined}
						disabled={saving}
						required
					/>
				</div>
			</div>
			{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
			<Dialog.Footer
				><Button type="button" variant="outline" disabled={saving} onclick={() => setOpen(false)}
					>ยกเลิก</Button
				><LoadingButton type="submit" loading={saving} loadingLabel="กำลังบันทึก..."
					>{error ? 'ลองบันทึกอีกครั้ง' : 'บันทึกช่วงวันที่'}</LoadingButton
				></Dialog.Footer
			>
		</form>
	</Dialog.Content>
</Dialog.Root>
