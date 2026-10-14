/** Convert browser-only codecs; File Platform authoritatively resizes every raster. */
const MAX_SOURCE_BYTES = 20 * 1024 * 1024;
const MAX_PIXELS = 80_000_000;

export async function normalizeCatalogImage(file: File): Promise<File> {
	if (file.size === 0 || file.size > MAX_SOURCE_BYTES) {
		throw new Error('เลือกรูปภาพขนาดไม่เกิน 20 MB');
	}
	const header = new Uint8Array(await file.slice(0, 64).arrayBuffer());
	const brands = new TextDecoder('ascii').decode(header);
	const avif = /avif|avis/.test(brands) || /\.avif$/i.test(file.name);
	const heif =
		!avif &&
		(/heic|heix|hevc|hevx|mif1|msf1/.test(brands) ||
			/\.hei[cf]$/i.test(file.name) ||
			/image\/hei[cf]/i.test(file.type));
	if (heif) {
		try {
			const { default: convert } = await import('heic2any');
			const converted = await convert({ blob: file, toType: 'image/jpeg', quality: 0.85 });
			const first = Array.isArray(converted) ? converted[0] : converted;
			if (!first || first.size === 0 || first.size > MAX_SOURCE_BYTES) {
				throw new Error('invalid converted image');
			}
			return new File([first], 'supply-image.jpg', { type: 'image/jpeg' });
		} catch {
			throw new Error('แปลงรูป HEIC/HEIF ไม่สำเร็จ กรุณาเลือกภาพอื่นหรือส่งออกเป็น JPEG');
		}
	}
	if (avif) {
		let bitmap: ImageBitmap | undefined;
		try {
			bitmap = await createImageBitmap(file);
			if (bitmap.width * bitmap.height > MAX_PIXELS) throw new Error('image too large');
			const scale = Math.min(1, 1600 / Math.max(bitmap.width, bitmap.height));
			const canvas = document.createElement('canvas');
			canvas.width = Math.max(1, Math.round(bitmap.width * scale));
			canvas.height = Math.max(1, Math.round(bitmap.height * scale));
			const context = canvas.getContext('2d');
			if (!context) throw new Error('canvas unavailable');
			context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
			const output = await new Promise<Blob>((resolve, reject) => {
				canvas.toBlob(
					(blob) => (blob ? resolve(blob) : reject(new Error('image conversion failed'))),
					'image/webp',
					0.85
				);
			});
			return new File([output], 'supply-image.webp', { type: output.type });
		} catch {
			throw new Error('เปิดรูป AVIF ไม่สำเร็จ กรุณาใช้เบราว์เซอร์รุ่นใหม่หรือส่งออกเป็น JPEG');
		} finally {
			bitmap?.close();
		}
	}
	// JPEG/PNG/WebP/GIF/BMP/TIFF are byte-detected, decoded and compressed server-side.
	return file;
}
