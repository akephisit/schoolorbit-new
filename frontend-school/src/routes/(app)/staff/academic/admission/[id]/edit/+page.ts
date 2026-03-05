import type { PageLoad } from './$types';
export const load: PageLoad = async ({ params }) => ({
    title: 'แก้ไขรอบรับสมัคร',
    periodId: params.id
});
