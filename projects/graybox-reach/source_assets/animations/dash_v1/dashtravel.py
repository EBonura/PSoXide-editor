"""The motor's decelerating roll travel (psx-engine character_motor.rs decelerating_roll_step),
stepped 2 ticks per 30 Hz frame as an entity's motor runs, in model units."""
KNOTS=[(0,0),(4,350),(12,3200),(22,6400),(30,8000),(38,9000),(60,10000)]
def fraction(tick,active=14):
    phase=tick*60*256//max(1,active)
    for (a,va),(b,vb) in zip(KNOTS,KNOTS[1:]):
        if phase<=b*256:return (va+(vb-va)*max(0,phase-a*256)//((b-a)*256))/10000
    return 1.0
def travel(frame,total,active=14,hz=30):
    """Cumulative travel after `frame` 30 Hz frames (2 ticks each)."""
    t=min(active,int(round(frame*60/hz)));return total*fraction(t,active)
# model units of travel: 1344 editor units (roll_speed 96 x 14 active ticks) at each model's world scale
LIGHT_TRAVEL=27400   # light.psxmdl ~34,000 tall shown 1024 x 417/256 = 1,668 editor units (grid-dungeon placement)
HEAVY_TRAVEL=27480   # heavy.psxmdl ~31,400 tall shown 1,536 editor units
