<script lang="ts">
	import { type Snippet } from 'svelte';
	import { cn } from '$lib/utils';
	import { User } from 'lucide-svelte';

	interface Props {
		src?: string | null | undefined;
		alt?: string;
		initials?: string | null;
		size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl';
		shape?: 'circle' | 'square';
		class?: string;
		status?: Snippet;
	}

	let {
		src = $bindable(null),
		alt = 'User avatar',
		initials = null,
		size = 'md',
		shape = 'circle',
		class: className = '',
		status,
		...restProps
	}: Props = $props();

	// Size classes
	const sizeClasses = {
		xs: 'w-6 h-6 text-xs',
		sm: 'w-8 h-8 text-sm',
		md: 'w-10 h-10 text-base',
		lg: 'w-12 h-12 text-lg',
		xl: 'w-16 h-16 text-xl'
	};

	// Icon size
	const iconSizes = {
		xs: 'w-3 h-3',
		sm: 'w-4 h-4',
		md: 'w-5 h-5',
		lg: 'w-6 h-6',
		xl: 'w-8 h-8'
	};

	// Generate initials from name
	export function generateInitials(name: string): string {
		const parts = name.trim().split(' ');
		if (parts.length >= 2) {
			return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
		}
		return name.substring(0, 2).toUpperCase();
	}

	// Handle image error
	function handleImageError() {
		src = null;
	}
</script>

<div
	class={cn(
		'relative inline-flex items-center justify-center overflow-hidden bg-muted',
		sizeClasses[size],
		shape === 'circle' ? 'rounded-full' : 'rounded-md',
		className
	)}
	{...restProps}
>
	{#if src}
		<!-- Avatar Image -->
		<img {src} {alt} class="w-full h-full object-cover" onerror={handleImageError} />
	{:else if initials}
		<!-- Initials Fallback -->
		<span class="font-semibold text-muted-foreground select-none">
			{initials}
		</span>
	{:else}
		<!-- Icon Fallback -->
		<User class={cn('text-muted-foreground', iconSizes[size])} />
	{/if}

	<!-- Status Indicator (Snippet) -->
	{#if status}
		{@render status()}
	{/if}
</div>
