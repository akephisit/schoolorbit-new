export interface TimetablePdfLogoDependencies {
	getLogoFileId: () => Promise<string | null | undefined>;
	downloadLogo: (fileId: string) => Promise<Blob>;
	readLogo: (blob: Blob) => Promise<string>;
}

export async function loadTimetablePdfLogoDataUrl(
	dependencies: TimetablePdfLogoDependencies
): Promise<string | null> {
	const logoFileId = await dependencies.getLogoFileId();
	if (!logoFileId) return null;

	const logo = await dependencies.downloadLogo(logoFileId);
	return dependencies.readLogo(logo);
}

export function blobToDataUrl(blob: Blob): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = () => {
			if (typeof reader.result === 'string') {
				resolve(reader.result);
			} else {
				reject(new Error('แปลงโลโก้โรงเรียนไม่สำเร็จ'));
			}
		};
		reader.onerror = () => reject(reader.error ?? new Error('แปลงโลโก้โรงเรียนไม่สำเร็จ'));
		reader.readAsDataURL(blob);
	});
}
