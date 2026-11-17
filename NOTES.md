## Cathedral budget (parked)

- Cause: grid-cathedral is one 33x26 room (57 pages) with no world stream settings, so it is held whole and its single room sets 27,966 B of renderer scratch; at SDK b49a18d18 the link guard is 4,746 B short, and the enemy dash clips add about 7.7 KB of resident clips on top.
- Profile gap fixed in SDK feat/cathedral-budget-2026-11-17 (e61018b92): a flood level held whole still drew only within the flood's near radius (RoomActivity read the core's draw_far) and baked chunk skies; now it draws every room and bakes none.
- Room split: cathedral re-cut with rechunk-hex size 10 (7 rooms, same 1662 faces) on the standard stream profile (flood on, held whole); link guard then passes with 692 B spare at the combat-integrate tip, so it still falls about 7 KB short once the dash clips land.
- Look: the same viewpoints match except for sub-pixel raster shifts (about 17 percent of pixels differ, mostly by 1 or 2 colour steps) from the per-room GTE translation; the 4-seed duel sweep after the split never ran (stopped when the lane was parked).
- Left: hex sizes 6 to 24 were tried and size 10 is the best a split can do (about 5.4 KB), so the remaining ~7 KB needs a decision: far panels (a visual approximation), a smaller quad format (quads are 66 KB of the 115 KB room), or a measured heap model (real heap use was 20.5 KB against a modelled 28.3 KB).
