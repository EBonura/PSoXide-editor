"""PSXM model and PSXA clip decoding plus the rigid deform, as in tools/enemy_reduction/validate_render.py."""
import struct,math
import numpy as np
def model(path):
    b = path.read_bytes()
    jc, pc, vc, fc, mc, *_ = struct.unpack_from('<8H', b, 12)
    po = 28 + 4 * jc + 8 * mc
    vo = po + 16 * pc
    fo = vo + 8 * vc
    parts = [struct.unpack_from('<6H', b, po + i * 16) for i in range(pc)]
    v = [struct.unpack_from('<3hBB', b, vo + i * 8) for i in range(vc)]
    fs = [[struct.unpack_from('<HBB', b, fo + i * 12 + k * 4) for k in range(3)] for i in range(fc)]
    owner = {i: p[0] for p in parts for i in range(p[1], p[1] + p[2])}
    banks = b[fo + fc * 12:]
    return {'b': b, 'v': v, 'f': fs, 'owner': owner, 'banks': banks, 'joints': jc, 'po': po}
def animation(path):
    b = path.read_bytes()
    ver = struct.unpack_from('<H', b, 4)[0]
    jc, fc, hz, shift = struct.unpack_from('<4H', b, 12)
    pose = []

    def q(c):
        return 4096 if c == 2047 else (c if c < 2048 else c - 4096) * 2
    for fi in range(fc):
        row = []
        for ji in range(jc):
            index = fi * jc + ji
            if ver == 5:
                di = struct.unpack_from('<H', b, 20 + index * 2)[0]
                off = (20 + jc * fc * 2 + 3) // 4 * 4 + di * 16
            else:
                off = 20 + index * {1: 30, 2: 24, 3: 20, 4: 16}[ver]
            if ver <= 2:
                m = list(struct.unpack_from('<9h', b, off))
                t = struct.unpack_from('<3i' if ver == 1 else '<3h', b, off + 18)
            else:
                vals = []
                for pair in range(3 if ver >= 4 else 4):
                    packed = int.from_bytes(b[off + pair * 3:off + pair * 3 + 3], 'little')
                    vals.extend([q(packed & 4095), q(packed >> 12 & 4095)])
                if ver >= 4:
                    cross = np.cross(vals[:3], vals[3:6])
                    third = [int((x + 2048) // 4096) if x >= 0 else -int((-x + 2048) // 4096) for x in cross]
                    code = b[off + 9]
                    axis = code & 3
                    correction = code >> 2
                    correction = correction if correction < 32 else correction - 64
                    if axis < 3:
                        third[axis] += correction
                    m = vals + [max(-4096, min(4096, x)) for x in third]
                    t = struct.unpack_from('<3h', b, off + 10)
                else:
                    m = vals + [q(struct.unpack_from('<H', b, off + 12)[0] & 4095)]
                    t = struct.unpack_from('<3h', b, off + 14)
            row.append((np.array(m, dtype=float).reshape(3, 3).T / 4096, np.array(t, dtype=float) * (1 if ver == 1 else 2 ** shift)))
        pose.append(row)
    return pose
def deform(m, pose):
    out = []
    for i, v in enumerate(m['v']):
        p = np.array(v[:3])
        a, t = pose[m['owner'][i]]
        p0 = a @ p + t
        if v[4]:
            a, t = pose[v[3]]
            p0 = (p0 * (255 - v[4]) + (a @ p + t) * v[4]) / 255
        out.append(p0)
    return np.array(out)
