/** Budget wire values are integer satang strings; browser arithmetic stays exact. */
export function satang(value: string): bigint {
	if (!/^\d+$/.test(value)) throw new Error('จำนวนเงินไม่ถูกต้อง');
	return BigInt(value);
}

export function bahtInput(value: string): string {
	if (!/^\d+$/.test(value)) throw new Error('จำนวนเงินไม่ถูกต้อง');
	const amount = BigInt(value);
	return `${amount / 100n}.${(amount % 100n).toString().padStart(2, '0')}`;
}

export function toSatang(value: string): string {
	if (!/^\d+(?:\.\d{1,2})?$/.test(value))
		throw new Error('กรอกจำนวนเงินตั้งแต่ 0 และทศนิยมไม่เกิน 2 ตำแหน่ง');
	const [whole, fraction = ''] = value.split('.');
	return (BigInt(whole) * 100n + BigInt(fraction.padEnd(2, '0'))).toString();
}

export function quantityMillis(value: string): bigint {
	if (!/^\d+(?:\.\d{1,3})?$/.test(value))
		throw new Error('กรอกจำนวนมากกว่า 0 และทศนิยมไม่เกิน 3 ตำแหน่ง');
	const [whole, fraction = ''] = value.split('.');
	const amount = BigInt(whole) * 1000n + BigInt(fraction.padEnd(3, '0'));
	if (amount <= 0n) throw new Error('จำนวนต้องมากกว่า 0');
	return amount;
}

/** Round a nonnegative exact quantity × unit price half up once, to satang. */
export function lineTotal(quantity: string, unitPriceSatang: string): string {
	return ((quantityMillis(quantity) * satang(unitPriceSatang) + 500n) / 1000n).toString();
}

export function sumSatang(values: readonly string[]): string {
	return values.reduce((sum, value) => sum + satang(value), 0n).toString();
}

export function money(value: string): string {
	if (!/^-?\d+$/.test(value)) return '—';
	const amount = BigInt(value),
		absolute = amount < 0n ? -amount : amount;
	return `${amount < 0n ? '−' : ''}${new Intl.NumberFormat('th-TH').format(absolute / 100n)}.${(absolute % 100n).toString().padStart(2, '0')}`;
}
