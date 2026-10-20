import type {
	BudgetPayment,
	BudgetSpendingRequestDetail,
	BudgetPaymentAllocationInput
} from '#lib/api/budget.js';
import { toSatang } from './money.js';

export function paymentLines(detail: BudgetSpendingRequestDetail, payment?: BudgetPayment) {
	if (!payment)
		return detail.lines.map((line) => ({
			planLineId: line.planLineId,
			amountSatang: line.amountSatang,
			remainingSatang: line.reservedSatang
		}));
	return payment.allocations.map((line) => ({
		...line,
		remainingSatang: (
			BigInt(line.amountSatang) -
			payment.refunds
				.flatMap((refund) => refund.allocations)
				.filter((refund) => refund.planLineId === line.planLineId)
				.reduce((sum, refund) => sum + BigInt(refund.amountSatang), 0n)
		).toString()
	}));
}

export function paymentAllocations(
	amounts: Readonly<Record<string, string>>,
	lines: ReturnType<typeof paymentLines>
): BudgetPaymentAllocationInput[] {
	const result = lines
		.filter((line) => amounts[line.planLineId])
		.map((line) => {
			const amountSatang = toSatang(amounts[line.planLineId]);
			if (BigInt(amountSatang) <= 0n) throw new Error('จำนวนเงินต้องมากกว่า 0');
			if (BigInt(amountSatang) > BigInt(line.remainingSatang))
				throw new Error('จำนวนเงินเกินยอดคงเหลือของรายการ');
			return { planLineId: line.planLineId, amountSatang };
		});
	if (!result.length) throw new Error('กรอกจำนวนเงินอย่างน้อยหนึ่งรายการ');
	return result;
}
