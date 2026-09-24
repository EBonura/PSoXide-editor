#!/usr/bin/env python3
"""Validate the editor blank-slate BSP acceptance artifact and headless run."""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import re
from dataclasses import dataclass
from pathlib import Path


# Positions are ENGINE units: the cook divides every authored length by
# psxed_project::units::WORLD_UNIT_DIVISOR (16). The spawn is authored at
# (192, 257, 192) on top of the hollow's 2048 x 2048 roof slab.
SPAWN_X = 12
SPAWN_Z = 12
# The route. The New Project template boots into a wake-up intro, so the
# Make recipe holds forward, holds Cross over polls 200-440 to skip it, and
# stops on the pad clock at poll 1200 (`--stop-at-poll`). Held forward walks
# +Z across the roof and stops at its +Z edge: the roof ends at authored
# z 2048 (engine 128) and the 12-unit player hull settles 11 units past it,
# still on the roof (Y stays at the roof top). Measured on 2026-09-24; the
# stop is reached by poll 800 and unchanged at polls 1200 and 2400.
ROOF_MAX_Z = 128
LEDGE_STOP_OVERHANG = 11
TELEMETRY_POSITION_BIAS = 1_000_000
EXPECTED_ROUTE_TICKS = 1_232
EXPECTED_PAD_POLLS = 1_201
EXPECTED_GUEST_FRAMES = 1_199
EXPECTED_VISUAL_FRAMES = 525
# This scene presents about one frame per two ticks through the intro and
# gameplay. The gate pins the reported cadence instead of requiring
# "steady", so a change either way is visible here.
EXPECTED_CADENCE_STATUS = "missed_or_late"
MIN_SKY_CYCLES = 100_000
EXPECTED_SKY_HITS = 524
EXPECTED_TRI_PRIMS = 125_409
EXPECTED_LAST_TRI_PRIMS = 214
EXPECTED_VRAM_HASH = "0xedddd3b2ab8700a3"
EXPECTED_DISPLAY_HASH = "0x74d51a87d19b267c"
EXPECTED_GPU_CENSUS: dict[str, int | str] = {
    "rows": 1_232,
    "commands": 304_680,
    "draws": 148_453,
    "fills": 526,
    "textured_tris": 117_501,
    "textured_quads": 12_817,
    "textured_rects": 3_519,
    "run_draw_words": 1_528_673,
    "run_draw_hash": "0xfe8276b2c475b980",
}
IMAGE_SUFFIXES = {".bmp", ".gif", ".jpeg", ".jpg", ".png", ".ppm", ".webp"}


@dataclass(frozen=True)
class ReplayEvidence:
    route_ticks: int
    pad_polls: int
    guest_frames: int
    sim_ticks: int
    visual_frames: int
    cadence_status: str
    sky_cycles: int
    sky_hits: int
    tri_prims: int
    last_tri_prims: int
    player_x: int
    player_z: int
    vram_hash: str
    display_hash: str
    display_width: int
    display_height: int


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"editor blank playtest check failed: {message}")


def read_required(path: Path) -> bytes:
    require(path.is_file(), f"missing {path}")
    data = path.read_bytes()
    require(bool(data), f"empty {path}")
    return data


def match_int(text: str, pattern: str, label: str) -> int:
    match = re.search(pattern, text, re.MULTILINE)
    require(match is not None, f"headless log has no {label}")
    return int(match.group(1))


