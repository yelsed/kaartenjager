import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},

			// adapter-node: de app draait als gewoon Node-proces op de server, zonder
			// platformgebonden bouwstap.
			adapter: adapter(),

			// De ingebouwde CSRF-controle staat uit, en hooks.server.ts doet hem zelf.
			//
			// De ingebouwde variant vergelijkt de Origin-kop letterlijk met de herkomst van de
			// server, schema en al. adapter-node kent zijn eigen schema niet en gokt daarom
			// "https" zolang ORIGIN niet gezet is, terwijl de app over gewoon HTTP op het
			// tailnet draait. Gevolg: elke formulierpost kreeg 403 en geen enkele knop deed
			// iets. Onze eigen controle vergelijkt op host, wat hier de juiste regel is.
			csrf: { trustedOrigins: ['*'] }
		})
	]
});
