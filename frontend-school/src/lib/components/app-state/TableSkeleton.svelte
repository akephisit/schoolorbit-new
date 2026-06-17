<script lang="ts">
	import { Skeleton } from '$lib/components/ui/skeleton';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';

	let {
		rows = 5,
		columns = 4,
		showHeader = true
	}: {
		rows?: number;
		columns?: number;
		showHeader?: boolean;
	} = $props();
</script>

<Table>
	{#if showHeader}
		<TableHeader>
			<TableRow>
				{#each Array.from({ length: columns }) as _, index (index)}
					<TableHead>
						<Skeleton class="h-4 w-24" />
					</TableHead>
				{/each}
			</TableRow>
		</TableHeader>
	{/if}
	<TableBody>
		{#each Array.from({ length: rows }) as _, rowIndex (rowIndex)}
			<TableRow>
				{#each Array.from({ length: columns }) as _, columnIndex (columnIndex)}
					<TableCell>
						<Skeleton class={columnIndex === 0 ? 'h-4 w-36' : 'h-4 w-24'} />
					</TableCell>
				{/each}
			</TableRow>
		{/each}
	</TableBody>
</Table>
