<script lang="ts">
	import { downloadFile } from '$lib/api/files';
	import type { Attachment } from 'svelte/attachments';

	interface FileImageParams {
		fileId: string;
		resourceId?: string;
	}

	interface Props {
		fileId: string;
		resourceId?: string;
		alt: string;
		class?: string;
	}

	let { fileId, resourceId, alt, class: className = '' }: Props = $props();

	function privateFileImage(params: FileImageParams): Attachment<HTMLImageElement> {
		return (node) => {
			let objectUrl: string | null = null;
			const controller = new AbortController();
			node.style.visibility = 'hidden';

			function revealImage() {
				if (!controller.signal.aborted) {
					node.style.visibility = 'visible';
				}
			}

			node.addEventListener('load', revealImage);

			function cleanup() {
				controller.abort();
				node.removeEventListener('load', revealImage);
				if (objectUrl) {
					URL.revokeObjectURL(objectUrl);
					objectUrl = null;
				}
				node.removeAttribute('src');
				node.style.visibility = 'hidden';
			}

			async function load() {
				try {
					const blob = await downloadFile(params.fileId, params.resourceId, controller.signal);
					if (controller.signal.aborted) return;
					objectUrl = URL.createObjectURL(blob);
					node.src = objectUrl;
				} catch (error) {
					if (!controller.signal.aborted) {
						console.error('Unable to load private file image', error);
					}
				}
			}

			void load();
			return cleanup;
		};
	}
</script>

<img
	style:visibility="hidden"
	{@attach privateFileImage({ fileId, resourceId })}
	{alt}
	class={className}
/>
