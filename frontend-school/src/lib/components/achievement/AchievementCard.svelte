<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import type { Achievement } from '$lib/types/achievement';
	import { Calendar, Trash2, Pencil } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import {
		Card,
		CardContent,
		CardDescription,
		CardFooter,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { cn } from '$lib/utils';

	export let achievement: Achievement;
	export let readonly: boolean = false;

	const dispatch = createEventDispatcher();

	function formatDate(dateStr: string) {
		const date = new Date(dateStr);
		return new Intl.DateTimeFormat('th-TH', {
			year: 'numeric',
			month: 'long',
			day: 'numeric'
		}).format(date);
	}
</script>

<Card class="overflow-hidden hover:shadow-md transition-shadow">
	{#if achievement.image_path}
		<div class="aspect-video w-full overflow-hidden bg-muted relative">
			<img
				src={achievement.image_path.startsWith('http')
					? achievement.image_path
					: `/api/files?path=${achievement.image_path}`}
				alt={achievement.title}
				class="w-full h-full object-cover transition-transform hover:scale-105"
			/>
		</div>
	{/if}
	<CardHeader class="p-4">
		<CardTitle class="text-lg line-clamp-1" title={achievement.title}>{achievement.title}</CardTitle
		>
		<CardDescription class="flex items-center gap-1 text-xs">
			<Calendar class="w-3 h-3" />
			{formatDate(achievement.achievement_date)}
		</CardDescription>
	</CardHeader>
	<CardContent class="p-4 pt-0">
		{#if achievement.description}
			<p class="text-sm text-muted-foreground line-clamp-3">
				{achievement.description}
			</p>
		{:else}
			<p class="text-sm text-muted-foreground/50 italic">ไม่มีรายละเอียด</p>
		{/if}
	</CardContent>
	{#if !readonly}
		<CardFooter class="p-4 bg-muted/20 border-t flex justify-end gap-2">
			<Button
				variant="ghost"
				size="icon"
				class="h-8 w-8 hover:text-primary"
				onclick={() => dispatch('edit', achievement)}
			>
				<Pencil class="w-4 h-4" />
			</Button>
			<Button
				variant="ghost"
				size="icon"
				class="h-8 w-8 hover:text-destructive text-muted-foreground"
				onclick={() => dispatch('delete', achievement)}
			>
				<Trash2 class="w-4 h-4" />
			</Button>
		</CardFooter>
	{/if}
</Card>
