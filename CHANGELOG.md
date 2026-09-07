# Changelog

## Unreleased

- Dead enemies now dissolve over roughly five seconds from the uppermost
  polygons down, with fragments rising and fading before the body is removed.

- The initial welcome panels now hold player controls until dismissed and include a guide to both analog sticks. Other messages remain interactive.

- New Game opens with a cinematic introduction: two views of the level, then Aletha rising from the ground. Hold X for half a second to skip.
- The opening includes smooth fades, a sound cue for each shot, and a burst of polygons as Aletha strikes the ground.

## Demo disc v0.35

- Enemies change stance to match the distance: melee up close and ranged attacks when you pull away, while respecting the swap cooldown.
- Combat music starts at a different point for each encounter and keeps looping.
- Inactive stance health recovers more slowly.
- The heavy enemy has three new attack animations, longer melee contact windows and a chest-fired projectile.
- The animation editor now supports separate hit windows, named combat volumes, and visibility controls for hurtboxes, hitboxes and projectiles.

- Dashes scatter Aletha's polygons behind her, leaving a wireframe that gradually rebuilds as she moves.
- Releasing the stick now starts the transition to idle without an extra stride.
- Phase swapping now recharges in five seconds for both the player and enemies.
- Stance changes burst the original mesh outward, then rebuild the body from translucent polygons over a wireframe, assembling from feet to head and holding the new colour until complete. The stance colour then fades smoothly back to Aletha's normal shading.
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