def parse_replay(text: str, label: str) -> ReplayEvidence:
    route = re.search(r"^route-ticks=(\d+)\s+port1-polls=(\d+)$", text, re.MULTILINE)
    require(route is not None, f"{label} has no route/pad counters")
    tri = re.search(
        r"^\s*tri prims\s+total=(\d+)\s+per_frame=\d+\s+latest=(\d+)$",
        text,
        re.MULTILINE,
    )
    require(tri is not None, f"{label} has no triangle counter")
    display = re.search(
        r"^display_fnv1a_64=(0x[0-9a-f]+)\s+w=(\d+)\s+h=(\d+)$",
        text,
        re.MULTILINE,
    )
    require(display is not None, f"{label} has no display hash or dimensions")
    vram = re.search(r"^vram_fnv1a_64=(0x[0-9a-f]+)$", text, re.MULTILINE)
    require(vram is not None, f"{label} has no VRAM hash")
    player_x = match_int(
        text, r"^\s*player local x\s+.*latest=(\d+)$", f"{label} player X counter"
    )
    player_z = match_int(
        text, r"^\s*player local z\s+.*latest=(\d+)$", f"{label} player Z counter"
    )
    require("visual_budget_status=pass" in text, f"{label} visual budget did not pass")
    cadence = re.search(r"^\s*cadence_status=(\S+)$", text, re.MULTILINE)
    require(cadence is not None, f"{label} has no cadence status")
    sky = re.search(
        r"^\s*sky\s+total=(\d+).*?hits=(\d+)$", text, re.MULTILINE
    )
    require(sky is not None, f"{label} has no sky-stage profile evidence")
    return ReplayEvidence(
        route_ticks=int(route.group(1)),
        pad_polls=int(route.group(2)),
        guest_frames=match_int(
            text, r"^guest_profile_frames=(\d+)$", f"{label} guest frame count"
        ),
        sim_ticks=match_int(text, r"^\s*sim_ticks=(\d+)$", f"{label} sim tick count"),
        visual_frames=match_int(
            text, r"^\s*visual_frames=(\d+)$", f"{label} visual frame count"
        ),
        cadence_status=cadence.group(1),
        sky_cycles=int(sky.group(1)),
        sky_hits=int(sky.group(2)),
        tri_prims=int(tri.group(1)),
        last_tri_prims=int(tri.group(2)),
        player_x=player_x - TELEMETRY_POSITION_BIAS,
        player_z=player_z - TELEMETRY_POSITION_BIAS,
        vram_hash=vram.group(1),
        display_hash=display.group(1),
        display_width=int(display.group(2)),
        display_height=int(display.group(3)),
    )


