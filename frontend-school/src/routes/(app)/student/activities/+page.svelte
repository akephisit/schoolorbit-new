<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import {
		listActivitySlots,
		listActivityGroups,
		selfEnrollActivity,
		selfUnenrollActivity,
		getMyActivityEnrollments,
		ACTIVITY_TYPE_LABELS,
		type ActivitySlot,
		type ActivityGroup
	} from '$lib/api/academic';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { toast } from 'svelte-sonner';
	import { Users, Check, X } from 'lucide-svelte';

	let loading = $state(true);
	let slots = $state<ActivitySlot[]>([]);
	let groups = $state<ActivityGroup[]>([]);
	let myEnrollments = new SvelteSet<string>();
	let enrolling = $state('');

	onMount(async () => {
		try {
			const [slotsRes, groupsRes, enrollRes] = await Promise.all([
				listActivitySlots({ student_reg_open: true }),
				listActivityGroups({}),
				getMyActivityEnrollments()
			]);
			slots = slotsRes.data ?? [];
			groups = groupsRes.data ?? [];
			myEnrollments.clear();
			for (const id of enrollRes.data ?? []) myEnrollments.add(id);
		} catch (e) {
			console.error(e);
			toast.error('ไม่สามารถโหลดข้อมูลได้');
		} finally {
			loading = false;
		}
	});

	function groupsForSlot(slotId: string) {
		return groups.filter((g) => g.slot_id === slotId && g.is_active);
	}

	function isEnrolledInSlot(slotId: string) {
		return groupsForSlot(slotId).some((g) => myEnrollments.has(g.id));
	}

	function enrolledGroupInSlot(slotId: string) {
		return groupsForSlot(slotId).find((g) => myEnrollments.has(g.id));
	}

	async function handleEnroll(groupId: string) {
		enrolling = groupId;
		try {
			const res = (await selfEnrollActivity(groupId)) as { error?: string };
			if (res.error) {
				toast.error(res.error);
			} else {
				toast.success('ลงทะเบียนสำเร็จ');
				myEnrollments.add(groupId);
			}
		} catch {
			toast.error('เกิดข้อผิดพลาด');
		} finally {
			enrolling = '';
		}
	}

	async function handleUnenroll(groupId: string) {
		if (!confirm('ยกเลิกการลงทะเบียนกิจกรรมนี้?')) return;
		enrolling = groupId;
		try {
			await selfUnenrollActivity(groupId);
			toast.success('ยกเลิกการลงทะเบียนแล้ว');
			myEnrollments.delete(groupId);
		} catch {
			toast.error('เกิดข้อผิดพลาด');
		} finally {
			enrolling = '';
		}
	}
</script>

<svelte:head>
	<title>ลงทะเบียนกิจกรรม</title>
</svelte:head>

<div class="space-y-4 p-4 max-w-2xl mx-auto">
	<div class="flex items-center gap-2">
		<Users class="h-5 w-5" />
		<h1 class="text-xl font-semibold">ลงทะเบียนกิจกรรม</h1>
	</div>

	{#if loading}
		<p class="text-muted-foreground text-sm">กำลังโหลด...</p>
	{:else if slots.length === 0}
		<div class="rounded-lg border p-8 text-center">
			<p class="text-muted-foreground">ไม่มีกิจกรรมที่เปิดลงทะเบียนในขณะนี้</p>
		</div>
	{:else}
		{#each slots as slot (slot.id)}
			{@const slotGroups = groupsForSlot(slot.id)}
			{@const enrolled = isEnrolledInSlot(slot.id)}
			{@const enrolledGroup = enrolledGroupInSlot(slot.id)}

			<div class="rounded-lg border bg-card">
				<div class="p-4 border-b">
					<div class="flex items-center gap-2 flex-wrap">
						<span class="font-semibold">{slot.name}</span>
						<Badge variant="secondary"
							>{ACTIVITY_TYPE_LABELS[slot.activity_type] ?? slot.activity_type}</Badge
						>
					</div>
					{#if enrolled && enrolledGroup}
						<div class="mt-2 flex items-center gap-2 text-sm text-green-600">
							<Check class="h-4 w-4" />
							<span>ลงทะเบียนแล้ว: <strong>{enrolledGroup.name}</strong></span>
						</div>
					{/if}
				</div>

				<div class="divide-y">
					{#each slotGroups as g (g.id)}
						{@const isMyGroup = myEnrollments.has(g.id)}
						{@const isFull = g.max_capacity ? (g.member_count ?? 0) >= g.max_capacity : false}
						<div class="flex items-center gap-3 px-4 py-3 {isMyGroup ? 'bg-green-50' : ''}">
							<div class="flex-1 min-w-0">
								<div class="font-medium text-sm">{g.name}</div>
								<div class="text-xs text-muted-foreground">
									{g.instructor_name ?? '—'}
									· {g.member_count ?? 0}{g.max_capacity ? `/${g.max_capacity}` : ''} คน
									{#if isFull}
										<Badge variant="destructive" class="ml-1 text-[10px]">เต็ม</Badge>
									{/if}
								</div>
							</div>
							<div class="shrink-0">
								{#if isMyGroup}
									<Button
										variant="outline"
										size="sm"
										onclick={() => handleUnenroll(g.id)}
										disabled={enrolling === g.id}
									>
										<X class="mr-1 h-3 w-3" />{enrolling === g.id ? '...' : 'ยกเลิก'}
									</Button>
								{:else if enrolled}
									<span class="text-xs text-muted-foreground">เลือกกิจกรรมอื่นแล้ว</span>
								{:else if isFull}
									<Badge variant="outline">เต็ม</Badge>
								{:else}
									<Button
										size="sm"
										onclick={() => handleEnroll(g.id)}
										disabled={enrolling === g.id}
									>
										{enrolling === g.id ? 'กำลังลงทะเบียน...' : 'ลงทะเบียน'}
									</Button>
								{/if}
							</div>
						</div>
					{/each}
					{#if slotGroups.length === 0}
						<div class="px-4 py-3 text-sm text-muted-foreground">ยังไม่มีกิจกรรมให้เลือก</div>
					{/if}
				</div>
			</div>
		{/each}
	{/if}
</div>
