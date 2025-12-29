/**
 * Admin Dashboard Page
 */

export const meta = {
    menu: {
        title: 'ระบบจัดการ',
        icon: 'Settings',
        group: 'settings',
        order: 999,
        permission: 'settings'
    }
};

export const load = async () => {
    return {
        title: 'ระบบจัดการ'
    };
};
