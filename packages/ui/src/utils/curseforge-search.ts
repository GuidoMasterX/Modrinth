import { getLoaderIcon } from '@modrinth/assets'
import type { Ref } from 'vue'
import { computed, h, markRaw, ref, shallowRef } from 'vue'
import { useRoute } from 'vue-router'

import { defineMessage, useVIntl } from '../composables/i18n'
import type { FilterType, FilterValue, SortType, Tags } from './search'
import { findFilterOption } from './search'
import { DEFAULT_MOD_LOADERS, formatLoader } from './tag-messages.ts'

export interface CurseforgeCategory {
	id: number
	name: string
	slug?: string | null
	iconUrl?: string | null
	classId?: number | null
	parentCategoryId?: number | null
	isClass?: boolean | null
}

export const CURSEFORGE_CLASS_IDS: Record<string, number> = {
	mod: 6,
	modpack: 4471,
	resourcepack: 12,
	shader: 6552,
	datapack: 6945,
}

const CURSEFORGE_MAX_PAGE_SIZE = 50

export const CURSEFORGE_MOD_LOADER_IDS: Record<string, number> = {
	forge: 1,
	fabric: 4,
	quilt: 5,
	neoforge: 6,
}

const CURSEFORGE_SORT_FIELDS: Record<string, { field: number; order: 'asc' | 'desc' }> = {
	relevance: { field: 13, order: 'desc' },
	downloads: { field: 6, order: 'desc' },
	popularity: { field: 2, order: 'desc' },
	updated: { field: 3, order: 'desc' },
	name: { field: 4, order: 'asc' },
	date_created: { field: 11, order: 'desc' },
}

export const CURSEFORGE_SORT_TYPES: SortType[] = [
	{ display: 'Relevance', name: 'relevance' },
	{ display: 'Downloads', name: 'downloads' },
	{ display: 'Popularity', name: 'popularity' },
	{ display: 'Date Updated', name: 'updated' },
	{ display: 'Date Created', name: 'date_created' },
	{ display: 'Name', name: 'name' },
]

export const CURSEFORGE_GAME_ID = 432

