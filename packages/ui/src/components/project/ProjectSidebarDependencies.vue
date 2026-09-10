<template>
	<div v-if="groups.length > 0" class="flex flex-col gap-3">
		<h2 class="text-lg m-0">{{ formatMessage(messages.title) }}</h2>
		<section v-for="group in groups" :key="group.key" class="flex flex-col gap-3">
			<h3 class="m-0 text-sm font-semibold text-secondary">{{ group.label }}</h3>
			<div class="flex flex-col gap-3">
				<AutoLink
					v-for="dependency in group.items"
					:key="dependency.project_id"
					:to="`/project/${dependency.project_id}`"
					class="flex w-fit items-center gap-2 text-primary leading-[1.2] hover:underline"
				>
					<Avatar :src="dependency.icon_url" size="24px" class="shrink-0" no-shadow />
					<span class="font-semibold">{{ dependency.title }}</span>
				</AutoLink>
			</div>
		</section>
	</div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

import { defineMessages, useVIntl } from '../../composables/i18n'
import { AutoLink, Avatar } from '../base'

export interface SidebarDependency {
	dependency_type: string
	project_id: string
	title: string
	icon_url?: string | null
}

const props = defineProps<{
	dependencies: SidebarDependency[]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: {
		id: 'project.about.dependencies.title',
		defaultMessage: 'Dependencies',
	},
	required: {
		id: 'project.about.dependencies.required',
		defaultMessage: 'Required',
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

const groups = computed(() =>
	[
		{ key: 'required', label: formatMessage(messages.required) },
		{ key: 'optional', label: formatMessage(messages.optional) },
		{ key: 'incompatible', label: formatMessage(messages.incompatible) },
	]
		.map((group) => ({
			...group,
			items: props.dependencies.filter((dependency) => dependency.dependency_type === group.key),
		}))
		.filter((group) => group.items.length > 0),
)
</script>
