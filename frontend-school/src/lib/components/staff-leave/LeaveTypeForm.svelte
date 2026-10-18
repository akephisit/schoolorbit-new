<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { saveLeaveType, type LeaveType, type LeaveKind } from '#lib/api/staff-leave.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	let {
		type,
		oncancel,
		onsaved
	}: { type?: LeaveType; oncancel: () => void; onsaved: (type: LeaveType) => void } = $props();
	let name = $state(untrack(() => type?.name ?? '')),
		code = $state(untrack(() => type?.code ?? '')),
		kind = $state<LeaveKind>(untrack(() => type?.kind ?? 'leave'));
	let error = $state(''),
		pending = $state(false),
		disposed = false;
	const intent = new LeaveMutationIntent();
	onDestroy(() => (disposed = true));
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		error = '';
		if (!name.trim() || !code.trim()) {
			error = 'กรอกรหัสและชื่อประเภทคำขอ';
			return;
		}
		pending = true;
		const basis = {
			expectedRevision: type?.revision ?? null,
			code: code.trim(),
			name: name.trim(),
			kind,
			archived: type?.archived ?? false
		};
		try {
			const result = await saveLeaveType({ requestId: intent.for(basis), ...basis }, type?.id);
			if (!disposed) onsaved(result);
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'บันทึกประเภทคำขอไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form class="space-y-4" onsubmit={save} data-testid="leave-type-form">
	<div class="space-y-2">
		<Label for="leave-type-code">รหัสประเภท *</Label><Input
			id="leave-type-code"
			bind:value={code}
			disabled={pending || !!type}
		/>{#if type}<p class="text-xs text-muted-foreground">
				รหัสและกลุ่มของประเภทที่บันทึกแล้วเปลี่ยนไม่ได้
			</p>{/if}
	</div>
	<div class="space-y-2">
		<Label for="leave-type-name">ชื่อประเภท *</Label><Input
			id="leave-type-name"
			bind:value={name}
			disabled={pending}
		/>
	</div>
	<div class="space-y-2">
		<Label>กลุ่มคำขอ *</Label><ToggleGroup.Root
			class="flex-wrap"
			type="single"
			aria-label="กลุ่มคำขอ"
			value={kind}
			onValueChange={(value) => {
				if (value === 'leave' || value === 'exit') kind = value;
			}}
			disabled={pending || !!type}
			><ToggleGroup.Item value="leave">การลา</ToggleGroup.Item><ToggleGroup.Item value="exit"
				>ออกนอกโรงเรียน</ToggleGroup.Item
			></ToggleGroup.Root
		>
	</div>
	{#if error}<PageState
			variant="error"
			title="บันทึกประเภทคำขอไม่สำเร็จ"
			description={error}
		/>{/if}
	<div class="flex justify-end gap-2">
		<Button type="button" variant="outline" disabled={pending} onclick={oncancel}>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending}>บันทึกประเภท</LoadingButton>
	</div>
</form>
