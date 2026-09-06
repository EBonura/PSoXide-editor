# Changelog

## Unreleased

- Phase changes raise a rotating glyph from the player's feet to above their head, following movement and the destination colour with a soft fade in and out.
- Sprint starts immediately, and evades cover less ground with a committed direction.
- Sword sounds, trails and damage follow each swing, including both cyan heavy strikes.
- Reworked the heavy enemy's two melee combos, attack tells and weapon contact.
- Corrected all three light-enemy swings so their damage, trails and sounds follow the claw.
- Improved the camera around pillars, low ceilings and fully compressed wall views.
- Added pickup and gameplay-entry sounds, clearer module instructions, and quicker message transitions.
- Combat music continues while the inventory is open. Default Cortex brightness is now 2.
- Fixed Select detaching the camera and stopping player movement in Cortex Ignition.
- New Game now starts a fresh run after returning to the title screen.
- Raised the default menu music volume; saved volume settings are preserved.

## Source 2026.09.05

This source snapshot is tagged `source-2026.09.05`. Download versions are
listed separately below; source cleanup does not replace an already published disc.

- The editor, engine and Cortex Ignition now live in one repository with pinned SDK and emulator dependencies.
- Removed unused renderer experiments and their public feature switches; the renderer paths used by Quake, Half-Life and editor projects remain.
- Cleaned host and guest compiler warnings and strengthened instruction-hazard checks.

## 0.4b-split.20260905 | 2026-09-05

Cortex Ignition published on itch.io.

- Light enemies charge one ranged shot; their melee attack has three separate damage/trail windows.
- Player and enemy swaps share a longer delay. Hits can interrupt attacks through the new poise rules.
- Updated music, combat/footstep audio, enemy awareness, lock-on camera and the closing demo message.
