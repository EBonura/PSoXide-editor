Superseded by the approved and installed v2 cross slash. Preserved as review history.

# Horizon heavy attack - cross slash, review v1

Review candidate; not installed. Builds on the existing Horizon Heavy source, retaining its heavy right blade, light left blade, staggered casts and inward crossing cut. Adds deep load, a 76 cm driving step, forward torso drive, expanded arms, knee absorption and recovery. Preview blade materialization is schematic.

30 Hz beat plan: neutral 0; right blade cast 6–11; left cast 12–17; loaded anticipation 17–21; drive 21–23; crossing cut 23–29; overshoot/absorption 29–33; weighted hold through 40; dissolve 45–51; recover 60. Hips initiate the drive; chest and arms follow closely, head follows the chest. The approved light attack and combo stay separate.

Editable `heavy_attack.blend`, sampled `heavy_attack.glb`, and `validation.json` accompany the authoring script. The script uses the local original studio stage at `review/heavy-attack-v1/original-preview.blend`; run `study.py` there first to regenerate that stage from the existing heavy source and archived sword sources. Studio videos are under `review/heavy-attack-v1/`.

Blade alignment pass pronates both forearms while preserving the authored wrist paths and local hand grips. The heavy blade broad face lies within about 5.5 degrees of its travel plane during frames 24–26; the light blade stays within about 16 degrees. Leg extension stays below 0.953; ankle target error is below one micrometre. Full sword silhouettes fit both cameras. These are authoring checks, not an in-engine test.

The preview video is `review/heavy-attack-v1/heavy-cross-slash-preview.mp4`: two normal-speed passes and one half-speed pass, front and three-quarter views. The installed default project remains unchanged. Reproduction order: `study.py`, `author.py`, `render.py` with headless Blender 5.2, then `encode.py` with Python/Pillow and FFmpeg.
