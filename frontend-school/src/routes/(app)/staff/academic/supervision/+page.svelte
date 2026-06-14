<script lang="ts">
	import { onMount } from 'svelte';
	import { ClipboardCheck, RefreshCw } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { listSupervisionCycles, type SupervisionCycle } from '$lib/api/supervision';
	import { toast } from 'svelte-sonner';

	let cycles = $state<SupervisionCycle[]>([]);
	let loading = $state(true);

	async function loadCycles() {
		loading = true;
		try {
			cycles = await listSupervisionCycles();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถโหลดรอบนิเทศได้');
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		void loadCycles();
	});
</script>

<svelte:head>
	<title>นิเทศการสอน</title>
</svelte:head>

<section class="space-y-6 p-6">
	<div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
		<div class="space-y-1">
			<div class="flex items-center gap-3">
				<ClipboardCheck class="h-8 w-8 text-primary" />
				<h1 class="text-3xl font-bold tracking-tight">นิเทศการสอน</h1>
			</div>
			<p class="text-sm text-muted-foreground">
				จองคาบนิเทศ ติดตามคำขอ ประเมิน และดูรายงานตามรอบนิเทศของโรงเรียน
			</p>
		</div>

		<Button variant="outline" onclick={loadCycles} disabled={loading}>
			<RefreshCw class="mr-2 h-4 w-4" />
			รีเฟรช
		</Button>
	</div>

	<div class="rounded-lg border bg-background">
		<div class="border-b px-4 py-3">
			<h2 class="font-semibold">รอบนิเทศล่าสุด</h2>
		</div>

		{#if loading}
			<div class="p-6 text-sm text-muted-foreground">กำลังโหลดข้อมูล...</div>
		{:else if cycles.length === 0}
			<div class="p-6 text-sm text-muted-foreground">ยังไม่มีรอบนิเทศที่แสดงได้</div>
		{:else}
			<div class="divide-y">
				{#each cycles as cycle (cycle.id)}
					<div class="flex flex-col gap-2 p-4 md:flex-row md:items-center md:justify-between">
						<div>
							<div class="font-medium">{cycle.title}</div>
							<div class="text-sm text-muted-foreground">
								ปีการศึกษา {cycle.academicYear} / {cycle.semester}
							</div>
						</div>
						<Badge variant="secondary">{cycle.status}</Badge>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</section>
