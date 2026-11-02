import { expect, test } from '@playwright/test';

/**
 * The workspace import's BROWSER boundary (the wire drive's inert leg, on
 * the lane that always runs): the Settings import card renders INERT in the
 * PWA — the browser has no Tauri IPC, so the card must offer nothing at all.
 * The wire drive (2026-11-02) drove the card's full consent loop through the
 * REAL app against scratch stores; this spec pins the other edge of the same
 * law: no picker, no report button, no act in a browser, ever. A card that
 * pretends to be usable where the commands do not exist is a lie the user
 * clicks into.
 */
test('pwa_settings_import_renders_inert_and_offers_nothing', async ({ page }) => {
	await page.goto('/settings');

	// The inert disclosure renders (the honest "nothing to do here" text).
	await expect(page.getByTestId('settings-import-inert')).toBeVisible({
		timeout: 15_000
	});

	// And no affordance exists: the resting state cannot land bytes.
	await expect(page.getByTestId('settings-import-picker')).toHaveCount(0);
	await expect(page.getByTestId('settings-import-read-report')).toHaveCount(0);
	await expect(page.getByTestId('settings-import-commit')).toHaveCount(0);
	await expect(page.getByTestId('settings-import-revert')).toHaveCount(0);
});
