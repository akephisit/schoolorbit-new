/**
 * Settings Page
 */

export const _meta = {
    menu: {
        title: 'ตั้งค่า',
        icon: 'Settings',
        group: 'system',
        order: 900,
        permission: null  // Everyone can access settings
    }
};

export const load = async () => {
    return {
        title: 'ตั้งค่า'
    };
};