def parse_gpu_census(data: bytes, label: str) -> tuple[int, dict[str, int | str]]:
    text = data.decode("utf-8")
    rows = list(csv.DictReader(io.StringIO(text)))
    require(bool(rows), f"{label} GPU census has no rows")
    totals: dict[str, int | str] = {
        "rows": len(rows),
        "commands": sum(int(row["commands"]) for row in rows),
        "draws": sum(int(row["draws"]) for row in rows),
        "fills": sum(int(row["fills"]) for row in rows),
        "textured_tris": sum(int(row["textured_tris"]) for row in rows),
        "textured_quads": sum(int(row["textured_quads"]) for row in rows),
        "textured_rects": sum(int(row["textured_rects"]) for row in rows),
        "run_draw_words": int(rows[-1]["run_draw_words"]),
        "run_draw_hash": rows[-1]["run_draw_hash"],
    }
    return len(rows), totals


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--pxbsp", type=Path, required=True)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--disc", type=Path, required=True)
    parser.add_argument("--log-a", type=Path, required=True)
    parser.add_argument("--log-b", type=Path, required=True)
    parser.add_argument("--gpu-a", type=Path, required=True)
    parser.add_argument("--gpu-b", type=Path, required=True)
    parser.add_argument("--artifact-root", type=Path, required=True)
    args = parser.parse_args()

    project = read_required(args.project).decode("utf-8")
    manifest = read_required(args.manifest).decode("utf-8")
    pxbsp = read_required(args.pxbsp)
    exe = read_required(args.exe)
    disc = read_required(args.disc)
    log_a = read_required(args.log_a).decode("utf-8")
    log_b = read_required(args.log_b).decode("utf-8")
    gpu_a = read_required(args.gpu_a)
    gpu_b = read_required(args.gpu_b)

    require(
        'name: "Editor Blank Playtest Acceptance"' in project,
        "exported project name is not deterministic",
    )
    require(
        "assets/textures/courtyard_cobbles.psxt" in project
        and "assets/textures/courtyard_brick.psxt" in project,
        "acceptance project lost the neutral courtyard materials",
    )
    require("pub const PLAYTEST_USES_PXBSP: bool = true;" in manifest, "cook is not BSP")
    pxbsp_room = re.search(
        r'LevelRoomRecord \{ name: "PXBSP World".*?'
        r'sky: LevelSkyRecord \{.*?flags: (\d+),.*?'
        r'cloud_layer: LevelCloudLayerRecord \{ texture_asset: AssetId\((\d+)\)',
        manifest,
        re.DOTALL,
    )
    # psx_level::sky_flags: ENABLED | PANORAMA.
    require(
        pxbsp_room is not None and int(pxbsp_room.group(1)) & 3 == 3,
        "PXBSP room has no enabled authored sky panorama",
    )
    sky_asset_id = int(pxbsp_room.group(2))
    sky_asset = re.search(
        rf"LevelAssetRecord \{{ id: AssetId\({sky_asset_id}\), "
        rf"kind: AssetKind::Texture, bytes: .*?, ram_bytes: (\d+), "
        rf"vram_bytes: (\d+), flags: asset_flags::STREAMED_GAMEPLAY_TRANSIENT \}}",
        manifest,
    )
    require(sky_asset is not None, "PXBSP sky is not a gameplay-streamed texture asset")
    sky_bytes = int(sky_asset.group(1))
    sky_vram_bytes = int(sky_asset.group(2))
    require(
        (sky_bytes, sky_vram_bytes) == (65_820, 65_792),
        f"unexpected 512x256 panorama envelope: {sky_bytes}/{sky_vram_bytes}",
    )
    require(
        re.search(
            rf"LevelWorldPackEntryRecord \{{ room: RoomIndex\({sky_asset_id}\), "
            rf"sector_offset: \d+, sector_count: \d+, byte_size: {sky_bytes}, "
            rf"checksum: \d+ \}}",
            manifest,
        )
        is not None,
        "PXBSP sky has no matching UI.PAK table entry",
    )
    gameplay_stage = re.search(
        r"pub const GAMEPLAY_PACK_MAX_CHUNK_BYTES: usize = (\d+);", manifest
    )
    require(
        gameplay_stage is not None and int(gameplay_stage.group(1)) >= sky_bytes,
        "gameplay streaming stage cannot hold the PXBSP sky",
    )
    room_vram = re.search(
        r"pub static ROOM_0_REQUIRED_VRAM: &\[AssetId\] = &\[(.*?)\];", manifest
    )
    require(
        room_vram is not None and f"AssetId({sky_asset_id})" in room_vram.group(1),
        "PXBSP room residency does not retain its sky texture",
    )
    mover_ids = re.search(
        r"pub static PXBSP_MOVER_NODE_IDS: &\[u32\] = &\[(\d+)\];", manifest
    )
    require(mover_ids is not None, "expected exactly one authored brush Door mover")
    require(
        "PlayerSpawnRecord { room: RoomIndex(0), x: 12, y: 17, z: 12" in manifest,
        "authored Player Spawn record is missing",
    )
    box_props = re.search(
        r"pub static BOX_PROPS:.*?= &\[(.*?)\n\];", manifest, re.DOTALL
    )
    require(box_props is not None, "cooked Box Prop table is missing")
    require(box_props.group(1).count("LevelBoxPropRecord {") == 1, "expected one cooked Box Prop")
    require(
        "x: 96, y: 16, z: 96" in box_props.group(1)
        and "flags: 1" in box_props.group(1),
        "Box Prop placement/collision record is missing",
    )
    require(manifest.count("PointLightRecord {") == 1, "expected one cooked Point Light")
    require(
        "PointLightRecord { room: RoomIndex(0), x: 32, y: 32, z: 32" in manifest,
        "authored Point Light record is missing",
    )
    require(pxbsp[:4] == b"PXB%", "cooked world has no PXBSP magic")
    require(exe[:8] == b"PS-X EXE", "runtime artifact is not a PlayStation executable")

    replay_a = parse_replay(log_a, "replay A")
    replay_b = parse_replay(log_b, "replay B")
    require(replay_a == replay_b, f"replay evidence drifted: {replay_a} != {replay_b}")
    require(
        replay_a.route_ticks == EXPECTED_ROUTE_TICKS,
        f"route tick pin drifted: {replay_a.route_ticks}",
    )
    require(
        replay_a.pad_polls == EXPECTED_PAD_POLLS,
        f"pad poll pin drifted: {replay_a.pad_polls}",
    )
    require(
        replay_a.guest_frames == EXPECTED_GUEST_FRAMES
        and replay_a.sim_ticks == EXPECTED_GUEST_FRAMES,
        f"guest/sim frame pin drifted: {replay_a.guest_frames}/{replay_a.sim_ticks}",
    )
    require(
        replay_a.visual_frames == EXPECTED_VISUAL_FRAMES,
        f"visual frame pin drifted: {replay_a.visual_frames}",
    )
    require(
        replay_a.cadence_status == EXPECTED_CADENCE_STATUS,
        f"cadence status pin drifted: {replay_a.cadence_status}",
    )
    require(
        replay_a.sky_cycles >= MIN_SKY_CYCLES
        and replay_a.sky_hits == EXPECTED_SKY_HITS,
        "PXBSP panorama did not perform the expected rendered work: "
        f"{replay_a.sky_cycles} cycles/{replay_a.sky_hits} hits",
    )
    require(
        replay_a.tri_prims == EXPECTED_TRI_PRIMS
        and replay_a.last_tri_prims == EXPECTED_LAST_TRI_PRIMS,
        f"triangle counter pin drifted: {replay_a.tri_prims}/{replay_a.last_tri_prims}",
    )
    require(
        replay_a.vram_hash == EXPECTED_VRAM_HASH
        and replay_a.display_hash == EXPECTED_DISPLAY_HASH,
        f"render hash pin drifted: {replay_a.vram_hash}/{replay_a.display_hash}",
    )
    require(
        (replay_a.player_x, replay_a.player_z) != (SPAWN_X, SPAWN_Z),
        "held input never moved the player away from the authored spawn",
    )
    expected_edge_stop = (SPAWN_X, ROOF_MAX_Z + LEDGE_STOP_OVERHANG)
    require(
        (replay_a.player_x, replay_a.player_z) == expected_edge_stop,
        "sustained forward input did not stop at the roof's +Z edge: "
        f"expected {expected_edge_stop}, got {(replay_a.player_x, replay_a.player_z)}",
    )
    require(
        (replay_a.display_width, replay_a.display_height) == (320, 240),
        f"unexpected display dimensions: {replay_a.display_width}x{replay_a.display_height}",
    )

    require(gpu_a == gpu_b, "GPU command census is not byte deterministic across replays")
    _, gpu_totals = parse_gpu_census(gpu_a, "replay A")
    require(
        gpu_totals == EXPECTED_GPU_CENSUS,
        f"GPU command census pin drifted: {gpu_totals}",
    )

    # Only what the gate itself writes counts: the exported project carries
    # the template's source images (UI prompt PNGs) under its assets/ tree,
    # which are inputs, not output of the headless run.
    project_assets = args.project.parent / "assets"
    image_artifacts = sorted(
        path
        for path in args.artifact_root.rglob("*")
        if path.is_file()
        and path.suffix.lower() in IMAGE_SUFFIXES
        and project_assets not in path.parents
    )
    require(
        not image_artifacts,
        "acceptance emitted image artifacts: " + ", ".join(map(str, image_artifacts)),
    )

    print("editor blank playtest check: PASS")
    print(f"  deterministic replays: 2 x {replay_a.route_ticks} route ticks")
    print(f"  guest/visual frames: {replay_a.guest_frames}/{replay_a.visual_frames}")
    print(
        f"  player XZ: ({SPAWN_X}, {SPAWN_Z}) -> "
        f"({replay_a.player_x}, {replay_a.player_z}) at the roof edge"
    )
    print(f"  vram/display: {replay_a.vram_hash} / {replay_a.display_hash}")
    print(
        "  GPU census: "
        f"rows={gpu_totals['rows']} commands={gpu_totals['commands']} "
        f"draws={gpu_totals['draws']} fills={gpu_totals['fills']} "
        f"textured_tris={gpu_totals['textured_tris']} "
        f"textured_quads={gpu_totals['textured_quads']} "
        f"textured_rects={gpu_totals['textured_rects']} "
        f"draw_words={gpu_totals['run_draw_words']} "
        f"draw_hash={gpu_totals['run_draw_hash']}"
    )
    print("  image artifacts: 0")
    for label, path, data in [
        ("PXBSP", args.pxbsp, pxbsp),
        ("MIPS EXE", args.exe, exe),
        ("disc BIN", args.disc, disc),
    ]:
        print(f"  {label}: {len(data)} bytes sha256={sha256(data)} ({path})")


if __name__ == "__main__":
    main()
