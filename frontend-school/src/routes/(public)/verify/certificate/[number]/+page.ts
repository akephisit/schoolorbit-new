import type { PageLoad } from './$types';

export const load: PageLoad = ({ params }) => ({
	title: `ตรวจสอบเกียรติบัตร ${params.number}`,
	description: 'ตรวจสอบสถานะเกียรติบัตรจาก QR Code',
	number: params.number
});
