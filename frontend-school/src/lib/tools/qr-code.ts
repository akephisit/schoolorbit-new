export const QR_SIZE = 1024;
export const MAX_LOGO_BYTES = 5 * 1024 * 1024;
const LOGO_TYPES = new Set(['image/png', 'image/jpeg', 'image/webp']);

export class QrTextError extends Error {}

export function validateQrText(text: string): string | null {
	return text.trim() ? null : 'กรุณาใส่ลิงก์หรือข้อความ';
}

export function validateLogo(file: Blob): string | null {
	if (!LOGO_TYPES.has(file.type)) return 'รองรับเฉพาะภาพ PNG, JPEG และ WebP';
	if (file.size > MAX_LOGO_BYTES) return 'ภาพต้องมีขนาดไม่เกิน 5 MB';
	return null;
}

export async function readLogo(file: Blob): Promise<HTMLImageElement> {
	const error = validateLogo(file);
	if (error) throw new Error(error);
	const url = URL.createObjectURL(file);
	try {
		const image = new Image();
		image.src = url;
		await image.decode();
		if (!image.naturalWidth || !image.naturalHeight) throw new Error('ภาพไม่มีขนาด');
		return image;
	} catch {
		throw new Error('อ่านภาพไม่สำเร็จ กรุณาเลือกไฟล์ภาพใหม่');
	} finally {
		URL.revokeObjectURL(url);
	}
}

export async function createQrPng(text: string, logo: HTMLImageElement | null): Promise<Blob> {
	const error = validateQrText(text);
	if (error) throw new QrTextError(error);
	const { create, toCanvas } = await import('qrcode');
	let moduleCount: number;
	try {
		moduleCount = create(text, { errorCorrectionLevel: 'H' }).modules.size;
	} catch {
		throw new QrTextError('ข้อความยาวเกินความจุ QR Code กรุณาลดความยาวแล้วลองใหม่');
	}
	// Integer modules keep edges sharp; padding avoids renderer width rounding.
	const symbol = document.createElement('canvas');
	await toCanvas(symbol, text, {
		scale: Math.floor(QR_SIZE / (moduleCount + 8)),
		margin: 4,
		errorCorrectionLevel: 'H',
		color: { dark: '#000000ff', light: '#ffffffff' }
	});
	const canvas = document.createElement('canvas');
	canvas.width = canvas.height = QR_SIZE;
	const context = canvas.getContext('2d');
	if (!context) throw new Error('เบราว์เซอร์ไม่สามารถสร้างภาพได้');
	context.fillStyle = '#ffffff';
	context.fillRect(0, 0, QR_SIZE, QR_SIZE);
	context.drawImage(
		symbol,
		Math.floor((QR_SIZE - symbol.width) / 2),
		Math.floor((QR_SIZE - symbol.height) / 2)
	);
	if (logo) {
		const frame = Math.floor(QR_SIZE * 0.22);
		const inset = 12;
		const scale = (frame - inset * 2) / Math.max(logo.naturalWidth, logo.naturalHeight);
		const width = logo.naturalWidth * scale;
		const height = logo.naturalHeight * scale;
		context.fillStyle = '#ffffff';
		context.fillRect((QR_SIZE - frame) / 2, (QR_SIZE - frame) / 2, frame, frame);
		context.drawImage(logo, (QR_SIZE - width) / 2, (QR_SIZE - height) / 2, width, height);
	}
	return new Promise((resolve, reject) => {
		canvas.toBlob((blob) => {
			if (blob) resolve(blob);
			else reject(new Error('สร้างไฟล์ PNG ไม่สำเร็จ กรุณาลองใหม่'));
		}, 'image/png');
	});
}
