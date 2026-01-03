/**
 * Settings Dashboard Page
 */

export const _meta = {
    menu: {
        title: 'ตั้งค่าระบบ',
        icon: 'Settings',
        group: 'settings',
        order: 999,
        permission: 'settings'
    }
};

export const load = async () => {
    return {
        title: 'ตั้งค่าระบบ'
    };
};
