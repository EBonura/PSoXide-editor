#!/usr/bin/env python3
"""Validate and assemble PSoXide hardware-test photo payloads.

PX7 carries a per-record median and explicit record ids, so a probe can be
added without shifting the meaning of every later record. PX8 adds per-block
flags and a variable page count. PX5 and PX6 captures are no longer parsed:
their timing records were positional and no such capture is archived.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import pathlib
import struct
import sys
from collections import Counter
from dataclasses import dataclass


LABELS = {
    0x00: "timer2_empty_harness",
    0x01: "nop_block",
    0x02: "dependent_alu",
    0x03: "cached_load_hazard",
    0x04: "taken_branch_delay",
    0x05: "multu_mflo_small",
    0x06: "multu_mflo_medium",
    0x07: "multu_mflo_large",
    0x08: "divu_mflo",
    0x09: "scratchpad_load_hazard",
    0x0A: "uncached_ram_load_hazard",
    0x0B: "ram_store",
    0x0C: "scratchpad_store",
    0x0D: "uncached_ram_store",
    0x0E: "gpustat_read_hazard",
    0x0F: "irqstat_read_hazard",
    0x10: "spin_64_system",
    0x11: "spin_64_div8",
    0x12: "spin_64_dot",
    0x13: "spin_256_system",
    0x14: "spin_256_div8",
    0x15: "spin_256_dot",
    0x16: "spin_1024_system",
    0x17: "spin_1024_div8",
    0x18: "spin_1024_dot",
    0x19: "spin_4096_system",
    0x1A: "spin_4096_div8",
    0x1B: "spin_4096_dot",
    0x1C: "icache_cold_4k",
    0x1D: "icache_warm_4k",
    0x20: "timer1_hblank_long",
    0x21: "gte_rtps_commands",
    0x22: "gte_rtpt_commands",
    0x23: "gte_nclip_commands",
    0x24: "gte_mvmva_commands",
    0x25: "gte_ncdt_commands",
    0x26: "gte_ncct_commands",
    0x30: "dma_otc_16_words",
    0x31: "dma_otc_64_words",
    0x32: "dma_otc_256_words",
    0x40: "cdrom_getstat_ack",
    0x41: "gpu_irq1_settle",
    0x42: "icache_cold_entry_word0",
    0x43: "icache_cold_entry_word1",
    0x44: "icache_cold_entry_word2",
    0x45: "icache_warm_entry_word0",
    0x46: "branch_not_taken",
    0x47: "cached_ram_byte_load_hazard",
    0x48: "cached_ram_half_load_hazard",
    0x49: "uncached_ram_byte_load_hazard",
    0x4A: "uncached_ram_half_load_hazard",
    0x4B: "bios_rom_word_load_hazard",
    0x4C: "bios_rom_half_load_hazard",
    0x4D: "bios_rom_byte_load_hazard",
    0x4E: "spustat_half_load_hazard",
    0x4F: "sio_stat_word_load_hazard",
    0x50: "cached_ram_byte_store",
    0x51: "cached_ram_half_store",
    0x52: "cdrom_byte_load_hazard",
    0x53: "cdrom_half_load_hazard",
    0x54: "cdrom_word_load_hazard",
    0x55: "expansion1_byte_load_hazard",
    0x56: "expansion1_half_load_hazard",
    0x57: "expansion1_word_load_hazard",
    0x58: "expansion2_byte_load_hazard",
    0x59: "expansion2_half_load_hazard",
    0x5A: "expansion2_word_load_hazard",
    0x5B: "expansion3_byte_load_hazard",
    0x5C: "expansion3_half_load_hazard",
    0x5D: "expansion3_word_load_hazard",
    0x5E: "spustat_byte_load_hazard",
    0x5F: "spu_aligned_word_load_hazard",
    0x60: "cache_control_byte_load_hazard",
    0x61: "cache_control_half_load_hazard",
    0x62: "cache_control_word_load_hazard",
    0x63: "memory_control_word_load_hazard",
    0x64: "spu_unaligned_lwl_lwr_pair",
    0x65: "spu_dma_write_512_halfwords",
    0x66: "gpu_dma_block_16x1",
    0x67: "gpu_dma_block_16x4",
    0x68: "gpu_dma_block_16x16",
    0x69: "gpu_dma_block_64x4",
    0x6A: "gpu_dma_block_256x1",
    0x6B: "gpu_dma_linked_2x128",
    0x6C: "gpu_line_mono_16x16",
    0x6D: "gpu_line_mono_256x8",
    0x6E: "gpu_line_gouraud_16x16",
    0x6F: "gpu_line_gouraud_256x8",
    0x70: "dram_refresh_period_cycles",
    0x71: "dram_refresh_stall_cycles",
    # v1.21 warm-harness performance probes (perf_probes.rs). Layout immune:
    # the timed block is the second pass of an in-assembly double run.
    0x72: "warm_nop_block",
    0x73: "warm_dependent_alu",
    0x74: "warm_cached_ram_load",
    0x75: "warm_scratchpad_load",
    0x76: "warm_ram_store",
    0x77: "warm_scratchpad_store",
    0x78: "warm_taken_branch",
    0x79: "multu_small_gap0",
    0x7A: "multu_small_gap5",
    0x7B: "multu_small_gap6",
    0x7C: "multu_small_gap7",
    0x7D: "multu_medium_gap0",
    0x7E: "multu_medium_gap8",
    0x7F: "multu_medium_gap9",
    0x80: "multu_medium_gap10",
    0x81: "multu_large_gap0",
    0x82: "multu_large_gap12",
    0x83: "multu_large_gap13",
    0x84: "multu_large_gap14",
    0x85: "divu_gap0",
    0x86: "divu_gap34",
    0x87: "divu_gap36",
    0x88: "divu_gap38",
    0x89: "divu_gap40",
    0x8A: "multu_rs_small_rt_large",
    0x8B: "mult_rs_negative_small",
    0x8C: "icache_alias_4k_call_pairs",
    0x8D: "icache_neighbour_call_pairs",
    # CD battery. Unlike every record above, these are Timer 1 HBLANK ticks
    # (~63.9 us each), not Timer 2 system-clock cycles: a seek is orders of
    # magnitude too slow for a 16-bit counter at the system clock.
    0x90: "cd_seek_1_sector_hblanks",
    0x91: "cd_seek_16_sectors_hblanks",
    0x92: "cd_seek_128_sectors_hblanks",
    0x93: "cd_seek_512_sectors_hblanks",
    0x94: "cd_read_8_sectors_single_hblanks",
    0x95: "cd_read_8_sectors_double_hblanks",
    0x96: "cd_getstat_hblanks",
    0x97: "cd_setmode_hblanks",
    0x98: "cd_getlocp_hblanks",
    0x99: "cd_pause_complete_hblanks",
    0x9A: "cd_init_complete_hblanks",
    0x9B: "cd_read_8_sectors_WITH_cdda_hblanks",
    0x9C: "cd_read_8_sectors_no_cdda_hblanks",
    0x9D: "cd_cdda_play_start_hblanks",
    0x9E: "cd_getlocp_during_cdda_hblanks",
    # GPU fill rate, Timer 2 system cycles. Identical pixel counts across
    # shading modes, so differences isolate interpolation/blend/dither cost.
    0xA0: "gpu_fill_tri_flat_16x32",
    0xA1: "gpu_fill_tri_gouraud_16x32",
    0xA2: "gpu_fill_tri_tex4_16x32",
    0xA3: "gpu_fill_tri_tex8_16x32",
    0xA4: "gpu_fill_tri_tex15_16x32",
    0xA5: "gpu_fill_quad_flat_16x32",
    0xA6: "gpu_fill_quad_gouraud_16x32",
    0xA7: "gpu_fill_quad_tex4_16x32",
    0xA8: "gpu_fill_tri_translucent_16x32",
    0xA9: "gpu_fill_tri_gouraud_dithered_16x32",
    0xAA: "gpu_fill_quad_flat_4x64",
    0xAB: "gpu_fill_quad_flat_64x8",
    0xAC: "gpu_fill_quad_tex_uvspan255",
    0xAD: "gpu_fill_quad_tex_uvspan8",
    0xAE: "gpu_fill_rect_mono_16x32",
    0xAF: "gpu_fill_rect_tex8clut_16x32",
    # MDEC, Timer 2 system cycles.
    0xB0: "mdec_quant_table_luma_16w",
    0xB1: "mdec_quant_table_luma_chroma_32w",
    0xB2: "mdec_scale_table_32w",
    0xB3: "mdec_reset_settle",
    0xB4: "mdec_decode_1_macroblock_24bpp",
    0xB5: "mdec_decode_2_macroblocks_24bpp",
    # SIO pad poll at four setup/inter-byte pacings.
    0xB6: "sio_pad_poll_variant0",
    0xB7: "sio_pad_poll_variant1",
    0xB8: "sio_pad_poll_variant2",
    0xB9: "sio_pad_poll_variant3",
    # Seek sweep: the four original distances were too few and non-monotonic.
    0xC0: "cd_seek_2_sectors_hblanks",
    0xC1: "cd_seek_4_sectors_hblanks",
    0xC2: "cd_seek_8_sectors_hblanks",
    0xC3: "cd_seek_32_sectors_hblanks",
    0xC4: "cd_seek_64_sectors_hblanks",
    0xC5: "cd_seek_256_sectors_hblanks",
    0xC6: "cd_seek_BACK_64_sectors_hblanks",
    0xC7: "cd_seek_BACK_256_sectors_hblanks",
    # SIO setup-delay sweep, bracketing where a real pad starts replying.
    0xD0: "sio_setup_0",
    0xD1: "sio_setup_64",
    0xD2: "sio_setup_128",
    0xD3: "sio_setup_192",
    0xD4: "sio_setup_256",
    0xD5: "sio_setup_320",
    0xD6: "sio_setup_448",
    0xD7: "sio_setup_512",
    0xD8: "sio_setup_640",
    0xD9: "sio_setup_896",
    0xDA: "sio_setup_1024",
    0xDB: "sio_setup_1536",
    # v1.22 performance sweep (TARGETED PROBES > PERF SWEEP). Warm harness
    # except the DMA and GPU records, which time the device, not the CPU.
    0x1E: "warm_nop_block_uncached",
    0x1F: "warm_ram_store_then_3_instructions",
    0x27: "warm_gte_rtps",
    0x28: "warm_gte_rtpt",
    0x29: "warm_gte_nclip",
    0x2A: "warm_gte_mvmva",
    0x2B: "warm_gte_avsz3",
    0x2C: "warm_gte_sqr",
    0x2D: "warm_gte_op",
    0x2E: "warm_gte_gpf",
    0x2F: "warm_gte_ncds",
    0x33: "dma_linked_256_empty_nodes",
    0x34: "dma_linked_1024_empty_nodes",
    0x35: "nops_dma_idle",
    0x36: "nops_during_linked_dma_512",
    0x37: "warm_ram_byte_store",
    0x38: "gpu_tiny_tri_gouraud_tex4_2px_x64",
    0x39: "warm_uncached_ram_store",
    0x3A: "rtpt_then_next_inputs_mtc2",
    0x3B: "gpu_tiny_tri_tex4_2px_x64",
    0x3C: "ab_ramsize_cold_load_sweep_control",
    0x3D: "ab_ramsize_cold_load_sweep_bit7_flipped",
    0x3E: "ab_spudelay_status_reads_control",
    0x3F: "ab_spudelay_status_reads_faster",
    0x8E: "multu_small_back_to_back",
    0x8F: "multu_large_back_to_back",
    0x9F: "ram_loads_dma_idle",
    0xBA: "gpu_fill_tri_tex4_raw_16x32",
    0xBB: "gpu_fill_tri_tex4_translucent_16x32",
    0xBC: "gpu_fill_tri_gouraud_tex4_16x32",
    0xBD: "gpu_clipped_tri_flat_16x32",
    0xBE: "gpu_vram_fill_16x32",
    0xBF: "gpu_vram_copy_16x32",
    0xC8: "warm_ram_loads_back_to_back",
    0xC9: "warm_scratchpad_loads_back_to_back",
    0xCA: "warm_ram_byte_load",
    0xCB: "gpu_rect_tex8_clut_alternating_16x32",
    0xCC: "warm_ram_unaligned_lwl_lwr",
    0xCD: "warm_ram_unaligned_swl_swr",
    0xCE: "warm_ram_load_then_4_instructions",
    0xCF: "warm_ram_sequential_loads",
    0xED: "rtpt_gap21",
    0xEE: "rtpt_gap23",
    0xEF: "rtpt_gap25",
    0xF0: "rtps_gap13",
    0xF1: "rtps_gap15",
    0xF2: "rtps_gap17",
    0xF3: "warm_gte_mtc2",
    0xF4: "warm_gte_ctc2",
    0xF5: "warm_gte_mfc2",
    0xF6: "gpu_fill_tri_tex4_letterboxed_16x32",
    0xF7: "lerp3_cpu_mult",
    0xF8: "lerp3_gte_gpf",
    0xF9: "warm_gpustat_read",
    0xFA: "warm_gp0_nop_write",
    0xFB: "warm_irqstat_read",
    0xFC: "warm_spustat_half_read",
    0xFD: "warm_spu_half_write",
    0xFE: "ram_loads_during_linked_dma_512",
    # v1.21 register A/B group. Present only in a PERF A/B capture.
    0xDC: "ab_ramsize_uncached_loads_control",
    0xDD: "ab_ramsize_uncached_loads_bit7_flipped",
    0xDE: "ab_ramsize_cached_loads_control",
    0xDF: "ab_ramsize_cached_loads_bit7_flipped",
    0xE0: "ab_cachectl_loads_control",
    0xE1: "ab_cachectl_cold_sweep_control",
    0xE2: "ab_cachectl_loads_rdpri_flipped",
    0xE3: "ab_cachectl_cold_sweep_rdpri_flipped",
    0xE4: "ab_cachectl_loads_nopad_flipped",
    0xE5: "ab_cachectl_cold_sweep_nopad_flipped",
    0xE6: "ab_cachectl_loads_ldsch_flipped",
    0xE7: "ab_cachectl_cold_sweep_ldsch_flipped",
    0xE8: "ab_cachectl_loads_nostr_flipped",
    0xE9: "ab_cachectl_cold_sweep_nostr_flipped",
    0xEA: "ab_cachectl_cold_sweep_iblksz_2_words",
    0xEB: "ab_cachectl_loads_bgnt_flipped",
    0xEC: "ab_cachectl_cold_sweep_bgnt_flipped",
}

# Warm-harness records: the timed block is the second pass of an in-assembly
# double run, so the minimum does not move when unrelated guest code shifts
# I-cache alignment. Every other CPU record does.
LAYOUT_IMMUNE_RECORDS = (
    frozenset(range(0x72, 0x8C))
    | frozenset({0x1F, 0x37, 0x39, 0x3A, 0x8E, 0x8F, 0xCE})
    | frozenset(range(0x27, 0x30))
    | frozenset({0xC8, 0xC9, 0xCA, 0xCC, 0xCD, 0xCF})
    | frozenset(range(0xED, 0xF6))
    | frozenset(range(0xF7, 0xFE))
)

# Records timed on Timer 1's HBlank clock rather than Timer 2's system clock.
HBLANK_RECORDS = frozenset(range(0x90, 0x9F)) | frozenset(range(0xC0, 0xC8))

GTE_SETTLE_FIRST_CASE = 116
GTE_SETTLE_CASE_COUNT = 22

# Work is fixed by record ID, so the wire carries only id/min/median/max.
WORK_BY_ID = {
    0x00: 0,
    0x01: 128,
    0x02: 128,
    0x03: 64,
    0x04: 64,
    0x05: 16,
    0x06: 16,
    0x07: 16,
    0x08: 8,
    **{record_id: 64 for record_id in range(0x09, 0x13)},
    **{record_id: 256 for record_id in range(0x13, 0x16)},
    **{record_id: 1024 for record_id in range(0x16, 0x19)},
    **{record_id: 4096 for record_id in range(0x19, 0x1C)},
    0x1C: 1024,
    0x1D: 1024,
    0x20: 0xFFFF,
    0x21: 16,
    0x22: 8,
    0x23: 16,
    0x24: 16,
    0x25: 4,
    0x26: 4,
    0x30: 16,
    0x31: 64,
    0x32: 256,
    **{record_id: 1 for record_id in range(0x40, 0x46)},
    **{record_id: 64 for record_id in range(0x46, 0x65)},
    0x65: 512,
    0x66: 16,
    0x67: 64,
    0x68: 256,
    0x69: 256,
    0x6A: 256,
    0x6B: 258,
    0x6C: 272,
    0x6D: 2056,
    0x6E: 272,
    0x6F: 2056,
    0x70: 4096,
    0x71: 4096,
    0x90: 1,
    0x91: 16,
    0x92: 128,
    0x93: 512,
    0x94: 8,
    0x95: 8,
    **{record_id: 1 for record_id in range(0x96, 0x9B)},
    0x9B: 8,
    0x9C: 8,
    0x9D: 1,
    0x9E: 1,
    **{record_id: 16 for record_id in range(0xA0, 0xAA)},
    0xAA: 4,
    0xAB: 64,
    **{record_id: 16 for record_id in range(0xAC, 0xB0)},
    0xB0: 16,
    0xB1: 32,
    0xB2: 32,
    0xB3: 1,
    0xB4: 1,
    0xB5: 2,
    **{record_id: 1 for record_id in range(0xB6, 0xBA)},
    0xC0: 2, 0xC1: 4, 0xC2: 8, 0xC3: 32, 0xC4: 64, 0xC5: 256, 0xC6: 64, 0xC7: 256,
    **{record_id: 0 for record_id in range(0xD0, 0xDC)},
    0x1E: 128,
    0x1F: 64,
    0x27: 16,
    0x28: 8,
    0x29: 16,
    0x2A: 16,
    0x2B: 16,
    0x2C: 16,
    0x2D: 16,
    0x2E: 16,
    0x2F: 8,
    0x33: 256,
    0x34: 1024,
    0x35: 128,
    0x36: 128,
    0x37: 64,
    0x38: 64,
    0x39: 64,
    0x3A: 8,
    0x3B: 64,
    0x3C: 511,
    0x3D: 511,
    0x3E: 64,
    0x3F: 64,
    0x8E: 16,
    0x8F: 16,
    0x9F: 64,
    0xBA: 16,
    0xBB: 16,
    0xBC: 16,
    0xBD: 16,
    0xBE: 16,
    0xBF: 16,
    0xC8: 64,
    0xC9: 64,
    0xCA: 64,
    0xCB: 16,
    0xCC: 64,
    0xCD: 64,
    0xCE: 64,
    0xCF: 64,
    0xED: 8,
    0xEE: 8,
    0xEF: 8,
    0xF0: 16,
    0xF1: 16,
    0xF2: 16,
    0xF3: 16,
    0xF4: 16,
    0xF5: 16,
    0xF6: 16,
    0xF7: 8,
    0xF8: 8,
    0xF9: 64,
    0xFA: 64,
    0xFB: 64,
    0xFC: 64,
    0xFD: 64,
    0xFE: 64,
    0x72: 128,
    0x73: 128,
    0x74: 64,
    0x75: 64,
    0x76: 64,
    0x77: 64,
    0x78: 64,
    0x79: 16,
    0x7A: 16,
    0x7B: 16,
    0x7C: 16,
    0x7D: 16,
    0x7E: 16,
    0x7F: 16,
    0x80: 16,
    0x81: 16,
    0x82: 16,
    0x83: 16,
    0x84: 16,
    0x85: 8,
    0x86: 8,
    0x87: 8,
    0x88: 8,
    0x89: 8,
    0x8A: 16,
    0x8B: 16,
    0x8C: 32,
    0x8D: 32,
    0xDC: 64,
    0xDD: 64,
    0xDE: 64,
    0xDF: 64,
    0xE0: 64,
    0xE1: 1024,
    0xE2: 64,
    0xE3: 1024,
    0xE4: 64,
    0xE5: 1024,
    0xE6: 64,
    0xE7: 1024,
    0xE8: 64,
    0xE9: 1024,
    0xEA: 1024,
    0xEB: 64,
    0xEC: 1024,
}


@dataclass(frozen=True)
class Record:
    record_id: int
    work: int
    minimum: int
    maximum: int
    # -1 means "not carried"; every supported schema carries a median.
    median: int = -1


@dataclass(frozen=True)
class CapturePage:
    schema: str
    number: int
    total: int
    chunk: str
    crc: int


@dataclass(frozen=True)
class ScanSummary:
    status: int
    items: int
    digest: int
    aux: int
    run: int


@dataclass(frozen=True)
class Failure:
    """One case a capture spent bytes on because it did not pass."""

    test_id: int
    expected: int
    observed: int


@dataclass(frozen=True)
class Capture:
    schema: str
    version: int
    # Which blocks the capture carried. Older schemas always carried them all.
    flags: int
    failures: tuple[Failure, ...]
    # How many pages this capture actually took. Fixed per schema before PX8.
    page_count: int
    # Suite version: what the record ids MEAN.
    suite_major: int
    suite_minor: int
    conformance_run: int
    timing_run: int
    conformance_digest: int
    gte_digest: int
    timing_digest: int
    timing_aux: int
    scans: tuple[ScanSummary, ...]
    observations: tuple[int, ...]
    statuses: tuple[int, ...]
    records: tuple[Record, ...]
    memory_control: tuple[int, ...]
    precision: tuple[int, ...]
    binary_crc: int


# PX7: explicit per-record ids and a median column. The slot count comes from
# the header: early v1.5 captures carried 144 slots, later ones 176.
PX7_PRECISION_COUNT = 192
PX7_STATUS_BITS = 3
PX7_SCAN_COUNT = 3
PX7_MEMORY_CONTROL_COUNT = 9
PX7_RECORD_UNUSED = 0xFF
# PX8: per-block flags. A routine capture carries verdicts and one record per
# FAILING case; a characterisation capture carries every block PX7 did, in the
# same field order, so an archived px7-* reference still describes the same run
# a full PX8 does. Counts widened to u16 and the page count is no longer fixed.
PX8_HEADER_LEN = 72
PX8_BLOCK_STATUS = 1 << 0
PX8_BLOCK_FAILURES = 1 << 1
PX8_BLOCK_OBSERVED = 1 << 2
PX8_BLOCK_TIMING = 1 << 3
PX8_BLOCK_MEMCTL = 1 << 4
PX8_BLOCK_PRECISION = 1 << 5
SCHEMAS = ("PX7", "PX8")
# In capture order. The first nine are 0x1F801000..0x1F801020; later suites
# append registers that are not on that linear run. A value past the end of
# this table still prints, under a positional name, rather than vanishing.
MEMORY_CONTROL_NAMES = (
    "exp1_base",
    "exp2_base",
    "exp1_delay",
    "exp3_delay",
    "bios_delay",
    "spu_delay",
    "cdrom_delay",
    "exp2_delay",
    "common_delay",
    "ram_size",
    "cache_control",
)
STATUS_LABELS = {0: "PENDING", 1: "PASS", 2: "FAIL", 3: "WARN", 4: "INFO"}


def memory_control_name(index: int) -> str:
    if index < len(MEMORY_CONTROL_NAMES):
        return MEMORY_CONTROL_NAMES[index]
    return f"register_{index:02d}"


def parse_capture_page(payload: str) -> CapturePage:
    payload = payload.strip()
    if not payload.startswith(tuple(f"{name}/" for name in SCHEMAS)):
        raise ValueError("not a PX7/PX8 hardware payload")
    try:
        body, claimed_crc = payload.rsplit("/C:", 1)
        marker, page_field, chunk = body.split("/", 2)
    except ValueError as exc:
        raise ValueError("malformed capture page") from exc
    if marker not in SCHEMAS or len(page_field) != 4 or not chunk:
        raise ValueError("malformed capture page header")
    actual_crc = binascii.crc32(chunk.encode("ascii")) & 0xFFFF_FFFF
    if int(claimed_crc, 16) != actual_crc:
        raise ValueError(
            f"{marker} page CRC mismatch: payload says {claimed_crc}, "
            f"calculated {actual_crc:08X}"
        )
    return CapturePage(
        marker,
        int(page_field[:2], 16),
        int(page_field[2:], 16),
        chunk,
        actual_crc,
    )


def parse_capture(payloads: list[str]) -> Capture:
    pages = [parse_capture_page(payload) for payload in payloads]
    if not pages:
        raise ValueError("no PX7/PX8 payloads found")
    schemas = {page.schema for page in pages}
    if len(schemas) != 1:
        raise ValueError("capture mixes schema versions")
    schema = schemas.pop()
    totals = {page.total for page in pages}
    # The page count is whatever the pages agree it is: a capture costs as
    # many pages as it has data.
    if len(totals) != 1:
        raise ValueError(f"{schema} pages disagree about how many pages there are")
    page_count = totals.pop()
    # The log can contain an early boot page followed by a freshly encoded
    # page after the pad state settles. Keep the last occurrence, matching the
    # state that is ultimately photographed.
    by_number = {page.number: page for page in pages}
    missing = sorted(set(range(1, page_count + 1)) - set(by_number))
    if missing:
        raise ValueError(f"missing {schema} page(s): " + ", ".join(map(str, missing)))

    encoded = "".join(by_number[number].chunk for number in range(1, page_count + 1))
    try:
        binary = base64.b64decode(encoded, validate=True)
    except binascii.Error as exc:
        raise ValueError(f"invalid {schema} Base64: {exc}") from exc
    claimed_binary_crc = struct.unpack_from("<I", binary, len(binary) - 4)[0]
    actual_binary_crc = binascii.crc32(binary[:-4]) & 0xFFFF_FFFF
    if claimed_binary_crc != actual_binary_crc:
        raise ValueError(
            f"{schema} binary CRC mismatch: payload says {claimed_binary_crc:08X}, "
            f"calculated {actual_binary_crc:08X}"
        )

    if binary[:4] != f"{schema}B".encode("ascii"):
        raise ValueError(f"{schema} binary magic mismatch")
    version = binary[4]
    expected_version = {"PX7": 3, "PX8": 4}[schema]
    if version != expected_version:
        raise ValueError(f"unsupported {schema} binary version {version}")
    # PX7 inserted the suite version after the schema version, so every later
    # header field shifts by two bytes. PX8 adds a flags byte and widens the
    # four counts to u16.
    flags = PX8_BLOCK_STATUS | PX8_BLOCK_OBSERVED | PX8_BLOCK_TIMING
    flags |= PX8_BLOCK_MEMCTL | PX8_BLOCK_PRECISION
    precision_count = None
    if schema == "PX8":
        suite_major = binary[5]
        suite_minor = binary[6]
        flags = binary[7]
        conformance_run = binary[8]
        timing_run = binary[9]
        test_count, timing_count, memory_count, precision_count = struct.unpack_from(
            "<HHHH", binary, 10
        )
        status_bits = binary[18]
        scan_count = binary[19]
        digest_offset = 20
    else:
        suite_major = binary[5]
        suite_minor = binary[6]
        conformance_run = binary[7]
        timing_run = binary[8]
        test_count = binary[9]
        timing_count = binary[10]
        memory_count = binary[11]
        status_bits = binary[12]
        scan_count = binary[13]
        expected_shape = (PX7_MEMORY_CONTROL_COUNT, PX7_STATUS_BITS, PX7_SCAN_COUNT)
        if (memory_count, status_bits, scan_count) != expected_shape:
            raise ValueError(f"PX7 binary shape does not match schema version {version}")
        digest_offset = 14
    conformance_digest, gte_digest, timing_digest, timing_aux = struct.unpack_from(
        "<IIII", binary, digest_offset
    )
    offset = digest_offset + 16
    scans: list[ScanSummary] = []
    for _ in range(scan_count):
        status, items, digest, aux, run = struct.unpack_from("<BH I I B", binary, offset)
        scans.append(ScanSummary(status, items, digest, aux, run))
        offset += 12

    # PX8 writes the status bitmap first and the observations last, because the
    # bitmap is the block a routine capture always has and the observations are
    # the block it usually omits. Older schemas had one fixed order.
    statuses: list[int] = []
    failures: list[Failure] = []
    observations: tuple[int, ...] = ()
    packed_statuses = b""
    if schema == "PX8":
        if flags & PX8_BLOCK_STATUS:
            packed_status_len = (test_count * status_bits + 7) // 8
            packed_statuses = binary[offset : offset + packed_status_len]
            offset += packed_status_len
        if flags & PX8_BLOCK_FAILURES:
            failure_count = struct.unpack_from("<H", binary, offset)[0]
            offset += 2
            for _ in range(failure_count):
                test_id, expected, observed = struct.unpack_from("<HII", binary, offset)
                offset += 10
                failures.append(Failure(test_id, expected, observed))
        if flags & PX8_BLOCK_OBSERVED:
            observations = struct.unpack_from(f"<{test_count}I", binary, offset)
            offset += test_count * 4
    else:
        observations = struct.unpack_from(f"<{test_count}I", binary, offset)
        offset += test_count * 4
        packed_status_len = (test_count * status_bits + 7) // 8
        packed_statuses = binary[offset : offset + packed_status_len]
        offset += packed_status_len
    for index in range(test_count if packed_statuses else 0):
        bit = index * status_bits
        window = packed_statuses[bit // 8]
        if bit // 8 + 1 < len(packed_statuses):
            window |= packed_statuses[bit // 8 + 1] << 8
        status = (window >> (bit % 8)) & 0x7
        if status > 4:
            raise ValueError(f"invalid {schema} status {status} for case {index}")
        statuses.append(status)

    records: list[Record] = []
    if flags & PX8_BLOCK_TIMING:
        # Ids are explicit, so an unfilled slot is skipped rather than
        # shifting every later record's meaning.
        for _ in range(timing_count):
            record_id, minimum, median, maximum = struct.unpack_from("<BHHH", binary, offset)
            offset += 7
            if record_id == PX7_RECORD_UNUSED:
                continue
            records.append(
                Record(record_id, WORK_BY_ID.get(record_id, 0), minimum, maximum, median)
            )
    memory_control: tuple[int, ...] = ()
    if schema != "PX8" or flags & PX8_BLOCK_MEMCTL:
        memory_control = struct.unpack_from(f"<{memory_count}I", binary, offset)
        offset += memory_count * 4
    precision: tuple[int, ...] = ()
    if schema == "PX8":
        if flags & PX8_BLOCK_PRECISION:
            precision = struct.unpack_from(f"<{precision_count}I", binary, offset)
            offset += precision_count * 4
    else:
        precision = struct.unpack_from(f"<{PX7_PRECISION_COUNT}I", binary, offset)
        offset += PX7_PRECISION_COUNT * 4
    if offset != len(binary) - 4:
        raise ValueError(f"{schema} binary parser did not consume the complete payload")

    return Capture(
        schema,
        version,
        flags,
        tuple(failures),
        page_count,
        suite_major,
        suite_minor,
        conformance_run,
        timing_run,
        conformance_digest,
        gte_digest,
        timing_digest,
        timing_aux,
        tuple(scans),
        tuple(observations),
        tuple(statuses),
        tuple(records),
        tuple(memory_control),
        tuple(precision),
        claimed_binary_crc,
    )


def payloads_from_paths(paths: list[str]) -> list[str]:
    def from_text(text: str) -> list[str]:
        found: list[str] = []
        for line in text.splitlines():
            positions = [p for p in (line.find(f"{n}/") for n in SCHEMAS) if p >= 0]
            if positions:
                found.append(line[min(positions):].split()[0])
        return found

    payloads: list[str] = []
    for value in paths:
        if value.startswith(tuple(f"{n}/" for n in SCHEMAS)):
            payloads.append(value)
            continue
        text = pathlib.Path(value).read_text(encoding="utf-8")
        payloads.extend(from_text(text))
    if not paths:
        payloads.extend(from_text(sys.stdin.read()))
    return payloads


def print_report(
    capture: Capture,
    baseline: Capture | None,
    fail_on_change: bool = False,
    layout_immune_only: bool = False,
) -> int:
    # Every baseline difference lands here so the summary can name what moved
    # instead of only reporting that something did.
    drift: list[str] = []
    layout_drift = 0
    page_count = capture.page_count
    print(
        f"# schema={capture.schema} suite=v{capture.suite_major}.{capture.suite_minor} "
        f"pages={page_count} run={capture.timing_run:02X} "
        f"digest={capture.timing_digest:08X} records={len(capture.records)} "
        f"binary_crc={capture.binary_crc:08X}"
    )
    tallies = Counter(STATUS_LABELS[status] for status in capture.statuses)
    print(
        f"# conformance_run={capture.conformance_run:02X} "
        f"digest={capture.conformance_digest:08X} cases={len(capture.statuses)} "
        + " ".join(f"{label.lower()}={count}" for label, count in sorted(tallies.items()))
    )

    # A conformance capture spends its bytes only on cases that did not pass, so
    # this is the whole result. Printed before the per-case table because on
    # such a capture the table is empty.
    if capture.failures:
        print("failure_id,expected,observed")
        for failure in capture.failures:
            print(
                f"{failure.test_id:#06x},0x{failure.expected:08X},0x{failure.observed:08X}"
            )
            if baseline is not None and failure not in baseline.failures:
                drift.append(
                    f"new failure {failure.test_id:#06x}: "
                    f"expected 0x{failure.expected:08X} observed 0x{failure.observed:08X}"
                )
    elif capture.flags & PX8_BLOCK_FAILURES:
        print("# no failures")
    if baseline is not None:
        healed = [f for f in baseline.failures if f not in capture.failures]
        for failure in healed:
            drift.append(f"failure {failure.test_id:#06x} no longer reproduces")

    case_columns = "case,status,observed"
    if baseline is not None:
        case_columns += ",baseline_observed,changed"
    print(case_columns)
    for index, (status, observed) in enumerate(
        zip(capture.statuses, capture.observations)
    ):
        row = f"{index},{STATUS_LABELS[status]},0x{observed:08X}"
        if baseline is not None:
            prior = baseline.observations[index]
            row += f",0x{prior:08X},{int(prior != observed)}"
            if prior != observed:
                drift.append(f"case {index}: 0x{prior:08X} -> 0x{observed:08X}")
        print(row)

    has_median = any(record.median >= 0 for record in capture.records)
    columns = "id,label,work,min,max,jitter" + (",median" if has_median else "")
    if baseline is not None:
        columns += ",baseline_min,delta_min"
    print(columns)
    baseline_records = (
        {record.record_id: record for record in baseline.records}
        if baseline is not None
        else None
    )
    for record in capture.records:
        row = (
            f"{record.record_id:02X},{LABELS.get(record.record_id, 'unlabelled')},"
            f"{record.work},{record.minimum},{record.maximum},"
            f"{record.maximum - record.minimum}"
        )
        if has_median:
            row += f",{record.median}"
        if baseline_records is not None:
            prior = baseline_records.get(record.record_id)
            if prior is None:
                # The operator can skip the rest of the timing scan (START),
                # leaving later records absent from a silicon capture. That
                # is a partial capture, not corruption: compare what exists.
                row += ",absent,n/a"
            else:
                row += f",{prior.minimum},{record.minimum - prior.minimum:+d}"
                tolerated = layout_immune_only and record.record_id not in LAYOUT_IMMUNE_RECORDS
                if prior.minimum != record.minimum and tolerated:
                    layout_drift += 1
                elif prior.minimum != record.minimum:
                    drift.append(
                        f"timing {record.record_id:02X} ({LABELS.get(record.record_id, 'unlabelled')}): "
                        f"min {prior.minimum} -> {record.minimum}"
                    )
        print(row)

    scan_names = ("cpu", "gte", "spu")
    print(
        "# scans="
        + ",".join(
            f"{name}:status={STATUS_LABELS[scan.status]}:items={scan.items}:"
            f"digest={scan.digest:08X}:aux={scan.aux:08X}:run={scan.run:02X}"
            for name, scan in zip(scan_names, capture.scans)
        )
    )
    print(
        "# memory_control="
        + ",".join(
            f"{memory_control_name(index)}:0x{value:08X}"
            for index, value in enumerate(capture.memory_control)
        )
    )
    if capture.precision:
        baseline_precision = baseline.precision if baseline is not None else ()
        print("precision,label,value" + (",baseline_value,changed" if baseline_precision else ""))
        for index, value in enumerate(capture.precision):
            label = precision_label(index)
            row = f"{index:03d},{label},0x{value:08X}"
            if baseline_precision:
                prior = baseline_precision[index]
                row += f",0x{prior:08X},{int(prior != value)}"
                if prior != value:
                    drift.append(f"precision {index:03d} ({label}): 0x{prior:08X} -> 0x{value:08X}")
            print(row)
    settle = capture.observations[
        GTE_SETTLE_FIRST_CASE : GTE_SETTLE_FIRST_CASE + GTE_SETTLE_CASE_COUNT
    ]
    print(
        f"# gte_settle_run={capture.conformance_run:02X} "
        f"digest={capture.gte_digest:08X} cases={len(settle)}"
    )
    print(
        "# gte_settle="
        + ",".join(
            f"{GTE_SETTLE_FIRST_CASE + offset}:0x{value:08X}"
            for offset, value in enumerate(settle)
        )
    )
    if baseline is not None:
        baseline_settle = baseline.observations[
            GTE_SETTLE_FIRST_CASE : GTE_SETTLE_FIRST_CASE + GTE_SETTLE_CASE_COUNT
        ]
        print(
            "# gte_settle_delta="
            + ",".join(
                f"{GTE_SETTLE_FIRST_CASE + offset}:{value - prior:+d}"
                for offset, (value, prior) in enumerate(zip(settle, baseline_settle))
            )
        )
    if baseline is not None:
        if layout_immune_only:
            print(f"# layout_drift_tolerated={layout_drift}")
        print(f"# drift={len(drift)}")
        for entry in drift:
            print(f"# drift: {entry}")
        if drift and fail_on_change:
            print(
                f"FAIL: {len(drift)} value(s) moved against the baseline. "
                "Re-baseline deliberately with `make hwtest-baseline` if this is intended.",
                file=sys.stderr,
            )
            return 1
    return 0


def precision_label(index: int) -> str:
    fixed = {
        0: "spu_delay_boot",
        1: "spu_ctrl_stat_boot",
        18: "spu_single_stop_mode_polls",
        35: "spu_four_stop_mode_polls",
        36: "spu_delay_forced_stable",
        37: "spu_stable_single_block_hash",
        38: "spu_stable_four_block_hash",
        43: "gpu_after_irq_clear",
        44: "gpu_irq_set_read0",
        45: "gpu_irq_set_read1",
        46: "gpu_irq_set_read2",
        47: "gpu_irq_clear_read0",
        48: "gpu_irq_clear_read1",
        61: "timer_target_mode_initial",
        62: "timer_target_counter_initial",
        63: "timer_target_counter_after",
        64: "timer_target_mode_read0",
        65: "timer_target_mode_read1",
        66: "timer_target_istat",
        67: "timer_wrap_mode_initial",
        68: "timer_wrap_counter_initial",
        69: "timer_wrap_counter_after",
        70: "timer_wrap_mode_read0",
        71: "timer_wrap_mode_read1",
        72: "timer_wrap_istat",
    }
    if index in fixed:
        return fixed[index]
    if 2 <= index <= 17:
        return f"spu_boot_single_block_word_{index - 2:02d}"
    if 19 <= index <= 34:
        return f"spu_boot_four_block_word_{index - 19:02d}"
    if 39 <= index <= 42:
        return f"spu_fifo_read_word_{index - 39:02d}"
    if 49 <= index <= 60:
        offset = index - 49
        return f"gpu_dma_dir_{offset // 3}_read{offset % 3}"
    if 73 <= index <= 90:
        return f"gte_nclip_scene_a_settle_gap{index - 26}_mac0"
    if 91 <= index <= 96:
        offset = index - 91
        mode = "immediate" if offset < 3 else "settled"
        return f"gte_op_full_{mode}_mac{offset % 3 + 1}"
    if 97 <= index <= 104:
        return f"spu_voice0_offset_{(index - 97) * 2:02X}_write_ffff"
    if index == 105:
        return "otc_chcr_before_start"
    if 106 <= index <= 111:
        return f"otc_chcr_read{index - 106}"
    if index == 112:
        return "otc_madr_after"
    if index == 113:
        return "otc_bcr_after"
    if index == 114:
        return "otc_remaining_busy_polls"
    if index == 115:
        return "otc_first_word"
    if index == 116:
        return "otc_last_word"
    if 117 <= index <= 119:
        return f"gte_nclip_scene_{index - 117}_mac0"
    if 120 <= index <= 123:
        return f"gte_rtpt_e_then_nclip_a_run{index - 120}"
    if index == 124:
        return "gte_rtpt_e_then_nclip_a_sequence"
    if index == 125:
        return "gte_rtpt_e_then_nclip_b_sequence"
    if index == 126:
        return "gte_rtpt_e_then_nclip_c_sequence"
    if index == 127:
        return "gte_nclip_a_after_c_sequence"
    # PX7 additions: console identity, then bit-exact raster hashes.
    if 128 <= index <= 131:
        return f"bios_date_word_{index - 128:02d}"
    if 132 <= index <= 135:
        return f"bios_version_word_{index - 132:02d}"
    if index == 136:
        return "gpustat_at_rest"
    if index == 137:
        return "mdec_status_after_reset"
    if 138 <= index <= 159:
        return f"raster_hash_{index - 138:02d}"
    if 160 <= index < PX7_PRECISION_COUNT:
        return f"reserved_{index:03d}"
    raise ValueError(f"unknown precision index {index}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--baseline",
        help="optional PX7/PX8 payload file to compare against",
    )
    parser.add_argument(
        "--allow-suite-mismatch",
        action="store_true",
        help="compare captures from different suite versions anyway (unsafe)",
    )
    parser.add_argument(
        "--fail-on-change",
        action="store_true",
        help="exit non-zero if any value moved against --baseline (CI gate)",
    )
    parser.add_argument(
        "--layout-immune-timing-only",
        action="store_true",
        help="count timing drift only for warm-harness records; the older CPU "
        "records move whenever guest code shifts I-cache alignment",
    )
    parser.add_argument("payload_or_file", nargs="*")
    args = parser.parse_args()
    try:
        capture = parse_capture(payloads_from_paths(args.payload_or_file))
        baseline = (
            parse_capture(payloads_from_paths([args.baseline]))
            if args.baseline
            else None
        )
        if baseline is not None and baseline.schema != capture.schema:
            raise ValueError("baseline and capture schemas differ")
        if baseline is not None and not args.allow_suite_mismatch:
            here = (capture.suite_major, capture.suite_minor)
            there = (baseline.suite_major, baseline.suite_minor)
            # A MAJOR difference means a record id can have been redefined, so
            # the diff would compare two different measurements under one name.
            # That is worse than no diff, so it fails rather than warns.
            if here[0] != there[0]:
                raise ValueError(
                    f"suite version mismatch: capture v{here[0]}.{here[1]} vs "
                    f"baseline v{there[0]}.{there[1]}. Record meanings may differ "
                    "across a MAJOR bump; re-baseline, or pass "
                    "--allow-suite-mismatch if you know they are comparable."
                )
            if here != there:
                print(
                    f"# note: capture v{here[0]}.{here[1]} vs baseline "
                    f"v{there[0]}.{there[1]}; shared records remain comparable "
                    "across a MINOR bump",
                    file=sys.stderr,
                )
        return print_report(
            capture, baseline, args.fail_on_change, args.layout_immune_timing_only
        )
    except (OSError, UnicodeError, ValueError) as exc:
        print(f"hwtest-report: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
