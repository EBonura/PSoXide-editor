# Locked-on side steps, review v1

Studio candidates only. Left/right game clips 82/83 remain original.

Built from the approved backward v3 rig and its dynamic weight acceptance. Leading foot steps sideways first; trailing foot closes the stance during the next half-cycle. Neither foot crosses over. Facing remains directed toward the lock target, with modest hip/chest counter-rotation, arm balance and delayed forearm/wrist response.

Cycle 42 frames at 30 fps, duplicate endpoint at 42. Each foot has 60% support, 28 cm lateral excursion and 7.5 cm return clearance. Lead/trailing phasing reverses for the right step. Neutral ankle centers are 34 cm apart; alternate opening/closing produces the side-step shape. Nominal lateral travel 0.333 m/s is for source contact validation, not an installed motor setting.

Beats: 0 leading-foot contact, 4 weight absorption, 12 support rise, 21 trailing-foot contact, 25 transfer/compression, 33 trailing support rise, 42 repeat. The torso remains upright with a slight lean toward travel. Targets keep both knees flexed.

Run author.py and render.py in headless Blender once normally (left) and once with `-- --right`. encode.py produces the old/new studio review videos. Approval and in-game validation are pending.

Superseded by approved and installed side-step v2.
