import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import basicSsl from '@vitejs/plugin-basic-ssl';

export default defineConfig({
	plugins: [
		sveltekit(),
		basicSsl()
	],
	server: {
		https: true,
		host: true, // อนุญาตให้เข้าผ่าน domain name ได้
		port: 5173,  // หรือ port ที่คุณใช้อยู่
		hmr: {
			host: 'local.schoolorbit.app',
			port: 5173
		}
	}
});
