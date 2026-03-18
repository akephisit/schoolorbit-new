<script lang="ts">
	import * as Popover from '$lib/components/ui/popover';
	import * as Command from '$lib/components/ui/command';
	import { Button } from '$lib/components/ui/button';
	import { Check, ChevronsUpDown } from 'lucide-svelte';
	import { cn } from '$lib/utils';

	interface School {
		name: string;
		province: string;
	}

	interface Props {
		value?: string;
		onProvinceSelect?: ((province: string) => void) | null;
	}

	let { value = $bindable(''), onProvinceSelect = null }: Props = $props();

	const MAX_RESULTS = 80;

	let open = $state(false);
	let searchInput = $state('');
	let schools = $state<School[]>([]);
	let triggerRef = $state<HTMLElement | null>(null);

	$effect(() => {
		import('$lib/data/thai-schools.json').then((mod) => {
			schools = mod.default as School[];
		});
	});

	// กรองเฉพาะเมื่อมีคำค้นหา และจำกัดจำนวนผลลัพธ์
	const filtered = $derived(() => {
		const q = searchInput.trim().toLowerCase();
		if (!q) return [];
		const results: School[] = [];
		for (const s of schools) {
			if (s.name.toLowerCase().includes(q) || s.province.toLowerCase().includes(q)) {
				results.push(s);
				if (results.length >= MAX_RESULTS) break;
			}
		}
		return results;
	});

	function selectSchool(school: School) {
		value = school.name;
		onProvinceSelect?.(school.province);
		open = false;
		searchInput = '';
	}

	function useTypedName() {
		const name = searchInput.trim();
		if (name) {
			value = name;
			open = false;
			searchInput = '';
		}
	}
</script>

<Popover.Root bind:open>
	<Popover.Trigger bind:ref={triggerRef}>
		{#snippet child({ props })}
			<Button
				variant="outline"
				class="w-full justify-between font-normal"
				{...props}
				role="combobox"
				aria-expanded={open}
			>
				<span class={cn('truncate', !value && 'text-muted-foreground')}>
					{value || 'ค้นหาชื่อโรงเรียน...'}
				</span>
				<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-full p-0">
		<Command.Root shouldFilter={false}>
			<Command.Input placeholder="พิมพ์ชื่อโรงเรียน..." bind:value={searchInput} />
			<Command.List class="max-h-60">
				{#if !searchInput.trim()}
					<div class="py-6 text-center text-sm text-muted-foreground">พิมพ์เพื่อค้นหาโรงเรียน</div>
				{:else if filtered().length === 0}
					<div class="flex flex-col items-center gap-2 py-3">
						<span class="text-sm text-muted-foreground">ไม่พบชื่อโรงเรียนในรายการ</span>
						<Button size="sm" variant="secondary" onclick={useTypedName}>
							ใช้ชื่อนี้: "{searchInput.trim()}"
						</Button>
					</div>
				{:else}
					<Command.Group>
						{#each filtered() as school (school.name)}
							<Command.Item value={school.name} onSelect={() => selectSchool(school)}>
								<Check
									class={cn('mr-2 h-4 w-4 shrink-0', value === school.name ? 'opacity-100' : 'opacity-0')}
								/>
								<span>{school.name}</span>
								<span class="ml-1 text-xs text-muted-foreground">({school.province})</span>
							</Command.Item>
						{/each}
					</Command.Group>
				{/if}
			</Command.List>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
