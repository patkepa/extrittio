import { defineConfig } from 'astro/config';
import { unified } from '@astrojs/markdown-remark';
import starlight from '@astrojs/starlight';
import docsTheme from '@patkepa/kantzen-starlight';
import starlightSidebarTopics from 'starlight-sidebar-topics';
import remarkRepositoryLinks from './src/remark-repository-links.mjs';

export default defineConfig({
	markdown: {
		processor: unified({ remarkPlugins: [remarkRepositoryLinks] }),
	},
	vite: {
		server: {
			watch: {
				usePolling: true,
			},
		},
	},
	integrations: [
		starlight({
			title: 'Extrittio Docs',
			description: 'Architecture, deployment, and operations documentation for Extrittio.',
			favicon: '/favicon.svg',
			customCss: ['./src/styles/header.css', './src/styles/site.css'],
			components: {
				Header: './src/components/Header.astro',
				SiteTitle: './src/components/SiteTitle.astro',
				PageTitle: './src/components/PageTitle.astro',
			},
			plugins: [
				docsTheme(),
				starlightSidebarTopics(
					[
						{
							id: 'start',
							label: 'Start',
							link: '/',
							icon: 'rocket',
							items: [{ label: 'Documentation home', slug: 'index' }],
						},
						{
							id: 'architecture',
							label: 'Architecture',
							link: '/architecture/overview/',
							icon: 'puzzle',
							items: [
								{ label: 'System overview', slug: 'architecture/overview' },
								{ label: 'Device blueprints', slug: 'architecture/device-blueprints' },
								{ label: 'Analytics', slug: 'architecture/analytics' },
								{ label: 'Firmware deployment protocol', slug: 'architecture/ota' },
							],
						},
						{
							id: 'deployment',
							label: 'Deployment',
							link: '/deployment/docker/',
							icon: 'cloud-download',
							items: [
								{ label: 'Docker production', slug: 'deployment/docker' },
								{ label: 'Edge', slug: 'deployment/edge' },
								{ label: 'OpenThread', slug: 'deployment/openthread' },
								{ label: 'Backend ownership', slug: 'deployment/backend-refactor' },
							],
						},
						{
							id: 'operations',
							label: 'Operations',
							link: '/operations/dependency-exceptions/',
							icon: 'padlock',
							items: [
								{ label: 'Dependency exceptions', slug: 'operations/dependency-exceptions' },
							],
						},
						{
							id: 'design',
							label: 'Design records',
							link: '/design/backend-crate-architecture-plan/',
							icon: 'open-book',
							items: [
								{ label: 'Architecture plan', slug: 'design/backend-crate-architecture-plan' },
								{ label: 'Execution plan', slug: 'design/backend-refactor-execution-plan' },
								{ label: 'Boundary audit', slug: 'design/backend-boundary-audit' },
								{ label: 'Identity closure', slug: 'design/backend-identity-closure' },
								{ label: 'Decision records', slug: 'design/backend-crate-refactor-decisions' },
								{ label: 'Persistence inventory', slug: 'design/backend-persistence-contract-inventory' },
								{ label: 'Continuation notes', slug: 'design/backend-refactor-continuation' },
								{ label: 'Implementation baseline', slug: 'design/backend-crate-refactor-baseline' },
							],
						},
					],
					{
						topics: {
							start: ['/index'],
							architecture: ['/architecture/**'],
							deployment: ['/deployment/**'],
							operations: ['/operations/**'],
							design: ['/design/**'],
						},
					},
				),
			],
			social: [
				{
					icon: 'github',
					label: 'GitHub',
					href: 'https://github.com/patkepa/extrittio',
				},
			],
		}),
	],
});
