/**
 * coreJourney — the customer journey (IMPROVE15): the chain-projected
 * transparency read, the operator's own view and the portal's reference
 * render.
 */

import RouteIcon from '@lucide/svelte/icons/route';
import type { ExtensionManifest } from './types.js';

export const coreJourney: ExtensionManifest = {
	id: 'coreJourney',
	version: '0.1.0',
	contributes: {
		navEntries: [
			{ id: 'journey', route: '/journey', labelKey: 'nav.journey', Icon: RouteIcon, order: 6 }
		]
	}
};
