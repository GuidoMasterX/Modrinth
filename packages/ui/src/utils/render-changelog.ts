import { renderHighlightedString } from '@modrinth/utils'
import DOMPurify from 'dompurify'

const HTML_TAG_RE =
	/<(?:h[1-6]|ul|ol|li|p|div|a|img|table|thead|tbody|tr|td|th|blockquote|pre|code|strong|em|br|hr|span)\b/i

export function renderChangelog(changelog: string): string {
	if (!HTML_TAG_RE.test(changelog)) return renderHighlightedString(changelog)
	return DOMPurify.sanitize(changelog, { ADD_ATTR: ['target', 'rel'] })
}
