# Backward walk - dynamic review v2

Review candidate; not installed. Replaces the rejected stiff v1 for review. Inherits the approved Aletha rig and relaxed hands; explicitly authored backward steps, not reversed forward playback.

## Beat plan

30 fps, 42-frame cycle plus an identical endpoint. Frame 0/21: backward toe contact; frame 4/25: weight acceptance and deepest compression; frame 12/33: rise over the supporting leg; frame 24/3: release into return; frame 33/12: passing and maximum clearance.

Pelvis shifts toward the supporting foot (6.8 cm lateral excursion), absorbs each step (5 cm vertical excursion), and rotates 6 degrees each way. Chest counter-rotation, forward pitch and delayed head response connect the torso to the gait. Arm arcs grow to 42 degrees with changing elbow bend and at least 7.5 degrees of clearance. Steps widen to 48 cm fore/aft excursion with 10 cm swing clearance and 58% support per foot, retaining double support.

The studio nominal speed is 0.591 m/s; game motor matching and transitions are deferred until approval. Existing game assets remain unchanged. author.py, render.py and encode.py recreate the master/export and studio comparison against v1. validation.json contains per-frame IK and rendered geometry measurements.

Superseded by approved and installed backward walk v3.
