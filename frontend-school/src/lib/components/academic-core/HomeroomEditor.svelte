<script lang="ts">
	import type { Homeroom, HomeroomAdvisor } from '$lib/api/academic-core';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { DoorOpen, Plus, Users } from 'lucide-svelte';

	let {
		academicYearId,
		homerooms,
		gradeLevelOptions,
		programOptions,
		advisorsByHomeroom,
		staffOptions,
		canManage = false,
		onCreate,
		onAddAdvisor
	}: {
		academicYearId: string;
		homerooms: Homeroom[];
		gradeLevelOptions: Array<{ id: string; name: string }>;
		programOptions: Array<{ id: string; name: string }>;
		advisorsByHomeroom: Map<string, HomeroomAdvisor[]>;
		staffOptions: Array<{ id: string; name: string }>;
		canManage?: boolean;
		onCreate: (draft: {
			code: string;
			name: string;
			gradeLevelId: string;
			studyProgramId: string;
			roomNumber: string;
			capacity: number;
		}) => Promise<void>;
		onAddAdvisor: (room: Homeroom, userId: string, role: string) => Promise<void>;
	} = $props();

	let draft = $state({
		code: '',
		name: '',
		gradeLevelId: '',
		studyProgramId: '',
		roomNumber: '',
		capacity: 40
	});
	let busy = $state(false);
	let errorMessage = $state('');
	let advisorDraft = $state({ homeroomId: '', userId: '', role: 'primary' });

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		errorMessage = '';
		try {
			await onCreate(draft);
			draft = { ...draft, code: '', name: '', roomNumber: '' };
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างห้องประจำชั้นไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}

	async function addAdvisor(event: SubmitEvent) {
		event.preventDefault();
		const room = homerooms.find((item) => item.id === advisorDraft.homeroomId);
		if (!room) return;
		busy = true;
		errorMessage = '';
		try {
			await onAddAdvisor(room, advisorDraft.userId, advisorDraft.role);
			advisorDraft = { ...advisorDraft, userId: '', role: 'primary' };
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกครูที่ปรึกษาไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<div class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_340px]">
	<section
		class="grid content-start gap-3 sm:grid-cols-2 2xl:grid-cols-3"
		aria-label="ห้องประจำชั้นในปีที่เลือก"
	>
		{#each homerooms as room (room.id)}
			<article class="rounded-xl border bg-card p-5 shadow-sm">
				<div class="flex items-start justify-between gap-3">
					<div class="rounded-lg bg-primary/10 p-2 text-primary"><DoorOpen class="size-5" /></div>
					<Badge variant="outline">{room.code}</Badge>
				</div>
				<h2 class="mt-4 font-semibold">{room.name}</h2>
				<p class="mt-1 text-xs text-muted-foreground">
					ห้อง {room.roomNumber ?? 'ยังไม่ระบุ'} · ความจุ {room.capacity} คน
				</p>
				<div class="mt-4 flex items-center gap-2 border-t pt-3 text-xs text-muted-foreground">
					<Users class="size-4" /><span
						>ระดับชั้น {room.gradeLevelId} · ที่ปรึกษา {advisorsByHomeroom.get(room.id)?.length ??
							0} คน</span
					>
				</div>
			</article>
		{:else}
			<div
				class="col-span-full rounded-xl border border-dashed p-10 text-center text-sm text-muted-foreground"
			>
				ปีที่เลือกยังไม่มีห้องประจำชั้น
			</div>
		{/each}
	</section>

	{#if canManage}
		<form class="space-y-3 rounded-xl border bg-card p-5 shadow-sm" onsubmit={submit}>
			<div>
				<h2 class="font-semibold">เพิ่มห้องประจำชั้น</h2>
				<p class="text-xs text-muted-foreground">สร้างเฉพาะในปีที่เลือก: {academicYearId}</p>
			</div>
			<div class="grid grid-cols-2 gap-3">
				<div class="space-y-1.5">
					<Label for="homeroom-code">รหัส</Label><Input
						id="homeroom-code"
						bind:value={draft.code}
						required
					/>
				</div>
				<div class="space-y-1.5">
					<Label for="homeroom-number">เลขห้อง</Label><Input
						id="homeroom-number"
						bind:value={draft.roomNumber}
					/>
				</div>
			</div>
			<div class="space-y-1.5">
				<Label for="homeroom-name">ชื่อห้อง</Label><Input
					id="homeroom-name"
					bind:value={draft.name}
					required
				/>
			</div>
			<div class="space-y-1.5">
				<Label for="homeroom-grade">ระดับชั้น</Label><select
					id="homeroom-grade"
					class="h-10 w-full rounded-md border bg-background px-3 text-sm"
					bind:value={draft.gradeLevelId}
					required
					><option value="">เลือกระดับชั้น</option
					>{#each gradeLevelOptions as option (option.id)}<option value={option.id}
							>{option.name}</option
						>{/each}</select
				>
			</div>
			<div class="space-y-1.5">
				<Label for="homeroom-program">แผนการเรียน</Label><select
					id="homeroom-program"
					class="h-10 w-full rounded-md border bg-background px-3 text-sm"
					bind:value={draft.studyProgramId}
					required
					><option value="">เลือกแผน</option>{#each programOptions as option (option.id)}<option
							value={option.id}>{option.name}</option
						>{/each}</select
				>
			</div>
			<div class="space-y-1.5">
				<Label for="homeroom-capacity">ความจุ</Label><Input
					id="homeroom-capacity"
					type="number"
					min="1"
					bind:value={draft.capacity}
					required
				/>
			</div>
			<Button class="w-full" type="submit" disabled={busy}><Plus class="size-4" /> สร้างห้อง</Button
			>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</form>
		<form class="space-y-3 rounded-xl border bg-card p-5 shadow-sm" onsubmit={addAdvisor}>
			<h2 class="font-semibold">เพิ่มครูที่ปรึกษา</h2>
			<label class="space-y-1.5 text-sm"
				><span class="font-medium">ห้องประจำชั้น</span><select
					class="h-10 w-full rounded-md border bg-background px-3"
					bind:value={advisorDraft.homeroomId}
					required
					><option value="">เลือกห้อง</option>{#each homerooms as room (room.id)}<option
							value={room.id}>{room.name}</option
						>{/each}</select
				></label
			>
			<label class="space-y-1.5 text-sm"
				><span class="font-medium">ครู</span><select
					class="h-10 w-full rounded-md border bg-background px-3"
					bind:value={advisorDraft.userId}
					required
					><option value="">เลือกครู</option>{#each staffOptions as staff (staff.id)}<option
							value={staff.id}>{staff.name}</option
						>{/each}</select
				></label
			>
			<div class="space-y-1.5">
				<Label for="advisor-role">บทบาท</Label><Input
					id="advisor-role"
					bind:value={advisorDraft.role}
					required
				/>
			</div>
			<Button class="w-full" type="submit" variant="outline" disabled={busy}
				><Plus class="size-4" /> เพิ่มครูที่ปรึกษา</Button
			>
		</form>
	{/if}
</div>
