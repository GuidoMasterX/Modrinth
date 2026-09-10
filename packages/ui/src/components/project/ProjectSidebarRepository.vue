<template>
	<div v-if="repo && repoStats" class="flex flex-col gap-3">
		<h2 class="text-lg m-0">{{ formatMessage(messages.title) }}</h2>
		<div
			class="flex flex-col gap-3 [&>a]:flex [&>a]:gap-2 [&>a]:items-center [&>a]:w-fit [&>a]:text-primary [&>a]:leading-[1.2] [&>a:hover]:underline"
		>
			<a :href="repo.url" target="_blank" rel="noopener nofollow ugc">
				<StarIcon aria-hidden="true" />
				{{ formatCompactNumber(repoStats.stargazers_count) }}
				{{
					repoStats.stargazers_count === 1
						? formatMessage(messages.star)
						: formatMessage(messages.stars)
				}}
				<ExternalIcon aria-hidden="true" class="external-icon" />
			</a>
			<a :href="`${repo.url}/issues`" target="_blank" rel="noopener nofollow ugc">
				<IssuesIcon aria-hidden="true" />
				{{ formatCompactNumber(repoStats.open_issues_count) }}
				{{
					repoStats.open_issues_count === 1
						? formatMessage(messages.issue)
						: formatMessage(messages.issues)
				}}
				<ExternalIcon aria-hidden="true" class="external-icon" />
			</a>
			<a :href="`${repo.url}/forks`" target="_blank" rel="noopener nofollow ugc">
				<GitForkIcon aria-hidden="true" />
				{{ formatCompactNumber(repoStats.forks_count) }}
				{{
					repoStats.forks_count === 1 ? formatMessage(messages.fork) : formatMessage(messages.forks)
				}}
				<ExternalIcon aria-hidden="true" class="external-icon" />
			</a>
		</div>
	</div>
</template>

<script setup lang="ts">
import { ExternalIcon, GitForkIcon, IssuesIcon, StarIcon } from '@modrinth/assets'
import { useQuery } from '@tanstack/vue-query'
import { computed } from 'vue'

import { useCompactNumber } from '../../composables'
import { defineMessages, useVIntl } from '../../composables/i18n'
import { injectModrinthClient } from '../../providers'

interface GitHubRepo {
	stargazers_count: number
	open_issues_count: number
	forks_count: number
}

const props = defineProps<{
	sourceUrl?: string | null
}>()

const { formatMessage } = useVIntl()
const { formatCompactNumber } = useCompactNumber()
const client = injectModrinthClient()

const repo = computed(() => {
	const match = props.sourceUrl?.match(
		/^https?:\/\/github\.com\/([A-Za-z0-9-_.]+)\/([A-Za-z0-9-_.]+?)(?:\.git)?\/?$/,
	)
	return match
		? { owner: match[1], name: match[2], url: `https://github.com/${match[1]}/${match[2]}` }
		: null
})

const { data: repoStats } = useQuery({
	queryKey: computed(() => ['github-repo', repo.value?.url] as const),
	queryFn: async () => {
		if (!repo.value) return null
		try {
			return await client.request<GitHubRepo>(`/${repo.value.owner}/${repo.value.name}`, {
				api: 'https://api.github.com',
				version: 'repos',
				skipAuth: true,
			})
		} catch {
			return null
		}
	},
	staleTime: 1000 * 60 * 60,
})

const messages = defineMessages({
	title: {
		id: 'project.about.repository.title',
		defaultMessage: 'Repository',
	},
	star: {
		id: 'project.about.repository.star',
		defaultMessage: 'star',
	},
	stars: {
		id: 'project.about.repository.stars',
		defaultMessage: 'stars',
	},
	issue: {
		id: 'project.about.repository.issue',
		defaultMessage: 'issue',
	},
	issues: {
		id: 'project.about.repository.issues',
		defaultMessage: 'issues',
	},
	fork: {
		id: 'project.about.repository.fork',
		defaultMessage: 'fork',
	},
	forks: {
		id: 'project.about.repository.forks',
		defaultMessage: 'forks',
	},
})
</script>
