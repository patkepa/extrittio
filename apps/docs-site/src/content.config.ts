import { readFile } from 'node:fs/promises';
import { defineCollection } from 'astro:content';
import { glob } from 'astro/loaders';
import type { Loader } from 'astro/loaders';
import { docsSchema } from '@astrojs/starlight/schema';

const docsRoot = new URL('../../../docs/', import.meta.url);
const sourceLoader = glob({
	base: docsRoot,
	pattern: '**/[^_]*.{md,mdx}',
	generateId: ({ entry }) => {
		if (entry.toLowerCase() === 'readme.md') return 'index';
		return entry.replace(/\.(?:md|mdx)$/i, '');
	},
});

const titlePattern = /^#\s+(.+)$/m;

const inferDescription = (source: string, title: string) => {
	const paragraph = source
		.replace(titlePattern, '')
		.trim()
		.split(/\n\s*\n/)
		.find((block) => !/^(?:#|\||```|-\s)/.test(block));

	if (!paragraph) return `${title} documentation for Extrittio.`;

	return paragraph
		.replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
		.replace(/[`*_]/g, '')
		.replace(/\s+/g, ' ')
		.trim();
};

const docsLoader: Loader = {
	name: 'extrittio-repository-docs',
	async load(context) {
		await sourceLoader.load({
			...context,
			parseData: async ({ id, data, filePath }) => {
				const source = filePath ? await readFile(filePath, 'utf8') : '';
				const configuredTitle = typeof data.title === 'string' ? data.title : undefined;
				const title = configuredTitle ?? source.match(titlePattern)?.[1] ?? id;
				const enhancedData = {
					title,
					description: data.description ?? inferDescription(source, title),
					...data,
				};

				return context.parseData({ id, data: enhancedData, filePath });
			},
		});
	},
};

export const collections = {
	docs: defineCollection({ loader: docsLoader, schema: docsSchema() }),
};
