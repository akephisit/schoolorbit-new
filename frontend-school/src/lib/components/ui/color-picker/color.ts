export type Rgb = [number, number, number];

export function hexToRgb(hex: string): Rgb {
	return [
		Number.parseInt(hex.slice(1, 3), 16),
		Number.parseInt(hex.slice(3, 5), 16),
		Number.parseInt(hex.slice(5, 7), 16)
	];
}

export function rgbToHex(channels: Rgb): string {
	return (
		'#' +
		channels
			.map((channel) =>
				Math.round(Math.max(0, Math.min(255, channel)))
					.toString(16)
					.padStart(2, '0')
			)
			.join('')
	);
}
