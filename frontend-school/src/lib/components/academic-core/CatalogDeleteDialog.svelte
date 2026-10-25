<script lang="ts">
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Trash2 } from '@lucide/svelte';

	let {
		kind,
		name,
		canDelete,
		onDelete
	}: {
		kind: 'subject' | 'activity';
		name: string;
		canDelete: boolean;
		onDelete: () => Promise<void>;
	} = $props();

	let open = $state(false);
	let deleting = $state(false);
	let errorMessage = $state('');
	const label = $derived(kind === 'subject' ? 'รายวิชา' : 'กิจกรรม');

	function changeOpen(value: boolean) {
		if (deleting) return;
		open = value;
		if (value) errorMessage = '';
	}

	async function confirm() {
		if (deleting || !canDelete) return;
		deleting = true;
		errorMessage = '';
		try {
			await onDelete();
			open = false;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : `ลบ${label}ไม่สำเร็จ`;
		} finally {
			deleting = false;
		}
	}
</script>

{#if canDelete}
	<Button variant="destructive" size="sm" onclick={() => changeOpen(true)}>
		<Trash2 class="size-4" aria-hidden="true" />ลบ{label}
	</Button>
{/if}

<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content
		class="sm:max-w-md"
		showCloseButton={!deleting}
		onInteractOutside={(event) => {
			if (deleting) event.preventDefault();
		}}
		onEscapeKeydown={(event) => {
			if (deleting) event.preventDefault();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>ยืนยันลบ{label}</Dialog.Title>
			<Dialog.Description>
				ลบ “{name}” และรุ่นร่างทั้งหมดออกจากทะเบียนอย่างถาวร การดำเนินการนี้ย้อนกลับไม่ได้
			</Dialog.Description>
		</Dialog.Header>
		{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		<Dialog.Footer>
			<Button variant="outline" disabled={deleting} onclick={() => changeOpen(false)}>ยกเลิก</Button
			>
			<LoadingButton
				variant="destructive"
				loading={deleting}
				disabled={!canDelete}
				loadingLabel="กำลังลบ..."
				onclick={confirm}>ยืนยันลบ{label}</LoadingButton
			>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
