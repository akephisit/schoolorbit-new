import type { PageLoad } from './$types';
import { getPublicRoundInfo } from '$lib/api/admission';
import { error } from '@sveltejs/kit';

export const load: PageLoad = async ({ params }) => {
    try {
        const info = await getPublicRoundInfo(params.id);
        return { info };
    } catch (e) {
        throw error(404, {
            message: e instanceof Error ? e.message : 'ไม่พบรอบรับสมัครนี้'
        });
    }
};
