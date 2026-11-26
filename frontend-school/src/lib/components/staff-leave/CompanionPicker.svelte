<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import { listLeaveCompanionOptions, type LeaveRoutingUser } from '#lib/api/staff-leave.js';
	import { LeaveRegion } from '#lib/staff-leave/region.svelte.js';
	import { PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { companionPageSize, companionMaxOffset } from '#lib/staff-leave/filters.js';

	let { chosen = $bindable(), disabled }: { chosen: LeaveRoutingUser[]; disabled: boolean } =
		$props();
	const region = new LeaveRegion<LeaveRoutingUser[]>();
	const maxCompanions = 100;
	let open = $state(false);
	let search = $state('');
	let appliedSearch = $state('');
	let offset = $state(0);
	let searchInput = $state<HTMLInputElement | null>(null);

	onDestroy(() => region.dispose());

	function read() {
		const query = { search: appliedSearch, offset };
		region.retry((signal) => listLeaveCompanionOptions({ signal, query }));
	}

	function searchNames() {
		appliedSearch = search.trim();
		offset = 0;
		read();
	}

	function changePage(next: number) {
		offset = next;
		read();
	}

	async function showPicker() {
		open = true;
		if (!region.value && !region.error) read();
		await tick();
		searchInput?.focus();
	}

	function add(user: LeaveRoutingUser) {
		if (chosen.length < maxCompanions && !chosen.some((person) => person.id === user.id))
			chosen = [...chosen, user];
	}
</script>

<fieldset class="space-y-3 rounded-lg border p-3" data-testid="leave-companion-picker">
	<legend class="px-1 text-sm font-medium">ผู้ร่วมเดินทาง</legend>
	<p class="text-sm text-muted-foreground">
		หลังส่งคำขอ ผู้ร่วมเดินทางแต่ละคนจะได้รับคำเชิญให้ยืนยัน
	</p>
	{#if chosen.length}
		<ul class="divide-y rounded-md border" aria-label="ผู้ร่วมเดินทางที่เลือก">
			{#each chosen as user (user.id)}
				<li class="flex flex-wrap items-center justify-between gap-2 px-3 py-1">
					<span class="min-w-0 break-words text-sm">{user.displayName}</span>
					<Button
						type="button"
						variant="ghost"
						size="sm"
						{disabled}
						aria-label={`นำ ${user.displayName} ออกจากผู้ร่วมเดินทาง`}
						onclick={() => (chosen = chosen.filter((person) => person.id !== user.id))}
						>นำออก</Button
					>
				</li>
			{/each}
		</ul>
	{/if}
	{#if !open}
		<Button type="button" variant="outline" {disabled} onclick={showPicker}
			>เพิ่มผู้ร่วมเดินทาง</Button
		>
	{:else}
		<div class="space-y-3">
			<div class="space-y-2">
				<Label for="leave-companion-search">ค้นหาชื่อบุคลากร</Label>
				<div class="flex flex-wrap gap-2">
					<Input
						id="leave-companion-search"
						bind:ref={searchInput}
						bind:value={search}
						{disabled}
						class="min-w-40 flex-1"
						maxlength={100}
						type="search"
						onkeydown={(event) => {
							if (event.key === 'Enter' && !event.isComposing) {
								event.preventDefault();
								searchNames();
							}
						}}
					/>
					<LoadingButton
						type="button"
						variant="outline"
						loading={region.loading}
						{disabled}
						onclick={searchNames}>ค้นหา</LoadingButton
					>
				</div>
			</div>
			<div aria-busy={region.loading} aria-live="polite">
				{#if region.error}
					<PageState
						variant="error"
						title="ค้นหาผู้ร่วมเดินทางไม่สำเร็จ"
						description={region.error}
						actionLabel="ลองอีกครั้ง"
						onaction={read}
					/>
				{:else if region.loading}
					<p class="text-sm text-muted-foreground">กำลังค้นหารายชื่อ…</p>
				{:else if region.value?.length}
					<ul
						class="max-h-72 divide-y overflow-auto rounded-md border"
						aria-label="ผลค้นหาผู้ร่วมเดินทาง"
					>
						{#each region.value as user (user.id)}
							{@const added = chosen.some((person) => person.id === user.id)}
							<li class="flex flex-wrap items-center justify-between gap-2 px-3 py-1">
								<span class="min-w-0 break-words text-sm">{user.displayName}</span>
								{#if added}
									<span class="px-2 py-2 text-sm text-muted-foreground">เลือกแล้ว</span>
								{:else}
									<Button
										type="button"
										variant="outline"
										size="sm"
										disabled={disabled || chosen.length >= maxCompanions}
										aria-label={`เพิ่ม ${user.displayName} เป็นผู้ร่วมเดินทาง`}
										onclick={() => add(user)}>เพิ่ม</Button
									>
								{/if}
							</li>
						{/each}
					</ul>
				{:else}
					<p class="text-sm text-muted-foreground">ไม่พบบุคลากร ลองค้นหาด้วยชื่ออื่น</p>
				{/if}
			</div>
			{#if !region.loading && !region.error && (offset > 0 || region.value?.length === companionPageSize)}
				<div class="flex flex-wrap gap-2" aria-label="หน้าผลค้นหา">
					<Button
						type="button"
						size="sm"
						variant="outline"
						disabled={disabled || !offset}
						onclick={() => changePage(offset - companionPageSize)}>ก่อนหน้า</Button
					>
					<Button
						type="button"
						size="sm"
						variant="outline"
						disabled={disabled ||
							offset >= companionMaxOffset ||
							region.value?.length !== companionPageSize}
						onclick={() => changePage(offset + companionPageSize)}>ถัดไป</Button
					>
				</div>
			{/if}
			{#if chosen.length >= maxCompanions}
				<p class="text-sm text-muted-foreground">เลือกผู้ร่วมเดินทางได้สูงสุด 100 คน</p>
			{/if}
			<Button type="button" variant="ghost" {disabled} onclick={() => (open = false)}
				>ปิดการค้นหา</Button
			>
		</div>
	{/if}
</fieldset>
