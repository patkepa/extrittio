import { dirname, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repositoryRoot = fileURLToPath(new URL('../../../', import.meta.url));
const docsRoot = resolve(repositoryRoot, 'docs');
const sourceBase = 'https://github.com/patkepa/extrittio/blob/main/';

const visit = (node, filePath) => {
	if (node?.type === 'code' && typeof node.lang === 'string' && node.lang.includes(',')) {
		const [language, ...modifiers] = node.lang.split(',');
		node.lang = language;
		node.meta = [node.meta, ...modifiers].filter(Boolean).join(' ');
	}

	if (node?.type === 'link' && typeof node.url === 'string' && node.url && !/^[a-z]+:/i.test(node.url)) {
		const [pathname, fragment = ''] = node.url.split('#', 2);
		if (pathname && !pathname.startsWith('/')) {
			const target = resolve(dirname(filePath), decodeURIComponent(pathname));
			const relativeToDocs = relative(docsRoot, target);
			const suffix = fragment ? `#${fragment}` : '';

			if (!relativeToDocs.startsWith(`..${sep}`) && relativeToDocs !== '..') {
				const route = relativeToDocs
					.replaceAll(sep, '/')
					.replace(/(?:^|\/)README\.(?:md|mdx)$/i, '')
					.replace(/\.(?:md|mdx)$/i, '');
				node.url = `/${route}${route ? '/' : ''}${suffix}`;
			} else {
				const repositoryPath = relative(repositoryRoot, target).replaceAll(sep, '/');
				node.url = `${sourceBase}${repositoryPath}${suffix}`;
			}
		}
	}

	if (Array.isArray(node?.children)) {
		for (const child of node.children) visit(child, filePath);
	}
};

export default function remarkRepositoryLinks() {
	return (tree, file) => {
		if (file.path) visit(tree, file.path);
	};
}
