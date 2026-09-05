# Changelog

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
