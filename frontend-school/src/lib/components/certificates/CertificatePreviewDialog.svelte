<script lang="ts">
	import type { CertificateRenderManifest } from '$lib/api/certificates';
	import type { CertificatePreviewState } from '$lib/certificates/preview-fit';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Maximize2 } from 'lucide-svelte';
	import CertificatePreviewFullscreenDialog from './CertificatePreviewFullscreenDialog.svelte';
	import CertificatePreviewSurface from './CertificatePreviewSurface.svelte';

	type Props = {
		open: boolean;
		title: string;
		description: string;
		manifest: CertificateRenderManifest | null;
		manifestLoading?: boolean;
		manifestError?: string;
		ariaLabel: string;
		loadingLabel: string;
		renderFailureMessage: string;
		retryLabel?: string;
		onretry: () => void;
		onopenchange: (open: boolean) => void;
		onstatechange?: (state: CertificatePreviewState) => void;
	};

	let {
		open,
		title,
		description,
		manifest,
		manifestLoading = false,
		manifestError = '',
		ariaLabel,
		loadingLabel,
		renderFailureMessage,
		retryLabel = 'ลองใหม่',
		onretry,
		onopenchange,
		onstatechange = () => undefined
	}: Props = $props();

	let fullscreenOpen = $state(false);
	let previewState = $state<CertificatePreviewState>('idle');

	function handleStateChange(state: CertificatePreviewState) {
		previewState = state;
		onstatechange(state);
	}

	function changeOpen(nextOpen: boolean) {
		if (!nextOpen) fullscreenOpen = false;
		onopenchange(nextOpen);
	}
</script>

<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content
		class="flex h-[94dvh] w-[96vw] max-w-[96vw] flex-col overflow-hidden p-3 sm:max-w-[96vw]"
		aria-busy={previewState === 'loading'}
	>
		<Dialog.Header class="shrink-0 px-2 pt-2 pr-10">
			<Dialog.Title>{title}</Dialog.Title>
			<Dialog.Description>{description}</Dialog.Description>
		</Dialog.Header>
		<div class="min-h-0 min-w-0 flex-1 py-2">
			<CertificatePreviewSurface
				{manifest}
				{manifestLoading}
				{manifestError}
				{ariaLabel}
				{loadingLabel}
				{renderFailureMessage}
				{retryLabel}
				{onretry}
				onstatechange={handleStateChange}
			/>
		</div>
		<div class="flex shrink-0 flex-wrap justify-end gap-2 px-2 pb-1">
			<Button variant="secondary" onclick={() => (fullscreenOpen = true)}>
				<Maximize2 class="size-4" aria-hidden="true" /> ขยายเต็มจอ
			</Button>
			<Button variant="outline" onclick={() => changeOpen(false)}>ปิด</Button>
		</div>
	</Dialog.Content>
</Dialog.Root>

<CertificatePreviewFullscreenDialog
	open={open && fullscreenOpen}
	{title}
	{manifest}
	{manifestLoading}
	{manifestError}
	{ariaLabel}
	{loadingLabel}
	{renderFailureMessage}
	{retryLabel}
	{onretry}
	onopenchange={(nextOpen) => (fullscreenOpen = nextOpen)}
	onstatechange={handleStateChange}
/>
