<script lang="ts">
	import { GraduationCap } from 'lucide-svelte';

	let { message }: { message: string } = $props();
</script>

<div
	class="flex min-h-screen items-center justify-center bg-background p-4"
	role="status"
	aria-live="polite"
	aria-atomic="true"
>
	<div class="text-center">
		<div
			class="mx-auto mb-4 flex h-16 w-16 animate-pulse items-center justify-center rounded-lg bg-primary motion-reduce:animate-none"
		>
			<GraduationCap class="h-8 w-8 text-primary-foreground" aria-hidden="true" />
		</div>
		<p class="text-muted-foreground">{message}</p>
	</div>
</div>