export function useCurseforgeSearch(opts: {
	projectType: Ref<string>
	tags: Ref<Tags>
	categories: Ref<CurseforgeCategory[]>
	query: Ref<string>
	maxResults: Ref<number>
	currentPage: Ref<number>
	providedFilters?: Ref<FilterValue[]>
}) {
	const { formatMessage } = useVIntl()
	const route = useRoute()

	const curseforgeCurrentSortType = shallowRef<SortType>(CURSEFORGE_SORT_TYPES[0])
	const curseforgeCurrentFilters = ref<FilterValue[]>([])
	const curseforgeToggledGroups = ref<string[]>([])
	const curseforgeOverriddenProvidedFilterTypes = ref<string[]>([])

	const classId = computed(() => CURSEFORGE_CLASS_IDS[opts.projectType.value])

	const curseforgeFilterTypes = computed<FilterType[]>(() => {
		const classCategories = (opts.categories.value ?? []).filter(
			(c) => !c.isClass && (c.classId ?? c.parentCategoryId) === classId.value,
		)
		const toCategoryOption = (c: CurseforgeCategory) => ({
			id: String(c.id),
			formatted_name: c.name,
			...(c.iconUrl
				? {
						icon: markRaw(() =>
							h('img', {
								src: c.iconUrl,
								class: 'h-4 w-4 grayscale group-hover:grayscale-0 transition-[filter]',
								alt: '',
							}),
						),
					}
				: {}),
			method: 'or' as const,
			value: String(c.id),
		})
		// CF API sets parentCategoryId equal to the class id for top-level categories
		const isTopLevel = (c: CurseforgeCategory) =>
			!c.parentCategoryId || c.parentCategoryId === classId.value
		const childrenByParent = new Map<number, CurseforgeCategory[]>()
		for (const category of classCategories) {
			if (category.parentCategoryId && !isTopLevel(category)) {
				const children = childrenByParent.get(category.parentCategoryId) ?? []
				children.push(category)
				childrenByParent.set(category.parentCategoryId, children)
			}
		}
		const categoryOptions = classCategories.filter(isTopLevel).map((parent) => {
			const children = childrenByParent.get(parent.id)
			return children?.length
				? { ...toCategoryOption(parent), sub_options: children.map(toCategoryOption) }
				: toCategoryOption(parent)
		})

		const filterTypes: FilterType[] = [
			{
				id: 'cf_game_version',
				// Ordering mirrors Modrinth per project type: mods put game version first,
				// modpacks put categories first, shaders put game version last.
				ordering:
					classId.value === CURSEFORGE_CLASS_IDS.mod
						? 2
						: classId.value === CURSEFORGE_CLASS_IDS.modpack
							? 1
							: classId.value === CURSEFORGE_CLASS_IDS.shader
								? -1
								: 0,
				formatted_name: formatMessage(
					defineMessage({
						id: 'search.filter_type.game_version',
						defaultMessage: 'Game version',
					}),
				),
				supported_project_types: ['mod', 'modpack', 'resourcepack', 'shader', 'datapack'],
				display: 'scrollable',
				query_param: 'cgv',
				supports: ['include'],
				searchable: true,
				toggle_groups: [
					{
						id: 'all_versions',
						formatted_name: formatMessage(
							defineMessage({
								id: 'search.filter_type.game_version.all_versions',
								defaultMessage: 'Show all versions',
							}),
						),
					},
				],
				options: (opts.tags.value?.gameVersions ?? []).map((gv) => ({
					id: gv.version,
					toggle_group: gv.version_type !== 'release' ? 'all_versions' : undefined,
					method: 'or' as const,
					value: gv.version,
					query_value: gv.version,
				})),
			},
		]

		if (
			classId.value === CURSEFORGE_CLASS_IDS.mod ||
			classId.value === CURSEFORGE_CLASS_IDS.modpack
		) {
			filterTypes.push({
				id: 'cf_loader',
				formatted_name: formatMessage(
					defineMessage({
						id: 'search.filter_type.mod_loader',
						defaultMessage: 'Mod loader',
					}),
				),
				supported_project_types: ['mod', 'modpack'],
				display: 'expandable',
				default_values: DEFAULT_MOD_LOADERS,
				query_param: 'cl',
				supports: ['include'],
				searchable: false,
				options: Object.entries(CURSEFORGE_MOD_LOADER_IDS).map(([name, id]) => ({
					id: name,
					formatted_name: formatLoader(formatMessage, name),
					icon: getLoaderIcon(name),
					method: 'or' as const,
					value: String(id),
				})),
				ordering: classId.value === CURSEFORGE_CLASS_IDS.mod ? 1 : 0,
			})
		}

		if (categoryOptions.length > 0) {
			filterTypes.push({
				id: 'cf_category',
				formatted_name: formatMessage(
					defineMessage({
						id: 'search.filter_type.curseforge_categories',
						defaultMessage: 'Categories',
					}),
				),
				supported_project_types: ['mod', 'modpack', 'resourcepack', 'shader', 'datapack'],
				display: 'all',
				query_param: 'cc',
				supports: ['include'],
				searchable: true,
				options: categoryOptions,
				ordering:
					classId.value === CURSEFORGE_CLASS_IDS.mod
						? 0
						: classId.value === CURSEFORGE_CLASS_IDS.modpack
							? 2
							: 1,
			})
		}

		return filterTypes.sort((a, b) => (b.ordering ?? 0) - (a.ordering ?? 0))
	})

	const curseforgeRequestParams = computed(() => {
		const params = [`gameId=${CURSEFORGE_GAME_ID}`]
		if (opts.query.value) {
			params.push(`searchFilter=${encodeURIComponent(opts.query.value)}`)
		}
		if (classId.value) {
			params.push(`classId=${classId.value}`)
		}

		const overridden = curseforgeOverriddenProvidedFilterTypes.value
		const validProvidedFilters = (opts.providedFilters?.value ?? []).filter(
			(providedFilter) => !overridden.includes(providedFilter.type),
		)
		const filteredFilters = curseforgeCurrentFilters.value.filter(
			(userFilter) =>
				!validProvidedFilters.some((providedFilter) => providedFilter.type === userFilter.type),
		)
		const included = [...filteredFilters, ...validProvidedFilters].filter((f) => !f.negative)
		const categoryIds = included
			.filter((f) => f.type === 'cf_category')
			.map((f) =>
				findFilterOption(
					curseforgeFilterTypes.value.find((t) => t.id === 'cf_category')?.options ?? [],
					f.option,
				),
			)
			.filter((option) => option && 'value' in option)
			.map((option) => (option as { value: string }).value)
		if (categoryIds.length > 0) {
			params.push(`categoryIds=${categoryIds.join(',')}`)
		}

		let gameVersion: string | undefined
		for (const filterType of curseforgeFilterTypes.value) {
			const matched = included.find((f) => f.type === filterType.id)
			if (!matched) continue
			const option = findFilterOption(filterType.options, matched.option)
			if (!option || !('value' in option)) continue

			if (filterType.id === 'cf_game_version') {
				gameVersion = option.value
				params.push(`gameVersion=${encodeURIComponent(option.value)}`)
			} else if (filterType.id === 'cf_loader' && gameVersion !== undefined) {
				params.push(`modLoaderType=${option.value}`)
			}
		}

		const sort = CURSEFORGE_SORT_FIELDS[curseforgeCurrentSortType.value.name]
		if (sort) {
			params.push(`sortField=${sort.field}&sortOrder=${sort.order}`)
		}

		const pageSize = Math.min(opts.maxResults.value, CURSEFORGE_MAX_PAGE_SIZE)
		const offset = (opts.currentPage.value - 1) * pageSize
		params.push(`index=${offset}&pageSize=${pageSize}`)

		return `?${params.join('&')}`
	})

	function readCurseforgeQueryParams() {
		const q = route.query

		if (q.cs) {
			curseforgeCurrentSortType.value =
				CURSEFORGE_SORT_TYPES.find((s) => s.name === String(q.cs)) ?? CURSEFORGE_SORT_TYPES[0]
		}

		for (const filterType of curseforgeFilterTypes.value) {
			const paramValue = q[filterType.query_param]
			if (!paramValue) continue

			const values =
				typeof paramValue === 'string'
					? [paramValue]
					: paramValue.filter((v): v is string => v !== null)

			for (const value of values) {
				const isNegative = value.startsWith('!')
				const cleanValue = isNegative ? value.slice(1) : value
				const option = findFilterOption(filterType.options, cleanValue)
				if (option) {
					curseforgeCurrentFilters.value.push({
						type: filterType.id,
						option: option.id,
						negative: isNegative,
					})
				}
			}
		}
	}

	function createCurseforgePageParams(): Record<string, string[]> {
		const items: Record<string, string[]> = {}

		if (opts.query.value) {
			items.q = [opts.query.value]
		}

		for (const filterValue of curseforgeCurrentFilters.value) {
			const type = curseforgeFilterTypes.value.find((t) => t.id === filterValue.type)
			if (type) {
				const value = filterValue.negative ? `!${filterValue.option}` : filterValue.option
				if (items[type.query_param]) {
					items[type.query_param].push(value)
				} else {
					items[type.query_param] = [value]
				}
			}
		}

		if (curseforgeCurrentSortType.value.name !== 'relevance') {
			items.cs = [curseforgeCurrentSortType.value.name]
		}

		if (opts.maxResults.value !== 20) {
			items.m = [String(opts.maxResults.value)]
		}

		if (opts.currentPage.value > 1) {
			items.page = [String(opts.currentPage.value)]
		}

		return items
	}

	readCurseforgeQueryParams()

	return {
		curseforgeCurrentSortType,
		curseforgeCurrentFilters,
		curseforgeToggledGroups,
		curseforgeOverriddenProvidedFilterTypes,
		curseforgeSortTypes: CURSEFORGE_SORT_TYPES,
		curseforgeFilterTypes,
		curseforgeRequestParams,
		createCurseforgePageParams,
	}
}
