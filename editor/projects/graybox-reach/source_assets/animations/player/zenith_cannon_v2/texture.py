"""Regenerate the cannon atlas without changing the approved scarf material."""
import subprocess, sys
from pathlib import Path
subprocess.run([sys.executable, str(Path(__file__).with_name('cannon_texture.py'))], check=True)
