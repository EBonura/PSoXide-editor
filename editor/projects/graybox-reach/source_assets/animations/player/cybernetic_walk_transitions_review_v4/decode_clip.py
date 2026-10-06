import struct,math
import numpy as np
def decode(path):
 b=path.read_bytes();ver=struct.unpack_from('<H',b,4)[0];j,n,hz,shift=struct.unpack_from('<4H',b,12);stride={1:30,2:24,3:20,4:16,5:16}[ver];dstart=20+((n*j*2+3)//4)*4 if ver==5 else 20;arr=[]
 def q11(c):return 4096 if c==2047 else (c if c<2048 else c-4096)*2
 def rnd(x):return int(math.copysign((abs(int(x))+2048)//4096,x))
 for i in range(n*j):
  idx=struct.unpack_from('<H',b,20+i*2)[0] if ver==5 else i;o=dstart+idx*stride
  if ver<=2:flat=list(struct.unpack_from('<9h',b,o));to=o+18
  else:
   flat=[]
   for k in range(3 if ver>=4 else 4):
    x=int.from_bytes(b[o+k*3:o+k*3+3],'little');flat.extend([q11(x&4095),q11((x>>12)&4095)])
   if ver==3:flat.append(q11(int.from_bytes(b[o+12:o+14],'little')&4095));to=o+14
   else:
    cross=np.cross(flat[:3],flat[3:6]);third=[rnd(v) for v in cross];axis=b[o+9]&3;c=b[o+9]>>2;c=c if c<32 else c-64
    if axis<3:third[axis]+=c
    flat.extend([max(-4096,min(4096,v)) for v in third]);to=o+10
  t=np.array(struct.unpack_from('<3i' if ver==1 else '<3h',b,to),float)*(1 if ver==1 else 1<<shift);arr.append((np.array(flat,float).reshape(3,3).T/4096,t))
 return np.array([v[0] for v in arr]).reshape(n,j,3,3),np.array([v[1] for v in arr]).reshape(n,j,3),n,hz
