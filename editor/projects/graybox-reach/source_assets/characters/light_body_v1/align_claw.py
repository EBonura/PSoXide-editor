"""Align the rigid claw to the neutral forearm without changing animation keys.

Run with Python + numpy. Default input is the retained unaligned body. Pass
--input <fresh unaligned model> after shape.py/detail.py when regenerating.
All transforms use bind/model coordinates, matching runtime skinning/capsules.
"""
from pathlib import Path
import argparse, json, struct, hashlib
import numpy as np

O=Path(__file__).resolve().parent;P=O.parents[2]

def layout(data):
    jc,pc,vc,fc,mc,*_=struct.unpack_from('<8H',data,12)
    po=28+jc*4+mc*8;vo=po+pc*16
    parts=[struct.unpack_from('<6H',data,po+i*16) for i in range(pc)]
    owners={i:p[0] for p in parts for i in range(p[1],p[1]+p[2])}
    return vo,vc,fc,owners

def correction(data):
    bind=np.load(O/'bind.npz')['bind']
    neutral=np.load(P/'source_assets/animations/light_walk_v5/walk.npz')['skin'][0]@bind
    vo,vc,fc,owners=layout(data)
    ids=[i for i in owners if owners[i]==9]
    points=np.array([struct.unpack_from('<3h',data,vo+i*8) for i in ids],dtype=float)
    # Average the three longest finger-tip vertices, excluding the thumb/cuff.
    tip=points[np.argsort(points[:,0])[-3:]].mean(0)
    pivot=bind[9,:3,3]
    current=neutral[9,:3,:3]@np.linalg.solve(bind[9,:3,:3],tip-pivot)
    desired=neutral[9,:3,3]-neutral[8,:3,3]
    u=current/np.linalg.norm(current);v=desired/np.linalg.norm(desired)
    axis=np.cross(u,v);cosine=float(np.dot(u,v))
    skew=np.array([[0,-axis[2],axis[1]],[axis[2],0,-axis[0]],[-axis[1],axis[0],0]])
    world=np.eye(3)+skew+skew@skew/(1+cosine)
    local=np.linalg.solve(neutral[9,:3,:3],world@neutral[9,:3,:3])
    rotation=bind[9,:3,:3]@local@np.linalg.inv(bind[9,:3,:3])
    return rotation,pivot,ids,float(np.degrees(np.arccos(np.clip(cosine,-1,1))))

def transform(data,rotation,pivot):
    out=bytearray(data);vo,vc,fc,owners=layout(data)
    # The clawless variant keeps the same vertex buffer but zeroes part 9's
    # counts. Use the full model's ownership for those hidden vertices.
    if len(owners)<vc:
        _,base_vc,_,base_owners=layout((O/'light-before-claw-alignment.psxmdl').read_bytes())
        assert vc==base_vc
        owners={**base_owners,**owners}
    for i in range(vc):
        if owners[i]!=9:continue
        assert data[vo+i*8+7]==0, 'Claw must remain rigidly weighted to joint 9'
        p=np.array(struct.unpack_from('<3h',data,vo+i*8),dtype=float)
        q=np.rint(pivot+rotation@(p-pivot)).astype(int)
        assert np.abs(q).max()<32768
        struct.pack_into('<3h',out,vo+i*8,*q)
    # Only the claw position fields may change: topology, UVs, weights, part
    # flags, palette banks and every other vertex stay byte-identical.
    masked=bytearray(out)
    for i in range(vc):
        if owners[i]==9:masked[vo+i*8:vo+i*8+6]=data[vo+i*8:vo+i*8+6]
    assert bytes(masked)==data
    return bytes(out)

if __name__=='__main__':
    args=argparse.ArgumentParser();args.add_argument('--input',type=Path);a=args.parse_args()
    original=(a.input or O/'light-before-claw-alignment.psxmdl').read_bytes()
    rotation,pivot,ids,angle=correction(original)
    adjusted=transform(original,rotation,pivot)
    (O/'light-body-study.psxmdl').write_bytes(adjusted)
    (P/'assets/models/light_body_v1/light.psxmdl').write_bytes(adjusted)
    clawless=(O/'light-clawless-before-alignment.psxmdl').read_bytes()
    if a.input:
        # Regenerated body geometry, retaining the existing disabled claw part.
        clawless=bytearray(original);jc,pc,vc,fc,mc,*_=struct.unpack_from('<8H',clawless,12)
        po=28+jc*4+mc*8
        old=(O/'light-clawless-before-alignment.psxmdl').read_bytes()
        for i in range(pc):
            if struct.unpack_from('<H',clawless,po+i*16)[0]==9:
                struct.pack_into('<H',clawless,po+i*16+4,0)
                struct.pack_into('<H',clawless,po+i*16+8,0)
        clawless=bytes(clawless)
    (P/'assets/models/light_body_v1/light_clawless.psxmdl').write_bytes(transform(clawless,rotation,pivot))
    report={'joint':9,'changed_vertices':len(ids),'rotation_degrees':angle,'pivot':pivot.tolist(),'rotation':rotation.tolist(),
            'unaligned_sha256':hashlib.sha256(original).hexdigest(),'aligned_sha256':hashlib.sha256(adjusted).hexdigest(),
            'unchanged':'All animation keys, skeleton, weights, topology, UVs, texture, non-claw geometry and part flags.'}
    (O/'claw-alignment.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report))
