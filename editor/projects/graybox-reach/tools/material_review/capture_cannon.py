"""Capture the current cannon without reinstalling material or scarf variants."""
from pathlib import Path
import json,hashlib
import capture as review
PROJECT=review.PROJECT
review.OUT=PROJECT/'validation/cannon-finish-v4'
review.OUT.mkdir(parents=True,exist_ok=True)
(review.OUT/'review-tape.csv').write_bytes((PROJECT/'validation/material-review-v3/review-tape.csv').read_bytes())
source=review.CAMERA.read_text()
assert 'Temporary material inspection' not in source
try:
 review.CAMERA.write_text(source.replace(review.BASE,review.REVIEW))
 review.build('inspection-build.log')
 for name,poll in [('full',800),('detail',900),('rotated',940)]:review.capture(name,poll)
finally:
 review.CAMERA.write_text(source)
 review.build('playable-build.log')
review.capture('gameplay',800)
(review.OUT/'capture.json').write_text(json.dumps({'resolution':[1280,960],'renderer':'actual guest, 4x hardware renderer',
 'inspection_camera_only':['full','detail','rotated'],'normal_camera':'gameplay',
 'disc_sha256':hashlib.sha256((PROJECT/'baked/graybox_reach.bin').read_bytes()).hexdigest()},indent=2)+'\n')
