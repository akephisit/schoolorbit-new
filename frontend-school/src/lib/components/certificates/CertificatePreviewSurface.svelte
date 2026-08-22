<script lang="ts">
	import type { CertificateRenderManifest } from '$lib/api/certificates';
	import {
		calculateCertificatePreviewFit,
		type CertificatePreviewState
	} from '$lib/certificates/preview-fit';
	import { loadCertificateRenderer } from '$lib/certificates/renderer';
	import { Button } from '$lib/components/ui/button';
	import { AlertTriangle, LoaderCircle, RefreshCw } from 'lucide-svelte';
	import type { Attachment } from 'svelte/attachments';

	type Props = {
		manifest: CertificateRenderManifest | null;
		manifestLoading?: boolean;
		manifestError?: string;
		ariaLabel: string;
		loadingLabel: string;
		renderFailureMessage: string;
		retryLabel?: string;
		onretry: () => void;
		onstatechange?: (state: CertificatePreviewState) => void;
	};

	let {
		manifest,
		manifestLoading = false,
		manifestError = '',
		ariaLabel,
		loadingLabel,
		renderFailureMessage,
		retryLabel = 'ลองใหม่',
		onretry,
		onstatechange = () => undefined
	}: Props = $props();

	let availableWidth = $state(0);
	let availableHeight = $state(0);
	let state = $state<CertificatePreviewState>('idle');
	let renderError = $state('');

	const fit = $derived.by(() => {
		if (!manifest) return null;
		return calculateCertificatePreviewFit({
			availableWidth,
			availableHeight,
			pageWidthPoints: manifest.pageGeometry.displayedWidthPoints,
			pageHeightPoints: manifest.pageGeometry.displayedHeightPoints,
			devicePixelRatio: typeof window === 'undefined' ? 1 : window.devicePixelRatio
		});
	});
	const visibleError = $derived(manifestError || renderError || renderFailureMessage);

	function setPreviewState(nextState: CertificatePreviewState, error = '') {
		state = nextState;
		renderError = error;
		onstatechange(nextState);
	}

	function observeStage(): Attachment<HTMLElement> {
		return (node) => {
			const updateSize = (width: number, height: number) => {
				availableWidth = Math.max(0, width);
				availableHeight = Math.max(0, height);
			};
			const observer = new ResizeObserver((entries) => {
				const entry = entries.at(-1);
				if (entry) updateSize(entry.contentRect.width, entry.contentRect.height);
			});
			observer.observe(node);
			return () => observer.disconnect();
		};
	}

	function renderCertificate(
		currentManifest: CertificateRenderManifest | null,
		currentFit: typeof fit,
		loading: boolean,
		externalError: string
	): Attachment<HTMLCanvasElement> {
		return (node) => {
			if (loading) {
				setPreviewState('loading');
				return;
			}
			if (externalError) {
				setPreviewState('error');
				return;
			}
			if (!currentManifest) {
				setPreviewState('idle');
				return;
			}
			if (!currentFit) {
				setPreviewState('loading');
				return;
			}

			const controller = new AbortController();
			setPreviewState('loading');
			node.width = 1;
			node.height = 1;
			void loadCertificateRenderer()
				.then((renderer) =>
					renderer.renderPreview(currentManifest, node, {
						scale: currentFit.renderScale,
						signal: controller.signal
					})
				)
				.then(() => {
					if (!controller.signal.aborted) setPreviewState('ready');
				})
				.catch(() => {
					if (!controller.signal.aborted) setPreviewState('error', renderFailureMessage);
				});

			return () => controller.abort();
		};
	}
</script>

<div
	class="relative grid size-full min-h-0 min-w-0 place-items-center overflow-hidden rounded-lg bg-slate-200 p-3"
	data-testid="certificate-preview-stage"
	aria-busy={state === 'loading'}
	{@attach observeStage()}
>
	{#if state === 'loading'}
		<div
			class="z-10 grid max-w-sm place-items-center gap-3 rounded-xl border bg-background/95 px-6 py-5 text-center text-sm text-muted-foreground shadow-sm"
			role="status"
			aria-live="polite"
		>
			<LoaderCircle
				class="size-7 animate-spin text-primary motion-reduce:animate-none"
				aria-hidden="true"
			/>
			<p>{loadingLabel}</p>
		</div>
	{:else if state === 'error'}
		<div
			class="z-10 max-w-md rounded-lg border border-destructive/30 bg-background p-4 text-center text-sm text-destructive shadow-sm"
			role="alert"
		>
			<AlertTriangle class="mx-auto mb-2 size-5" aria-hidden="true" />
			<p>{visibleError}</p>
			<Button class="mt-3" variant="secondary" size="sm" onclick={onretry}>
				<RefreshCw class="size-4" aria-hidden="true" /> {retryLabel}
			</Button>
		</div>
	{/if}
	<canvas
		{@attach renderCertificate(manifest, fit, manifestLoading, manifestError)}
		hidden={state !== 'ready'}
		class="max-h-full max-w-full bg-white shadow-xl"
		style:width={fit ? `${fit.cssWidth}px` : undefined}
		style:height={fit ? `${fit.cssHeight}px` : undefined}
		aria-label={ariaLabel}
	></canvas>
</div>
