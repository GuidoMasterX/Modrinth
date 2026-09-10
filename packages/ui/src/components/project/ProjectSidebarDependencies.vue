<template>
	<div v-if="dependencies.length > 0" class="flex flex-col gap-3">
		<h2 class="text-lg m-0">{{ formatMessage(messages.title) }}</h2>
		<div class="flex flex-col gap-3">
			<AutoLink
				v-for="dependency in dependencies"
				:key="dependency.project_id"
				:to="`/project/${dependency.project_id}`"
				class="flex w-fit items-center gap-2 text-primary leading-[1.2] hover:underline"
			>
				<Avatar :src="dependency.icon_url" size="24px" class="shrink-0" no-shadow />
				<span class="font-semibold">{{ dependency.title }}</span>
				<span
					v-if="dependency.dependency_type === 'optional'"
					class="rounded-full bg-surface-4 px-2 py-0.5 text-xs font-semibold text-secondary"
				>
					{{ formatMessage(messages.optional) }}
				</span>
				<span
					v-else-if="dependency.dependency_type === 'incompatible'"
					class="rounded-full bg-surface-4 px-2 py-0.5 text-xs font-semibold text-red"
				>
					{{ formatMessage(messages.incompatible) }}
				</span>
			</AutoLink>
		</div>
	</div>
</template>

<script setup lang="ts">
import { defineMessages, useVIntl } from '../../composables/i18n'
import { AutoLink, Avatar } from '../base'

export interface SidebarDependency {
	dependency_type: string
	project_id: string
	title: string
	icon_url?: string | null
}

defineProps<{
	dependencies: SidebarDependency[]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: {
		id: 'project.about.dependencies.title',
		defaultMessage: 'Dependencies',
	},
	optional: {
		id: 'project.about.dependencies.optional',
		defaultMessage: 'Optional',
	},
	incompatible: {
		id: 'project.about.dependencies.incompatible',
		defaultMessage: 'Incompatible',
	},
})
</script>
