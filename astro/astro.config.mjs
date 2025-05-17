// @ts-check
import { defineConfig } from 'astro/config';
import { searchForWorkspaceRoot } from 'vite';
import { viteStaticCopy } from 'vite-plugin-static-copy';

// https://astro.build/config
export default defineConfig({
  site: process.env.SITE,
  base: process.env.BASE,
  vite: {
    plugins: [
      viteStaticCopy({
        targets: [
          {
            src: '../assets/*', // Any folder with files
            dest: 'assets' // Destination within the dist folder
          }
        ]
      })
    ],
    server: {
      fs: {
        allow: [
          // search up for workspace root
          searchForWorkspaceRoot(process.cwd()),
          // your custom rules
          '../dist',
        ],
      },
    },
  }
});
