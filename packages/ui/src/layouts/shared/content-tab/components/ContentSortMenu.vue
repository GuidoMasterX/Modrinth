<script setup lang="ts">
import {
	ArrowDownAZIcon,
	ArrowUpZAIcon,
	ClockArrowDownIcon,
	ClockArrowUpIcon,
	DropdownIcon,
	SortAscIcon,
	SortDescIcon,
} from '@modrinth/assets'
import { type Component, computed } from 'vue'

import { type ButtonMenuOption, TeleportOverflowMenu } from '#ui/components/base/buttons'
import { defineMessages, useVIntl } from '#ui/composables/i18n'

import type { ContentSortMode } from '../composables/content-sort'

const props = withDefaults(defineProps<{ showDateOptions?: boolean }>(), {
	showDateOptions: true,
})

const sortMode = defineModel<ContentSortMode>({ required: true })

const { formatMessage } = useVIntl()

const messages = defineMessages({
	sortAlphabeticalAscending: {
		id: 'content.page-layout.sort.alphabetical-ascending',
		defaultMessage: 'Name (A-Z)',
	},
	sortAlphabeticalDescending: {
		id: 'content.page-layout.sort.alphabetical-descending',
		defaultMessage: 'Name (Z-A)',
	},
	sortDownloadsMost: {
		id: 'content.page-layout.sort.downloads-most',
		defaultMessage: 'Downloads (most first)',
	},
	sortDownloadsLeast: {
		id: 'content.page-layout.sort.downloads-least',
		defaultMessage: 'Downloads (least first)',
	},
	sortDateUpdatedNewest: {
		id: 'content.page-layout.sort.date-updated-newest',
		defaultMessage: 'Date updated (newest)',
	},
	sortDateUpdatedOldest: {
		id: 'content.page-layout.sort.date-updated-oldest',
		defaultMessage: 'Date updated (oldest)',
	},
	sortDateAddedNewest: {
		id: 'content.page-layout.sort.date-added-newest',
		defaultMessage: 'Newest first',
	},
	sortDateAddedOldest: {
		id: 'content.page-layout.sort.date-added-oldest',
		defaultMessage: 'Oldest first',
	},
	sortByLabel: {
		id: 'content.page-layout.sort.label',
		defaultMessage: 'Sort by {mode}',
	},
})

const sortLabels: Record<ContentSortMode, () => string> = {
	'alphabetical-asc': () => formatMessage(messages.sortAlphabeticalAscending),
	'alphabetical-desc': () => formatMessage(messages.sortAlphabeticalDescending),
	'downloads-most': () => formatMessage(messages.sortDownloadsMost),
	'downloads-least': () => formatMessage(messages.sortDownloadsLeast),
	'date-updated-newest': () => formatMessage(messages.sortDateUpdatedNewest),
	'date-updated-oldest': () => formatMessage(messages.sortDateUpdatedOldest),
	'date-added-newest': () => formatMessage(messages.sortDateAddedNewest),
	'date-added-oldest': () => formatMessage(messages.sortDateAddedOldest),
}

const sortIcons = {
	'alphabetical-asc': ArrowDownAZIcon,
	'alphabetical-desc': ArrowUpZAIcon,
	'downloads-most': SortDescIcon,
	'downloads-least': SortAscIcon,
	'date-updated-newest': ClockArrowDownIcon,
	'date-updated-oldest': ClockArrowUpIcon,
	'date-added-newest': ClockArrowDownIcon,
	'date-added-oldest': ClockArrowUpIcon,
} satisfies Record<ContentSortMode, Component>

const sortOptions = computed<ButtonMenuOption[]>(() => {
	const modes: ContentSortMode[] = props.showDateOptions
		? [
				'alphabetical-asc',
				'alphabetical-desc',
				'downloads-most',
				'downloads-least',
				'date-updated-newest',
				'date-updated-oldest',
				'date-added-newest',
				'date-added-oldest',
			]
		: ['alphabetical-asc', 'alphabetical-desc', 'downloads-most', 'downloads-least']

	return modes.map((mode) => ({
		id: mode,
		label: sortLabels[mode](),
		icon: sortIcons[mode],
		action: () => (sortMode.value = mode),
	}))
})
</script>

<template>
	<TeleportOverflowMenu
		class="!h-[34px] !text-sm !font-medium"
		:icon-only="false"
		:label="formatMessage(messages.sortByLabel, { mode: sortLabels[sortMode]() })"
		:options="sortOptions"
	>
		<component :is="sortIcons[sortMode]" />
		{{ sortLabels[sortMode]() }}
		<DropdownIcon />
	</TeleportOverflowMenu>
</template>
