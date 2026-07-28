<script lang="ts">
	import { portalDownloadDocument } from '$lib/api/admission';
	import type { Attachment } from 'svelte/attachments';

	interface PortalImageParams {
		fileId: string;
		nationalId: string;
		dateOfBirth: string;
	}

	interface Props extends PortalImageParams {
		alt: string;
		class?: string;
	}

	let { fileId, nationalId, dateOfBirth, alt, class: className = '' }: Props = $props();

	function portalFileImage(params: PortalImageParams): Attachment<HTMLImageElement> {
		return (node) => {
			let active = true;
			let objectUrl: string | null = null;

			void portalDownloadDocument(params.fileId, params.nationalId, params.dateOfBirth)
				.then((blob) => {
					if (!active) return;
					objectUrl = URL.createObjectURL(blob);
					node.src = objectUrl;
				})
				.catch(() => {
					node.removeAttribute('src');
				});

			return () => {
				active = false;
				if (objectUrl) URL.revokeObjectURL(objectUrl);
				node.removeAttribute('src');
			};
		};
	}
</script>

<img {@attach portalFileImage({ fileId, nationalId, dateOfBirth })} {alt} class={className} />
