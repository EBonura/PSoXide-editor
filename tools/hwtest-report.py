#!/usr/bin/env python3
"""Validate and assemble PSoXide hardware-test photo payloads.

PX7 carries a per-record median and explicit record ids, so a probe can be
added without shifting the meaning of every later record. PX8 adds per-block
flags and a variable page count. v2.0 (one linear run) adds a run id to every
page and `--compare`, the one command that decodes a recording and diffs it
against the silicon baselines. PX5 and PX6 captures are no longer parsed:
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
    0x38: "v122_only_unpaced_gpu_tiny_tri_gouraud_tex4_2px_x64",
    0x39: "warm_uncached_ram_store",
    0x3A: "rtpt_then_next_inputs_mtc2",
    0x3B: "v122_only_unpaced_gpu_tiny_tri_tex4_2px_x64",
    0x3C: "ab_ramsize_cold_load_sweep_control",
    0x3D: "ab_ramsize_cold_load_sweep_bit7_flipped",
    0x3E: "ab_spudelay_status_reads_control",
    0x3F: "ab_spudelay_status_reads_faster",
    0x8E: "multu_small_back_to_back",
    0x8F: "multu_large_back_to_back",
    0x9F: "ram_loads_dma_idle",
    0xBA: "v122_only_unpaced_gpu_fill_tri_tex4_raw_16x32",
    0xBB: "v122_only_unpaced_gpu_fill_tri_tex4_translucent_16x32",
    0xBC: "v122_only_unpaced_gpu_fill_tri_gouraud_tex4_16x32",
    0xBD: "v122_only_unpaced_gpu_clipped_tri_flat_16x32",
    0xBE: "v122_only_unpaced_gpu_vram_fill_16x32",
    0xBF: "v122_only_unpaced_gpu_vram_copy_16x32",
    0xC8: "warm_ram_loads_back_to_back",
    0xC9: "warm_scratchpad_loads_back_to_back",
    0xCA: "warm_ram_byte_load",
    0xCB: "v122_only_unpaced_gpu_rect_tex8_clut_alternating_16x32",
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
    0xF6: "v122_only_unpaced_gpu_fill_tri_tex4_letterboxed_16x32",
    0xF7: "lerp3_cpu_mult",
    0xF8: "lerp3_gte_gpf",
    0xF9: "warm_gpustat_read",
    0xFA: "warm_gp0_nop_write",
    0xFB: "warm_irqstat_read",
    0xFC: "warm_spustat_half_read",
    0xFD: "warm_spu_half_write",
    0xFE: "ram_loads_during_linked_dma_512",
    # v1.23, extended ids (the PX8 TIMING_EXT block). GPU batches submitted as a
    # DMA list that ends in a GP0(1Fh) interrupt request, and the CPU shapes
    # the v1.22 console captures left open.
    0x100: "gpu_list_tri_flat_16x32",
    0x101: "gpu_list_tri_gouraud_16x32",
    0x102: "gpu_list_tri_gouraud_dithered_16x32",
    0x103: "gpu_list_tri_tex4_16x32",
    0x104: "gpu_list_tri_tex4_raw_16x32",
    0x105: "gpu_list_tri_tex4_translucent_16x32",
    0x106: "gpu_list_tri_gouraud_tex4_16x32",
    0x107: "gpu_list_tri_flat_translucent_16x32",
    0x108: "gpu_list_rect_flat_16x32",
    0x109: "gpu_list_rect_tex4_16x32",
    0x10A: "gpu_list_rect_tex8_16x32",
    0x10B: "gpu_list_rect_tex8_clut_alternating_16x32",
    0x10C: "gpu_list_tri_tex4_page_alternating_16x32",
    0x10D: "gpu_list_tri_tex4_uvspan63_16x32",
    0x10E: "gpu_list_tri_flat_clipped_16x32",
    0x10F: "gpu_list_tri_tex4_letterboxed_16x32",
    0x110: "gpu_list_vram_fill_16x32",
    0x111: "gpu_list_vram_copy_16x32",
    0x112: "gpu_list_tiny_tri_flat_2px_x64",
    0x113: "gpu_list_tiny_tri_tex4_2px_x64",
    0x114: "gpu_list_tiny_tri_gouraud_tex4_2px_x64",
    0x120: "multu_small_gap1",
    0x121: "multu_small_gap2",
    0x122: "multu_small_gap3",
    0x123: "multu_small_gap4",
    0x124: "divu_gap35",
    0x125: "warm_ram_load_then_2_instructions",
    0x126: "warm_ram_load_then_3_instructions",
    0x127: "warm_ram_load_then_6_instructions",
    0x128: "warm_ram_load_then_8_instructions",
    0x129: "warm_ram_store_then_1_instruction",
    0x12A: "warm_ram_store_then_2_instructions",
    0x12B: "warm_ram_store_bursts_of_2",
    0x12C: "warm_ram_store_bursts_of_4",
    0x12D: "warm_ram_store_bursts_of_8",
    0x12E: "warm_gp0_nop_write_then_3_instructions",
    0x130: "rtps_then_read_sxy2",
    0x131: "rtps_then_read_mac0",
    0x132: "rtps_then_read_mac1",
    0x133: "rtps_then_read_ir1",
    0x134: "rtps_then_read_otz_after_16_nops",
    0x135: "icache_alias_4k_call_pairs_cached_caller",
    0x136: "icache_neighbour_call_pairs_cached_caller",
    # v1.24: one level2 call of hello-spstack's workload (lever_probes.rs).
    0x137: "spstack_level2_on_ram_stack",
    0x138: "spstack_level2_on_scratchpad_stack",
    0x139: "spstack_level2_on_ram_stack_during_list_dma",
    0x13A: "spstack_level2_on_scratchpad_stack_during_list_dma",
    0x140: "spu_dma_idle_ram_loads",
    0x141: "spu_dma_running_ram_loads",
    0x142: "otc_dma_idle_ram_loads",
    0x143: "otc_dma_running_ram_loads",
    0x144: "gpu_block_dma_idle_ram_loads",
    0x145: "gpu_block_dma_running_ram_loads",
    0x150: "gte_rtps_then_read_mac0",
    0x151: "gte_rtps_then_read_mac1",
    0x152: "gte_rtps_back_to_back",
    0x153: "gte_nclip_then_read_mac0",
    0x154: "gte_nclip_then_read_mac1",
    0x155: "gte_nclip_back_to_back",
    0x156: "gte_op_then_read_mac0",
    0x157: "gte_op_then_read_mac1",
    0x158: "gte_op_back_to_back",
    0x159: "gte_dpcs_then_read_mac0",
    0x15A: "gte_dpcs_then_read_mac1",
    0x15B: "gte_dpcs_back_to_back",
    0x15C: "gte_intpl_then_read_mac0",
    0x15D: "gte_intpl_then_read_mac1",
    0x15E: "gte_intpl_back_to_back",
    0x15F: "gte_mvmva_then_read_mac0",
    0x160: "gte_mvmva_then_read_mac1",
    0x161: "gte_mvmva_back_to_back",
    0x162: "gte_ncds_then_read_mac0",
    0x163: "gte_ncds_then_read_mac1",
    0x164: "gte_ncds_back_to_back",
    0x165: "gte_cdp_then_read_mac0",
    0x166: "gte_cdp_then_read_mac1",
    0x167: "gte_cdp_back_to_back",
    0x168: "gte_ncdt_then_read_mac0",
    0x169: "gte_ncdt_then_read_mac1",
    0x16A: "gte_ncdt_back_to_back",
    0x16B: "gte_nccs_then_read_mac0",
    0x16C: "gte_nccs_then_read_mac1",
    0x16D: "gte_nccs_back_to_back",
    0x16E: "gte_cc_then_read_mac0",
    0x16F: "gte_cc_then_read_mac1",
    0x170: "gte_cc_back_to_back",
    0x171: "gte_ncs_then_read_mac0",
    0x172: "gte_ncs_then_read_mac1",
    0x173: "gte_ncs_back_to_back",
    0x174: "gte_nct_then_read_mac0",
    0x175: "gte_nct_then_read_mac1",
    0x176: "gte_nct_back_to_back",
    0x177: "gte_sqr_then_read_mac0",
    0x178: "gte_sqr_then_read_mac1",
    0x179: "gte_sqr_back_to_back",
    0x17A: "gte_dcpl_then_read_mac0",
    0x17B: "gte_dcpl_then_read_mac1",
    0x17C: "gte_dcpl_back_to_back",
    0x17D: "gte_dpct_then_read_mac0",
    0x17E: "gte_dpct_then_read_mac1",
    0x17F: "gte_dpct_back_to_back",
    0x180: "gte_avsz3_then_read_mac0",
    0x181: "gte_avsz3_then_read_mac1",
    0x182: "gte_avsz3_back_to_back",
    0x183: "gte_avsz4_then_read_mac0",
    0x184: "gte_avsz4_then_read_mac1",
    0x185: "gte_avsz4_back_to_back",
    0x186: "gte_rtpt_then_read_mac0",
    0x187: "gte_rtpt_then_read_mac1",
    0x188: "gte_rtpt_back_to_back",
    0x189: "gte_gpf_then_read_mac0",
    0x18A: "gte_gpf_then_read_mac1",
    0x18B: "gte_gpf_back_to_back",
    0x18C: "gte_gpl_then_read_mac0",
    0x18D: "gte_gpl_then_read_mac1",
    0x18E: "gte_gpl_back_to_back",
    0x18F: "gte_ncct_then_read_mac0",
    0x190: "gte_ncct_then_read_mac1",
    0x191: "gte_ncct_back_to_back",
    0x192: "m1_rtps_then_swc2_sxy2_scratchpad",
    0x193: "m1_rtps_then_cfc2_flag",
    0x194: "m1_rtps_then_lwc2_zero_scratchpad",
    0x195: "m2_rtps_then_swc2_sxy2_cached_ram",
    0x1A0: "mmio_timer0_counter_read",
    0x1A1: "mmio_dma2_chcr_read",
    0x1A2: "mmio_dpcr_read",
    0x1A3: "mmio_timer2_counter_read_timed_by_timer0",
    0x1A8: "scratchpad_lb",
    0x1A9: "scratchpad_lbu",
    0x1AA: "scratchpad_lh",
    0x1AB: "scratchpad_lhu",
    0x1AC: "scratchpad_sb_nop",
    0x1AD: "scratchpad_sh_nop",
    0x1B0: "isc_clear_scratchpad_sw_control",
    0x1B1: "isc_clear_scratchpad_lw_control",
    0x1B2: "isc_set_sw",
    0x1B3: "isc_set_lw",
    0x1B4: "isc_set_tag_sw",
    0x1B5: "isc_set_tag_lw",
    0x1B8: "cold_after_evictor_64_sw_ram",
    0x1B9: "cold_after_evictor_64_sw_scratchpad",
    0x1BA: "cold_after_evictor_32_gpustat_lw_nop",
    0x1BB: "cold_after_evictor_32_ram_lw_nop",
    0x1BC: "cold_after_evictor_64_nops",
    0x1C0: "warm_bios_rom_lw_nop",
    0x1C1: "warm_bios_rom_bare_lw",
    0x1C2: "warm_bios_rom_lhu_nop",
    0x1C3: "warm_bios_rom_lbu_nop",
    0x1C4: "warm_exp1_lw_nop",
    0x1C5: "warm_exp1_bare_lw",
    0x1C6: "warm_exp1_lhu_nop",
    0x1C7: "warm_exp1_lbu_nop",
    0x1C8: "warm_exp3_lw_nop",
    0x1C9: "warm_exp3_bare_lw",
    0x1CA: "warm_exp3_lhu_nop",
    0x1CB: "warm_exp3_lbu_nop",
    0x1D0: "divu_multu_small_gap0_mflo",
    0x1D1: "divu_multu_small_gap10_mflo",
    0x1D2: "divu_multu_small_gap30_mflo",
    0x1D3: "divu_multu_small_gap36_mflo",
    0x1D4: "divu_36_nops_control",
    0x1D5: "multu_large_mtlo_mflo",
    0x1D6: "multu_small_mthi_mfhi",
    0x1D7: "multu_large_mthi_mfhi",
    0x1D8: "multu_small_mtlo_mflo",
    0x1E0: "gpu_walk_idle_64_sw_back_to_back",
    0x1E1: "gpu_walk_during_64_sw_back_to_back",
    0x1E2: "gpu_walk_idle_64_sw_3_nops",
    0x1E3: "gpu_walk_during_64_sw_3_nops",
    0x1E4: "gpu_walk_idle_64_lw_3_nops",
    0x1E5: "gpu_walk_during_64_lw_3_nops",
    0x1E6: "gpu_walk_idle_64_scratchpad_lw_nop",
    0x1E7: "gpu_walk_during_64_scratchpad_lw_nop",
    0x200: "tri_list16_8x8_tex4_same_page_clut",
    0x201: "tri_list16_8x8_tex4_alternating_page",
    0x202: "tri_list16_8x8_tex4_alternating_clut",
    0x203: "tri_list16_8x8_tex4_moving_uv_window",
    0x204: "tri_list16_8x8_tex8_same_page_clut",
    0x205: "tri_list16_8x8_tex8_alternating_page",
    0x206: "tri_list16_8x8_tex8_alternating_clut",
    0x207: "tri_list16_8x8_tex8_moving_uv_window",
    0x208: "tri_list16_8x8_tex15_same_page_clut",
    0x209: "tri_list16_8x8_tex15_alternating_page",
    0x20A: "tri_list16_8x8_tex15_alternating_clut",
    0x20B: "tri_list16_8x8_tex15_moving_uv_window",
    0x20C: "tri_list16_16x32_tex4_same_page_clut",
    0x20D: "tri_list16_16x32_tex4_alternating_page",
    0x20E: "tri_list16_16x32_tex4_alternating_clut",
    0x20F: "tri_list16_16x32_tex4_moving_uv_window",
    0x210: "tri_list16_16x32_tex8_same_page_clut",
    0x211: "tri_list16_16x32_tex8_alternating_page",
    0x212: "tri_list16_16x32_tex8_alternating_clut",
    0x213: "tri_list16_16x32_tex8_moving_uv_window",
    0x214: "tri_list16_16x32_tex15_same_page_clut",
    0x215: "tri_list16_16x32_tex15_alternating_page",
    0x216: "tri_list16_16x32_tex15_alternating_clut",
    0x217: "tri_list16_16x32_tex15_moving_uv_window",
    0x218: "tri_list16_32x32_tex4_same_page_clut",
    0x219: "tri_list16_32x32_tex4_alternating_page",
    0x21A: "tri_list16_32x32_tex4_alternating_clut",
    0x21B: "tri_list16_32x32_tex4_moving_uv_window",
    0x21C: "tri_list16_32x32_tex8_same_page_clut",
    0x21D: "tri_list16_32x32_tex8_alternating_page",
    0x21E: "tri_list16_32x32_tex8_alternating_clut",
    0x21F: "tri_list16_32x32_tex8_moving_uv_window",
    0x220: "tri_list16_32x32_tex15_same_page_clut",
    0x221: "tri_list16_32x32_tex15_alternating_page",
    0x222: "tri_list16_32x32_tex15_alternating_clut",
    0x223: "tri_list16_32x32_tex15_moving_uv_window",
    # v1.27 CONSOLE TESTS (src/console_tests.rs); v2.0 runs them as steps of
    # the linear run. console_rows() below names the fields.
    0x2C0: "kernel_enter_critical_section_cycles",
    0x2C1: "kernel_exit_critical_section_cycles",
    0x2C2: "kernel_empty_call_cycles",
    0x2C3: "kernel_bios_vblank_round_trip_cycles",
    0x2C4: "kernel_bios_vblank_gaps_events_flags",
    0x2C5: "kernel_runtime_vblank_round_trip_cycles",
    **{0x2D0 + k: f"display_width_{w}" for k, w in enumerate((256, 320, 368, 384, 512, 640))},
    0x2D6: "interlace_status_field_changes_frames",
    0x2D7: "interlace_frames_interlaced_480_status_low",
    0x2E0: "xa_flags_loops_seconds",
    0x2E1: "xa_loop_gap_ms",
    0x2E2: "xa_loop_period_ms",
    0x2E3: "xa_first_start_ms_positions_max_stall_ms",
    # v1.28 CD STREAM cases (src/cdstream_cases.rs): the transport's cost, the
    # CD-DA hand-off, the motor after Pause and Stop. console_rows() names the
    # fields.
    0x2F0: "cdcost_rate_2x_x10",
    0x2F1: "cdcost_rate_1x_x10",
    0x2F2: "cdcost_pio_us_per_sector_2x",
    0x2F3: "cdcost_pio_us_per_sector_1x",
    0x2F4: "cdcost_lost_permille_2x_1x_irq_x100",
    0x2F5: "cdcost_handler_us_2x_1x_stack_unused",
    0x2F6: "cdcost_first_sector_ms_2x_1x_discarded",
    0x2F7: "cdcost_flags_chained_2x_1x",
    0x300: "cdda_lease_stops_read_ms",
    0x301: "cdda_pause_complete_x10_ms",
    0x302: "cdda_pause_ack_x10_stat_held_still",
    0x303: "cdda_first_sector_after_audio_ms",
    0x304: "cdda_sectors_after_audio_ms",
    0x305: "cdda_resume_playing_ms",
    0x306: "cdda_resume_moved_in_place_playing",
    0x307: "cdda_recovery_first_done_code",
    0x308: "cdda_recovery_flags_signal_quiet",
    0x309: "cdda_bare_first_done_code",
    0x30A: "cdda_bare_flags_signal_quiet",
    0x30B: "cdda_control_signal_samples_flags",
    0x310: "cdmotor_pause_wait_0s",
    0x311: "cdmotor_pause_wait_5s",
    0x312: "cdmotor_pause_wait_15s",
    0x313: "cdmotor_stop_ack_at_once_done_code",
    0x314: "cdmotor_stop_complete_motor_off_settled_done",
    0x315: "cdmotor_flags_recovered_first_at_once",
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
    | frozenset(range(0x120, 0x137))
)

# Records timed on Timer 1's HBlank clock rather than Timer 2's system clock.
HBLANK_RECORDS = frozenset(range(0x90, 0x9F)) | frozenset(range(0xC0, 0xC8))

GTE_SETTLE_FIRST_CASE = 116
GTE_SETTLE_CASE_COUNT = 22

# v1.24 list-busy probe (list_busy_probes.rs, case ids 0xD3-0xE9). Conformance
# cases travel by index, so these name indices 211-233. Stamps are Timer 2
# clocks from the DMA kick, 0xFFFFFFFF for an event never seen. A packed loop
# case carries walk iterations in bits 0-15 and idle clocks for 256 iterations
# in bits 16-31.
LIST_BUSY_FIRST_CASE = 211
LIST_BUSY_LABELS = tuple(
    f"{kind}_list_{event}"
    for kind in ("empty", "cheap", "expensive", "packed")
    for event in ("chcr_clear", "gp0_1f_irq", "gpustat28_settled", "gpustat26_settled")
) + (
    "packed_list_pixels_match",
    "alu_loop_iterations",
    "alu_loop_walk_clocks",
    "ram_load_loop_iterations",
    "ram_load_loop_walk_clocks",
    "scratchpad_load_loop_iterations",
    "scratchpad_load_loop_walk_clocks",
)
LIST_BUSY_NOT_SEEN = 0xFFFFFFFF


def list_busy_rows(capture: Capture) -> list[str]:
    """The list-busy cases, labelled and unpacked. Empty unless the capture
    carries every observation (a FULL CHARACTERISATION capture)."""
    end = LIST_BUSY_FIRST_CASE + len(LIST_BUSY_LABELS)
    if len(capture.observations) < end:
        return []
    rows = ["list_busy,label,value"]
    for offset, label in enumerate(LIST_BUSY_LABELS):
        index = LIST_BUSY_FIRST_CASE + offset
        value = capture.observations[index]
        if label == "packed_list_pixels_match":
            rows.append(f"{index},{label},{STATUS_LABELS[capture.statuses[index]]}")
        elif label.endswith("_iterations"):
            loop = label.removesuffix("_iterations")
            rows.append(f"{index},{loop}_walk_iterations,{value & 0xFFFF}")
            rows.append(f"{index},{loop}_idle_clocks_per_256,{value >> 16}")
        elif value == LIST_BUSY_NOT_SEEN:
            rows.append(f"{index},{label},never")
        else:
            rows.append(f"{index},{label},{value}")
    return rows

# ---------------------------------------------------------------------------
# v2.0 records (the linear run). The guest documents each in the `/// rec`
# comment above its id; tools/test_hwtest_tools.py keeps this table equal to
# those comments. Each entry: label, (min field, median field, max field).
V2_RECORDS = {
    0x400: ("boot_vector", ("bios_stub_standard", "vector_hash_lo", "vector_hash_hi")),
    0x401: ("boot_reverb_a", ("spucnt", "spustat", "reverb_volume_left")),
    0x402: ("boot_reverb_b", ("reverb_volume_right", "reverb_work_base", "eon_low")),
    0x403: ("boot_reverb_c", ("config_hash_low", "config_hash_high", "config_nonzero_words")),
    0x404: ("post_init_reverb", ("volume_left", "volume_right", "work_base")),
    0x41A: ("silence_final", ("flags_0x7f_is_silent", "cd_capture_peak", "voices_with_envelope")),
    0x41B: ("run_info", ("skipped_risky", "steps", "records_taken")),
    0x41C: ("handoff_baseline", ("irq_mask_baseline", "dpcr_baseline_low_half", "dpcr_baseline_high_half")),
    0x420: ("mdec_setup", ("worked_mask", "driver_notes", "failing_steps")),
    0x421: ("mdec_probe", ("words_of_128", "all_equal", "first_word_low")),
    0x422: ("mdec_probe_word", ("first_word_high", "decode_to_request_clocks", "busy_run_words")),
    0x423: ("mdec_status", ("after_reset_high", "after_tables_high", "after_probe_high")),
    0x424: ("mdec_trace_idle", ("samples", "last_status_high", "last_change_clocks")),
    0x425: ("mdec_trace_busy", ("samples", "last_status_high", "last_change_clocks")),
    0x426: ("mdec_timeout", ("chcr_low", "bcr_high", "madr_low")),
    0x430: ("sb1_audit", ("blocks_or_loop_starts", "flags_or_and_last_or_readback_low", "first_end_block_or_readback_high")),
    0x431: ("sb1_upload", ("loop_starts", "readback_fnv_low", "readback_fnv_high")),
}
for _k in range(5):
    V2_RECORDS[0x432 + _k] = (
        f"sb1_stage_{_k + 1}",
        ("envelope_at_frame_32", "envelope_at_frame_100", "endx_bit_per_checkpoint"),
    )
for _k in range(10):
    V2_RECORDS[0x410 + _k] = (
        f"handoff_area_{_k}",
        ("clean_flags_0x3f_is_clean", "interrupt_mask", "voices_active_high_dma_busy_mask_low"),
    )
for _k in range(22):
    V2_RECORDS[0x440 + _k] = (
        f"sb2_tone_{_k}",
        ("late_envelope", "repeat_address", "start_address"),
    )
    V2_RECORDS[0x460 + _k] = (
        f"sb2_early_{_k}",
        ("early_envelope_or_endx", "late_pitch", "table_word"),
    )
for _k in range(7):
    V2_RECORDS[0x480 + _k] = (
        f"sb2_ram_{_k}",
        ("mismatching_words", "first_bad_index", "word_read_there"),
    )
for _k in range(5):
    V2_RECORDS[0x490 + _k] = (
        f"sb4_hash_{_k}",
        ("ring_half_crc_low", "ring_half_crc_high", "first_nonzero_sample"),
    )
    V2_RECORDS[0x49A + _k] = (
        f"sb4_state_{_k}",
        ("spustat", "envelope", "raw_sample_16"),
    )
    V2_RECORDS[0x510 + _k] = (
        f"cd_route_stage_{_k}",
        ("capture_peak_left", "capture_peak_right", "state_word"),
    )
for _slot in range(2):
    for _k in range(7):
        V2_RECORDS[0x4A0 + _slot * 8 + _k] = (
            f"handoff_stage_{('baseline', 'safe2')[_slot]}_{_k}",
            ("voices_with_volume_low", "blocking_event_vblanks_or_readback_match", "endx_low_or_readback_hash_low"),
        )
for _k in range(8):
    V2_RECORDS[0x500 + _k] = (
        f"cl2_variant_{_k}",
        ("ok_bits_and_data_match", "diag_or_fifo_wait_low", "drive_state_high"),
    )
# SIO0 measurements (src/sio_timing.rs), the pad engine (src/pad_engine.rs)
# and the Timer 1 rate (src/timer1_rate.rs).
_ACK = ("ack_rise_cycles", "ack_width_cycles", "byte_done_cycles")
_ANSWER = ("replies_0_and_1", "replies_2_and_3", "ack_mask_and_flags")
for _port in range(2):
    V2_RECORDS[0x600 + _port] = (
        f"sio_setup_p{_port + 1}",
        ("first_ok_delay_cycles", "last_failed_delay_cycles", "ok_mask_per_delay"),
    )
    for _k in range(9):
        V2_RECORDS[0x610 + 16 * _port + _k] = (f"sio_pad_ack_p{_port + 1}_b{_k}", _ACK)
    for _k in range(4):
        V2_RECORDS[0x6C0 + 8 * _port + _k] = (f"sio_card_ack_p{_port + 1}_b{_k}", _ACK)
    V2_RECORDS[0x630 + _port] = (f"sio_pad_answer_p{_port + 1}", _ANSWER)
    V2_RECORDS[0x634 + _port] = (f"sio_card_answer_p{_port + 1}", _ANSWER)
    V2_RECORDS[0x638 + _port] = (
        f"engine_hotplug_p{_port + 1}",
        ("transitions", "initial_health_and_final_health", "frames_absent"),
    )
    V2_RECORDS[0x63A + _port] = (
        f"engine_hotplug_frames_p{_port + 1}",
        ("first_event_frame", "last_event_frame", "frames_watched"),
    )
    for _load in range(2):
        _base = 0x640 + 4 * (2 * _port + _load)
        _tag = f"p{_port + 1}_{('idle', 'loaded')[_load]}"
        V2_RECORDS[_base] = (
            f"sio_mix_count_{_tag}",
            ("pad_ok_and_tried", "card_ok_and_tried", "error_counts"),
        )
        V2_RECORDS[_base + 1] = (
            f"sio_mix_card_{_tag}",
            ("card_frame_hblanks_min", "card_frame_hblanks_med", "card_frame_hblanks_max"),
        )
        V2_RECORDS[_base + 2] = (
            f"sio_mix_pad_{_tag}",
            ("pad_poll_cycles_min", "pad_poll_cycles_med", "pad_poll_cycles_max"),
        )
_TICK_CASES = ("t2_sysclk", "t0_sysclk", "t2_sysclk_div8", "t0_dotclock")
for _k, _name in enumerate(_TICK_CASES):
    for _shape, _label in enumerate(("6_instr", "14_instr")):
        V2_RECORDS[0x6B0 + 2 * _k + _shape] = (
            f"tick_loss_{_name}_{_label}",
            ("reference_hblanks", "ticks_seen_kilo", "ticks_per_hblank_x16"),
        )
for _group, _tag in enumerate(("spu", "otc", "gpu_block", "gpu_list")):
    V2_RECORDS[0x780 + 2 * _group] = (
        f"dma_end_{_tag}",
        ("timeout_flags", "chcr_high_half", "device_status"),
    )
    V2_RECORDS[0x781 + 2 * _group] = (
        f"dma_end_registers_{_tag}",
        ("madr_low_half", "bcr_high_half", "bcr_low_half"),
    )
V2_RECORDS[0x788] = (
    "spu_mode_wait",
    ("iterations_first", "iterations_longest", "mode_never_matched"),
)
for _k in range(10):
    V2_RECORDS[0x790 + _k] = (
        f"dma_matrix_state_{('cd_1x_2048_lw', 'cd_1x_2048_sw', 'cd_1x_2340_lw', 'cd_1x_2340_sw', 'cd_2x_2048_lw', 'cd_2x_2048_sw', 'cd_2x_2340_lw', 'cd_2x_2340_sw', 'mdec_lw', 'mdec_sw')[_k]}",
        ("chcr_high_half", "madr_low_half", "device_status"),
    )
V2_RECORDS[0x650] = ("timer1_free", ("hblanks_in_window", "frames_in_window", "spins_without_vblank"))
V2_RECORDS[0x651] = ("timer1_polled", ("hblanks_in_window", "distinct_values_seen", "largest_step_between_reads"))
V2_RECORDS[0x652] = ("timer1_polled_reads", ("reads_low", "reads_high", "steps_larger_than_one"))
for _k in range(7):
    V2_RECORDS[0x660 + _k] = (
        f"engine_setup_{2000 + 1000 * _k}",
        ("clean_updates_port1", "faults_port1", "health_port1_and_port2"),
    )
for _pacing, _base in (("ack", 0x670), ("timed", 0x678)):
    V2_RECORDS[_base] = (f"engine_port_{_pacing}_p1", ("updates", "faults", "health_and_last_fault"))
    V2_RECORDS[_base + 1] = (f"engine_port_{_pacing}_p2", ("updates", "faults", "health_and_last_fault"))
    V2_RECORDS[_base + 2] = (f"engine_stats_{_pacing}", ("events", "stalls", "spurious"))
    V2_RECORDS[_base + 3] = (f"engine_work_{_pacing}", ("work_avg", "work_min", "work_idle_avg"))
    V2_RECORDS[_base + 4] = (
        f"engine_pad_{_pacing}",
        ("mode_changes", "kicks", "final_mode_and_buttons_seen"),
    )
V2_RECORDS[0x690] = ("engine_card_pad", ("pad_faults", "leased_skips", "card_checksum_errors"))
V2_RECORDS[0x691] = ("engine_card_ops", ("card_ok", "card_tried", "slot_with_card"))
V2_RECORDS[0x692] = ("engine_card_wait", ("lease_wait_med_cycles", "lease_wait_max_cycles", "pad_updates"))
V2_RECORDS[0x694] = (
    "engine_card_writes",
    ("writes_ok", "writes_tried", "write_frame_hblanks_med"),
)
V2_RECORDS[0x693] = (
    "engine_card_frame",
    ("card_frame_hblanks_min", "card_frame_hblanks_med", "card_frame_hblanks_max"),
)
_LOAD_PHASES = (
    "no_ports_all_load",
    "both_ports_alone",
    "gpu",
    "spu",
    "cd_reader_mask",
    "cd_pad_irqs_open",
    "all_load_pad_irqs_open",
)
for _phase, _tag in enumerate(_LOAD_PHASES):
    _base = 0x720 + 6 * _phase
    V2_RECORDS[_base] = (f"engine_load_work_{_tag}", ("rounds_avg", "rounds_min", "rounds_max"))
    V2_RECORDS[_base + 1] = (f"engine_load_health_{_tag}", ("pad_faults", "stalls", "spurious"))
    V2_RECORDS[_base + 2] = (
        f"engine_load_stack_{_tag}",
        ("handler_stack_unused_bytes", "events", "kicks"),
    )
    V2_RECORDS[_base + 3] = (
        f"engine_load_irq_{_tag}",
        ("longest_load_call_cycles", "load_calls_over_a_byte", "entered_ie_clear_and_pending"),
    )
    V2_RECORDS[_base + 4] = (
        f"engine_load_mask_{_tag}",
        ("i_mask_after_load_start", "i_mask_at_end", "i_stat_at_end"),
    )
    V2_RECORDS[_base + 5] = (
        f"engine_load_totals_{_tag}",
        ("port1_faults_since_install", "stalls_since_install", "port1_updates_since_install"),
    )
for _port in range(2):
    _base = 0x760 + 3 * _port
    V2_RECORDS[_base] = (
        f"rumble_config_p{_port + 1}",
        ("pad_id_plain", "pad_id_in_config_mode", "config_flags"),
    )
    V2_RECORDS[_base + 1] = (
        f"rumble_mapping_p{_port + 1}",
        ("old_mapping_bytes_0_1", "old_mapping_bytes_2_3", "old_mapping_bytes_4_5"),
    )
    V2_RECORDS[_base + 2] = (
        f"rumble_after_p{_port + 1}",
        ("id_and_5a_after_motor_poll", "buttons_in_that_poll", "id_after_stop_all"),
    )
    V2_RECORDS[0x767 + _port] = (
        f"rumble_poll_cost_p{_port + 1}",
        ("poll_cycles_idle", "poll_cycles_motors_on", "polls_each"),
    )
for _port in range(2):
    _tag = f"p{_port + 1}"
    V2_RECORDS[0x769 + 2 * _port] = (
        f"rumble_enter_a_{_tag}",
        ("last_attempt_replies_1_2", "replies_3_4", "replies_5_6"),
    )
    V2_RECORDS[0x76A + 2 * _port] = (
        f"rumble_enter_b_{_tag}",
        ("last_attempt_replies_7_8", "attempts_made", "first_attempt_replies_1_2"),
    )
    V2_RECORDS[0x76D + _port] = (
        f"pad_identity_{_tag}",
        ("reply_id_and_5a", "reply_buttons", "reply_sticks_01"),
    )
    V2_RECORDS[0x76F + _port] = (
        f"pad_model_{_tag}",
        ("query_replies_3_4", "query_replies_5_6", "query_replies_7_8"),
    )
for _group, _tag in enumerate(("p1_idle", "p1_loaded", "p2_idle", "p2_loaded")):
    V2_RECORDS[0x6D0 + 3 * _group] = (
        f"sio_mix_fault_{_tag}",
        ("round_and_kind", "fault_and_exchange", "exchanges_and_acks"),
    )
    V2_RECORDS[0x6D1 + 3 * _group] = (
        f"sio_mix_fault_prefix_a_{_tag}",
        ("response_bytes_0_1", "response_bytes_2_3", "response_bytes_4_5"),
    )
    V2_RECORDS[0x6D2 + 3 * _group] = (
        f"sio_mix_fault_prefix_b_{_tag}",
        ("response_bytes_6_7", "response_bytes_8_9", "failures_of_any_kind"),
    )
V2_RECORDS[0x7A0] = (
    "dma_rekick_first",
    ("bcr_after_first_high_half", "bcr_after_first_low_half", "flags"),
)
V2_RECORDS[0x7A1] = (
    "dma_rekick_second",
    ("bcr_after_second_high_half", "bcr_after_second_low_half", "flags"),
)
V2_RECORDS[0x7A2] = (
    "dma_rekick_progress",
    ("blocks_past_source", "bound_iterations_used", "madr_low_half_at_the_bound"),
)
V2_RECORDS[0x766] = (
    "rumble_operator",
    ("answers_two_bits_each", "stimuli_asked", "port_tested_and_api_enabled_in_bit_8"),
)
for _speed, _speed_tag in enumerate(("1x", "2x")):
    for _size, _size_tag in enumerate(("2048", "2340")):
        for _loop, _loop_tag in enumerate(("lw", "sw")):
            V2_RECORDS[0x700 + _speed * 4 + _size * 2 + _loop] = (
                f"cd_dma_loop_{_speed_tag}_{_size_tag}_{_loop_tag}",
                ("loop_idle_clocks", "loop_during_clocks", "transfer_clocks_div32"),
            )
for _loop, _loop_tag in enumerate(("lw", "sw")):
    V2_RECORDS[0x708 + _loop] = (
        f"mdec_dma_loop_{_loop_tag}",
        ("loop_idle_clocks", "loop_during_clocks", "transfer_clocks_div32"),
    )
V2_HANDOFF_CLEAN = 0x3F
V2_SILENT = 0x7F


def v2_rows(capture: Capture) -> list[str]:
    """Every v2.0 record the capture carries, one row per field."""
    rows = []
    for record in capture.records:
        entry = V2_RECORDS.get(record.record_id)
        if entry is None:
            continue
        if not rows:
            rows.append("v2,record,field,value")
        label, names = entry
        for name, value in zip(names, (record.minimum, record.median, record.maximum)):
            rows.append(f"v2,{record.record_id:03X}_{label},{name},{value}")
    return rows


def v2_verdicts(capture: Capture) -> list[str]:
    """The run's own pass criteria, re-derived from the records: every area
    handoff clean, the final silence check passed. Empty list = all good."""
    if capture.suite_major < 2:
        return []
    by_id = {record.record_id: record for record in capture.records}
    problems = []
    for area in range(10):
        record = by_id.get(0x410 + area)
        if record is None:
            problems.append(f"area {area}: no handoff record")
            continue
        if record.minimum != V2_HANDOFF_CLEAN or record.maximum != 0:
            problems.append(
                f"area {area}: handoff flags 0x{record.minimum:02X} "
                f"(clean is 0x{V2_HANDOFF_CLEAN:02X}), busy/active 0x{record.maximum:04X}"
            )
    silence = by_id.get(0x41A)
    if silence is None:
        problems.append("no final silence record")
    elif silence.minimum != V2_SILENT:
        problems.append(
            f"final silence flags 0x{silence.minimum:02X} (silent is 0x{V2_SILENT:02X}), "
            f"capture peak {silence.median}, voices with envelope {silence.maximum}"
        )
    return problems


# v1.27 CONSOLE TESTS (src/console_tests.rs): the field names of each record.
CONSOLE_KERNEL_FIRST = 0x2C0
CONSOLE_KERNEL = (
    ("enter_cs_min", "enter_cs_med", "enter_cs_max"),
    ("exit_cs_min", "exit_cs_med", "exit_cs_max"),
    ("empty_call_min", "empty_call_med", "empty_call_max"),
    ("bios_vblank_min", "bios_vblank_med", "bios_vblank_max"),
    ("bios_vblank_gaps", "bios_vblank_events_ready", "flags"),
    ("runtime_vblank_min", "runtime_vblank_med", "runtime_vblank_max"),
)
CONSOLE_WIDTHS = (256, 320, 368, 384, 512, 640)


def console_rows(capture: Capture) -> list[str]:
    """The v1.27 CONSOLE TESTS results, unpacked. Empty unless a case ran
    before the capture encoded. Nothing here is a verdict: each case is read
    off the screen (and the film of it), and these are the numbers behind it."""
    by_id = {record.record_id: record for record in capture.records}
    rows = []

    def triple(record_id):
        record = by_id[record_id]
        return record.minimum, record.median, record.maximum

    if all(CONSOLE_KERNEL_FIRST + k in by_id for k in range(len(CONSOLE_KERNEL))):
        fields = {}
        for k, names in enumerate(CONSOLE_KERNEL):
            fields.update(zip(names, triple(CONSOLE_KERNEL_FIRST + k)))
        flags = fields.pop("flags")
        harness = fields["empty_call_med"]
        rows.append("console_kernel,field,value")
        for name, value in fields.items():
            rows.append(f"console_kernel,{name},{value}")
        rows.append(f"console_kernel,enter_cs_net,{max(fields['enter_cs_med'] - harness, 0)}")
        rows.append(f"console_kernel,exit_cs_net,{max(fields['exit_cs_med'] - harness, 0)}")
        rows.append(f"console_kernel,standard_vector,{flags & 1}")
        rows.append(f"console_kernel,event_opened,{(flags >> 1) & 1}")
        rows.append(f"console_kernel,bios_loop_finished,{(flags >> 2) & 1}")
        rows.append(f"console_kernel,runtime_vblank_gaps,{flags >> 8}")
    for k, width in enumerate(CONSOLE_WIDTHS):
        if 0x2D0 + k in by_id:
            status, dots, span = triple(0x2D0 + k)
            if not rows or not rows[-1].startswith("console_width"):
                rows.append("console_width,pixels,gpustat_high16,dot_ticks_in_64_lines,window_span_gpu_clocks")
            rows.append(f"console_width,{width},0x{status:04X},{dots},{span}")
    if 0x2D6 in by_id and 0x2D7 in by_id:
        status, flips, frames = triple(0x2D6)
        interlaced, tall, low = triple(0x2D7)
        rows.append("console_interlace,field,value")
        rows.append(f"console_interlace,gpustat,0x{status:04X}{low:04X}")
        rows.append(f"console_interlace,field_parity_changes,{flips}")
        rows.append(f"console_interlace,frames_sampled,{frames}")
        rows.append(f"console_interlace,frames_with_interlace_bit,{interlaced}")
        rows.append(f"console_interlace,frames_with_480_bit,{tall}")
    if all(0x2E0 + k in by_id for k in range(4)):
        flags, loops, seconds = triple(0x2E0)
        names = ("file_found", "play_refused", "head_in_song", "looped", "getlocp_updating", "no_loop_seen")
        rows.append("console_xa,field,value")
        for bit, name in enumerate(names):
            rows.append(f"console_xa,{name},{(flags >> bit) & 1}")
        rows.append(f"console_xa,loops,{loops}")
        rows.append(f"console_xa,seconds_run,{seconds}")
        for record_id, name in ((0x2E1, "loop_gap_ms"), (0x2E2, "loop_period_ms")):
            lo, med, hi = triple(record_id)
            text = "none" if lo == 0xFFFF else f"min={lo} med={med} max={hi}"
            rows.append(f"console_xa,{name},{text}")
        first, positions, stall = triple(0x2E3)
        rows.append(f"console_xa,first_start_ms,{first}")
        rows.append(f"console_xa,head_positions_seen,{positions}")
        rows.append(f"console_xa,longest_unchanged_head_ms,{stall}")
    rows.extend(cdstream_rows(by_id))
    return rows


CDDA_AUDIO_FLAGS = (
    "playing_before",
    "getlocp_advancing_before",
    "capture_signal_before",
    "playing_after",
    "getlocp_advancing_after",
    "capture_signal_after",
    "read_intact",
    "ran",
)


def cdstream_rows(by_id) -> list[str]:
    """The v1.28 CD STREAM cases, unpacked. Nothing here is a verdict about a
    console unless the capture came from one: the emulator has no laser, so
    its CD-DA readings only show the suite runs."""
    rows = []

    def triple(record_id):
        record = by_id[record_id]
        return record.minimum, record.median, record.maximum

    def spread(record_id, scale=1):
        low, med, high = triple(record_id)
        return f"min={low / scale:g} med={med / scale:g} max={high / scale:g}"

    if all(0x2F0 + k in by_id for k in range(8)):
        rows.append("cdstream_cost,field,value")
        for speed, first in (("2x", 0x2F0), ("1x", 0x2F1)):
            rows.append(f"cdstream_cost,sectors_per_second_{speed},{spread(first, 10)}")
        for speed, first in (("2x", 0x2F2), ("1x", 0x2F3)):
            rows.append(f"cdstream_cost,pio_us_per_sector_{speed},{spread(first)}")
        lost_2x, lost_1x, irq = triple(0x2F4)
        rows.append(f"cdstream_cost,cpu_lost_percent_2x,{lost_2x / 10:g}")
        rows.append(f"cdstream_cost,cpu_lost_percent_1x,{lost_1x / 10:g}")
        rows.append(f"cdstream_cost,irq_per_sector_2x,{irq / 100:g}")
        h2, h1, unused = triple(0x2F5)
        rows.append(f"cdstream_cost,handler_peak_us_2x,{h2}")
        rows.append(f"cdstream_cost,handler_peak_us_1x,{h1}")
        rows.append(f"cdstream_cost,handler_stack_unused_bytes,{unused}")
        f2, f1, discarded = triple(0x2F6)
        rows.append(f"cdstream_cost,first_sector_ms_2x,{f2}")
        rows.append(f"cdstream_cost,first_sector_ms_1x,{f1}")
        rows.append(f"cdstream_cost,discarded_sectors,{discarded}")
        flags, chained_2x, chained_1x = triple(0x2F7)
        rows.append(f"cdstream_cost,installed,{flags & 1}")
        rows.append(f"cdstream_cost,intact_2x,{(flags >> 1) & 1}")
        rows.append(f"cdstream_cost,intact_1x,{(flags >> 2) & 1}")
        rows.append(f"cdstream_cost,chained_2x,{chained_2x}")
        rows.append(f"cdstream_cost,chained_1x,{chained_1x}")
    if all(0x300 + k in by_id for k in range(12)):
        rows.append("cdstream_cdda,field,value")
        rows.append(f"cdstream_cdda,lease_stops_read_ms,{spread(0x300)}")
        rows.append(f"cdstream_cdda,t_pause_ms,{spread(0x301, 10)}")
        ack, stat, still = triple(0x302)
        rows.append(f"cdstream_cdda,pause_ack_ms,{ack / 10:g}")
        rows.append(f"cdstream_cdda,status_after_pause,0x{stat:02X}")
        rows.append(f"cdstream_cdda,getlocp_held_still_after_pause,{still}")
        rows.append(f"cdstream_cdda,first_sector_after_audio_ms,{spread(0x303)}")
        rows.append(f"cdstream_cdda,sectors_after_audio_ms,{spread(0x304)}")
        rows.append(f"cdstream_cdda,resume_to_playing_ms,{spread(0x305)}")
        moved, in_place, playing = triple(0x306)
        moved = moved - 0x10000 if moved >= 0x8000 else moved
        rows.append(f"cdstream_cdda,resume_moved_frames_median,{moved}")
        rows.append(f"cdstream_cdda,resume_in_place_of_3,{in_place}")
        rows.append(f"cdstream_cdda,resume_reported_playing_of_3,{playing}")
        for name, first, flags_id in (("recovery_pause", 0x307, 0x308), ("no_pause", 0x309, 0x30A)):
            first_ms, done_ms, code = triple(first)
            flags, signal, quiet = triple(flags_id)
            rows.append(f"cdstream_cdda,{name}_first_sector_ms,{first_ms}")
            rows.append(f"cdstream_cdda,{name}_sectors_ms,{done_ms}")
            rows.append(f"cdstream_cdda,{name}_read_failure_code,{code}")
            for bit, label in enumerate(CDDA_AUDIO_FLAGS):
                rows.append(f"cdstream_cdda,{name}_{label},{(flags >> bit) & 1}")
            rows.append(f"cdstream_cdda,{name}_signal_during_read_percent,{signal / 10:g}")
            quiet_text = "never" if quiet == 0xFFFF else quiet
            rows.append(f"cdstream_cdda,{name}_first_quiet_sample_ms,{quiet_text}")
        signal, samples, flags = triple(0x30B)
        rows.append(f"cdstream_cdda,paused_signal_during_read_percent,{signal / 10:g}")
        rows.append(f"cdstream_cdda,paused_capture_samples,{samples}")
        for bit, label in enumerate(("playing_before_pause", "getlocp_advancing", "capture_signal_before", "playing_after_pause")):
            rows.append(f"cdstream_cdda,control_{label},{(flags >> bit) & 1}")
    if all(0x310 + k in by_id for k in range(6)):
        rows.append("cdstream_motor,field,value")
        for k, wait in enumerate((0, 5, 15)):
            first, done, code = triple(0x310 + k)
            rows.append(f"cdstream_motor,pause_wait_{wait}s_first_sector_ms,{first}")
            rows.append(f"cdstream_motor,pause_wait_{wait}s_sectors_ms,{done}")
            rows.append(f"cdstream_motor,pause_wait_{wait}s_failure_code,{code}")
        ack, done, code = triple(0x313)
        rows.append(f"cdstream_motor,stop_ack_ms,{ack}")
        rows.append(f"cdstream_motor,read_at_once_after_stop_sectors_ms,{'failed' if done == 0xFFFF else done}")
        rows.append(f"cdstream_motor,read_at_once_after_stop_failure_code,{code}")
        complete, off, settled = triple(0x314)
        rows.append(f"cdstream_motor,stop_complete_ms,{complete}")
        rows.append(f"cdstream_motor,motor_off_after_ms,{'never' if off == 0xFFFF else off}")
        rows.append(f"cdstream_motor,settled_read_sectors_ms,{'failed' if settled == 0xFFFF else settled}")
        flags, recovered, at_once_first = triple(0x315)
        rows.append(f"cdstream_motor,read_at_once_after_stop_intact,{flags & 1}")
        rows.append(f"cdstream_motor,settled_read_intact,{(flags >> 1) & 1}")
        rows.append(f"cdstream_motor,transport_read_afterwards_intact,{(flags >> 2) & 1}")
        rows.append(f"cdstream_motor,read_at_once_after_stop_first_sector_ms,{at_once_first}")
    return rows


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
    0x100: 16,
    0x101: 16,
    0x102: 16,
    0x103: 16,
    0x104: 16,
    0x105: 16,
    0x106: 16,
    0x107: 16,
    0x108: 16,
    0x109: 16,
    0x10A: 16,
    0x10B: 16,
    0x10C: 16,
    0x10D: 16,
    0x10E: 16,
    0x10F: 16,
    0x110: 16,
    0x111: 16,
    0x112: 64,
    0x113: 64,
    0x114: 64,
    0x120: 16,
    0x121: 16,
    0x122: 16,
    0x123: 16,
    0x124: 8,
    0x125: 64,
    0x126: 64,
    0x127: 64,
    0x128: 64,
    0x129: 64,
    0x12A: 64,
    0x12B: 64,
    0x12C: 64,
    0x12D: 64,
    0x12E: 64,
    0x130: 16,
    0x131: 16,
    0x132: 16,
    0x133: 16,
    0x134: 16,
    0x135: 32,
    0x136: 32,
    0x137: 16,
    0x138: 16,
    0x139: 16,
    0x13A: 16,
    **{record_id: 64 for record_id in range(0x140, 0x146)},
    0x150: 16,
    0x151: 16,
    0x152: 16,
    0x153: 16,
    0x154: 16,
    0x155: 16,
    0x156: 16,
    0x157: 16,
    0x158: 16,
    0x159: 16,
    0x15A: 16,
    0x15B: 16,
    0x15C: 16,
    0x15D: 16,
    0x15E: 16,
    0x15F: 16,
    0x160: 16,
    0x161: 16,
    0x162: 8,
    0x163: 8,
    0x164: 8,
    0x165: 8,
    0x166: 8,
    0x167: 8,
    0x168: 8,
    0x169: 8,
    0x16A: 8,
    0x16B: 8,
    0x16C: 8,
    0x16D: 8,
    0x16E: 16,
    0x16F: 16,
    0x170: 16,
    0x171: 8,
    0x172: 8,
    0x173: 8,
    0x174: 8,
    0x175: 8,
    0x176: 8,
    0x177: 16,
    0x178: 16,
    0x179: 16,
    0x17A: 16,
    0x17B: 16,
    0x17C: 16,
    0x17D: 8,
    0x17E: 8,
    0x17F: 8,
    0x180: 16,
    0x181: 16,
    0x182: 16,
    0x183: 16,
    0x184: 16,
    0x185: 16,
    0x186: 8,
    0x187: 8,
    0x188: 8,
    0x189: 16,
    0x18A: 16,
    0x18B: 16,
    0x18C: 16,
    0x18D: 16,
    0x18E: 16,
    0x18F: 8,
    0x190: 8,
    0x191: 8,
    0x192: 16,
    0x193: 16,
    0x194: 16,
    0x195: 16,
    0x1A0: 64,
    0x1A1: 64,
    0x1A2: 64,
    0x1A3: 64,
    0x1A8: 64,
    0x1A9: 64,
    0x1AA: 64,
    0x1AB: 64,
    0x1AC: 64,
    0x1AD: 64,
    0x1B0: 64,
    0x1B1: 64,
    0x1B2: 64,
    0x1B3: 64,
    0x1B4: 64,
    0x1B5: 64,
    0x1B8: 64,
    0x1B9: 64,
    0x1BA: 32,
    0x1BB: 32,
    0x1BC: 64,
    0x1C0: 64,
    0x1C1: 64,
    0x1C2: 64,
    0x1C3: 64,
    0x1C4: 64,
    0x1C5: 64,
    0x1C6: 64,
    0x1C7: 64,
    0x1C8: 64,
    0x1C9: 64,
    0x1CA: 64,
    0x1CB: 64,
    0x1D0: 8,
    0x1D1: 8,
    0x1D2: 8,
    0x1D3: 8,
    0x1D4: 8,
    0x1D5: 16,
    0x1D6: 16,
    0x1D7: 16,
    0x1D8: 16,
    0x1E0: 64,
    0x1E1: 64,
    0x1E2: 64,
    0x1E3: 64,
    0x1E4: 64,
    0x1E5: 64,
    0x1E6: 64,
    0x1E7: 64,
    0x200: 16,
    0x201: 16,
    0x202: 16,
    0x203: 16,
    0x204: 16,
    0x205: 16,
    0x206: 16,
    0x207: 16,
    0x208: 16,
    0x209: 16,
    0x20A: 16,
    0x20B: 16,
    0x20C: 16,
    0x20D: 16,
    0x20E: 16,
    0x20F: 16,
    0x210: 16,
    0x211: 16,
    0x212: 16,
    0x213: 16,
    0x214: 16,
    0x215: 16,
    0x216: 16,
    0x217: 16,
    0x218: 16,
    0x219: 16,
    0x21A: 16,
    0x21B: 16,
    0x21C: 16,
    0x21D: 16,
    0x21E: 16,
    0x21F: 16,
    0x220: 16,
    0x221: 16,
    0x222: 16,
    0x223: 16,
    **{record_id: 0 for record_id in LABELS if 0x200 <= record_id < 0x2B0},
    **{record_id: 0 for record_id in LABELS if 0x2C0 <= record_id < 0x320},
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
    run_id: int | None = None


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
PX8_BLOCK_TIMING_EXT = 1 << 6
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
    # v2.0 pages carry the run id after the page numbers: PPTTRRRR.
    if marker not in SCHEMAS or len(page_field) not in (4, 8) or not chunk:
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
        int(page_field[2:4], 16),
        chunk,
        actual_crc,
        int(page_field[4:], 16) if len(page_field) == 8 else None,
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
    if schema == "PX8" and flags & PX8_BLOCK_TIMING_EXT:
        # Records whose id does not fit a byte. Same fields, wider id.
        (extended_count,) = struct.unpack_from("<H", binary, offset)
        offset += 2
        for _ in range(extended_count):
            record_id, minimum, median, maximum = struct.unpack_from("<HHHH", binary, offset)
            offset += 8
            records.append(
                Record(record_id, WORK_BY_ID.get(record_id, 0), minimum, maximum, median)
            )
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


def run_id_text(capture: Capture) -> str:
    """v2.0 and later: the 16-bit run id (also in every page). Before: the
    timing run counter."""
    if capture.suite_major >= 2:
        return f"{capture.conformance_run | capture.timing_run << 8:04X}"
    return f"{capture.timing_run:02X}"


def record_label(record_id: int) -> str:
    entry = V2_RECORDS.get(record_id)
    if entry is not None:
        return entry[0]
    return LABELS.get(record_id, "unlabelled")


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
        f"pages={page_count} run={run_id_text(capture)} "
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
            if index >= len(baseline.observations):
                # A MINOR bump appends cases; the baseline has nothing to say
                # about them.
                row += ",absent,n/a"
                print(row)
                continue
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
            f"{record.record_id:02X},{record_label(record.record_id)},"
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
                        f"timing {record.record_id:02X} ({record_label(record.record_id)}): "
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
    for row in list_busy_rows(capture):
        print(row)
    for row in v2_rows(capture):
        print(row)
    for row in console_rows(capture):
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


# ---------------------------------------------------------------------------
# The one command: decode a recording (or a page file) and diff it against the
# last silicon baselines.

REPO = pathlib.Path(__file__).resolve().parent.parent
# Oldest first: where two baselines carry the same record, the later one wins.
SILICON_REFERENCES = (
    ("docs/hardware-refs/px8-silicon-2026-09-23-v1.24-full.txt", "v1.24 full"),
    ("docs/hardware-refs/px8-silicon-2026-10-08-v1.28-cdstream.txt", "v1.28 CD stream"),
)
# Records whose meaning v2.0 changed; they cannot be compared across the bump.
# Every other id that exists in both means what it meant (the v2.0 bump is MAJOR
# because records were removed and regrouped, not because these were redefined).
REDEFINED_IN_V2 = frozenset(range(0x2D0, 0x2D8))
# Records that carry flags or counts: exact match, not a timing tolerance.
EXACT_RECORDS = frozenset({0x2C4, 0x2E0, 0x2F7, 0x306, 0x308, 0x30A, 0x30B, 0x315})
TIMING_TOLERANCE = 0.10
TIMING_SLACK = 8


@dataclass
class SiliconReference:
    cases: dict
    records: dict
    precision: tuple
    sources: list


def load_silicon_reference(root: pathlib.Path = REPO) -> SiliconReference:
    cases: dict = {}
    records: dict = {}
    precision: tuple = ()
    sources = []
    for relative, label in SILICON_REFERENCES:
        path = root / relative
        if not path.exists():
            continue
        capture = parse_capture(payloads_from_paths([str(path)]))
        sources.append(f"{label} (v{capture.suite_major}.{capture.suite_minor})")
        for index, (status, observed) in enumerate(zip(capture.statuses, capture.observations)):
            cases[index] = (status, observed)
        for record in capture.records:
            records[record.record_id] = record
        if capture.precision:
            precision = capture.precision
    if not sources:
        raise ValueError("no silicon baseline found under docs/hardware-refs")
    return SiliconReference(cases, records, precision, sources)


def compare_to_silicon(capture: Capture, reference: SiliconReference) -> dict:
    """Regressions and mismatches of `capture` against the silicon baselines.

    A regression is something that is wrong on its own terms or worse than
    silicon was: a conformance case that passed on silicon and does not now, an
    area that did not hand off clean, noise at the capture pages. A mismatch
    is a value that differs from silicon's: the list of what an emulator
    capture still gets wrong, or what moved between two silicon runs."""
    regressions: list = [f"run check: {problem}" for problem in v2_verdicts(capture)]
    mismatches: list = []
    improvements: list = []
    shared = {"cases": 0, "records": 0, "precision": 0}
    observations = capture.observations or ()
    for index, status in enumerate(capture.statuses):
        if index not in reference.cases:
            continue
        shared["cases"] += 1
        silicon_status, silicon_observed = reference.cases[index]
        observed = observations[index] if index < len(observations) else None
        name = f"case {index}"
        if silicon_status == 1 and status in (2, 3):
            seen = f", observed 0x{observed:08X}" if observed is not None else ""
            regressions.append(f"{name}: {STATUS_LABELS[status]} (passed on silicon){seen}")
        elif silicon_status in (2, 3) and status == 1:
            improvements.append(f"{name}: passes (silicon {STATUS_LABELS[silicon_status]})")
        if observed is not None and observed != silicon_observed:
            mismatches.append(
                f"{name}: 0x{observed:08X}, silicon 0x{silicon_observed:08X} "
                f"({STATUS_LABELS[status]} vs {STATUS_LABELS[silicon_status]})"
            )
    for record in capture.records:
        silicon = reference.records.get(record.record_id)
        if silicon is None or record.record_id in REDEFINED_IN_V2:
            continue
        shared["records"] += 1
        if record.record_id in EXACT_RECORDS:
            moved = (record.minimum, record.median, record.maximum) != (
                silicon.minimum,
                silicon.median,
                silicon.maximum,
            )
        else:
            allowed = max(TIMING_SLACK, int(silicon.minimum * TIMING_TOLERANCE))
            moved = abs(record.minimum - silicon.minimum) > allowed
        if moved:
            mismatches.append(
                f"record {record.record_id:03X} {record_label(record.record_id)}: "
                f"min/med/max {record.minimum}/{record.median}/{record.maximum}, "
                f"silicon {silicon.minimum}/{silicon.median}/{silicon.maximum}"
            )
    for index, (value, silicon_value) in enumerate(zip(capture.precision, reference.precision)):
        shared["precision"] += 1
        if value != silicon_value:
            mismatches.append(
                f"precision {index:03d} {precision_label(index)}: "
                f"0x{value:08X}, silicon 0x{silicon_value:08X}"
            )
    return {
        "regressions": regressions,
        "mismatches": mismatches,
        "improvements": improvements,
        "shared": shared,
    }


def pages_from_recording(path: pathlib.Path) -> list[str]:
    """Run the QR extractor over a video and return the page lines."""
    import importlib.util

    spec = importlib.util.spec_from_file_location("hwtest_video_qr", REPO / "tools" / "hwtest-video-qr.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.pages_from_video(path)


def run_compare(source: str, show_report: bool, allow_failure: bool) -> int:
    path = pathlib.Path(source)
    if path.suffix.lower() in (".mov", ".mp4", ".mkv", ".avi", ".m4v"):
        payloads = pages_from_recording(path)
    else:
        payloads = payloads_from_paths([source])
    capture = parse_capture(payloads)
    reference = load_silicon_reference()
    if show_report:
        print_report(capture, None)
    result = compare_to_silicon(capture, reference)
    print(
        f"# compare: capture v{capture.suite_major}.{capture.suite_minor} run={run_id_text(capture)} "
        f"pages={capture.page_count} crc={capture.binary_crc:08X}"
    )
    print("# silicon: " + ", ".join(reference.sources))
    shared = result["shared"]
    print(
        f"# compared: {shared['cases']} cases, {shared['records']} records, "
        f"{shared['precision']} precision values; records redefined in v2.0 are skipped"
    )
    for line in result["regressions"]:
        print(f"REGRESSION {line}")
    for line in result["improvements"]:
        print(f"IMPROVED {line}")
    for line in result["mismatches"]:
        print(f"MISMATCH {line}")
    print(
        f"# summary: regressions={len(result['regressions'])} "
        f"mismatches={len(result['mismatches'])} improvements={len(result['improvements'])}"
    )
    if result["regressions"] and not allow_failure:
        return 1
    return 0


# ---------------------------------------------------------------------------
# Silence of the capture pages, from the audio the emulator dumped.


def check_silence(wav_path: str, tail_seconds: float) -> int:
    import array
    import wave

    with wave.open(wav_path, "rb") as wav:
        if wav.getsampwidth() != 2:
            raise ValueError("expected 16-bit samples")
        channels = wav.getnchannels()
        rate = wav.getframerate()
        samples = array.array("h")
        samples.frombytes(wav.readframes(wav.getnframes()))
    frames = len(samples) // channels
    duration = frames / rate
    # Non-silent spans, in seconds, merged when closer than 0.25 s.
    window = rate // 20
    loud = []
    for start in range(0, frames, window):
        chunk = samples[start * channels : (start + window) * channels]
        if chunk and max(max(chunk), -min(chunk)) > 0:
            loud.append(start / rate)
    spans: list[list[float]] = []
    for t in loud:
        if spans and t - spans[-1][1] <= 0.25:
            spans[-1][1] = t + 0.05
        else:
            spans.append([t, t + 0.05])
    print(f"# audio: {duration:.1f} s, {len(spans)} non-silent span(s)")
    for begin, end in spans:
        print(f"# sound {begin:.1f}-{end:.1f} s")
    last_sound = spans[-1][1] if spans else 0.0
    silent_tail = duration - last_sound
    print(f"# silent tail: {silent_tail:.1f} s (needs {tail_seconds:.1f} s: the capture pages)")
    if silent_tail < tail_seconds:
        print(f"FAIL: sound continues until {last_sound:.1f} s of {duration:.1f} s", file=sys.stderr)
        return 1
    print("silent at the capture pages")
    return 0


# ---------------------------------------------------------------------------


def emulator_baseline(log: str, git: str, exe_sha: str, steps: str) -> int:
    import datetime

    pages = payloads_from_paths([log])
    parse_capture(pages)  # refuse a log that does not decode
    print("# PSoXide hardware-test emulator baseline")
    print("#")
    print("# SOURCE: PSoXide EMULATOR, headless. This is NOT a silicon capture.")
    print("#   It detects emulator-side drift only. It is not hardware truth and")
    print("#   must never be cited as a console measurement.")
    print("#")
    print(f"# captured:  {datetime.datetime.now(datetime.timezone.utc).date().isoformat()}")
    print(f"# git:       {git}")
    print(f"# guest exe: sha256:{exe_sha}")
    print(f"# emulator:  frontend launch --steps {steps} (one CROSS pulse on RUN HARDWARE TEST)")
    print("#")
    last: dict[int, str] = {}
    for line in pages:
        last[parse_capture_page(line).number] = line
    for number in sorted(last):
        print(last[number])
    return 0



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
    parser.add_argument(
        "--compare",
        metavar="INPUT",
        help="decode INPUT (a recording or a page file) and diff it against the last "
        "silicon baselines: regressions and mismatches",
    )
    parser.add_argument(
        "--report",
        action="store_true",
        help="with --compare, print the full decoded report first",
    )
    parser.add_argument(
        "--no-fail",
        action="store_true",
        help="with --compare, exit 0 even when there are regressions",
    )
    parser.add_argument(
        "--check-silence",
        metavar="WAV",
        help="check that the audio the emulator dumped ends in silence",
    )
    parser.add_argument("--tail-seconds", type=float, default=3.0)
    parser.add_argument(
        "--emulator-baseline",
        metavar="LOG",
        help="print an emulator baseline file made from a run log",
    )
    parser.add_argument("--git", default="unknown")
    parser.add_argument("--exe-sha", default="unknown")
    parser.add_argument("--steps", default="unknown")
    parser.add_argument("payload_or_file", nargs="*")
    args = parser.parse_args()
    try:
        if args.compare:
            return run_compare(args.compare, args.report, args.no_fail)
        if args.check_silence:
            return check_silence(args.check_silence, args.tail_seconds)
        if args.emulator_baseline:
            return emulator_baseline(args.emulator_baseline, args.git, args.exe_sha, args.steps)
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
