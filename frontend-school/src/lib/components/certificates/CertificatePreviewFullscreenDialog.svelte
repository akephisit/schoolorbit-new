<script lang="ts">
	import type { CertificateRenderManifest } from '$lib/api/certificates';
	import type { CertificatePreviewState } from '$lib/certificates/preview-fit';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import CertificatePreviewSurface from './CertificatePreviewSurface.svelte';

	type Props = {
		open: boolean;
		title: string;
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

	let previewState = $state<CertificatePreviewState>('idle');

	function handleStateChange(state: CertificatePreviewState) {
		previewState = state;
		onstatechange(state);
	}
</script>

<Dialog.Root {open} onOpenChange={onopenchange}>
	<Dialog.Content
		class="flex h-dvh w-screen max-w-none flex-col overflow-hidden rounded-none border-0 p-3 sm:max-w-none"
		aria-busy={previewState === 'loading'}
	>
		<Dialog.Header class="shrink-0 pr-10">
			<Dialog.Title>{title}แบบเต็มจอ</Dialog.Title>
			<Dialog.Description>แสดงเกียรติบัตรให้พอดีกับพื้นที่หน้าจอ</Dialog.Description>
		</Dialog.Header>
		<div class="min-h-0 min-w-0 flex-1">
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
		<div class="flex shrink-0 justify-end pt-2">
			<Button variant="outline" onclick={() => onopenchange(false)}>ปิดเต็มจอ</Button>
		</div>
	</Dialog.Content>
</Dialog.Root>
