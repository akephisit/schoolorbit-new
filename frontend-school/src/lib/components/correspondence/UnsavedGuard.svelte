<script lang="ts">
	import { beforeNavigate, goto } from '$app/navigation';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	let { dirty, pending = false }: { dirty: boolean; pending?: boolean } = $props();
	let destination = $state<string | null>(null);
	let open = $state(false);
	let leaving = false;
	$effect(() => {
		if (!dirty && !pending) open = false;
	});
	let navigationError = $state('');
	async function leave() {
		if (!destination) return;
		leaving = true;
		open = false;
		navigationError = '';
		try {
			await goto(destination);
		} catch (cause) {
			leaving = false;
			open = true;
			navigationError = cause instanceof Error ? cause.message : 'ออกจากหน้าไม่สำเร็จ ลองอีกครั้ง';
		}
	}
	beforeNavigate(({ cancel, to, willUnload, shallow }) => {
		if (to && to.url.pathname === location.pathname && to.url.search === location.search) return;
		if (shallow || leaving || (!dirty && !pending) || willUnload) return;
		cancel();
		destination = to?.url.href ?? null;
		open = true;
	});
	function unload(event: BeforeUnloadEvent) {
		if (!dirty && !pending) return;
		event.preventDefault();
		event.returnValue = '';
	}
</script>

<svelte:window onbeforeunload={unload} />
<AlertDialog.Root bind:open>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title
				>{pending ? 'กำลังบันทึกข้อมูล' : 'มีข้อมูลที่ยังไม่ได้บันทึก'}</AlertDialog.Title
			>
			<AlertDialog.Description
				>{pending
					? 'รอให้การบันทึกเสร็จก่อนออกจากหน้านี้'
					: 'บันทึกฉบับร่างก่อนออกเพื่อเก็บข้อมูลและไฟล์ที่เลือก เส้นทางผู้รับที่ยังไม่ส่งจะเก็บอยู่เฉพาะหน้านี้'}</AlertDialog.Description
			>
		</AlertDialog.Header>
		{#if navigationError}<p role="alert" class="text-sm text-destructive">{navigationError}</p>{/if}
		<AlertDialog.Footer>
			<AlertDialog.Cancel>อยู่หน้านี้ต่อ</AlertDialog.Cancel>
			{#if !pending}<Button variant="destructive" onclick={leave}>ออกโดยไม่บันทึก</Button>{/if}
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
