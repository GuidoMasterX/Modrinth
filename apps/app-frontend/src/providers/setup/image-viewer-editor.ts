import {
	type ImageViewerEditorData,
	type ImageViewerEditorSource,
	provideImageViewerEditor,
} from '@modrinth/ui'
import { readFile } from '@tauri-apps/plugin-fs'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'

export function setupImageViewerEditorProvider() {
	provideImageViewerEditor({
		async loadEditorData(source: ImageViewerEditorSource): Promise<ImageViewerEditorData> {
			return {
				source: new Blob([await readFile(source.path)], { type: 'image/png' }),
			}
		},
		async fetchBlob(url: string): Promise<Blob> {
			const response = await tauriFetch(url)
			if (!response.ok) throw new Error(`Could not load image: ${response.statusText}`)
			return await response.blob()
		},
	})
}
